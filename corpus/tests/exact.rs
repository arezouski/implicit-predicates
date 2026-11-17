//! Tests of the exact predicates, the LNC coordinates and the verifier.

mod common;

use common::{call, corpus_from};
use implicit_predicates_corpus::exact::{
    REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR, coordinates, dihedral_sign, dot_sign_3d,
    insphere, orient3d,
};
use implicit_predicates_corpus::verify::{Mismatch, verify, verify_with};
use implicit_predicates_corpus::{PointDef, Predicate, Stage};
use implicit_predicates_oracle::Dyadic;

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
fn dot_sign_3d_of_right_acute_and_obtuse_angles() {
    let (p, q) = (pt(1.0, 0.0, 0.0), pt(0.0, 0.0, 0.0));
    assert_eq!(dot_sign_3d(&p, &q, &pt(0.0, 1.0, 0.0)), 0);
    assert_eq!(dot_sign_3d(&p, &q, &pt(1.0, 1.0, 0.0)), 1);
    assert_eq!(dot_sign_3d(&p, &q, &pt(-1.0, 1.0, 0.0)), -1);
    // The apex is the middle argument: at p, the angle between q and r =
    // (-1, 1, 0) is acute, (q - p) . (r - p) = (-1, 0, 0) . (-2, 1, 0) = 2.
    assert_eq!(dot_sign_3d(&q, &p, &pt(-1.0, 1.0, 0.0)), 1);
    // A side of length zero.
    assert_eq!(dot_sign_3d(&q, &q, &p), 0);
    assert_eq!(dot_sign_3d(&p, &q, &q), 0);
    // Away from the origin: (3, 5, -2) - (2, 3, 1) = (1, 2, -3) and
    // (0, 4, 2) - (2, 3, 1) = (-2, 1, 1), whose dot product is -3.
    let (p, q, r) = (pt(3.0, 5.0, -2.0), pt(2.0, 3.0, 1.0), pt(0.0, 4.0, 2.0));
    assert_eq!(dot_sign_3d(&p, &q, &r), -1);
    assert_eq!(dot_sign_3d(&r, &q, &p), -1);
}

#[test]
fn dihedral_sign_of_a_cube_and_of_a_regular_tetrahedron() {
    // Two faces of the unit cube meet at the edge from c = 0 to d = e_z at a
    // right angle: (b - d) x (c - d) = (-1, 0, 0) and (a - d) x (c - d) =
    // (0, 1, 0).
    let (a, b, c, d) = (
        pt(1.0, 0.0, 0.0),
        pt(0.0, 1.0, 0.0),
        pt(0.0, 0.0, 0.0),
        pt(0.0, 0.0, 1.0),
    );
    assert_eq!(dihedral_sign(&a, &b, &c, &d), 0);
    // Turning b towards a makes the angle acute, away from it obtuse.
    assert_eq!(dihedral_sign(&a, &pt(1.0, 1.0, 0.0), &c, &d), 1);
    assert_eq!(dihedral_sign(&a, &pt(-1.0, 1.0, 0.0), &c, &d), -1);
    // A regular tetrahedron with integer corners: its dihedral angles,
    // arccos(1/3), are acute. With d = (-1, -1, 1) and c = (-1, 1, -1) the
    // cross products are (4, 4, 4) and (-4, 4, 4), whose dot product is 16.
    let (a, b, c, d) = (
        pt(1.0, 1.0, 1.0),
        pt(1.0, -1.0, -1.0),
        pt(-1.0, 1.0, -1.0),
        pt(-1.0, -1.0, 1.0),
    );
    assert_eq!(dihedral_sign(&a, &b, &c, &d), 1);
}

#[test]
fn dihedral_sign_of_degenerate_triangles_is_zero() {
    let (a, b, c, d) = (
        pt(1.0, 2.0, 0.0),
        pt(-3.0, 1.0, 5.0),
        pt(0.0, 0.0, 0.0),
        pt(1.0, 1.0, 1.0),
    );
    assert_ne!(dihedral_sign(&a, &b, &c, &d), 0);
    // The edge has length zero.
    assert_eq!(dihedral_sign(&a, &b, &d, &d), 0);
    // a, then b, on the line through c and d.
    let on_line = pt(-2.0, -2.0, -2.0);
    assert_eq!(dihedral_sign(&on_line, &b, &c, &d), 0);
    assert_eq!(dihedral_sign(&a, &on_line, &c, &d), 0);
}

/// A generator of pseudo-random numbers, splitmix64, for the properties of
/// the oracles: deterministic, so that a failure repeats.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A coordinate: half the time a small integer, so that many signs are
    /// zero; otherwise any sign, 53 random bits and an exponent in
    /// `-40..40`.
    fn coordinate(&mut self) -> f64 {
        let bits = self.next();
        if bits & 1 == 0 {
            return f64::from(i32::try_from((bits >> 1) % 5).unwrap() - 2);
        }
        let mantissa = (bits >> 11) as f64 / (1u64 << 53) as f64;
        let exponent = i32::try_from((bits >> 1) % 80).unwrap() - 40;
        let x = mantissa * 2f64.powi(exponent);
        if bits & 2 == 0 { x } else { -x }
    }

    fn point(&mut self) -> [Dyadic; 3] {
        pt(self.coordinate(), self.coordinate(), self.coordinate())
    }
}

#[test]
fn dot_sign_3d_is_symmetric_in_its_ends() {
    let mut rng = Rng(1);
    for _ in 0..2000 {
        let (p, q, r) = (rng.point(), rng.point(), rng.point());
        assert_eq!(dot_sign_3d(&p, &q, &r), dot_sign_3d(&r, &q, &p));
    }
}

#[test]
fn dihedral_sign_is_symmetric_in_the_triangles_and_in_the_edge() {
    let mut rng = Rng(2);
    for _ in 0..2000 {
        let (a, b, c, d) = (rng.point(), rng.point(), rng.point(), rng.point());
        let s = dihedral_sign(&a, &b, &c, &d);
        assert_eq!(dihedral_sign(&b, &a, &c, &d), s);
        assert_eq!(dihedral_sign(&a, &b, &d, &c), s);
        assert_eq!(dihedral_sign(&b, &a, &d, &c), s);
    }
}

#[test]
fn dihedral_sign_is_the_sign_of_the_lagrange_identity() {
    // [B x C] . [A x C] = (B . A)(C . C) - (B . C)(C . A), with A = a - d,
    // B = b - d and C = c - d, written here without cross products.
    fn sub(x: &[Dyadic; 3], y: &[Dyadic; 3]) -> [Dyadic; 3] {
        std::array::from_fn(|i| &x[i] - &y[i])
    }
    fn inner(x: &[Dyadic; 3], y: &[Dyadic; 3]) -> Dyadic {
        (0..3).fold(Dyadic::from_f64(0.0), |s, i| &s + &(&x[i] * &y[i]))
    }
    let mut rng = Rng(3);
    let mut signs = [0; 3];
    for _ in 0..4000 {
        let (a, b, c, d) = (rng.point(), rng.point(), rng.point(), rng.point());
        let (x, y, z) = (sub(&a, &d), sub(&b, &d), sub(&c, &d));
        let identity = &(&inner(&y, &x) * &inner(&z, &z)) - &(&inner(&y, &z) * &inner(&z, &x));
        let s = dihedral_sign(&a, &b, &c, &d);
        assert_eq!(s, identity.sign(), "{a:?} {b:?} {c:?} {d:?}");
        signs[usize::try_from(s + 1).unwrap()] += 1;
    }
    // Every sign comes up.
    assert!(signs.iter().all(|&n| n > 0), "{signs:?}");
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
