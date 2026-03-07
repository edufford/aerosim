#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::adsb::types::ICAOAddress;
use crate::types::downlink_format::bds::BDS;

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExtendedSquitterMilitaryApplication {
    pub reserved: u8,
}

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CommBAltitudeReply {
    pub icao: ICAOAddress,
    pub altitude: u16,
    pub bds: BDS,
}

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CommBIdentityReply {
    pub icao: ICAOAddress,
    pub squawk: u32,
    pub bds: BDS,
}

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Copy, Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CommDExtendedLengthMessage {
    pub icao: ICAOAddress,
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl CommBAltitudeReply {
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }

    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        let _ = dict.set_item("altitude", self.altitude);
        let _ = dict.set_item("bds", self.bds.py_to_dict(py)?);
        Ok(dict.into())
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl CommBIdentityReply {
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }

    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        let _ = dict.set_item("squawk", self.squawk);
        let _ = dict.set_item("bds", self.bds.py_to_dict(py)?);
        Ok(dict.into())
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl CommDExtendedLengthMessage {
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }

    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        Ok(dict.into())
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl ExtendedSquitterMilitaryApplication {
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }

    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("reserved", self.reserved);
        Ok(dict.into())
    }
}
