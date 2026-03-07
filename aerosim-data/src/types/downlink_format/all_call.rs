#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::adsb::types::{Capability, ICAOAddress};

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AllCallReply {
    pub icao: ICAOAddress,
    pub capability: Capability,
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl AllCallReply {
    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        let _ = dict.set_item("capability", self.capability);
        Ok(dict.into())
    }

    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }
}
