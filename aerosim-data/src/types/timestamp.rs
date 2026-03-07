use crate::AerosimMessage;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(feature = "python")]
use {
    crate::types::PyTypeSupport,
    pyo3::{
        prelude::*,
        types::{PyCapsule, PyDict},
    },
};

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    JsonSchema,
    aerosim_macros::AerosimMessage,
)]
pub struct TimeStamp {
    pub sec: i32,
    pub nanosec: u32,
}

// Base Rust implementation
impl TimeStamp {
    pub fn new(sec: i32, nanosec: u32) -> Self {
        TimeStamp { sec, nanosec }
    }

    pub fn from_sec(sec: f64) -> Self {
        TimeStamp {
            sec: sec.trunc() as i32,
            nanosec: (sec.fract() * 1e9) as u32,
        }
    }

    pub fn to_sec(&self) -> f64 {
        self.sec as f64 + self.nanosec as f64 * 1e-9
    }

    pub fn to_sec_rounded(&self, round_num_digits: u32) -> f64 {
        let raw_sec: f64 = self.to_sec();
        let multiplier = 10.0_f64.powi(round_num_digits as i32);
        (raw_sec * multiplier).round() / multiplier
    }

    pub fn from_millis(millisec: u64) -> Self {
        let dur = Duration::from_millis(millisec);
        TimeStamp {
            sec: dur.as_secs() as i32,
            nanosec: dur.subsec_nanos(),
        }
    }

    pub fn to_millis(&self) -> u64 {
        self.sec as u64 * 1e3 as u64 + self.nanosec as u64 / 1e6 as u64
    }

    pub fn from_nanos(nanosec: u64) -> Self {
        let dur = Duration::from_nanos(nanosec);
        TimeStamp {
            sec: dur.as_secs() as i32,
            nanosec: dur.subsec_nanos(),
        }
    }

    pub fn to_nanos(&self) -> u64 {
        self.sec as u64 * 1e9 as u64 + self.nanosec as u64
    }

    // Create a new TimeStamp from current time
    pub fn now() -> Self {
        let now: DateTime<Utc> = Utc::now();
        TimeStamp {
            sec: now.timestamp() as i32,           // Seconds since epoch
            nanosec: now.timestamp_subsec_nanos(), // Nanoseconds since last second
        }
    }
}

// Python interface layer
#[cfg(feature = "python")]
#[pymethods]
impl TimeStamp {
    #[new]
    fn py_new(sec: i32, nanosec: u32) -> Self {
        Self::new(sec, nanosec)
    }

    #[staticmethod]
    #[pyo3(name = "from_sec")]
    fn py_from_sec(sec: f64) -> Self {
        Self::from_sec(sec)
    }

    #[pyo3(name = "to_sec")]
    fn py_to_sec(&self) -> f64 {
        self.to_sec()
    }

    #[pyo3(name = "to_sec_rounded")]
    fn py_to_sec_rounded(&self, round_num_digits: u32) -> f64 {
        self.to_sec_rounded(round_num_digits)
    }

    #[staticmethod]
    #[pyo3(name = "from_millis")]
    fn py_from_millis(millisec: u64) -> Self {
        Self::from_millis(millisec)
    }

    #[pyo3(name = "to_millis")]
    fn py_to_millis(&self) -> u64 {
        self.to_millis()
    }

    #[staticmethod]
    #[pyo3(name = "from_nanos")]
    fn py_from_nanos(nanosec: u64) -> Self {
        Self::from_nanos(nanosec)
    }

    #[pyo3(name = "to_nanos")]
    fn py_to_nanos(&self) -> u64 {
        self.to_nanos()
    }

    #[staticmethod]
    #[pyo3(name = "now")]
    fn py_now() -> Self {
        Self::now()
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("sec", self.sec)?;
        dict.set_item("nanosec", self.nanosec)?;
        Ok(dict.into())
    }

    #[classattr]
    fn __type_support__() -> Py<PyCapsule> {
        PyTypeSupport::create::<Self>()
    }
}
