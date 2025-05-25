use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::task;

use crate::middleware::{
    CallbackClosureRaw, Middleware, MiddlewareRaw, Serializer, SerializerEnum,
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
    session: tokio::sync::OnceCell<Arc<zenoh::Session>>,
}

impl ZenohMiddleware {
    pub fn new() -> Self {
        ZenohMiddleware {
            session: tokio::sync::OnceCell::new(),
        }
    }
}

#[async_trait]
impl MiddlewareRaw for ZenohMiddleware {
    async fn publish_raw(
        &self,
        message_type: &str,
        topic: &str,
        payload: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        println!(
            "ZenohMiddleware: publish_raw called with message_type: {}, topic: {}",
            message_type, topic
        );

        let session = Arc::clone(
            self.session
                .get_or_init(async || {
                    Arc::new(
                        zenoh::open(zenoh::Config::default())
                            .await
                            .expect("Failed to open Zenoh session"),
                    )
                })
                .await,
        );

        session
            .put(topic, payload)
            .await
            .expect("Failed to publish message");

        Ok(())
    }

    async fn subscribe_raw(
        &self,
        message_type: &str,
        topic: &str,
        callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        println!(
            "ZenohMiddleware: subscribe_raw called with message_type: {}, topic: {}",
            message_type, topic
        );

        let session = Arc::clone(
            self.session
                .get_or_init(async || {
                    Arc::new(
                        zenoh::open(zenoh::Config::default())
                            .await
                            .expect("Failed to open Zenoh session"),
                    )
                })
                .await,
        );

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
        _callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        println!(
            "ZenohMiddleware: subscribe_all_raw called with topics: {:?}",
            topics
        );

        let session = Arc::clone(
            self.session
                .get_or_init(async || {
                    Arc::new(
                        zenoh::open(zenoh::Config::default())
                            .await
                            .expect("Failed to open Zenoh session"),
                    )
                })
                .await,
        );

        for (_message_type, topic) in topics {
            let subscriber = session.declare_subscriber(topic).await.unwrap();

            task::spawn(async move {
                while let Ok(sample) = subscriber.recv_async().await {
                    println!(
                        "Received: {:?}",
                        sample
                            .payload()
                            .try_to_string()
                            .expect("Failed to convert payload to string")
                    );
                    // TODO Can't move same callback into multiple tasks
                    // callback(&sample.payload().to_bytes());
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
