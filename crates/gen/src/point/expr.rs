//! Reads the expression of a point: operations on `f64` and `F64x3` values,
//! turned into nodes of `f64` operations, one per coordinate.
//!
//! ```text
//! expr   := term (('+' | '-') term)*
//! term   := unary ('*' unary)*
//! unary  := '-'* atom
//! atom   := INTEGER | NAME | '(' expr ')'
//! ```
//!
//! Chains are read in loops and only parenthesized expressions recurse, within
//! the limit on the depth of brackets.

use std::collections::HashMap;

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};

use super::{ParamType, PointBuilder, PointNode, PointParam};
use crate::cursor::Cursor;
use crate::error::{Res, diag};
use crate::expr::int_literal;
use crate::limits;

/// The value of an expression: an `f64` or the three coordinates of an
/// `F64x3`, as node indices.
#[derive(Clone, Copy)]
pub(super) enum Value {
    Scalar(usize),
    Vector([usize; 3]),
}

impl Value {
    fn name(self) -> &'static str {
        match self {
            Value::Scalar(_) => "an `f64`",
            Value::Vector(_) => "an `F64x3`",
        }
    }
}

/// The graph under construction and the parameters in scope.
pub(super) struct Scope<'a> {
    pub(super) builder: PointBuilder,
    pub(super) params: &'a [PointParam],
    pub(super) env: HashMap<&'a str, usize>,
}

impl Scope<'_> {
    /// Applies `make` to each coordinate of `value`.
    fn map(&mut self, value: Value, make: fn(usize) -> PointNode) -> Value {
        match value {
            Value::Scalar(x) => Value::Scalar(self.builder.intern(make(x))),
            Value::Vector(v) => Value::Vector(v.map(|x| self.builder.intern(make(x)))),
        }
    }

    /// Combines the coordinates of `left` and `right` with `make`.
    fn zip(
        &mut self,
        left: [usize; 3],
        right: [usize; 3],
        make: fn(usize, usize) -> PointNode,
    ) -> Value {
        Value::Vector([0, 1, 2].map(|i| self.builder.intern(make(left[i], right[i]))))
    }

    /// `left op right` where `op` is `+` or `-`.
    fn sum(&mut self, left: Value, right: Value, sub: bool, at: Span) -> Res<Value> {
        let make: fn(usize, usize) -> PointNode = if sub { PointNode::Sub } else { PointNode::Add };
        match (left, right) {
            (Value::Scalar(x), Value::Scalar(y)) => {
                Ok(Value::Scalar(self.builder.intern(make(x, y))))
            }
            (Value::Vector(x), Value::Vector(y)) => Ok(self.zip(x, y, make)),
            _ => Err(diag(
                at,
                format!(
                    "cannot combine {} and {} with `{}`",
                    left.name(),
                    right.name(),
                    if sub { '-' } else { '+' }
                ),
            )),
        }
    }

    /// `left * right`.
    fn product(&mut self, left: Value, right: Value, at: Span) -> Res<Value> {
        match (left, right) {
            (Value::Scalar(x), Value::Scalar(y)) => {
                Ok(Value::Scalar(self.builder.intern(PointNode::Mul(x, y))))
            }
            (Value::Scalar(s), Value::Vector(v)) => Ok(self.zip([s; 3], v, PointNode::Mul)),
            (Value::Vector(v), Value::Scalar(s)) => Ok(self.zip(v, [s; 3], PointNode::Mul)),
            (Value::Vector(_), Value::Vector(_)) => Err(diag(
                at,
                "a product of two `F64x3` is not supported; one factor must be an `f64`",
            )),
        }
    }
}

pub(super) fn parse_expr(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<Value> {
    let mut value = parse_term(cur, scope, depth)?;
    loop {
        let sub = if cur.peek_punct('+') {
            false
        } else if cur.peek_punct('-') {
            true
        } else {
            return Ok(value);
        };
        let at = operator_span(cur);
        cur.bump();
        let right = parse_term(cur, scope, depth)?;
        value = scope.sum(value, right, sub, at)?;
    }
}

fn parse_term(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<Value> {
    let mut value = parse_unary(cur, scope, depth)?;
    while cur.peek_punct('*') {
        let at = operator_span(cur);
        cur.bump();
        let right = parse_unary(cur, scope, depth)?;
        value = scope.product(value, right, at)?;
    }
    Ok(value)
}

fn parse_unary(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<Value> {
    let mut signs = 0usize;
    while cur.peek_punct('-') {
        cur.bump();
        signs += 1;
    }
    let mut value = parse_atom(cur, scope, depth)?;
    for _ in 0..signs {
        value = scope.map(value, PointNode::Neg);
    }
    Ok(value)
}

/// The position of the next token, which the caller knows is there.
pub(super) fn operator_span(cur: &Cursor) -> Span {
    cur.peek().map_or_else(|| cur.close(), TokenTree::span)
}

/// What an atom looks like, copied out of the token so the cursor can move.
enum Atom {
    Name(String, Span),
    Int(String, Span),
    Paren(TokenStream, Span, Span),
}

fn parse_atom(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<Value> {
    let atom = match cur.peek() {
        Some(TokenTree::Ident(id)) => Atom::Name(id.to_string(), id.span()),
        Some(TokenTree::Literal(lit)) => Atom::Int(lit.to_string(), lit.span()),
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
            Atom::Paren(g.stream(), g.span_open(), g.span_close())
        }
        _ => return Err(cur.expected("an expression")),
    };
    cur.bump();
    match atom {
        Atom::Int(text, span) => {
            let value = int_literal(&text, span)?;
            Ok(Value::Scalar(scope.builder.intern(PointNode::Int(value))))
        }
        Atom::Paren(stream, open, close) => {
            if depth + 1 > limits::MAX_NESTING {
                return Err(diag(open, limits::too_deep()));
            }
            let mut inner = Cursor::new(stream, Some(close));
            let value = parse_expr(&mut inner, scope, depth + 1)?;
            if inner.at_end() {
                Ok(value)
            } else {
                Err(inner.expected("`)`"))
            }
        }
        Atom::Name(name, span) => {
            let Some(&index) = scope.env.get(name.as_str()) else {
                return Err(diag(span, format!("unknown name `{name}`")));
            };
            if cur.peek_punct('.') {
                return Err(diag(
                    operator_span(cur),
                    format!("`{name}` is used whole: its coordinates cannot be accessed"),
                ));
            }
            Ok(match scope.params[index].ty {
                ParamType::F64 => Value::Scalar(scope.builder.intern(PointNode::Scalar(index))),
                ParamType::F64x3 => Value::Vector([0u8, 1, 2].map(|axis| {
                    scope
                        .builder
                        .intern(PointNode::Coord { param: index, axis })
                })),
            })
        }
    }
}
