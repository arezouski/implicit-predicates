//! The value of emitted code, read back from its text as a graph over the
//! coordinates, so that a wrong rendering of an operation cannot pass.

use std::collections::BTreeMap;

use crate::graph::{Graph, Node, NodeId};

/// A graph under construction, without sharing.
#[derive(Default)]
pub(super) struct Symbolic {
    nodes: Vec<Node>,
}

impl Symbolic {
    pub(super) fn push(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub(super) fn coordinate(&mut self, point: usize, axis: u8) -> NodeId {
        self.push(Node::Coord { point, axis })
    }

    pub(super) fn negate(&mut self, x: NodeId) -> NodeId {
        let zero = self.push(Node::Int(0));
        self.push(Node::Sub(zero, x))
    }

    pub(super) fn finish(self, root: NodeId) -> Graph {
        Graph {
            nodes: self.nodes,
            root,
        }
    }
}

/// `point` and `axis` of a coordinate written `{array}[point][axis]`.
fn coordinate(text: &str, array: &str) -> Option<(usize, u8)> {
    let rest = text.strip_prefix(array)?.strip_prefix('[')?;
    let (point, axis) = rest.strip_suffix(']')?.split_once("][")?;
    Some((point.parse().ok()?, axis.parse().ok()?))
}

/// The value of the statements `let t{n} = ...;` and `let det = ...;` of
/// the filter (`array` is `p`, operands by value) or of the exact
/// evaluator of the checks (`array` is `c`, operands borrowed).
pub(super) fn interpret_statements(code: &str, array: &str) -> Graph {
    let mut g = Symbolic::default();
    let mut names: BTreeMap<String, NodeId> = BTreeMap::new();
    let operand = |g: &mut Symbolic, names: &BTreeMap<String, NodeId>, text: &str| {
        let text = text.strip_prefix('&').unwrap_or(text);
        match coordinate(text, array) {
            Some((point, axis)) => g.coordinate(point, axis),
            None => names[text],
        }
    };
    let mut last = None;
    for line in code.lines() {
        let Some(statement) = line.strip_prefix("let ") else {
            continue;
        };
        let Some((name, value)) = statement.trim_end_matches(';').split_once(" = ") else {
            continue;
        };
        if !(name == "det" || name.starts_with('t')) {
            continue;
        }
        let tokens: Vec<&str> = value.split(' ').collect();
        let id = match tokens.as_slice() {
            [x] => {
                let x = x.strip_prefix('-').expect("a negation");
                let x = operand(&mut g, &names, x);
                g.negate(x)
            }
            [x, op, y] => {
                let (x, y) = (operand(&mut g, &names, x), operand(&mut g, &names, y));
                g.push(match *op {
                    "+" => Node::Add(x, y),
                    "-" => Node::Sub(x, y),
                    "*" => Node::Mul(x, y),
                    _ => panic!("{line}"),
                })
            }
            _ => panic!("{line}"),
        };
        names.insert(name.to_string(), id);
        last = Some(name.to_string());
    }
    assert_eq!(last.as_deref(), Some("det"));
    let root = names["det"];
    g.finish(root)
}

/// The text of the function `name` in the unformatted module `code`.
pub(super) fn function<'a>(code: &'a str, name: &str) -> &'a str {
    let start = code
        .find(&format!("pub fn {name}("))
        .unwrap_or_else(|| panic!("no function {name}"));
    let rest = &code[start..];
    &rest[..rest.find("\n}\n\n").expect("the end of the function")]
}

/// The arguments of the call `prefix(...)` that ends `line`, before `);`.
fn arguments<'a>(line: &'a str, prefix: &str) -> Option<Vec<&'a str>> {
    let inner = line.strip_prefix(prefix)?.strip_suffix(");")?;
    Some(inner.split(", ").collect())
}

/// The number `n` of a name written `{letter}{n}`.
fn numbered(text: &str, letter: char) -> Option<usize> {
    text.strip_prefix(letter)?.parse().ok()
}

/// The value of an emitted exact stage, read back from its text: every
/// call of the arithmetic core is read as the operation it performs, the
/// slices it reads are checked to be those of the expansions they name,
/// and the result is the expansion whose sign is returned.
pub(super) fn interpret_stage(code: &str) -> Graph {
    let mut g = Symbolic::default();
    // The value of each scalar `v{n}` and of each expansion `e{n}`.
    let mut scalars: BTreeMap<usize, NodeId> = BTreeMap::new();
    let mut expansions: BTreeMap<usize, NodeId> = BTreeMap::new();
    let scalar = |g: &mut Symbolic, scalars: &BTreeMap<usize, NodeId>, text: &str| {
        if let Some((point, axis)) = coordinate(text, "p") {
            g.coordinate(point, axis)
        } else if let Some(n) = numbered(text, 'v') {
            scalars[&n]
        } else {
            let value: f64 = text.parse().unwrap_or_else(|_| panic!("{text}"));
            assert_eq!(value.fract(), 0.0, "{text}");
            g.push(Node::Int(value as i64))
        }
    };
    // `&e{n}[..l{n}]`, which must name the same node twice.
    let slice = |expansions: &BTreeMap<usize, NodeId>, text: &str| {
        let (e, l) = text
            .strip_prefix("&e")
            .and_then(|t| t.strip_suffix(']'))
            .and_then(|t| t.split_once("[..l"))
            .unwrap_or_else(|| panic!("not a slice: {text}"));
        assert_eq!(e, l, "{text}");
        expansions[&e.parse::<usize>().unwrap()]
    };
    let output = |text: &str| -> usize {
        numbered(text.strip_prefix("&mut ").expect("an output"), 'e').expect("an expansion")
    };
    let mut root = None;
    for line in code.lines().map(str::trim) {
        if let Some((n, call)) = line
            .strip_prefix("let (v")
            .and_then(|t| t.split_once(") = "))
        {
            // `let (v{n}, r{n}) = eft::two_diff(a, b);`
            let (n, r) = n.split_once(", r").unwrap_or_else(|| panic!("{line}"));
            assert_eq!(n, r, "{line}");
            let a = arguments(call, "eft::two_diff(").unwrap_or_else(|| panic!("{line}"));
            let (x, y) = (
                scalar(&mut g, &scalars, a[0]),
                scalar(&mut g, &scalars, a[1]),
            );
            scalars.insert(n.parse().unwrap(), g.push(Node::Sub(x, y)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("let v") {
            let (n, value) = rest.split_once(" = -").unwrap_or_else(|| panic!("{line}"));
            let x = scalar(&mut g, &scalars, value.trim_end_matches(';'));
            scalars.insert(n.parse().unwrap(), g.negate(x));
            continue;
        }
        if let Some(rest) = line.strip_prefix("let e") {
            if let Some(n) = rest.strip_suffix(": [f64; 0] = [];") {
                expansions.insert(n.parse().unwrap(), g.push(Node::Int(0)));
            }
            continue;
        }
        if let Some((target, call)) = line.strip_prefix("let l").and_then(|t| t.split_once(" = ")) {
            let n: usize = target.parse().unwrap_or_else(|_| panic!("{line}"));
            let call_of = |name: &str| arguments(call, name);
            let value = if let Some(a) = call_of("expansion::from_f64(") {
                assert_eq!(output(a[1]), n, "{line}");
                scalar(&mut g, &scalars, a[0])
            } else if let Some(a) = call_of("expansion::from_two_diff(") {
                assert_eq!(output(a[2]), n, "{line}");
                let (x, y) = (
                    scalar(&mut g, &scalars, a[0]),
                    scalar(&mut g, &scalars, a[1]),
                );
                g.push(Node::Sub(x, y))
            } else if let Some(a) = call_of("expansion::from_two_product(") {
                assert_eq!(output(a[2]), n, "{line}");
                let (x, y) = (
                    scalar(&mut g, &scalars, a[0]),
                    scalar(&mut g, &scalars, a[1]),
                );
                g.push(Node::Mul(x, y))
            } else if let Some(a) = call_of("expansion::scale(") {
                assert_eq!(output(a[2]), n, "{line}");
                let (x, y) = (slice(&expansions, a[0]), scalar(&mut g, &scalars, a[1]));
                g.push(Node::Mul(x, y))
            } else if let Some(a) = call_of("product(") {
                assert_eq!(a[2..4], ["&mut scaled", "&mut spare"], "{line}");
                assert_eq!(output(a[4]), n, "{line}");
                let (x, y) = (slice(&expansions, a[0]), slice(&expansions, a[1]));
                g.push(Node::Mul(x, y))
            } else if let Some(a) = call_of("expansion::sum(") {
                assert_eq!(output(a[2]), n, "{line}");
                let (x, y) = (slice(&expansions, a[0]), slice(&expansions, a[1]));
                g.push(Node::Add(x, y))
            } else if let Some(a) = call_of("expansion::diff(") {
                assert_eq!(output(a[2]), n, "{line}");
                let (x, y) = (slice(&expansions, a[0]), slice(&expansions, a[1]));
                g.push(Node::Sub(x, y))
            } else if call == "0;" {
                // The length of an expansion of capacity zero.
                assert!(expansions.contains_key(&n), "{line}");
                continue;
            } else if call.strip_prefix('l').is_some() {
                // The length of the negated copy written just before.
                assert!(expansions.contains_key(&n), "{line}");
                continue;
            } else {
                panic!("unknown call: {line}");
            };
            assert!(expansions.insert(n, value).is_none(), "e{n} written twice");
            continue;
        }
        if let Some((target, source)) = line
            .strip_suffix(");")
            .and_then(|t| t.split_once(".copy_from_slice("))
        {
            // `e{n}[..l{x}].copy_from_slice(&e{x}[..l{x}]);`, negated next.
            let (n, length) = target
                .strip_prefix('e')
                .and_then(|t| t.strip_suffix(']'))
                .and_then(|t| t.split_once("[..l"))
                .unwrap_or_else(|| panic!("{line}"));
            assert_eq!(format!("&e{length}[..l{length}]"), source, "{line}");
            let copy = expansions[&length.parse::<usize>().unwrap()];
            assert!(
                expansions.insert(n.parse().unwrap(), copy).is_none(),
                "{line}"
            );
            continue;
        }
        if let Some(target) = line
            .strip_prefix("expansion::negate(&mut e")
            .and_then(|t| t.strip_suffix(");"))
        {
            let (n, _) = target
                .split_once("[..l")
                .unwrap_or_else(|| panic!("{line}"));
            let n: usize = n.parse().unwrap();
            let negated = g.negate(expansions[&n]);
            expansions.insert(n, negated);
            continue;
        }
        if let Some(text) = line
            .strip_prefix("Some(expansion::sign(")
            .and_then(|t| t.strip_suffix("))"))
        {
            root = Some(slice(&expansions, text));
        }
    }
    g.finish(root.expect("the sign of an expansion is returned"))
}
