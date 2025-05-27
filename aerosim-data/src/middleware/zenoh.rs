use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::task;

use crate::{
    middleware::{
        CallbackClosureRaw, Metadata, Middleware, MiddlewareRaw, PyMiddleware, PySerializer,
        Serializer, SerializerEnum,
    },
    types::TimeStamp,
};

#[pyclass]
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

#[pyclass]
pub struct ZenohMiddleware {
    session: tokio::sync::OnceCell<zenoh::Session>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ZenohMiddleware {
    pub fn new() -> Self {
        ZenohMiddleware {
            session: tokio::sync::OnceCell::new(),
            runtime: Arc::new(
                tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"),
            ),
        }
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
            .await
            .expect("Failed to publish message");

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

        task::spawn(async move {
            while let Ok(sample) = subscriber.recv_async().await {
                let _ = callback(&sample.payload().to_bytes());
            }
        });

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
            task::spawn(async move {
                while let Ok(sample) = subscriber.recv_async().await {
                    let _ = callback_clone(&sample.payload().to_bytes());
                }
            });
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

impl PyMiddleware for ZenohMiddleware {}

#[pymethods]
impl ZenohMiddleware {
    #[new]
    fn pynew(_py: Python) -> PyResult<Self> {
        Ok(Self::new())
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

impl PySerializer for ZenohSerializer {}

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
