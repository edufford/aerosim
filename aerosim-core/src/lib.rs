// use pyo3::wrap_pyfunction;

// -------------------------------------------------------------------------
// Aerosimcore classes

pub mod actor;

pub mod math;

pub mod coordinate_system;
use crate::coordinate_system::conversion_utils::*;
use crate::coordinate_system::geo::*;

pub mod trajectory;

pub mod path;

#[cfg(feature = "python")]
use {
    pyo3::prelude::*,
    crate::actor::Actor,
    crate::math::rotator::Rotator,
    crate::coordinate_system::world_coordinate::*,
};

// -------------------------------------------------------------------------
// Python module exports

#[cfg(feature = "python")]
#[pymodule]
fn _aerocore(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Actor>()?;
    m.add_class::<Rotator>()?;
    m.add_class::<Ellipsoid>()?;
    m.add_class::<Geoid>()?;
    m.add_class::<GeodeticBounds>()?;
    m.add_class::<OffsetMap>()?;
    m.add_class::<WorldCoordinate>()?;

    m.add_function(wrap_pyfunction!(lla_to_ned, m)?)?;
    m.add_function(wrap_pyfunction!(ned_to_lla, m)?)?;
    m.add_function(wrap_pyfunction!(lla_to_cartesian, m)?)?;
    m.add_function(wrap_pyfunction!(ned_to_cartesian, m)?)?;
    m.add_function(wrap_pyfunction!(cartesian_to_lla, m)?)?;
    m.add_function(wrap_pyfunction!(cartesian_to_ned, m)?)?;
    m.add_function(wrap_pyfunction!(msl_to_hae, m)?)?;
    m.add_function(wrap_pyfunction!(hae_to_msl, m)?)?;
    m.add_function(wrap_pyfunction!(trajectory::py_generate_trajectory, m)?)?;
    m.add_function(wrap_pyfunction!(trajectory::py_generate_trajectory_linear, m)?)?;
    m.add_function(wrap_pyfunction!(
        trajectory::py_generate_trajectory_from_adsb_csv,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(msl_to_hae_with_offset, m)?)?;
    m.add_function(wrap_pyfunction!(hae_to_msl_with_offset, m)?)?;
    m.add_function(wrap_pyfunction!(py_haversine_distance_meters, m)?)?;
    m.add_function(wrap_pyfunction!(py_bearing_deg, m)?)?;
    m.add_function(wrap_pyfunction!(py_deviation_from_course_meters, m)?)?;

    m.add_function(wrap_pyfunction!(path::py_read_config_file, m)?)?;
    Ok(())
}
