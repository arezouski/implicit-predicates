//! Tests of the exact predicates, the LNC coordinates and the verifier.

mod common;

use common::{call, corpus_from};
use indirect_predicates_corpus::exact::{
    REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR, coordinates, insphere, orient3d,
};
use indirect_predicates_corpus::verify::{Mismatch, verify, verify_with};
use indirect_predicates_corpus::{PointDef, Predicate, Stage};
use indirect_predicates_oracle::Dyadic;

fn pt(x: f64, y: f64, z: f64) -> [Dyadic; 3] {
    [x, y, z].map(Dyadic::from_f64)
}

#[test]
fn orient3d_of_a_unit_tetrahedron() {
    // a - d = (1,0,0), b - d = (0,1,0), c - d = (0,0,1): the identity matrix,
    // whose determinant is +1.
    let (a, b, c, d) = (
        pt(1.0, 0.0, 0.0),
        pt(0.0, 1.0, 0.0),
        pt(0.0, 0.0, 1.0),
        pt(0.0, 0.0, 0.0),
    );
    assert_eq!(orient3d(&a, &b, &c, &d), 1);
}

#[test]
fn orient3d_changes_sign_with_a_swap() {
    let (a, b, c, d) = (
        pt(1.0, 0.0, 0.0),
        pt(0.0, 1.0, 0.0),
        pt(0.0, 0.0, 1.0),
        pt(0.0, 0.0, 0.0),
    );
    assert_eq!(orient3d(&b, &a, &c, &d), -1);
    assert_eq!(orient3d(&a, &b, &d, &c), -1);
}

#[test]
fn insphere_of_the_origin_in_a_unit_tetrahedron() {
    // a=(1,0,0) b=(0,1,0) c=(0,0,1) d=(-1,0,0) lie on the unit sphere and
    // orient3d(a,b,c,d) = det[(2,0,0);(1,1,0);(1,0,1)] = 2 > 0. With e the
    // origin the rows (p - e, |p - e|^2) are (1,0,0,1), (0,1,0,1), (0,0,1,1),
    // (-1,0,0,1). Adding row 1 to row 4 gives (0,0,0,2), so the determinant is
    // 1 * 1 * 1 * 2 = 2 > 0.
    let (a, b, c, d, e) = (
        pt(1.0, 0.0, 0.0),
        pt(0.0, 1.0, 0.0),
        pt(0.0, 0.0, 1.0),
        pt(-1.0, 0.0, 0.0),
        pt(0.0, 0.0, 0.0),
    );
    assert_eq!(orient3d(&a, &b, &c, &d), 1);
    assert_eq!(insphere(&a, &b, &c, &d, &e), 1);
    assert_eq!(insphere(&b, &a, &c, &d, &e), -1);
    // A point far outside the sphere has the opposite sign.
    assert_eq!(insphere(&a, &b, &c, &d, &pt(5.0, 5.0, 5.0)), -1);
}

#[test]
fn coplanar_and_cospherical_are_zero() {
    // All four points lie in the plane z = 0.
    let (a, b, c, d) = (
        pt(1.0, 0.0, 0.0),
        pt(0.0, 1.0, 0.0),
        pt(0.0, 0.0, 0.0),
        pt(1.0, 1.0, 0.0),
    );
    assert_eq!(orient3d(&a, &b, &c, &d), 0);
    // e = (0,-1,0) is on the unit sphere through a, b, c' and d'.
    let (c2, d2, e) = (pt(0.0, 0.0, 1.0), pt(-1.0, 0.0, 0.0), pt(0.0, -1.0, 0.0));
    assert_eq!(insphere(&a, &b, &c2, &d2, &e), 0);
}

fn lnc_points(t: f64) -> Vec<PointDef> {
    vec![
        PointDef::Explicit([1.0, 2.0, 3.0]),
        PointDef::Explicit([2.0, 4.0, 7.0]),
        PointDef::Lnc { p: 0, q: 1, t },
    ]
}

#[test]
fn lnc_at_zero_is_p_and_at_one_is_q() {
    let at0 = coordinates(&lnc_points(0.0), 2).unwrap();
    assert_eq!(at0, pt(1.0, 2.0, 3.0));
    let at1 = coordinates(&lnc_points(1.0), 2).unwrap();
    assert_eq!(at1, pt(2.0, 4.0, 7.0));
}

#[test]
fn lnc_outside_the_segment_is_exact() {
    // q - p = (1, 2, 4); p + 3.5 (q - p) = (4.5, 9, 17).
    assert_eq!(
        coordinates(&lnc_points(3.5), 2).unwrap(),
        pt(4.5, 9.0, 17.0)
    );
    // x = 1 + t * 1 with t = -2^-60, which no f64 can hold.
    let t = -(2f64.powi(-60));
    let got = coordinates(&lnc_points(t), 2).unwrap();
    let expected_x = &Dyadic::from_f64(1.0) - &Dyadic::from_f64(2f64.powi(-60));
    assert_eq!(got[0], expected_x);
    assert_ne!(got[0], Dyadic::from_f64(1.0));
}

#[test]
fn lnc_on_its_own_segment_is_coplanar_with_any_two_points() {
    let points = vec![
        PointDef::Explicit([0.1, -0.7, 2.3]),
        PointDef::Explicit([1.9, 0.3, -4.1]),
        PointDef::Explicit([-3.3, 5.5, 0.2]),
        PointDef::Lnc { p: 0, q: 1, t: 0.3 },
        PointDef::Lnc {
            p: 0,
            q: 1,
            t: -1.7,
        },
        PointDef::Lnc { p: 0, q: 1, t: 8.9 },
    ];
    let a = coordinates(&points, 0).unwrap();
    let b = coordinates(&points, 1).unwrap();
    let c = coordinates(&points, 2).unwrap();
    for i in 3..6 {
        let l = coordinates(&points, i).unwrap();
        assert_eq!(orient3d(&l, &a, &b, &c), 0);
    }
}

#[test]
fn lnc_with_equal_parents_is_the_parent() {
    let points = vec![
        PointDef::Explicit([0.1, 0.2, 0.3]),
        PointDef::Lnc { p: 0, q: 0, t: 7.7 },
    ];
    assert_eq!(coordinates(&points, 1), coordinates(&points, 0));
}

#[test]
fn lpi_has_no_coordinates() {
    let mut points = vec![PointDef::Explicit([0.0; 3]); 5];
    points.push(PointDef::Lpi {
        line: [0, 1],
        plane: [2, 3, 4],
    });
    points.push(PointDef::Tpi {
        planes: [[0, 1, 2], [1, 2, 3], [2, 3, 4]],
    });
    assert!(coordinates(&points, 5).is_none());
    assert!(coordinates(&points, 6).is_none());
}

fn tetra_points() -> Vec<PointDef> {
    vec![
        PointDef::Explicit([1.0, 0.0, 0.0]),
        PointDef::Explicit([0.0, 1.0, 0.0]),
        PointDef::Explicit([0.0, 0.0, 1.0]),
        PointDef::Explicit([0.0, 0.0, 0.0]),
        PointDef::Lpi {
            line: [0, 1],
            plane: [1, 2, 3],
        },
    ]
}

#[test]
fn a_wrong_recorded_sign_is_reported_with_its_index() {
    let exact = 1;
    let ok = REFERENCE_ORIENT3D_FACTOR * exact;
    let calls = vec![
        call(Predicate::Orient3d, [0, 1, 2, 3, 0], ok, Stage::Exact),
        call(Predicate::Orient3d, [0, 1, 2, 3, 0], -ok, Stage::Exact),
        call(Predicate::Orient3d, [1, 0, 2, 3, 0], ok, Stage::Exact),
    ];
    let v = verify(&corpus_from(tetra_points(), calls));
    assert_eq!((v.checked, v.skipped), (3, 0));
    assert_eq!(
        v.mismatches,
        vec![
            Mismatch {
                call: 1,
                recorded: -ok,
                exact: 1
            },
            Mismatch {
                call: 2,
                recorded: ok,
                exact: -1
            },
        ]
    );
}

#[test]
fn calls_with_lpi_are_skipped_and_counted() {
    let calls = vec![
        call(Predicate::Orient3d, [0, 1, 2, 4, 0], 1, Stage::Filtered),
        call(Predicate::InSphere, [0, 1, 2, 3, 3], 0, Stage::Exact),
    ];
    let v = verify(&corpus_from(tetra_points(), calls));
    assert_eq!((v.checked, v.skipped), (1, 1));
    assert!(v.mismatches.is_empty());
}

#[test]
fn verify_with_applies_the_factor_of_each_predicate() {
    let mut points = tetra_points();
    points[4] = PointDef::Explicit([0.1, 0.1, 0.1]);
    let c: Vec<_> = (0..5).map(|i| coordinates(&points, i).unwrap()).collect();
    let o = orient3d(&c[0], &c[1], &c[2], &c[3]);
    let s = insphere(&c[0], &c[1], &c[2], &c[3], &c[4]);
    assert!(o != 0 && s != 0);
    let calls = vec![
        call(Predicate::Orient3d, [0, 1, 2, 3, 0], -o, Stage::Filtered),
        call(Predicate::InSphere, [0, 1, 2, 3, 4], s, Stage::Exact),
    ];
    let corpus = corpus_from(points, calls);
    assert!(verify_with(&corpus, -1, 1).mismatches.is_empty());
    let orient_wrong = verify_with(&corpus, 1, 1);
    assert_eq!(orient_wrong.checked, 2);
    assert_eq!(
        orient_wrong.mismatches,
        vec![Mismatch {
            call: 0,
            recorded: -o,
            exact: o
        }]
    );
    let insphere_wrong = verify_with(&corpus, -1, -1);
    assert_eq!(insphere_wrong.mismatches.len(), 1);
    assert_eq!(insphere_wrong.mismatches[0].call, 1);
    assert_eq!(
        verify(&corpus),
        verify_with(
            &corpus,
            REFERENCE_ORIENT3D_FACTOR,
            REFERENCE_INSPHERE_FACTOR
        )
    );
}
