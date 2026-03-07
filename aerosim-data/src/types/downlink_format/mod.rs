#[cfg(feature = "python")]
use pyo3::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::adsb::types;

pub mod all_call;
pub mod bds;
pub mod comm;
pub mod long_air_air;
pub mod surveillance;
pub mod tisb;

#[cfg_attr(feature = "python", pyclass)]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DownlinkFormat {
    GNSSPositionData(super::adsb::gnss_position_data::GNSSPositionData),
    ShortAirAir(surveillance::ShortAirAirSurveillance),
    SurveillanceAltitude(surveillance::SurveillanceAltitudeReply),
    SurveillanceIdentity(surveillance::SurveillanceIdentityReply),
    AllCall(all_call::AllCallReply),
    LongAirAir(long_air_air::LongAirAir),
    ADSB(types::ADSB),
    TisB(tisb::TisB),
    ExtendedSquitterMilitaryApplication(comm::ExtendedSquitterMilitaryApplication),
    CommBAltitude(comm::CommBAltitudeReply),
    CommBIdentity(comm::CommBIdentityReply),
    CommDExtendedLength(comm::CommDExtendedLengthMessage),
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl DownlinkFormat {
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        match self {
            DownlinkFormat::ShortAirAir(s) => s.py_to_dict(py),
            DownlinkFormat::SurveillanceAltitude(s) => s.py_to_dict(py),
            DownlinkFormat::SurveillanceIdentity(s) => s.py_to_dict(py),
            DownlinkFormat::AllCall(s) => s.py_to_dict(py),
            DownlinkFormat::LongAirAir(s) => s.py_to_dict(py),
            DownlinkFormat::ADSB(s) => s.py_to_dict(py),
            DownlinkFormat::TisB(s) => s.py_to_dict(py),
            DownlinkFormat::ExtendedSquitterMilitaryApplication(s) => s.py_to_dict(py),
            DownlinkFormat::CommBAltitude(s) => s.py_to_dict(py),
            DownlinkFormat::CommBIdentity(s) => s.py_to_dict(py),
            DownlinkFormat::CommDExtendedLength(s) => s.py_to_dict(py),
            DownlinkFormat::GNSSPositionData(s) => s.py_to_dict(py),
        }
    }
}
