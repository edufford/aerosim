use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::adsb::types::EmergencyState;

#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AircraftStatus {
    pub emergency_state: EmergencyState,
    pub squawk: u32,
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl AircraftStatus {
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("emergency_state", self.emergency_state)?;
        let _ = dict.set_item("squawk", self.squawk)?;
        Ok(dict.into())
    }

    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        self.py_to_dict(py)
    }
}
