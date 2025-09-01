pub mod quaternion;

pub mod rotator;
pub use rotator::Rotator;

pub mod vector;
pub use vector::Vector3;

pub fn round_to_decimal_places(value: f64, decimal_places: u32) -> f64 {
    let multiplier = 10.0_f64.powi(decimal_places as i32);
    (value * multiplier).round() / multiplier
}
