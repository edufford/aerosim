use aerosim_core::math::quaternion::{Quaternion, RotationSequence, RotationType};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI};

const TOL: f64 = 1e-10;

fn assert_near(a: f64, b: f64, label: &str) {
    assert!(
        (a - b).abs() < TOL,
        "{}: expected {}, got {} (diff {})",
        label,
        b,
        a,
        (a - b).abs()
    );
}

/// Multiply two quaternions (w, x, y, z) to compose rotations.
fn qmul(a: &Quaternion, b: &Quaternion) -> Quaternion {
    Quaternion::new(
        a.w() * b.w() - a.x() * b.x() - a.y() * b.y() - a.z() * b.z(),
        a.w() * b.x() + a.x() * b.w() + a.y() * b.z() - a.z() * b.y(),
        a.w() * b.y() - a.x() * b.z() + a.y() * b.w() + a.z() * b.x(),
        a.w() * b.z() + a.x() * b.y() - a.y() * b.x() + a.z() * b.w(),
    )
}

/// Rotate a 3D vector by a quaternion: q * v * q_conj
fn qrot(q: &Quaternion, v: [f64; 3]) -> [f64; 3] {
    let qv = Quaternion::new(0.0, v[0], v[1], v[2]);
    let q_conj = Quaternion::new(q.w(), -q.x(), -q.y(), -q.z());
    let result = qmul(&qmul(q, &qv), &q_conj);
    [result.x(), result.y(), result.z()]
}

// ============================================================================
// Test: Identity quaternion decomposes to zero angles
// ============================================================================
#[test]
fn test_identity_decomposes_to_zero() {
    let q = Quaternion::new(1.0, 0.0, 0.0, 0.0);
    let [yaw, pitch, roll] =
        q.to_euler_angles(RotationType::Intrinsic, RotationSequence::ZYX);
    assert_near(roll, 0.0, "roll");
    assert_near(pitch, 0.0, "pitch");
    assert_near(yaw, 0.0, "yaw");
}

// ============================================================================
// Test: Pure yaw (90° about Z) produces correct quaternion
// ============================================================================
#[test]
fn test_pure_yaw_90() {
    let yaw = FRAC_PI_2;
    let q = Quaternion::from_euler_angles(
        [yaw, 0.0, 0.0], // [yaw, pitch, roll]
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );

    // A pure Z rotation quaternion: (cos(π/4), 0, 0, sin(π/4))
    assert_near(q.w(), FRAC_PI_4.cos(), "w");
    assert_near(q.x(), 0.0, "x");
    assert_near(q.y(), 0.0, "y");
    assert_near(q.z(), FRAC_PI_4.sin(), "z");

    // Rotating [1,0,0] (North) by 90° yaw should give [0,1,0] (East) in NED
    let rotated = qrot(&q, [1.0, 0.0, 0.0]);
    assert_near(rotated[0], 0.0, "rotated x (should be 0)");
    assert_near(rotated[1], 1.0, "rotated y (should be 1 = East)");
    assert_near(rotated[2], 0.0, "rotated z");

    // Decompose back
    let [y, p, r] = q.to_euler_angles(RotationType::Intrinsic, RotationSequence::ZYX);
    assert_near(r, 0.0, "decomposed roll");
    assert_near(p, 0.0, "decomposed pitch");
    assert_near(y, FRAC_PI_2, "decomposed yaw");
}

// ============================================================================
// Test: Pure pitch (30° about Y) produces correct quaternion
// ============================================================================
#[test]
fn test_pure_pitch_30() {
    let pitch = FRAC_PI_6;
    let q = Quaternion::from_euler_angles(
        [0.0, pitch, 0.0], // [yaw, pitch, roll]
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );

    // A pure Y rotation quaternion: (cos(π/12), 0, sin(π/12), 0)
    assert_near(q.w(), (FRAC_PI_6 / 2.0).cos(), "w");
    assert_near(q.x(), 0.0, "x");
    assert_near(q.y(), (FRAC_PI_6 / 2.0).sin(), "y");
    assert_near(q.z(), 0.0, "z");

    // Rotating [1,0,0] (North) by 30° pitch should give [cos30, 0, -sin30]
    // In NED, positive pitch = nose up (right-hand rule about Y/East axis, North toward -Z/Up)
    let rotated = qrot(&q, [1.0, 0.0, 0.0]);
    assert_near(rotated[0], pitch.cos(), "rotated x");
    assert_near(rotated[1], 0.0, "rotated y");
    assert_near(rotated[2], -pitch.sin(), "rotated z (up = -down)");

    // Decompose back
    let [y, p, r] = q.to_euler_angles(RotationType::Intrinsic, RotationSequence::ZYX);
    assert_near(r, 0.0, "decomposed roll");
    assert_near(p, FRAC_PI_6, "decomposed pitch");
    assert_near(y, 0.0, "decomposed yaw");
}

// ============================================================================
// Test: Pure roll (45° about X) produces correct quaternion
// ============================================================================
#[test]
fn test_pure_roll_45() {
    let roll = FRAC_PI_4;
    let q = Quaternion::from_euler_angles(
        [0.0, 0.0, roll], // [yaw, pitch, roll]
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );

    // A pure X rotation quaternion: (cos(π/8), sin(π/8), 0, 0)
    assert_near(q.w(), (FRAC_PI_4 / 2.0).cos(), "w");
    assert_near(q.x(), (FRAC_PI_4 / 2.0).sin(), "x");
    assert_near(q.y(), 0.0, "y");
    assert_near(q.z(), 0.0, "z");

    // Rotating [0,1,0] (East) by 45° roll should give [0, cos45, sin45]
    // In NED, positive roll = right wing down (rotation about North/X axis)
    let rotated = qrot(&q, [0.0, 1.0, 0.0]);
    assert_near(rotated[0], 0.0, "rotated x");
    assert_near(rotated[1], roll.cos(), "rotated y");
    assert_near(rotated[2], roll.sin(), "rotated z (down)");

    // Decompose back
    let [y, p, r] = q.to_euler_angles(RotationType::Intrinsic, RotationSequence::ZYX);
    assert_near(r, FRAC_PI_4, "decomposed roll");
    assert_near(p, 0.0, "decomposed pitch");
    assert_near(y, 0.0, "decomposed yaw");
}

// ============================================================================
// Test: Combined yaw+pitch+roll roundtrip
// ============================================================================
#[test]
fn test_combined_roundtrip() {
    let yaw = 0.7;
    let pitch = 0.3;
    let roll = 0.1;

    let q = Quaternion::from_euler_angles(
        [yaw, pitch, roll],
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );

    let [y, p, r] = q.to_euler_angles(RotationType::Intrinsic, RotationSequence::ZYX);
    assert_near(r, roll, "roundtrip roll");
    assert_near(p, pitch, "roundtrip pitch");
    assert_near(y, yaw, "roundtrip yaw");
}

// ============================================================================
// Test: Verify axis directions — X is North, Y is East, Z is Down (NED)
// After pure yaw rotation, North vector should rotate toward East
// After pure pitch rotation, North vector should rotate toward Down
// After pure roll rotation, East vector should rotate toward Down
// ============================================================================
#[test]
fn test_axis_directions_ned() {
    let north = [1.0, 0.0, 0.0];
    let east = [0.0, 1.0, 0.0];
    let _down = [0.0, 0.0, 1.0];

    // Yaw +90° should rotate North to East
    let q_yaw = Quaternion::from_euler_angles(
        [FRAC_PI_2, 0.0, 0.0],
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );
    let r = qrot(&q_yaw, north);
    assert_near(r[0], 0.0, "yaw90: north.x -> 0");
    assert_near(r[1], 1.0, "yaw90: north.y -> 1 (East)");
    assert_near(r[2], 0.0, "yaw90: north.z -> 0");

    // Pitch +90° should rotate North to Up (-Down)
    // Right-hand rule about Y/East: positive pitch = nose up
    let q_pitch = Quaternion::from_euler_angles(
        [0.0, FRAC_PI_2, 0.0],
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );
    let r = qrot(&q_pitch, north);
    assert_near(r[0], 0.0, "pitch90: north.x -> 0");
    assert_near(r[1], 0.0, "pitch90: north.y -> 0");
    assert_near(r[2], -1.0, "pitch90: north.z -> -1 (Up)");

    // Roll +90° should rotate East to Down
    let q_roll = Quaternion::from_euler_angles(
        [0.0, 0.0, FRAC_PI_2],
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );
    let r = qrot(&q_roll, east);
    assert_near(r[0], 0.0, "roll90: east.x -> 0");
    assert_near(r[1], 0.0, "roll90: east.y -> 0");
    assert_near(r[2], 1.0, "roll90: east.z -> 1 (Down)");
}

// ============================================================================
// Test: Compare old (Extrinsic ZYX) vs new (Intrinsic ZYX) decomposition
// For a known quaternion, verify they produce different array order but
// the FFI output (out_roll, out_pitch, out_yaw) is correct.
// ============================================================================
#[test]
fn test_old_vs_new_convention() {
    // Create a quaternion for 45° yaw, 20° pitch, 10° roll using the NEW convention
    let yaw = PI / 4.0;
    let pitch = PI / 9.0;
    let roll = PI / 18.0;

    let q_new = Quaternion::from_euler_angles(
        [yaw, pitch, roll],
        RotationType::Intrinsic,
        RotationSequence::ZYX,
    );

    // Decompose with NEW convention: returns [yaw, pitch, roll]
    let new_angles = q_new.to_euler_angles(RotationType::Intrinsic, RotationSequence::ZYX);
    assert_near(new_angles[0], yaw, "new[0] = yaw");
    assert_near(new_angles[1], pitch, "new[1] = pitch");
    assert_near(new_angles[2], roll, "new[2] = roll");

    // Create with OLD convention (same quaternion should result from swapped array)
    let q_old = Quaternion::from_euler_angles(
        [roll, pitch, yaw], // old: [roll, pitch, yaw] in wrong slots
        RotationType::Extrinsic,
        RotationSequence::ZYX,
    );

    // Decompose with OLD convention: returns [Z, Y, X] labeled as [roll, pitch, yaw]
    let old_angles = q_old.to_euler_angles(RotationType::Extrinsic, RotationSequence::ZYX);
    assert_near(old_angles[0], roll, "old[0] = roll (actually Z angle)");
    assert_near(old_angles[1], pitch, "old[1] = pitch");
    assert_near(old_angles[2], yaw, "old[2] = yaw (actually X angle)");

    // The quaternions themselves should be DIFFERENT
    // old creates R = Rx(yaw)·Ry(pitch)·Rz(roll)
    // new creates R = Rz(yaw)·Ry(pitch)·Rx(roll)
    let same_quat = (q_new.w() - q_old.w()).abs() < TOL
        && (q_new.x() - q_old.x()).abs() < TOL
        && (q_new.y() - q_old.y()).abs() < TOL
        && (q_new.z() - q_old.z()).abs() < TOL;
    assert!(
        !same_quat,
        "Old and new quaternions should be DIFFERENT (different rotation)"
    );

    // But the FFI output (after naming correction) gives the same roll/pitch/yaw VALUES
    // Old: old_angles[0] is Z=roll, old_angles[2] is X=yaw
    // New: new_angles[0] is yaw, new_angles[2] is roll
    // Both end up with roll=π/18, pitch=π/9, yaw=π/4 in the named outputs
    println!("New quaternion: w={:.4} x={:.4} y={:.4} z={:.4}", q_new.w(), q_new.x(), q_new.y(), q_new.z());
    println!("Old quaternion: w={:.4} x={:.4} y={:.4} z={:.4}", q_old.w(), q_old.x(), q_old.y(), q_old.z());
    println!("New decomposition [yaw, pitch, roll]: [{:.4}, {:.4}, {:.4}]", new_angles[0], new_angles[1], new_angles[2]);
    println!("Old decomposition [Z, Y, X]: [{:.4}, {:.4}, {:.4}]", old_angles[0], old_angles[1], old_angles[2]);
}
