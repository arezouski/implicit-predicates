//! What the tests of the point module share: the declarations they read
//! back and the reading of the emitted text as graphs over the parameters.

// Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::collections::BTreeMap;

use implicit_predicates_gen::{
    Generated, Graph, Node, ParamType, PointNode, PointType, Polynomial, Position, canonical,
    generate_points, parse_points,
};

pub const POINTS: &str = include_str!("../fixtures/points.pred");
pub const ORIENT3D: &str = include_str!("../fixtures/orient3d.pred");

/// Declarations that use every operation of the emitted code: negations of
/// scalars and of expansions, sums and differences of every kind, products
/// of two scalars, of a scalar and an expansion and of two expansions, a
/// scalar minus an expansion, integers, and coordinates that are leaves.
pub const VARIED: &str = "
point Mix(a: F64x3, b: F64x3, s: f64, t: f64) = -a - b * s * 2 + (t + 3) * a - -b
    where 0 <= s, 2 <= t + 1;
point Products(a: F64x3, s: f64, t: f64) = (s - t) * (a - a * s) + (1 - s) * (s * a);
point Twice(a: F64x3) = --a;
point Negated(a: F64x3, b: F64x3, s: f64) = -(a * s) + b - b * s;
point Shared(p: F64x3, q: F64x3, t: f64) = q + t * (p - q) + 0 * t * p;
";

pub fn generated(source: &str) -> Generated {
    generate_points(source, "predicates/points.pred", "run the generator")
        .unwrap()
        .expect("the source declares points")
}

pub fn declarations() -> Vec<(PointType, String, String)> {
    let mut out = Vec::new();
    for source in [POINTS, VARIED] {
        let g = generated(source);
        for point in parse_points(source, "t.pred").unwrap() {
            out.push((point, g.stages.clone(), g.checks.clone()));
        }
    }
    out
}

/// The canonical polynomial of `graph`.
pub fn expand(graph: &Graph) -> Polynomial {
    canonical(graph, "t.pred", Position { line: 1, column: 1 }).unwrap()
}

/// The polynomial of coordinate `axis` of `point`, a scalar parameter being
/// the variable of its first coordinate.
pub fn declared(point: &PointType, axis: usize) -> Polynomial {
    let mut graph = Graph {
        nodes: Vec::new(),
        root: 0,
    };
    let mut id = Vec::new();
    for node in &point.graph.nodes {
        let new = match *node {
            PointNode::Coord { param, axis } => {
                push(&mut graph, Node::Coord { point: param, axis })
            }
            PointNode::Scalar(param) => push(
                &mut graph,
                Node::Coord {
                    point: param,
                    axis: 0,
                },
            ),
            PointNode::Int(n) => push(&mut graph, Node::Int(n)),
            PointNode::Neg(x) => {
                let zero = push(&mut graph, Node::Int(0));
                push(&mut graph, Node::Sub(zero, id[x]))
            }
            PointNode::Add(x, y) => push(&mut graph, Node::Add(id[x], id[y])),
            PointNode::Sub(x, y) => push(&mut graph, Node::Sub(id[x], id[y])),
            PointNode::Mul(x, y) => push(&mut graph, Node::Mul(id[x], id[y])),
        };
        id.push(new);
    }
    graph.root = id[point.graph.roots[axis]];
    expand(&graph)
}

pub fn push(graph: &mut Graph, node: Node) -> usize {
    graph.nodes.push(node);
    graph.nodes.len() - 1
}

/// Which parameter each slot `c[i]` and `s[i]` of an emitted function
/// holds, by index into the parameters of the point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub vectors: Vec<usize>,
    pub scalars: Vec<usize>,
}

impl Binding {
    /// The slots in the order of the declaration, as the checks take them.
    pub fn declared(point: &PointType) -> Binding {
        let of = |ty: ParamType| {
            (point.params.iter().enumerate())
                .filter(|(_, p)| p.ty == ty)
                .map(|(i, _)| i)
                .collect()
        };
        Binding {
            vectors: of(ParamType::F64x3),
            scalars: of(ParamType::F64),
        }
    }

    /// The slots that the call `{callee}({before}[a, b], [c])` in `body`
    /// fills, each argument being `{prefix}{name}` for a parameter of
    /// `point`, of the type of its slot. A point without `f64` parameters
    /// has no second array.
    pub fn from_call(
        point: &PointType,
        body: &str,
        callee: &str,
        before: &str,
        prefix: &str,
    ) -> Binding {
        let start = body
            .find(&format!("{callee}({before}"))
            .unwrap_or_else(|| panic!("no call of {callee} in {body}"));
        let rest = &body[start + callee.len() + 1 + before.len()..];
        let end = [");", ")\n"]
            .iter()
            .filter_map(|e| rest.find(e))
            .min()
            .expect("the end of the call");
        let args = &rest[..end];
        let arrays: Vec<&str> = args
            .split("], ")
            .map(|a| a.trim_start_matches('[').trim_end_matches(']'))
            .collect();
        let slots = |text: &str, ty: ParamType| -> Vec<usize> {
            if text.is_empty() {
                return Vec::new();
            }
            text.split(", ")
                .map(|arg| {
                    let name = arg.strip_prefix(prefix).unwrap_or_else(|| panic!("{arg}"));
                    let i = (point.params.iter())
                        .position(|p| p.name == name)
                        .unwrap_or_else(|| panic!("{name} is not a parameter"));
                    assert_eq!(point.params[i].ty, ty, "{name} in a slot of another type");
                    i
                })
                .collect()
        };
        let binding = Binding {
            vectors: slots(arrays[0], ParamType::F64x3),
            scalars: arrays
                .get(1)
                .map_or(Vec::new(), |a| slots(a, ParamType::F64)),
        };
        let declared = Binding::declared(point);
        assert_eq!(binding.vectors.len(), declared.vectors.len(), "{args}");
        assert_eq!(binding.scalars.len(), declared.scalars.len(), "{args}");
        binding
    }
}

/// The node of a leaf written `c[i][k]`, `s[i]` or as an integer literal,
/// with or without `.clone()` or `Dyadic::from_f64(..)` around it.
pub fn leaf(g: &mut Graph, binding: &Binding, text: &str) -> Option<usize> {
    let text = text.strip_suffix(".clone()").unwrap_or(text);
    let text = (text.strip_prefix("Dyadic::from_f64("))
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(text);
    if let Some(rest) = text.strip_prefix("c[") {
        let (slot, rest) = rest.split_once("][")?;
        let axis = rest.strip_suffix(']')?.parse().ok()?;
        let param = binding.vectors[slot.parse::<usize>().ok()?];
        return Some(push(g, Node::Coord { point: param, axis }));
    }
    if let Some(rest) = text.strip_prefix("s[") {
        let param = binding.scalars[rest.strip_suffix(']')?.parse::<usize>().ok()?];
        return Some(push(
            g,
            Node::Coord {
                point: param,
                axis: 0,
            },
        ));
    }
    let value: f64 = text.parse().ok()?;
    assert_eq!(value, value.trunc(), "{text}");
    Some(push(g, Node::Int(value as i64)))
}

pub fn negate(g: &mut Graph, x: usize) -> usize {
    let zero = push(g, Node::Int(0));
    push(g, Node::Sub(zero, x))
}

/// The body of the function `fn {name}(` of the implementation of `Point`
/// for `ty`.
pub fn point_method<'a>(code: &'a str, ty: &str, name: &str) -> &'a str {
    method_in(code, &format!("impl Point for {ty} {{\n"), name)
}

/// The body of the function `fn {name}(` that follows `impl {ty} {`.
pub fn method<'a>(code: &'a str, ty: &str, name: &str) -> &'a str {
    method_in(code, &format!("impl {ty} {{\n"), name)
}

/// The body of the function `fn {name}(` after the first `block`.
fn method_in<'a>(code: &'a str, block: &str, name: &str) -> &'a str {
    let start = code.find(block).expect("the impl block");
    let rest = &code[start..];
    let start = rest.find(&format!("fn {name}(")).expect("the function");
    let rest = &rest[start..];
    &rest[..rest.find("\n}\n\n").expect("the end of the function")]
}

/// The value statements `let v{n} = ...;` of `evaluate`, read back, and the
/// graph whose node `v{n}` is at `values[n]`.
pub fn read_values(binding: &Binding, body: &str) -> (Graph, BTreeMap<usize, usize>) {
    let mut g = Graph {
        nodes: Vec::new(),
        root: 0,
    };
    let mut values = BTreeMap::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("let v") else {
            continue;
        };
        let (name, value) = rest.trim_end_matches(';').split_once(" = ").unwrap();
        let id: usize = name.parse().unwrap();
        let operand =
            |text: &str| values[&text.strip_prefix('v').unwrap().parse::<usize>().unwrap()];
        let node = match value.split(' ').collect::<Vec<_>>().as_slice() {
            [x] if x.starts_with("-v") => {
                let x = operand(&x[1..]);
                negate(&mut g, x)
            }
            [x] => leaf(&mut g, binding, x).unwrap_or_else(|| panic!("{line}")),
            [x, op, y] => {
                let (x, y) = (operand(x), operand(y));
                push(
                    &mut g,
                    match *op {
                        "+" => Node::Add(x, y),
                        "-" => Node::Sub(x, y),
                        "*" => Node::Mul(x, y),
                        _ => panic!("{line}"),
                    },
                )
            }
            _ => panic!("{line}"),
        };
        values.insert(id, node);
    }
    (g, values)
}

/// The roots `[v{a}, v{b}, v{c}]` of `let approx = ...;`.
pub fn approx_roots(body: &str) -> [usize; 3] {
    let line = body
        .lines()
        .find(|l| l.starts_with("let approx = ["))
        .unwrap();
    let inner = line
        .strip_prefix("let approx = [")
        .unwrap()
        .strip_suffix("];")
        .unwrap();
    let ids: Vec<usize> = inner.split(", ").map(|v| v[1..].parse().unwrap()).collect();
    [ids[0], ids[1], ids[2]]
}
