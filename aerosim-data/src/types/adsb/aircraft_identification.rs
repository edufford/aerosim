use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AircraftIdentification {
    /// Type Code
    pub tc: u8,
    /// Aircraft Category
    pub ca: u8,
    /// A character
    pub cn: String,
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl AircraftIdentification {
    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("tc", self.tc)?;
        let _ = dict.set_item("ca", self.ca)?;
        let _ = dict.set_item("cn", self.cn.clone())?;
        Ok(dict.into())
    }

    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        self.to_dict(py)
    }
}
