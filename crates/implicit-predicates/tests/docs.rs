//! The numbers that the public documentation quotes are the constants of the
//! code, and the example of the README at the root is the one of the crate.

mod common;

use common::pow2;
use implicit_predicates::stages::{insphere, orient3d};

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
    format!("{},{:03} `f64`", words / 1000, words % 1000)
}

/// The arena that suffices for explicit points: `orient3d` in the rustdoc and
/// the changelog, `insphere` in the same places; no README quotes either.
#[test]
fn the_arenas_that_suffice_for_explicit_points_are_the_constants() {
    for words in [
        orient3d::BIG_INTEGERS_EXPLICIT_ARENA,
        insphere::BIG_INTEGERS_EXPLICIT_ARENA,
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
/// allocates nothing.
#[test]
fn the_documented_range_without_allocation_is_the_translated_range() {
    let (low, high) = orient3d::TRANSLATED_RANGE;
    assert_eq!((low, high), (pow2(-188), pow2(238)));
    for (file, text) in [("README.md", ROOT_README), ("crate README", CRATE_README)] {
        assert!(flat(text).contains("`2^-188..=2^238`"), "{file}");
    }
    assert!(flat(CHANGELOG).contains("`2^-188..=2^238`"));
}

/// The crate README opens with the example that the README at the root
/// shows.
#[test]
fn the_root_readme_shows_the_example_of_the_crate_readme() {
    fn first_example(text: &str) -> &str {
        let start = text.find("```rust").expect("an example");
        let end = start + text[start..].find("\n```\n").expect("its end");
        &text[start..end]
    }
    assert_eq!(first_example(ROOT_README), first_example(CRATE_README));
}
