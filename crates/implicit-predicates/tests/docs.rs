//! The numbers that the public documentation quotes are the constants of the
//! code, and the example of the README at the root is the one of the crate.

mod common;

use common::pow2;
use implicit_predicates::stages::{dihedral_sign, dot_sign_3d, insphere, orient3d};

const LIB: &str = include_str!("../src/lib.rs");
const CRATE_README: &str = include_str!("../README.md");
const ROOT_README: &str = include_str!("../../../README.md");
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Whitespace and the comment markers of rustdoc collapsed to single spaces,
/// so that a phrase broken over two lines is found.
fn flat(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.trim_start()
                .trim_start_matches("///")
                .trim_start_matches("//!")
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn thousands(words: usize) -> String {
    if words < 1000 {
        format!("{words} `f64`")
    } else {
        format!("{},{:03} `f64`", words / 1000, words % 1000)
    }
}

/// The arena that suffices for explicit points: `orient3d`, `insphere`,
/// `dot_sign_3d` and `dihedral_sign` in the rustdoc and the changelog; no
/// README quotes any of them.
#[test]
fn the_arenas_that_suffice_for_explicit_points_are_the_constants() {
    for words in [
        orient3d::BIG_INTEGERS_EXPLICIT_ARENA,
        insphere::BIG_INTEGERS_EXPLICIT_ARENA,
        dot_sign_3d::BIG_INTEGERS_EXPLICIT_ARENA,
        dihedral_sign::BIG_INTEGERS_EXPLICIT_ARENA,
    ] {
        let documented = thousands(words);
        for (file, text) in [("lib.rs", LIB), ("CHANGELOG.md", CHANGELOG)] {
            assert!(flat(text).contains(&documented), "{file}: {documented}");
        }
        for (file, text) in [("README.md", ROOT_README), ("crate README", CRATE_README)] {
            assert!(!flat(text).contains(&documented), "{file}: {documented}");
        }
    }
}

/// The range of magnitudes of explicit coordinates for which `orient3d`
/// allocates nothing in release builds.
#[test]
fn the_documented_range_without_allocation_is_the_translated_range() {
    let (low, high) = orient3d::TRANSLATED_RANGE;
    assert_eq!((low, high), (pow2(-188), pow2(238)));
    for (file, text) in [
        ("lib.rs", LIB),
        ("README.md", ROOT_README),
        ("crate README", CRATE_README),
    ] {
        assert!(flat(text).contains("`2^-188..=2^238`"), "{file}");
    }
    assert!(flat(CHANGELOG).contains("`2^-188..=2^238`"));
}

/// The ranges of magnitudes of explicit coordinates for which `dot_sign_3d`
/// and `dihedral_sign` decide on the stack, as the rustdoc and the changelog
/// quote them. No README quotes either.
#[test]
fn the_documented_ranges_on_the_stack_of_the_angle_predicates_are_the_constants() {
    let (low, high) = dot_sign_3d::TRANSLATED_RANGE;
    assert_eq!((low, high), (pow2(-428), pow2(478)));
    let (low, high) = dihedral_sign::DIFFERENCES_RANGE;
    assert_eq!((low, high), (pow2(-188), pow2(239)));
    for (file, text) in [("lib.rs", LIB), ("CHANGELOG.md", CHANGELOG)] {
        for range in ["`2^-428..=2^478`", "`2^-188..=2^239`"] {
            assert!(flat(text).contains(range), "{file}: {range}");
        }
    }
    for (file, text) in [("README.md", ROOT_README), ("crate README", CRATE_README)] {
        for range in ["2^-428", "2^478", "2^239"] {
            assert!(!flat(text).contains(range), "{file}: {range}");
        }
    }
}

/// The crate README and the README at the root show the same examples, in
/// the same order.
#[test]
fn the_root_readme_shows_the_examples_of_the_crate_readme() {
    fn examples(text: &str) -> Vec<&str> {
        let mut found = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("```rust") {
            let end = start + rest[start..].find("\n```\n").expect("its end");
            found.push(&rest[start..end]);
            rest = &rest[end..];
        }
        found
    }
    let (root, krate) = (examples(ROOT_README), examples(CRATE_README));
    // The README of the crate also shows the examples of its own sections.
    assert_eq!(root[0], krate[0]);
    assert!(root.len() >= 2);
    let angles = krate.iter().find(|e| e.contains("dihedral_sign"));
    assert_eq!(Some(&root[1]), angles);
}
