//! Reads expressions and builds their graph nodes.
//!
//! The grammar, from loosest to tightest binding:
//!
//! ```text
//! expr  := term (('+' | '-') term)*
//! term  := unary ('*' unary)*
//! unary := '-'* atom
//! atom  := INTEGER | NAME | NAME '.' ('x' | 'y' | 'z') | '(' expr ')'
//! ```
//!
//! Sums, products and runs of unary minus are read in loops. Only a
//! parenthesized expression recurses, and the depth of brackets is bounded.

use std::collections::HashMap;

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};

use crate::cursor::Cursor;
use crate::error::{Res, diag};
use crate::graph::{Builder, Node, NodeId};
use crate::limits;

/// What a name stands for inside a function body.
#[derive(Clone, Copy)]
pub(crate) enum Binding {
    Param(usize),
    Let(NodeId),
}

/// The graph under construction and the names in scope.
pub(crate) struct Scope<'a> {
    pub(crate) builder: &'a mut Builder,
    pub(crate) env: &'a HashMap<String, Binding>,
}

/// Reads an expression from `cur`, which sits at bracket depth `depth`, and
/// returns the id of its value. It stops at the first token that cannot
/// continue the expression.
pub(crate) fn parse_expr(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<NodeId> {
    let mut value = parse_term(cur, scope, depth)?;
    loop {
        let make: fn(NodeId, NodeId) -> Node = if cur.peek_punct('+') {
            Node::Add
        } else if cur.peek_punct('-') {
            Node::Sub
        } else {
            return Ok(value);
        };
        cur.bump();
        let right = parse_term(cur, scope, depth)?;
        value = scope.builder.intern(make(value, right));
    }
}

fn parse_term(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<NodeId> {
    let mut value = parse_unary(cur, scope, depth)?;
    while cur.peek_punct('*') {
        cur.bump();
        let right = parse_unary(cur, scope, depth)?;
        value = scope.builder.intern(Node::Mul(value, right));
    }
    Ok(value)
}

fn parse_unary(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<NodeId> {
    let mut signs = 0usize;
    while cur.peek_punct('-') {
        cur.bump();
        signs += 1;
    }
    let mut value = parse_atom(cur, scope, depth)?;
    for _ in 0..signs {
        value = scope.builder.negate(value);
    }
    Ok(value)
}

/// What an atom looks like, copied out of the token so the cursor can move.
enum Atom {
    Name(String, Span),
    Int(String, Span),
    Paren(TokenStream, Span, Span),
}

fn parse_atom(cur: &mut Cursor, scope: &mut Scope, depth: usize) -> Res<NodeId> {
    let atom = match cur.peek() {
        Some(TokenTree::Ident(id)) if id != "let" && id != "fn" => {
            Atom::Name(id.to_string(), id.span())
        }
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
            Ok(scope.builder.intern(Node::Int(value)))
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
        Atom::Name(name, span) => parse_name(&name, span, cur, scope),
    }
}

/// Reads what follows a name: a coordinate access, or nothing for a binding.
fn parse_name(name: &str, span: Span, cur: &mut Cursor, scope: &mut Scope) -> Res<NodeId> {
    let Some(&binding) = scope.env.get(name) else {
        return Err(diag(span, format!("unknown name `{name}`")));
    };
    if !cur.peek_punct('.') {
        return match binding {
            Binding::Let(id) => Ok(id),
            Binding::Param(_) => Err(diag(
                span,
                format!("point `{name}` can only be used through its coordinates, as `{name}.x`"),
            )),
        };
    }
    let Binding::Param(point) = binding else {
        return Err(diag(span, format!("`{name}` is not a point")));
    };
    cur.bump();
    let axis = match cur.peek() {
        Some(TokenTree::Ident(id)) if id == "x" => 0,
        Some(TokenTree::Ident(id)) if id == "y" => 1,
        Some(TokenTree::Ident(id)) if id == "z" => 2,
        Some(token) => {
            let text: String = token.to_string().chars().take(24).collect();
            return Err(diag(
                token.span(),
                format!("unknown field `{text}`; expected x, y or z"),
            ));
        }
        None => return Err(cur.expected("a field: x, y or z")),
    };
    cur.bump();
    Ok(scope.builder.intern(Node::Coord { point, axis }))
}

/// Converts the text of a literal token into an integer.
fn int_literal(text: &str, span: Span) -> Res<i64> {
    let starts_with_digit = text.starts_with(|c: char| c.is_ascii_digit());
    let is_float = starts_with_digit
        && !text.starts_with("0x")
        && (text.contains('.') || text.contains(['e', 'E']));
    if is_float {
        return Err(diag(span, "float literals are not supported"));
    }
    if !starts_with_digit {
        return Err(diag(span, "only integer literals are supported"));
    }
    if !text.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return Err(diag(
            span,
            "only plain decimal integer literals are supported",
        ));
    }
    let digits: String = text.chars().filter(|&c| c != '_').collect();
    digits
        .parse::<i64>()
        .map_err(|_| diag(span, "integer literal does not fit in 64 bits"))
}
