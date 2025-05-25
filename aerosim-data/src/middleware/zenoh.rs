use std::error::Error;

use async_trait::async_trait;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;

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
pub struct ZenohMiddleware {}

impl ZenohMiddleware {
    pub fn new() -> Self {
        ZenohMiddleware {}
    }
}

#[async_trait]
impl MiddlewareRaw for ZenohMiddleware {
    async fn publish_raw(
        &self,
        message_type: &str,
        topic: &str,
        _payload: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        println!(
            "ZenohMiddleware: publish_raw called with message_type: {}, topic: {}",
            message_type, topic
        );
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
        self.subscribe_all_raw(
            vec![(message_type.to_string(), topic.to_string())],
            callback,
        )
        .await
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
        Ok(())
    }
}

#[async_trait]
impl Middleware for ZenohMiddleware {
    fn get_serializer(&self) -> SerializerEnum {
        SerializerEnum::from(ZenohSerializer {})
    }
}
