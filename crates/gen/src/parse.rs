//! Reads `.pred` files with `syn` and builds expression graphs.

use std::collections::HashMap;

use proc_macro2::Span;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{
    BinOp, Expr, FnArg, Item, ItemFn, Lit, Member, Meta, Pat, ReturnType, Stmt, Type, UnOp,
    Visibility,
};

use crate::error::Error;
use crate::graph::{Builder, Graph, Node, NodeId};

/// A predicate read from a definition file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Predicate {
    /// Name of the function.
    pub name: String,
    /// Names of the point parameters, in order.
    pub params: Vec<String>,
    /// The main form of the polynomial.
    pub main: Graph,
    /// The form marked `#[exact]`, if the file has one.
    pub exact: Option<Graph>,
}

/// A problem at a position of the source, before the file name is known.
struct Diag {
    line: usize,
    column: usize,
    message: String,
}

type Res<T> = Result<T, Diag>;

fn diag(span: Span, message: impl Into<String>) -> Diag {
    let start = span.start();
    Diag {
        line: start.line,
        column: start.column + 1,
        message: message.into(),
    }
}

/// One function of the file, before forms are paired.
struct Form {
    name: String,
    name_span: Span,
    params: Vec<String>,
    graph: Graph,
    exact: bool,
}

/// What a name stands for inside a function body.
#[derive(Clone, Copy)]
enum Binding {
    Param(usize),
    Let(NodeId),
}

/// Reads every predicate defined in `source`.
///
/// `file` is only used to label errors. Predicates come in the order of their
/// main forms.
///
/// # Errors
///
/// Returns the first problem found: a syntax error, a construct outside the
/// accepted subset, an undefined or redefined name, an unused parameter, or a
/// misplaced `#[exact]` form.
pub fn parse(source: &str, file: &str) -> Result<Vec<Predicate>, Error> {
    parse_source(source).map_err(|d| Error {
        file: file.to_string(),
        line: d.line,
        column: d.column,
        message: d.message,
    })
}

/// Deepest bracket nesting accepted. The tokenizer recurses on brackets, so
/// the depth is checked before any parsing.
const MAX_NESTING: usize = 64;

/// Most tokens (words, numbers and punctuation marks other than brackets, `,`
/// and `;`) accepted between two `;`. The syntax tree of a long chain of
/// operators, casts or fields is as deep as the chain, and the parser recurses
/// on it.
const MAX_TOKENS: usize = 1000;

/// Deepest expression tree accepted, so that evaluation cannot exhaust the stack.
const MAX_EXPR_DEPTH: usize = 200;

/// Rejects sources that would nest too deeply for the recursive parser.
///
/// The check looks at raw characters, so brackets and operators inside
/// comments and strings count too; it is conservative.
fn check_limits(source: &str) -> Res<()> {
    let (mut depth, mut tokens) = (0usize, 0usize);
    let (mut line, mut column) = (1usize, 1usize);
    let mut in_word = false;
    for ch in source.chars() {
        let mut exceeded = None;
        let word = ch.is_alphanumeric() || ch == '_';
        if word && !in_word || ch.is_ascii_punctuation() && !"()[]{},;_".contains(ch) {
            tokens += 1;
            if tokens > MAX_TOKENS {
                exceeded = Some("too many tokens in one statement");
            }
        }
        in_word = word;
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                if depth > MAX_NESTING {
                    exceeded = Some("brackets are nested too deeply");
                }
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    tokens = 0;
                }
            }
            ';' => tokens = 0,
            _ => {}
        }
        if let Some(message) = exceeded {
            return Err(Diag {
                line,
                column,
                message: message.into(),
            });
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Ok(())
}

fn parse_source(source: &str) -> Res<Vec<Predicate>> {
    check_limits(source)?;
    let ast = syn::parse_file(source).map_err(|e| diag(e.span(), e.to_string()))?;
    if let Some(attr) = ast.attrs.first() {
        return Err(diag(attr.span(), "file attributes are not supported"));
    }
    let mut predicates: Vec<Predicate> = Vec::new();
    let mut exacts = Vec::new();
    for item in &ast.items {
        let Item::Fn(function) = item else {
            return Err(diag(item.span(), "only function definitions are allowed"));
        };
        let form = parse_fn(function)?;
        if form.exact {
            if exacts.iter().any(|e: &Form| e.name == form.name) {
                return Err(diag(
                    form.name_span,
                    format!("`{}` already has an exact form", form.name),
                ));
            }
            exacts.push(form);
        } else if predicates.iter().any(|p| p.name == form.name) {
            return Err(diag(
                form.name_span,
                format!("`{}` is already defined", form.name),
            ));
        } else {
            predicates.push(Predicate {
                name: form.name,
                params: form.params,
                main: form.graph,
                exact: None,
            });
        }
    }
    for form in exacts {
        let Some(predicate) = predicates.iter_mut().find(|p| p.name == form.name) else {
            return Err(diag(
                form.name_span,
                format!("the exact form of `{}` has no main form", form.name),
            ));
        };
        if predicate.params != form.params {
            return Err(diag(
                form.name_span,
                format!(
                    "the exact form of `{}` must have the parameters of the main form",
                    form.name
                ),
            ));
        }
        predicate.exact = Some(form.graph);
    }
    if predicates.is_empty() {
        return Err(diag(Span::call_site(), "no predicate definitions found"));
    }
    Ok(predicates)
}

fn parse_fn(function: &ItemFn) -> Res<Form> {
    let sig = &function.sig;
    let exact = parse_attrs(function)?;
    let plain = sig.constness.is_none()
        && sig.asyncness.is_none()
        && sig.unsafety.is_none()
        && sig.abi.is_none()
        && sig.variadic.is_none()
        && sig.generics.params.is_empty()
        && sig.generics.where_clause.is_none()
        && matches!(function.vis, Visibility::Inherited);
    if !plain {
        return Err(diag(
            sig.ident.span(),
            "only plain `fn name(...) -> Sign` functions are allowed",
        ));
    }
    match &sig.output {
        ReturnType::Type(_, ty) if is_path(ty, "Sign") => {}
        ReturnType::Type(_, ty) => return Err(diag(ty.span(), "the return type must be `Sign`")),
        ReturnType::Default => {
            return Err(diag(sig.ident.span(), "the return type must be `Sign`"));
        }
    }

    let mut env: HashMap<String, Binding> = HashMap::new();
    let mut params: Vec<(String, Span)> = Vec::new();
    for arg in &sig.inputs {
        let FnArg::Typed(typed) = arg else {
            return Err(diag(arg.span(), "methods are not supported"));
        };
        let Some(ident) = simple_ident(&typed.pat) else {
            return Err(diag(typed.pat.span(), "a parameter must be a plain name"));
        };
        if !is_path(&typed.ty, "Point3") {
            return Err(diag(typed.ty.span(), "a parameter must have type `Point3`"));
        }
        let name = ident.to_string();
        if env
            .insert(name.clone(), Binding::Param(params.len()))
            .is_some()
        {
            return Err(diag(ident.span(), format!("`{name}` redefines a name")));
        }
        params.push((name, ident.span()));
    }

    let mut builder = Builder::default();
    let mut root = None;
    let count = function.block.stmts.len();
    for (i, stmt) in function.block.stmts.iter().enumerate() {
        match stmt {
            Stmt::Local(local) => {
                let (Some(ident), Some(init)) = (simple_ident(&local.pat), &local.init) else {
                    return Err(diag(local.span(), "expected `let name = expression;`"));
                };
                if init.diverge.is_some() {
                    return Err(diag(local.span(), "expected `let name = expression;`"));
                }
                let name = ident.to_string();
                if env.contains_key(&name) {
                    return Err(diag(ident.span(), format!("`{name}` redefines a name")));
                }
                let value = eval(&init.expr, &mut builder, &env)?;
                env.insert(name, Binding::Let(value));
            }
            Stmt::Expr(expr, None) if i + 1 == count => {
                root = Some(eval(expr, &mut builder, &env)?);
            }
            other => {
                return Err(diag(
                    other.span(),
                    "expected `let` bindings followed by a final expression without `;`",
                ));
            }
        }
    }
    let Some(root) = root else {
        return Err(diag(
            function.block.span(),
            "the body needs a final expression",
        ));
    };
    let graph = builder.finish(root);

    for (point, (name, span)) in params.iter().enumerate() {
        let used = graph
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Coord { point: p, .. } if *p == point));
        if !used {
            return Err(diag(*span, format!("unused parameter `{name}`")));
        }
    }
    Ok(Form {
        name: sig.ident.to_string(),
        name_span: sig.ident.span(),
        params: params.into_iter().map(|(name, _)| name).collect(),
        graph,
        exact,
    })
}

/// Checks the attributes of a function and tells whether it is `#[exact]`.
fn parse_attrs(function: &ItemFn) -> Res<bool> {
    let mut exact = false;
    for attr in &function.attrs {
        match &attr.meta {
            Meta::Path(path) if path.is_ident("exact") && !exact => exact = true,
            _ => {
                return Err(diag(
                    attr.span(),
                    "the only attribute allowed is a single `#[exact]`",
                ));
            }
        }
    }
    Ok(exact)
}

fn simple_ident(pat: &Pat) -> Option<&syn::Ident> {
    match pat {
        Pat::Ident(p) if p.by_ref.is_none() && p.mutability.is_none() && p.subpat.is_none() => {
            Some(&p.ident)
        }
        _ => None,
    }
}

fn is_path(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(p) if p.qself.is_none() && p.path.is_ident(name))
}

fn eval(expr: &Expr, builder: &mut Builder, env: &HashMap<String, Binding>) -> Res<NodeId> {
    eval_at(expr, 0, builder, env)
}

fn eval_at(
    expr: &Expr,
    depth: usize,
    builder: &mut Builder,
    env: &HashMap<String, Binding>,
) -> Res<NodeId> {
    if depth > MAX_EXPR_DEPTH {
        return Err(diag(expr.span(), "the expression is nested too deeply"));
    }
    let depth = depth + 1;
    match expr {
        Expr::Paren(e) => eval_at(&e.expr, depth, builder, env),
        Expr::Group(e) => eval_at(&e.expr, depth, builder, env),
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
                let x = eval_at(&e.expr, depth, builder, env)?;
                Ok(builder.negate(x))
            }
            _ => Err(diag(e.op.span(), "only unary minus is supported")),
        },
        Expr::Binary(e) => {
            let make: fn(NodeId, NodeId) -> Node = match e.op {
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
            let x = eval_at(&e.left, depth, builder, env)?;
            let y = eval_at(&e.right, depth, builder, env)?;
            Ok(builder.intern(make(x, y)))
        }
        Expr::Path(p) if p.qself.is_none() && p.path.get_ident().is_some() => {
            let ident = p.path.get_ident().expect("checked by the guard");
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
            let point = match &*e.base {
                Expr::Path(p) if p.qself.is_none() && p.path.get_ident().is_some() => {
                    let ident = p.path.get_ident().expect("checked by the guard");
                    match env.get(&ident.to_string()) {
                        Some(Binding::Param(point)) => *point,
                        Some(Binding::Let(_)) => {
                            return Err(diag(ident.span(), format!("`{ident}` is not a point")));
                        }
                        None => return Err(diag(ident.span(), format!("unknown name `{ident}`"))),
                    }
                }
                other => {
                    return Err(diag(
                        other.span(),
                        "fields can only be taken from a point parameter",
                    ));
                }
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
