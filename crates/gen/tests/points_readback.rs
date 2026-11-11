//! The text of the point module, read back: `evaluate`, `write_exact` and
//! the checks compute the polynomial of each declaration in every
//! coordinate, and every operation of `write_exact` is preceded by the check
//! of the requirements of its operands.

mod point_text;

use std::collections::BTreeMap;

use implicit_predicates_gen::{Graph, Node, PointType};
use point_text::{
    approx_roots, declarations, declared, expand, leaf, method, negate, push, read_values,
};

#[test]
fn evaluate_computes_the_declared_polynomial_of_each_coordinate() {
    for (point, stages, _) in declarations() {
        if point.is_explicit() {
            continue;
        }
        let body = method(&stages, &point.name, "evaluate");
        let (mut g, values) = read_values(&point, body);
        for (axis, root) in approx_roots(body).into_iter().enumerate() {
            g.root = values[&root];
            assert_eq!(expand(&g), declared(&point, axis), "{} {axis}", point.name);
        }
    }
}

/// An operand of an expansion function, by the node it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Operand {
    Scalar(usize),
    Expansion(usize),
    Lifted(usize),
    Empty,
}

fn operand(text: &str) -> (Operand, bool) {
    if text == "&[]" {
        return (Operand::Empty, false);
    }
    if let Some(v) = text.strip_prefix("&[v").and_then(|t| t.strip_suffix(']')) {
        return (Operand::Scalar(v.parse().unwrap()), false);
    }
    if let Some(v) = text.strip_prefix("-v") {
        return (Operand::Scalar(v.parse().unwrap()), true);
    }
    if let Some(v) = text.strip_prefix('v') {
        return (Operand::Scalar(v.parse().unwrap()), false);
    }
    for (prefix, kind) in [("&x", 'x'), ("&l", 'l')] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let (id, length) = rest.split_once("[..").unwrap();
            let id: usize = id.parse().unwrap();
            let expected = format!("{}{id}]", if kind == 'x' { 'n' } else { 'm' });
            assert_eq!(length, expected, "{text}");
            return (
                if kind == 'x' {
                    Operand::Expansion(id)
                } else {
                    Operand::Lifted(id)
                },
                false,
            );
        }
    }
    panic!("unknown operand {text}");
}

/// The arguments of the call `name(...)` that `text` holds, before `)`.
fn call_args(text: &str) -> (&str, Vec<&str>) {
    let (name, rest) = text.split_once('(').unwrap();
    let inner = rest.rsplit_once(')').unwrap().0;
    (name, inner.split(", ").collect())
}

/// One coordinate closure of `write_exact`, read back: its value as a node
/// of `g`, with every operation checked to be preceded by the check of the
/// requirements of its operands.
fn read_closure(g: &mut Graph, point: &PointType, lines: &[&str]) -> usize {
    let mut scalars: BTreeMap<usize, usize> = BTreeMap::new();
    let mut expansions: BTreeMap<usize, usize> = BTreeMap::new();
    let mut lifted: BTreeMap<usize, usize> = BTreeMap::new();
    let mut check: Option<(String, Vec<Operand>)> = None;
    for line in lines {
        if let Some(rest) = line.strip_prefix("let v") {
            let (name, value) = rest.trim_end_matches(';').split_once(" = ").unwrap();
            let node = match value.strip_prefix("-v") {
                Some(x) => {
                    let x = scalars[&x.parse::<usize>().unwrap()];
                    negate(g, x)
                }
                None => leaf(g, point, value).unwrap_or_else(|| panic!("{line}")),
            };
            scalars.insert(name.parse().unwrap(), node);
            continue;
        }
        if let Some(condition) = line
            .strip_prefix("if !(")
            .and_then(|l| l.strip_suffix(") {"))
        {
            let parts: Vec<(String, Operand)> = condition
                .split(" && ")
                .map(|part| {
                    let (name, args) = call_args(part);
                    assert_eq!(args.len(), 1, "{line}");
                    (name.to_string(), operand(args[0]).0)
                })
                .collect();
            let kind = parts[0].0.clone();
            assert!(parts.iter().all(|(k, _)| *k == kind), "{line}");
            check = Some((kind, parts.into_iter().map(|(_, o)| o).collect()));
            continue;
        }
        if line.starts_with("let mut ") || ["in_range = false;", "return 0;", "}"].contains(line) {
            continue;
        }
        if let Some(rest) = line.strip_prefix("let m") {
            let (id, call) = rest.trim_end_matches(';').split_once(" = ").unwrap();
            let (name, args) = call_args(call);
            assert_eq!(name, "expansion::from_f64");
            let (Operand::Scalar(s), false) = operand(args[0]) else {
                panic!("{line}");
            };
            assert_eq!(args[1], format!("&mut l{id}"));
            lifted.insert(id.parse().unwrap(), scalars[&s]);
            continue;
        }
        let (target, call) = match line.strip_prefix("let n") {
            Some(rest) => {
                let (id, call) = rest.trim_end_matches(';').split_once(" = ").unwrap();
                (Some(id.parse::<usize>().unwrap()), call)
            }
            None => (None, *line),
        };
        let (name, args) = call_args(call);
        let destination = match target {
            Some(id) => format!("&mut x{id}"),
            None => "out".to_string(),
        };
        let name = name.strip_prefix("expansion::").unwrap_or(name);
        let (inputs, rest) = if name == "product" {
            assert!(args[3].starts_with("&mut w"), "{line}");
            (&args[..2], &args[2..3])
        } else {
            args.split_at(args.len() - 1)
        };
        assert_eq!(rest[0], destination, "{line}");
        let read: Vec<(Operand, bool)> = inputs.iter().map(|a| operand(a)).collect();
        let value = |g: &mut Graph, (o, negated): (Operand, bool)| {
            let node = match o {
                Operand::Scalar(s) => scalars[&s],
                Operand::Expansion(e) => expansions[&e],
                Operand::Lifted(l) => lifted[&l],
                Operand::Empty => push(g, Node::Int(0)),
            };
            if negated { negate(g, node) } else { node }
        };
        let kind = match name {
            "from_f64" => None,
            "from_two_product" | "scale" | "product" => Some("multipliable"),
            _ => Some("summable"),
        };
        if let Some(kind) = kind {
            let (checked, operands) = check
                .take()
                .unwrap_or_else(|| panic!("no check before {line}"));
            assert_eq!(checked, kind, "{line}");
            let mut expected: Vec<Operand> = read
                .iter()
                .map(|&(o, _)| o)
                .filter(|&o| o != Operand::Empty)
                .collect();
            let mut operands = operands;
            expected.sort();
            operands.sort();
            assert_eq!(operands, expected, "{line}");
        }
        let node = match name {
            "from_f64" => value(g, read[0]),
            _ => {
                let x = value(g, read[0]);
                let y = value(g, read[1]);
                push(
                    g,
                    match name {
                        "from_two_sum" | "grow" | "sum" => Node::Add(x, y),
                        "from_two_diff" | "diff" => Node::Sub(x, y),
                        "from_two_product" | "scale" | "product" => Node::Mul(x, y),
                        _ => panic!("{line}"),
                    },
                )
            }
        };
        match target {
            Some(id) => {
                expansions.insert(id, node);
            }
            None => return node,
        }
    }
    panic!("the closure has no final expression");
}

#[test]
fn write_exact_computes_the_declared_polynomial_of_each_coordinate_with_its_checks() {
    for (point, stages, _) in declarations() {
        if point.is_explicit() {
            continue;
        }
        let body = method(&stages, &point.name, "write_exact");
        let lines: Vec<&str> = body.lines().collect();
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            let start = lines
                .iter()
                .position(|l| l.starts_with(&format!("let {name} = scratch.write(")))
                .unwrap();
            let end = start + lines[start..].iter().position(|l| *l == "});").unwrap();
            let mut g = Graph {
                nodes: Vec::new(),
                root: 0,
            };
            g.root = read_closure(&mut g, &point, &lines[start + 1..end]);
            assert_eq!(expand(&g), declared(&point, axis), "{} {name}", point.name);
            assert!(
                lines[end + 1]
                    .starts_with(&format!("let {name} = written(scratch, mark, {name}, ")),
                "{}",
                lines[end + 1]
            );
        }
    }
}

#[test]
fn the_checks_compute_the_declared_polynomial_of_each_coordinate() {
    for (point, _, checks) in declarations() {
        if point.is_explicit() {
            continue;
        }
        let start = checks
            .find(&format!("pub fn exact_{}(", point.name.to_lowercase()))
            .unwrap_or_else(|| panic!("no check of {}", point.name));
        let body = &checks[start..];
        let body = &body[..body.find("\n}\n").unwrap()];
        let mut g = Graph {
            nodes: Vec::new(),
            root: 0,
        };
        let mut values: BTreeMap<String, usize> = BTreeMap::new();
        let mut roots = Vec::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("let t") {
                let (name, value) = rest.trim_end_matches(';').split_once(" = ").unwrap();
                let operand = |t: &str| values[t.strip_prefix("&t").unwrap()];
                let node = match value.split(' ').collect::<Vec<_>>().as_slice() {
                    [x] if x.starts_with("-&t") => {
                        let x = operand(&x[1..]);
                        negate(&mut g, x)
                    }
                    [x] => leaf(&mut g, &point, x).unwrap_or_else(|| panic!("{line}")),
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
                values.insert(name.to_string(), node);
            } else if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                for root in inner.split(", ") {
                    let root = root.strip_suffix(".clone()").unwrap_or(root);
                    roots.push(values[root.strip_prefix('t').unwrap()]);
                }
            }
        }
        assert_eq!(roots.len(), 3, "{}", point.name);
        for (axis, root) in roots.into_iter().enumerate() {
            g.root = root;
            assert_eq!(expand(&g), declared(&point, axis), "{} {axis}", point.name);
        }
    }
}
