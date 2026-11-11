//! Turns the expression of a definition into graph nodes.

use std::collections::HashMap;

use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, Lit, Member, UnOp};

use crate::error::{Res, diag};
use crate::graph::{Builder, Node, NodeId};

/// What a name stands for inside a function body.
#[derive(Clone, Copy)]
pub(crate) enum Binding {
    Param(usize),
    Let(NodeId),
}

/// Builds a node from the ids of its two operands.
type Make = fn(NodeId, NodeId) -> Node;

/// Adds the nodes of `expr` to `builder` and returns the id of its value.
///
/// Recursion follows brackets, unary operators and right operands only. The
/// chain of left operands of `+`, `-` and `*` is walked in a loop, so a long
/// sum does not deepen the stack.
pub(crate) fn eval(
    expr: &Expr,
    builder: &mut Builder,
    env: &HashMap<String, Binding>,
) -> Res<NodeId> {
    match expr {
        Expr::Paren(e) => eval(&e.expr, builder, env),
        Expr::Group(e) => eval(&e.expr, builder, env),
        Expr::Lit(e) => match &e.lit {
            Lit::Int(int) if int.suffix().is_empty() => {
                let value = int
                    .base10_parse::<i64>()
                    .map_err(|_| diag(int.span(), "integer literal does not fit in 64 bits"))?;
                Ok(builder.intern(Node::Int(value)))
            }
            Lit::Int(int) => Err(diag(int.span(), "integer literals take no suffix")),
            Lit::Float(float) => Err(diag(float.span(), "float literals are not supported")),
            other => Err(diag(other.span(), "only integer literals are supported")),
        },
        Expr::Unary(e) => match e.op {
            UnOp::Neg(_) => {
                let x = eval(&e.expr, builder, env)?;
                Ok(builder.negate(x))
            }
            _ => Err(diag(e.op.span(), "only unary minus is supported")),
        },
        Expr::Binary(_) => eval_binary(expr, builder, env),
        Expr::Path(_) => {
            let ident =
                plain_ident(expr).ok_or_else(|| diag(expr.span(), "this name is not supported"))?;
            match env.get(&ident.to_string()) {
                Some(Binding::Let(id)) => Ok(*id),
                Some(Binding::Param(_)) => Err(diag(
                    ident.span(),
                    format!(
                        "point `{ident}` can only be used through its coordinates, as `{ident}.x`"
                    ),
                )),
                None => Err(diag(ident.span(), format!("unknown name `{ident}`"))),
            }
        }
        Expr::Field(e) => {
            let Some(ident) = plain_ident(&e.base) else {
                return Err(diag(
                    e.dot_token.span(),
                    "fields can only be taken from a point parameter",
                ));
            };
            let point = match env.get(&ident.to_string()) {
                Some(Binding::Param(point)) => *point,
                Some(Binding::Let(_)) => {
                    return Err(diag(ident.span(), format!("`{ident}` is not a point")));
                }
                None => return Err(diag(ident.span(), format!("unknown name `{ident}`"))),
            };
            let axis = match &e.member {
                Member::Named(id) if id == "x" => 0,
                Member::Named(id) if id == "y" => 1,
                Member::Named(id) if id == "z" => 2,
                member => {
                    return Err(diag(
                        member.span(),
                        format!(
                            "unknown field `{}`; expected x, y or z",
                            member.to_token_stream()
                        ),
                    ));
                }
            };
            Ok(builder.intern(Node::Coord { point, axis }))
        }
        other => Err(diag(other.span(), "this expression is not supported")),
    }
}

/// Evaluates a chain of binary operators without recursing along its left
/// operands.
fn eval_binary(expr: &Expr, builder: &mut Builder, env: &HashMap<String, Binding>) -> Res<NodeId> {
    let mut pending: Vec<(Make, &Expr)> = Vec::new();
    let mut leftmost = expr;
    while let Expr::Binary(e) = leftmost {
        let make: Make = match e.op {
            BinOp::Add(_) => Node::Add,
            BinOp::Sub(_) => Node::Sub,
            BinOp::Mul(_) => Node::Mul,
            BinOp::Div(_) => return Err(diag(e.op.span(), "division is not supported")),
            _ => {
                let op = e.op.to_token_stream();
                return Err(diag(
                    e.op.span(),
                    format!("operator `{op}` is not supported"),
                ));
            }
        };
        pending.push((make, &e.right));
        leftmost = &e.left;
    }
    let mut value = eval(leftmost, builder, env)?;
    for (make, right) in pending.into_iter().rev() {
        let right = eval(right, builder, env)?;
        value = builder.intern(make(value, right));
    }
    Ok(value)
}

/// The name an expression consists of, if it is a single plain identifier.
pub(crate) fn plain_ident(expr: &Expr) -> Option<&syn::Ident> {
    match expr {
        Expr::Path(p) if p.qself.is_none() => p.path.get_ident(),
        _ => None,
    }
}
