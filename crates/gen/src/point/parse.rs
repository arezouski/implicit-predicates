//! Reads point type declarations.
//!
//! ```text
//! decl   := 'point' NAME '(' params? ')' '=' expr ('where' cmps)? ';'
//! params := param (',' param)* ','?        param := NAME ':' ('F64x3' | 'f64')
//! cmps   := cmp (',' cmp)*                 cmp   := sum '<=' sum
//! sum    := (NAME | INTEGER) ('+' (NAME | INTEGER))*
//! expr   := term (('+' | '-') term)*
//! term   := unary ('*' unary)*
//! unary  := '-'* atom
//! atom   := INTEGER | NAME | '(' expr ')'
//! ```
//!
//! Operations on `F64x3` are per coordinate. A product is an `f64` by an
//! `F64x3` (in either order) or an `f64` by an `f64`; the sum, the difference
//! and the negation keep the type; an integer is an `f64`. The value of the
//! whole expression is an `F64x3`. In a comparison, names are `f64`
//! parameters.
//!
//! Like the expressions of predicates, chains are read in loops and only
//! parenthesized expressions recurse, within the limit on the depth of
//! brackets.

use proc_macro2::{Delimiter, LexError, Span, TokenStream, TokenTree};

use super::expr::{Scope, Value, operator_span, parse_expr};
use super::{
    Constraint, LinearSum, ParamType, PointBuilder, PointGraph, PointNode, PointParam, PointType,
};
use crate::cursor::Cursor;
use crate::error::{Diag, Error, Res, diag};
use crate::expr::int_literal;
use crate::limits;
use crate::parse::{Position, can_name_a_module};

/// Reads every point type declared in `source`.
///
/// `file` is only used to label errors. Points come in the order of their
/// declarations. A file without points gives an empty list. The functions of
/// the file are skipped without being read, since [`crate::parse`] reads them
/// and reports their errors; but what is skipped must start with `fn` or
/// `#`, and reach a `{` body, or it is an error at its position, so no
/// declaration is lost.
///
/// # Limits
///
/// No input makes this function panic or overflow the stack of the calling
/// thread. The only limit is the nesting of brackets, at most 32 deep, counting
/// the parentheses of the expression of a point; sums, products and runs of
/// unary minus are read in loops and may be of any length.
///
/// # Errors
///
/// Returns the first problem found, at the offending token: a token that the
/// grammar does not allow, a type other than `F64x3` and `f64`, a name that is
/// a Rust keyword or is defined twice, an unused parameter, a name that is not
/// a parameter, a product of two `F64x3`, a sum of an `F64x3` and an `f64`, an
/// expression that is not an `F64x3`, or a `where` clause over an `F64x3`.
pub fn parse_points(source: &str, file: &str) -> Result<Vec<PointType>, Error> {
    parse_source(source).map_err(|d| Error {
        file: file.to_string(),
        line: d.line,
        column: d.column,
        message: d.message,
    })
}

fn parse_source(source: &str) -> Res<Vec<PointType>> {
    limits::check_text(source)?;
    let tokens: TokenStream = source
        .parse()
        .map_err(|e: LexError| diag(e.span(), e.to_string()))?;
    let mut cur = Cursor::new(tokens, None);
    let mut points: Vec<PointType> = Vec::new();
    while !cur.at_end() {
        if cur.peek_word("point") {
            read_declaration(&mut cur, &mut points)?;
        } else {
            skip_function(&mut cur)?;
        }
    }
    Ok(points)
}

/// Reads the declaration at `cur` and adds it to `points`, which holds the
/// ones read before it (their names cannot be repeated).
pub(crate) fn read_declaration(cur: &mut Cursor, points: &mut Vec<PointType>) -> Res<()> {
    let point = parse_declaration(cur, points)?;
    points.push(point);
    Ok(())
}

/// Moves past a function of a predicate, with its attributes, without reading
/// it: [`crate::parse`] does. Anything that does not start like one is an
/// error, so a declaration cannot be lost behind a stray token.
fn skip_function(cur: &mut Cursor) -> Res<()> {
    if !(cur.peek_word("fn") || cur.peek_punct('#')) {
        return Err(cur.expected("`point` or `fn`"));
    }
    loop {
        let Some(token) = cur.peek() else {
            return Err(cur.expected("`{`"));
        };
        let is_body = matches!(token, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace);
        cur.bump();
        if is_body {
            return Ok(());
        }
    }
}

fn parse_declaration(cur: &mut Cursor, seen: &[PointType]) -> Res<PointType> {
    cur.bump();
    let (name, name_span) = parse_name(cur, "a point name", "a point")?;
    if seen.iter().any(|p| p.name == name) {
        return Err(diag(name_span, format!("`{name}` is already defined")));
    }
    let params = parse_params(cur)?;
    if !cur.peek_punct('=') {
        return Err(cur.expected("`=`"));
    }
    cur.bump();
    let start = cur.peek().map_or_else(|| cur.close(), TokenTree::span);
    let mut scope = Scope {
        builder: PointBuilder::default(),
        params: &params,
        env: params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.as_str(), i))
            .collect(),
    };
    let value = parse_expr(cur, &mut scope, 0)?;
    let Value::Vector(roots) = value else {
        return Err(diag(
            start,
            "the value of a point must be an `F64x3`, not an `f64`",
        ));
    };
    let constraints = if cur.peek_word("where") {
        cur.bump();
        parse_constraints(cur, &scope)?
    } else {
        Vec::new()
    };
    if !cur.peek_punct(';') {
        return Err(cur.expected("an operator, `where` or `;`"));
    }
    cur.bump();
    let graph = scope.builder.finish(roots);
    if let Some(param) = params
        .iter()
        .enumerate()
        .find_map(|(i, p)| (!is_used(&graph, i)).then_some(p))
    {
        return Err(Diag {
            line: param.position.line,
            column: param.position.column,
            message: format!(
                "unused parameter `{}`: the expression does not use it",
                param.name
            ),
        });
    }
    Ok(PointType {
        name,
        position: Position::of(name_span),
        params,
        graph,
        constraints,
    })
}

/// Tells whether the graph reads parameter `index`.
fn is_used(graph: &PointGraph, index: usize) -> bool {
    graph.nodes.iter().any(|n| {
        matches!(n, PointNode::Coord { param, .. } | PointNode::Scalar(param) if *param == index)
    })
}

/// Reads a name that is not a Rust keyword. `what` is for the message when
/// there is none, `of` for the one when it is reserved.
fn parse_name(cur: &mut Cursor, what: &str, of: &str) -> Res<(String, Span)> {
    let Some(TokenTree::Ident(id)) = cur.peek() else {
        return Err(cur.expected(what));
    };
    let (name, span) = (id.to_string(), id.span());
    if !can_name_a_module(&name) {
        return Err(diag(
            span,
            format!(
                "`{name}` cannot be used as the name of {of}: \
                 it is a Rust keyword or a raw identifier"
            ),
        ));
    }
    cur.bump();
    Ok((name, span))
}

/// Reads the parameter list.
fn parse_params(cur: &mut Cursor) -> Res<Vec<PointParam>> {
    let (stream, close) = match cur.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
            (g.stream(), g.span_close())
        }
        _ => return Err(cur.expected("`(`")),
    };
    cur.bump();
    let mut inside = Cursor::new(stream, Some(close));
    let mut params: Vec<PointParam> = Vec::new();
    while !inside.at_end() {
        let (name, span) = parse_name(&mut inside, "a parameter name", "a parameter")?;
        if params.iter().any(|p| p.name == name) {
            return Err(diag(span, format!("`{name}` redefines a name")));
        }
        if !inside.peek_punct(':') {
            return Err(inside.expected("`:`"));
        }
        inside.bump();
        let ty = match inside.peek() {
            Some(TokenTree::Ident(id)) if id == "F64x3" => ParamType::F64x3,
            Some(TokenTree::Ident(id)) if id == "f64" => ParamType::F64,
            Some(token) => {
                return Err(diag(
                    token.span(),
                    "a parameter must have type `F64x3` or `f64`",
                ));
            }
            None => return Err(inside.expected("a type: `F64x3` or `f64`")),
        };
        inside.bump();
        params.push(PointParam {
            name,
            ty,
            position: Position::of(span),
        });
        if !inside.at_end() {
            if !inside.peek_punct(',') {
                return Err(inside.expected("`,`"));
            }
            inside.bump();
        }
    }
    Ok(params)
}

/// Reads the comparisons after `where`, up to the `;`.
fn parse_constraints(cur: &mut Cursor, scope: &Scope) -> Res<Vec<Constraint>> {
    let mut constraints = Vec::new();
    loop {
        let start = operator_span(cur);
        let lhs = parse_sum(cur, scope)?;
        let is_le = cur.peek_punct('<')
            && matches!(cur.peek(), Some(TokenTree::Punct(p)) if p.spacing() == proc_macro2::Spacing::Joint)
            && cur
                .peek_at(1)
                .is_some_and(|t| crate::cursor::is_punct(Some(t), '='));
        if !is_le {
            return Err(cur.expected("`<=`: a `where` clause only has `<=` comparisons"));
        }
        cur.bump();
        cur.bump();
        let rhs = parse_sum(cur, scope)?;
        constraints.push(Constraint {
            lhs,
            rhs,
            position: Position::of(start),
        });
        if !cur.peek_punct(',') {
            return Ok(constraints);
        }
        cur.bump();
    }
}

/// Reads `a + b + 3`: `f64` parameters and integers joined by `+`.
fn parse_sum(cur: &mut Cursor, scope: &Scope) -> Res<LinearSum> {
    let mut sum = LinearSum {
        params: Vec::new(),
        constant: 0,
    };
    loop {
        match cur.peek() {
            Some(TokenTree::Ident(id)) => {
                let (name, span) = (id.to_string(), id.span());
                match scope.env.get(name.as_str()) {
                    None => return Err(diag(span, format!("unknown name `{name}`"))),
                    Some(&i) if scope.params[i].ty == ParamType::F64x3 => {
                        return Err(diag(
                            span,
                            format!(
                                "`{name}` is an `F64x3`: a `where` clause only uses `f64` parameters"
                            ),
                        ));
                    }
                    Some(&i) => sum.params.push(i),
                }
            }
            Some(TokenTree::Literal(lit)) => {
                let span = lit.span();
                let value = int_literal(&lit.to_string(), span)?;
                sum.constant = sum
                    .constant
                    .checked_add(value)
                    .ok_or_else(|| diag(span, "the sum of the integers does not fit in 64 bits"))?;
            }
            _ => return Err(cur.expected("an `f64` parameter or an integer")),
        }
        cur.bump();
        if cur.peek_punct('-') || cur.peek_punct('*') {
            return Err(diag(
                operator_span(cur),
                "only sums are supported in a `where` clause",
            ));
        }
        if !cur.peek_punct('+') {
            return Ok(sum);
        }
        cur.bump();
    }
}
