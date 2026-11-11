//! Reads `.pred` files with `syn` and builds expression graphs.

use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use syn::spanned::Spanned;
use syn::{FnArg, Item, ItemFn, Meta, Pat, ReturnType, Stmt, Type, Visibility};

use crate::error::{Error, Res, diag};
use crate::eval::{Binding, eval};
use crate::graph::{Builder, Graph, Node};
use crate::limits;

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

/// One function of the file, before forms are paired.
struct Form {
    name: String,
    name_span: Span,
    params: Vec<String>,
    graph: Graph,
    exact: bool,
}

/// Reads every predicate defined in `source`.
///
/// `file` is only used to label errors. Predicates come in the order of their
/// main forms.
///
/// # Limits
///
/// No input makes this function panic or overflow the stack of the calling
/// thread, whatever its size or shape; the limits are sized so that the worst
/// input they accept fits in a 1 MiB stack of a debug build. Input is refused
/// if it breaks one of these limits:
///
/// - brackets nest at most 16 deep, counting the body of the function
///   (brackets inside comments and literals do not count);
/// - on the way from a statement to any leaf, at most 600 of `+`, `-`, `*`
///   and `>` (the `->` of the header counts as two), at most 320 `.` and at
///   most 4 `=` (the counts include the enclosing statements, and comments
///   cost nothing);
/// - at most 2 punctuation tokens in a row, so `a - -b` is accepted but
///   `a - - -b` is not;
/// - only the tokens of the language of definitions are accepted: Rust
///   keywords other than `fn` and `let`, and punctuation other than
///   `+ - * . : , ; = # >` and brackets, are refused (`/` with its own
///   message, since division is not supported).
///
/// Whatever the limit, the message names it and tells the author to split the
/// expression with `let`. A flat sum of 150 terms and the shape of an in-sphere
/// determinant are well within them.
///
/// # Errors
///
/// Returns the first problem found: a syntax error, a construct outside the
/// accepted subset, an exceeded limit, an undefined or redefined name, an
/// unused parameter, or a misplaced `#[exact]` form.
pub fn parse(source: &str, file: &str) -> Result<Vec<Predicate>, Error> {
    parse_source(source).map_err(|d| Error {
        file: file.to_string(),
        line: d.line,
        column: d.column,
        message: d.message,
    })
}

fn parse_source(source: &str) -> Res<Vec<Predicate>> {
    limits::check_nesting(source)?;
    let tokens: TokenStream = syn::parse_str(source).map_err(|e| diag(e.span(), e.to_string()))?;
    limits::check_tokens(tokens.clone())?;
    let ast: syn::File = syn::parse2(tokens).map_err(|e| diag(e.span(), e.to_string()))?;
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
