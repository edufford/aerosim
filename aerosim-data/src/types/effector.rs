use crate::types::geometry::Pose;
use crate::AerosimMessage;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "python")]
use {
    crate::types::PyTypeSupport,
    pyo3::{
        prelude::*,
        types::{PyCapsule, PyDict},
    },
};

#[derive(Debug, Default, Clone, Serialize, Deserialize, AerosimMessage, JsonSchema)]
#[cfg_attr(feature = "python", pyclass(get_all))]
pub struct EffectorState {
    pub pose: Pose,
}

impl EffectorState {
    pub fn new(pose: Pose) -> Self {
        EffectorState { pose }
    }
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl EffectorState {
    #[new]
    #[pyo3(signature = (pose=Pose::default()))]
    fn py_new(pose: Pose) -> Self {
        Self::new(pose)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("pose", self.pose.to_dict(py)?)?;
        Ok(dict.into())
    }

    #[classattr]
    fn __type_support__() -> Py<PyCapsule> {
        PyTypeSupport::create::<Self>()
    }
}
