use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::TimeStamp;

#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};

const SENTINEL_SECONDS: i32 = i32::MIN;

#[cfg_attr(feature = "python", pyclass(get_all))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Metadata {
    pub topic: String,
    pub type_name: String,

    /// Discrete simulation time.
    /// A sentinel value {sec: i32::MIN, nanosec: 0} is set when the simulation time is not specified.
    pub timestamp_sim: TimeStamp,

    /// Absolute platform time since the Unix Epoch.
    pub timestamp_platform: TimeStamp,
}

impl Metadata {
    pub fn new(
        topic: &str,
        type_name: &str,
        timestamp_sim: Option<TimeStamp>,
        timestamp_platform: Option<TimeStamp>,
    ) -> Self {
        Metadata {
            topic: topic.to_string(),
            type_name: type_name.to_string(),
            timestamp_sim: timestamp_sim.unwrap_or(TimeStamp::new(SENTINEL_SECONDS, 0)),
            timestamp_platform: timestamp_platform.unwrap_or(TimeStamp::now()),
        }
    }

    pub fn is_sim_time_valid(&self) -> bool {
        self.timestamp_sim.sec >= 0
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Metadata {
    #[new]
    #[pyo3(signature = (topic, type_name, timestamp_sim=None, timestamp_platform=None))]
    fn py_new(
        topic: &str,
        type_name: &str,
        timestamp_sim: Option<TimeStamp>,
        timestamp_platform: Option<TimeStamp>,
    ) -> Self {
        Self::new(topic, type_name, timestamp_sim, timestamp_platform)
    }

    #[pyo3(name = "is_sim_time_valid")]
    fn py_is_sim_time_valid(&self) -> bool {
        self.is_sim_time_valid()
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("topic", self.topic.clone())?;
        dict.set_item("type_name", self.type_name.clone())?;
        dict.set_item("timestamp_sim", self.timestamp_sim.to_dict(py)?)?;
        dict.set_item("timestamp_platform", self.timestamp_platform.to_dict(py)?)?;
        Ok(dict.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_new_with_defaults() {
        let metadata = Metadata::new("test_topic", "TestType", None, None);

        assert_eq!(metadata.topic, "test_topic");
        assert_eq!(metadata.type_name, "TestType");
        // When timestamp_sim is None, it should be set to sentinel value
        assert_eq!(metadata.timestamp_sim.sec, SENTINEL_SECONDS);
        assert_eq!(metadata.timestamp_sim.nanosec, 0);
        // timestamp_platform should be set to current time (non-negative)
        assert!(metadata.timestamp_platform.sec >= 0);
    }

    #[test]
    fn test_metadata_new_with_timestamps() {
        let sim_time = TimeStamp::new(100, 500);
        let platform_time = TimeStamp::new(200, 1000);
        let metadata = Metadata::new("topic", "Type", Some(sim_time), Some(platform_time));

        assert_eq!(metadata.topic, "topic");
        assert_eq!(metadata.type_name, "Type");
        assert_eq!(metadata.timestamp_sim.sec, 100);
        assert_eq!(metadata.timestamp_sim.nanosec, 500);
        assert_eq!(metadata.timestamp_platform.sec, 200);
        assert_eq!(metadata.timestamp_platform.nanosec, 1000);
    }

    #[test]
    fn test_metadata_is_sim_time_valid_with_valid_time() {
        let sim_time = TimeStamp::new(0, 0);
        let metadata = Metadata::new("topic", "Type", Some(sim_time), None);

        assert!(metadata.is_sim_time_valid());
    }

    #[test]
    fn test_metadata_is_sim_time_valid_with_positive_time() {
        let sim_time = TimeStamp::new(100, 500);
        let metadata = Metadata::new("topic", "Type", Some(sim_time), None);

        assert!(metadata.is_sim_time_valid());
    }

    #[test]
    fn test_metadata_is_sim_time_valid_with_sentinel() {
        let metadata = Metadata::new("topic", "Type", None, None);

        assert!(!metadata.is_sim_time_valid());
    }

    #[test]
    fn test_metadata_serialize_deserialize() {
        let sim_time = TimeStamp::new(50, 250);
        let platform_time = TimeStamp::new(1000, 500);
        let original = Metadata::new("ser_topic", "SerType", Some(sim_time), Some(platform_time));

        // Serialize to JSON
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.topic, original.topic);
        assert_eq!(deserialized.type_name, original.type_name);
        assert_eq!(deserialized.timestamp_sim, original.timestamp_sim);
        assert_eq!(deserialized.timestamp_platform, original.timestamp_platform);
    }

    #[test]
    fn test_metadata_clone() {
        let metadata = Metadata::new("clone_topic", "CloneType", None, None);
        let cloned = metadata.clone();

        assert_eq!(cloned.topic, metadata.topic);
        assert_eq!(cloned.type_name, metadata.type_name);
        assert_eq!(cloned.timestamp_sim, metadata.timestamp_sim);
        assert_eq!(cloned.timestamp_platform, metadata.timestamp_platform);
    }

    #[test]
    fn test_metadata_equality() {
        let sim_time = TimeStamp::new(10, 20);
        let platform_time = TimeStamp::new(30, 40);
        let meta1 = Metadata::new("topic", "Type", Some(sim_time), Some(platform_time));
        let meta2 = Metadata::new("topic", "Type", Some(sim_time), Some(platform_time));

        assert_eq!(meta1, meta2);
    }

    #[test]
    fn test_metadata_inequality() {
        let meta1 = Metadata::new("topic1", "Type", None, None);
        let meta2 = Metadata::new("topic2", "Type", None, None);

        assert_ne!(meta1, meta2);
    }
}
