//! Reads `.pred` files and builds expression graphs.
//!
//! The grammar is exactly the accepted subset:
//!
//! ```text
//! file   := item*
//! item   := ('#' '[' 'exact' ']')? 'fn' NAME '(' params? ')' '->' 'Sign' block
//! params := param (',' param)* ','?        param := NAME ':' 'Point3'
//! block  := '{' ('let' NAME '=' expr ';')* expr '}'
//! ```
//!
//! Expressions are read by [`crate::expr`].

use std::collections::HashMap;

use proc_macro2::{Delimiter, LexError, Span, TokenStream, TokenTree};

use crate::cursor::{Cursor, is_punct};
use crate::error::{Error, Res, diag};
use crate::expr::{Binding, Scope, parse_expr};
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
    /// Position of the name of the main form.
    pub position: Position,
    /// Position of the name of the exact form, if there is one.
    pub exact_position: Option<Position>,
}

/// A position in a definition file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// Line, starting at 1.
    pub line: usize,
    /// Column, starting at 1.
    pub column: usize,
}

impl Position {
    fn of(span: Span) -> Self {
        let start = span.start();
        Position {
            line: start.line,
            column: start.column + 1,
        }
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

/// Reads every predicate defined in `source`.
///
/// `file` is only used to label errors. Predicates come in the order of their
/// main forms.
///
/// # Limits
///
/// No input makes this function panic or overflow the stack of the calling
/// thread, whatever its size or shape; a debug build with a 1 MiB stack is
/// enough. The only limit is the nesting of brackets: at most 32 deep, counting
/// the braces of the function body, so a parenthesized expression may nest 31
/// deep. Comments and literals do not count toward it. Sums, products and runs
/// of unary minus are read in loops and have no limit beyond memory. An error
/// for brackets that are nested too deeply names the limit and suggests
/// splitting the expression with `let`.
///
/// # Errors
///
/// Returns the first problem found: a token that the grammar of definitions
/// does not allow, an undefined or redefined name, an
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
    let tokens: TokenStream = source
        .parse()
        .map_err(|e: LexError| diag(e.span(), e.to_string()))?;
    let mut cur = Cursor::new(tokens, None);
    let mut predicates: Vec<Predicate> = Vec::new();
    let mut exacts: Vec<Form> = Vec::new();
    while !cur.at_end() {
        let form = parse_item(&mut cur)?;
        if form.exact {
            if exacts.iter().any(|e| e.name == form.name) {
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
                position: Position::of(form.name_span),
                exact_position: None,
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
        predicate.exact_position = Some(Position::of(form.name_span));
    }
    if predicates.is_empty() {
        return Err(diag(Span::call_site(), "no predicate definitions found"));
    }
    Ok(predicates)
}

/// Reads one function, with its optional `#[exact]` attribute.
fn parse_item(cur: &mut Cursor) -> Res<Form> {
    let exact = parse_attr(cur)?;
    if !cur.peek_word("fn") {
        return Err(cur.expected("`fn`"));
    }
    cur.bump();
    let (name, name_span) = parse_name(cur, "a function name")?;
    let (params, spans) = parse_params(cur)?;
    if !(is_punct(cur.peek(), '-') && is_punct(cur.peek_at(1), '>')) {
        return Err(cur.expected("`->`"));
    }
    cur.bump();
    cur.bump();
    if !cur.peek_word("Sign") {
        return Err(match cur.peek() {
            Some(token) => diag(token.span(), "the return type must be `Sign`"),
            None => cur.expected("`Sign`"),
        });
    }
    cur.bump();
    let (body, close) = match cur.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
            (g.stream(), g.span_close())
        }
        _ => return Err(cur.expected("`{`")),
    };
    cur.bump();
    let graph = parse_body(Cursor::new(body, Some(close)), &params)?;
    for (point, (name, span)) in params.iter().zip(&spans).enumerate() {
        let used = graph
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Coord { point: p, .. } if *p == point));
        if !used {
            return Err(diag(*span, format!("unused parameter `{name}`")));
        }
    }
    Ok(Form {
        name,
        name_span,
        params,
        graph,
        exact,
    })
}

/// Reads `#[exact]` if it is there and tells whether it was.
fn parse_attr(cur: &mut Cursor) -> Res<bool> {
    let Some(TokenTree::Punct(hash)) = cur.peek() else {
        return Ok(false);
    };
    if hash.as_char() != '#' {
        return Ok(false);
    }
    let span = hash.span();
    cur.bump();
    let is_exact = match cur.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket => {
            let mut inside = Cursor::new(g.stream(), Some(g.span_close()));
            let found = inside.peek_word("exact");
            inside.bump();
            found && inside.at_end()
        }
        _ => false,
    };
    if !is_exact {
        return Err(diag(span, "the only attribute allowed is `#[exact]`"));
    }
    cur.bump();
    if is_punct(cur.peek(), '#') {
        return Err(diag(
            cur.peek().map_or(span, TokenTree::span),
            "the only attribute allowed is a single `#[exact]`",
        ));
    }
    Ok(true)
}

/// Reads a name: any identifier except `fn` and `let`.
fn parse_name(cur: &mut Cursor, what: &str) -> Res<(String, Span)> {
    match cur.peek() {
        Some(TokenTree::Ident(id)) if id != "fn" && id != "let" => {
            let result = (id.to_string(), id.span());
            cur.bump();
            Ok(result)
        }
        _ => Err(cur.expected(what)),
    }
}

/// Reads the parameter list and returns the names with their positions.
fn parse_params(cur: &mut Cursor) -> Res<(Vec<String>, Vec<Span>)> {
    let (stream, close) = match cur.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
            (g.stream(), g.span_close())
        }
        _ => return Err(cur.expected("`(`")),
    };
    cur.bump();
    let mut inside = Cursor::new(stream, Some(close));
    let (mut names, mut spans): (Vec<String>, Vec<Span>) = (Vec::new(), Vec::new());
    while !inside.at_end() {
        let (name, span) = parse_name(&mut inside, "a parameter name")?;
        if names.contains(&name) {
            return Err(diag(span, format!("`{name}` redefines a name")));
        }
        if !inside.peek_punct(':') {
            return Err(inside.expected("`:`"));
        }
        inside.bump();
        match inside.peek() {
            Some(TokenTree::Ident(id)) if id == "Point3" => inside.bump(),
            Some(token) => {
                return Err(diag(token.span(), "a parameter must have type `Point3`"));
            }
            None => return Err(inside.expected("the type `Point3`")),
        }
        names.push(name);
        spans.push(span);
        if !inside.at_end() {
            if !inside.peek_punct(',') {
                return Err(inside.expected("`,`"));
            }
            inside.bump();
        }
    }
    Ok((names, spans))
}

/// Reads the body of a function: `let` bindings and a final expression.
fn parse_body(mut body: Cursor, params: &[String]) -> Res<Graph> {
    let mut env: HashMap<String, Binding> = params
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), Binding::Param(i)))
        .collect();
    let mut builder = Builder::default();
    loop {
        if body.at_end() {
            return Err(diag(body.close(), "the body needs a final expression"));
        }
        if !body.peek_word("let") {
            let mut scope = Scope {
                builder: &mut builder,
                env: &env,
            };
            let root = parse_expr(&mut body, &mut scope, 1)?;
            if let Some(token) = body.peek() {
                return Err(if is_punct(Some(token), ';') {
                    diag(
                        token.span(),
                        "expected `let` bindings followed by a final expression without `;`",
                    )
                } else {
                    body.expected("an operator or the end of the body")
                });
            }
            return Ok(builder.finish(root));
        }
        body.bump();
        let (name, span) = parse_name(&mut body, "a name")?;
        if env.contains_key(&name) {
            return Err(diag(span, format!("`{name}` redefines a name")));
        }
        if !body.peek_punct('=') {
            return Err(body.expected("`=`"));
        }
        body.bump();
        let mut scope = Scope {
            builder: &mut builder,
            env: &env,
        };
        let value = parse_expr(&mut body, &mut scope, 1)?;
        if !body.peek_punct(';') {
            return Err(body.expected("an operator or `;`"));
        }
        body.bump();
        env.insert(name, Binding::Let(value));
    }
}
