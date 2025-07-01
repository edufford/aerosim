pub mod components;
pub mod data_manager;
pub mod fmu_driver;
pub mod fmu_utils;
pub mod logging;
pub mod orchestrator;
pub mod scene_graph;
pub mod sim_clock;

use pyo3::prelude::*;

// -------------------------------------------------------------------------
// Python module exports
#[pymodule]
fn _aerosim_world(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<orchestrator::Orchestrator>()?;
    m.add_class::<fmu_driver::FmuDriverRust>()?;
    Ok(())
}
