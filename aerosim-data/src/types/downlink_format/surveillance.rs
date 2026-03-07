#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::adsb::types::{FlightStatus, ICAOAddress};

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShortAirAirSurveillance {
    pub icao: ICAOAddress,
    pub altitude: u16,
}

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SurveillanceAltitudeReply {
    pub icao: ICAOAddress,
    pub altitude: u16,
    pub flight_status: FlightStatus,
}

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SurveillanceIdentityReply {
    pub icao: ICAOAddress,
    pub identity: u16,
    pub flight_status: FlightStatus,
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl ShortAirAirSurveillance {
    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        let _ = dict.set_item("altitude", self.altitude);
        Ok(dict.into())
    }

    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl SurveillanceAltitudeReply {
    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        let _ = dict.set_item("altitude", self.altitude);
        let _ = dict.set_item("flight_status", self.flight_status);
        Ok(dict.into())
    }

    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl SurveillanceIdentityReply {
    pub fn __dict__(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("icao", self.icao.to_hex());
        let _ = dict.set_item("identity", self.identity);
        let _ = dict.set_item("flight_status", self.flight_status);
        Ok(dict.into())
    }

    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        self.__dict__(py)
    }
}
