use super::timestamp::TimeStamp;
use serde::{Deserialize, Serialize};

#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};

const SENTINEL_SECONDS: i32 = i32::MIN;

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Header {
    /// Discrete simulation time.
    /// A sentinel value {sec: i32::MIN, nanosec: 0} is set when the simulation time is not specified.
    pub timestamp_sim: TimeStamp,

    /// Absolute platform time since the Unix Epoch.
    pub timestamp_platform: TimeStamp,

    /// Identifier for the coordinate frame.
    pub frame_id: String,
}

impl Header {
    pub fn new(timestamp_sim: TimeStamp, timestamp_platform: TimeStamp, frame_id: &str) -> Self {
        Header {
            timestamp_sim,
            timestamp_platform,
            frame_id: frame_id.to_string(),
        }
    }

    pub fn with_timestamp(timestamp: TimeStamp) -> Self {
        Header {
            timestamp_sim: timestamp,
            timestamp_platform: TimeStamp::now(),
            frame_id: "".to_string(),
        }
    }

    pub fn is_sim_time_valid(&self) -> bool {
        self.timestamp_sim.sec >= 0
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Header {
    #[new]
    #[pyo3(signature = (timestamp_sim, timestamp_platform, frame_id))]
    fn py_new(timestamp_sim: TimeStamp, timestamp_platform: TimeStamp, frame_id: &str) -> Self {
        Self::new(timestamp_sim, timestamp_platform, frame_id)
    }

    #[staticmethod]
    #[pyo3(name = "default")]
    pub fn py_default() -> Self {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "with_timestamp")]
    fn py_with_timestamp(timestamp: TimeStamp) -> Self {
        Self::with_timestamp(timestamp)
    }

    #[pyo3(name = "is_sim_time_valid")]
    fn py_is_sim_time_valid(&self) -> bool {
        self.is_sim_time_valid()
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("timestamp_sim", self.timestamp_sim.to_dict(py)?)?;
        dict.set_item("timestamp_platform", self.timestamp_platform.to_dict(py)?)?;
        dict.set_item("frame_id", self.frame_id.clone())?;
        Ok(dict.into())
    }
}

impl Default for Header {
    fn default() -> Self {
        Header {
            timestamp_sim: TimeStamp::new(SENTINEL_SECONDS, 0),
            timestamp_platform: TimeStamp::now(),
            frame_id: "".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
        let timestamp_sim = TimeStamp::new(0, 0);
        let timestamp_platform = TimeStamp::new(1, 0);
        let frame_id = "world";
        let header = Header::new(timestamp_sim, timestamp_platform, frame_id);
        assert_eq!(header.timestamp_sim.sec, timestamp_sim.sec);
        assert_eq!(header.timestamp_sim.nanosec, timestamp_sim.nanosec);
        assert_eq!(header.timestamp_platform.sec, timestamp_platform.sec);
        assert_eq!(header.timestamp_platform.nanosec, timestamp_platform.nanosec);
        assert_eq!(header.frame_id, frame_id);
    }
}
