use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::middleware::{
    CallbackClosureRaw, Middleware, MiddlewareRaw, Serializer, SerializerEnum,
};

#[cfg(feature = "python")]
use {
    crate::middleware::{Metadata, PyMiddleware, PySerializer},
    crate::types::TimeStamp,
    pyo3::prelude::*,
};

#[cfg_attr(feature = "python", pyclass)]
pub struct ZenohSerializer;

impl Serializer for ZenohSerializer {
    fn serializer(&self) -> SerializerEnum {
        SerializerEnum::from(Self {})
    }

    fn serialize<T: Serialize>(&self, data: &T) -> Option<Vec<u8>> {
        serde_json::to_vec(data).ok()
    }

    fn deserialize<T: for<'de> Deserialize<'de>>(&self, payload: &[u8]) -> Option<T> {
        serde_json::from_slice::<T>(payload).ok()
    }
}

#[cfg_attr(feature = "python", pyclass)]
pub struct ZenohMiddleware {
    session: tokio::sync::OnceCell<zenoh::Session>,
    runtime: Arc<tokio::runtime::Runtime>,
    subscriber_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl ZenohMiddleware {
    pub fn new() -> Self {
        ZenohMiddleware {
            session: tokio::sync::OnceCell::new(),
            runtime: Arc::new(
                tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"),
            ),
            subscriber_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Shutdown the Zenoh middleware, cancelling all subscriber tasks and closing the session.
    pub fn shutdown(&self) {
        // Cancel all subscriber tasks
        let handles = self.subscriber_handles.clone();
        let session = self.session.get().cloned();

        self.runtime.block_on(async {
            let mut handles_lock = handles.lock().await;
            let num_handles = handles_lock.len();
            for handle in handles_lock.drain(..) {
                handle.abort();
            }

            // Close the session if it was initialized
            if let Some(sess) = session {
                if let Err(e) = sess.close().await {
                    error!("Error closing Zenoh session: {:?}", e);
                }
            }

            if num_handles > 0 {
                info!(
                    "ZenohMiddleware: Shutdown complete ({} subscriber tasks cancelled)",
                    num_handles
                );
            }
        });
    }
}

#[async_trait]
impl MiddlewareRaw for ZenohMiddleware {
    async fn publish_raw(
        &self,
        _message_type: &str,
        topic: &str,
        payload: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let session = self
            .session
            .get_or_init(|| async {
                zenoh::open(zenoh::Config::default())
                    .await
                    .expect("Failed to open Zenoh session")
            })
            .await;

        session
            .put(topic, payload)
            .congestion_control(zenoh::qos::CongestionControl::Block)
            .await
            .map_err(|e| -> Box<dyn Error> {
                format!("Failed to publish topic {} with error: {}", topic, e).into()
            })?;

        Ok(())
    }

    async fn subscribe_raw(
        &self,
        _message_type: &str,
        topic: &str,
        callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        let session = self
            .session
            .get_or_init(|| async {
                zenoh::open(zenoh::Config::default())
                    .await
                    .expect("Failed to open Zenoh session")
            })
            .await;

        let subscriber = session.declare_subscriber(topic).await.unwrap();

        let handle = tokio::task::spawn(async move {
            while let Ok(sample) = subscriber.recv_async().await {
                let _ = callback(&sample.payload().to_bytes());
            }
        });

        // Track the subscriber handle for cleanup
        self.subscriber_handles.lock().await.push(handle);

        Ok(())
    }

    async fn subscribe_all_raw(
        &self,
        topics: Vec<(String, String)>,
        callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        let session = self
            .session
            .get_or_init(|| async {
                zenoh::open(zenoh::Config::default())
                    .await
                    .expect("Failed to open Zenoh session")
            })
            .await;

        let callback_arc = Arc::new(callback);

        for (_message_type, topic) in topics {
            let subscriber = session.declare_subscriber(&topic).await.unwrap();
            let callback_clone = Arc::clone(&callback_arc);
            let handle = tokio::task::spawn(async move {
                while let Ok(sample) = subscriber.recv_async().await {
                    let _ = callback_clone(&sample.payload().to_bytes());
                }
            });

            // Track the subscriber handle for cleanup
            self.subscriber_handles.lock().await.push(handle);
        }

        Ok(())
    }
}

#[async_trait]
impl Middleware for ZenohMiddleware {
    fn get_serializer(&self) -> SerializerEnum {
        SerializerEnum::from(ZenohSerializer {})
    }
}

#[cfg(feature = "python")]
impl PyMiddleware for ZenohMiddleware {}

#[cfg(feature = "python")]
#[pymethods]
impl ZenohMiddleware {
    #[new]
    fn pynew(_py: Python) -> PyResult<Self> {
        Ok(Self::new())
    }

    /// Close the Zenoh middleware, cancelling all subscriber tasks and closing the session.
    /// This should be called before the Python process exits to ensure clean shutdown.
    #[pyo3(name = "close")]
    fn pyclose(&self) {
        self.shutdown();
    }

    #[pyo3(name = "publish")]
    #[pyo3(signature = (topic, message, timestamp_sim=None))]
    fn pypublish(
        &self,
        py: Python,
        topic: &str,
        message: PyObject,
        timestamp_sim: Option<TimeStamp>,
    ) -> PyResult<()> {
        let res = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(self.pypublish_impl(py, topic, message, timestamp_sim))
            }),
            Err(_) => self
                .runtime
                .block_on(self.pypublish_impl(py, topic, message, timestamp_sim)),
        };
        res
    }

    #[pyo3(name = "subscribe")]
    fn pysubscribe(
        &self,
        py: Python,
        message_type: PyObject,
        topic: &str,
        callback: PyObject,
    ) -> PyResult<()> {
        let res = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(self.pysubscribe_impl(py, message_type, topic, callback))
            }),
            Err(_) => {
                self.runtime
                    .block_on(self.pysubscribe_impl(py, message_type, topic, callback))
            }
        };
        res
    }

    #[pyo3(name = "subscribe_all")]
    fn pysubscribe_all(
        &self,
        py: Python,
        message_type: PyObject,
        topics: Vec<String>,
        callback: PyObject,
    ) -> PyResult<()> {
        let res = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(self.pysubscribe_all_impl(py, message_type, topics, callback))
            }),
            Err(_) => {
                self.runtime
                    .block_on(self.pysubscribe_all_impl(py, message_type, topics, callback))
            }
        };
        res
    }

    #[pyo3(name = "publish_raw")]
    fn pypublish_raw(
        &self,
        py: Python,
        message_type: &str,
        topic: &str,
        payload: Py<pyo3::types::PyBytes>,
    ) -> PyResult<()> {
        let res = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(self.pypublish_raw_impl(py, message_type, topic, payload))
            }),
            Err(_) => {
                self.runtime
                    .block_on(self.pypublish_raw_impl(py, message_type, topic, payload))
            }
        };
        res
    }

    #[pyo3(name = "subscribe_raw")]
    fn pysubscribe_raw(
        &self,
        py: Python<'_>,
        message_type: &str,
        topic: &str,
        callback: PyObject,
    ) -> PyResult<()> {
        let res = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(self.pysubscribe_raw_impl(py, message_type, topic, callback))
            }),
            Err(_) => {
                self.runtime
                    .block_on(self.pysubscribe_raw_impl(py, message_type, topic, callback))
            }
        };
        res
    }

    #[pyo3(name = "subscribe_all_raw")]
    fn pysubscribe_all_raw(
        &self,
        py: Python,
        topics: Vec<(String, String)>,
        callback: PyObject,
    ) -> PyResult<()> {
        let res = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(self.pysubscribe_all_raw_impl(py, topics, callback))
            }),
            Err(_) => self
                .runtime
                .block_on(self.pysubscribe_all_raw_impl(py, topics, callback)),
        };
        res
    }
}

#[cfg(feature = "python")]
impl PySerializer for ZenohSerializer {}

#[cfg(feature = "python")]
#[pymethods]
impl ZenohSerializer {
    #[new]
    fn pynew(_py: Python) -> PyResult<Self> {
        Ok(Self {})
    }

    #[pyo3(name = "serialize_message")]
    fn pyserialize_message(
        &self,
        py: Python<'_>,
        metadata: Metadata,
        data: PyObject,
    ) -> Option<Vec<u8>> {
        let serializer = SerializerEnum::from(ZenohSerializer {});
        self.pyserialize_message_impl(py, &serializer, metadata, data)
    }

    #[pyo3(name = "deserialize_message")]
    fn pydeserialize_message(
        &self,
        py: Python<'_>,
        message_type: PyObject,
        payload: &[u8],
    ) -> Option<(Metadata, PyObject)> {
        let serializer = SerializerEnum::from(ZenohSerializer {});
        self.pydeserialize_message_impl(py, &serializer, message_type, payload)
    }

    #[pyo3(name = "deserialize_metadata")]
    fn pydeserialize_metadata(&self, py: Python<'_>, payload: &[u8]) -> Option<Metadata> {
        let serializer = SerializerEnum::from(ZenohSerializer {});
        self.pydeserialize_metadata_impl(py, &serializer, payload)
    }

    #[pyo3(name = "deserialize_data")]
    fn pydeserialize_data(
        &self,
        py: Python<'_>,
        message_type: PyObject,
        payload: &[u8],
    ) -> Option<PyObject> {
        let serializer = SerializerEnum::from(ZenohSerializer {});
        self.pydeserialize_data_impl(py, &serializer, message_type, payload)
    }

    #[pyo3(name = "from_json")]
    fn pyserialize_from_json(
        &self,
        py: Python<'_>,
        type_name: &str,
        metadata: &Metadata,
        data: PyObject,
    ) -> Option<Vec<u8>> {
        let serializer = SerializerEnum::from(ZenohSerializer {});
        self.pyserialize_from_json_impl(py, &serializer, type_name, metadata, data)
    }

    #[pyo3(name = "to_json")]
    fn pydeserialize_to_json(
        &self,
        py: Python<'_>,
        type_name: &str,
        payload: &[u8],
    ) -> Option<PyObject> {
        let serializer = SerializerEnum::from(ZenohSerializer {});
        self.pydeserialize_to_json_impl(py, &serializer, type_name, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestData {
        id: u32,
        name: String,
        value: f64,
    }

    #[test]
    fn test_zenoh_serializer_roundtrip() {
        let serializer = ZenohSerializer;
        let original = TestData {
            id: 42,
            name: "test".to_string(),
            value: 3.14,
        };

        let serialized = serializer.serialize(&original);
        assert!(serialized.is_some());

        let deserialized: Option<TestData> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());
        assert_eq!(deserialized.unwrap(), original);
    }

    #[test]
    fn test_zenoh_serializer_json_format() {
        let serializer = ZenohSerializer;
        let data = TestData {
            id: 1,
            name: "json_test".to_string(),
            value: 2.5,
        };

        let serialized = serializer.serialize(&data).unwrap();
        let json_str = String::from_utf8(serialized).unwrap();

        // Verify it's valid JSON
        assert!(json_str.contains("\"id\":1"));
        assert!(json_str.contains("\"name\":\"json_test\""));
        assert!(json_str.contains("\"value\":2.5"));
    }

    #[test]
    fn test_zenoh_serializer_deserialize_invalid_json() {
        let serializer = ZenohSerializer;
        let invalid_json = b"not valid json{{{";
        let result: Option<TestData> = serializer.deserialize(invalid_json);
        assert!(result.is_none());
    }

    #[test]
    fn test_zenoh_serializer_returns_self() {
        let serializer = ZenohSerializer;
        let returned = serializer.serializer();
        match returned {
            SerializerEnum::ZenohSerializer(_) => {}
            _ => panic!("Expected ZenohSerializer variant"),
        }
    }

    #[test]
    fn test_zenoh_serializer_serialize_message_with_metadata() {
        let serializer = ZenohSerializer;
        let metadata = Metadata::new("test_topic", "TestData", None, None);
        let data = TestData {
            id: 100,
            name: "message_test".to_string(),
            value: 99.9,
        };

        let payload = serializer.serialize_message(&metadata, &data);
        assert!(payload.is_some());

        let (deserialized_meta, deserialized_data): (Metadata, TestData) =
            serializer.deserialize_message(&payload.unwrap()).unwrap();

        assert_eq!(deserialized_meta.topic, "test_topic");
        assert_eq!(deserialized_meta.type_name, "TestData");
        assert_eq!(deserialized_data, data);
    }

    #[test]
    fn test_zenoh_serializer_deserialize_metadata_only() {
        let serializer = ZenohSerializer;
        let metadata = Metadata::new("meta_topic", "MetaType", None, None);
        let data = TestData {
            id: 1,
            name: "test".to_string(),
            value: 1.0,
        };

        let payload = serializer.serialize_message(&metadata, &data).unwrap();
        let deserialized_meta = serializer.deserialize_metadata(&payload);

        assert!(deserialized_meta.is_some());
        let meta = deserialized_meta.unwrap();
        assert_eq!(meta.topic, "meta_topic");
        assert_eq!(meta.type_name, "MetaType");
    }

    #[test]
    fn test_zenoh_serializer_deserialize_data_only() {
        let serializer = ZenohSerializer;
        let metadata = Metadata::new("data_topic", "DataType", None, None);
        let data = TestData {
            id: 55,
            name: "data_only".to_string(),
            value: 55.5,
        };

        let payload = serializer.serialize_message(&metadata, &data).unwrap();
        let deserialized_data: Option<TestData> = serializer.deserialize_data(&payload);

        assert!(deserialized_data.is_some());
        assert_eq!(deserialized_data.unwrap(), data);
    }
}
