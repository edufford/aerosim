use crate::middleware::{CallbackClosureRaw, Middleware, MiddlewareRaw, Serializer, SerializerEnum};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[cfg(feature = "python")]
use {
    pyo3::prelude::*,
    crate::middleware::PyMiddleware,
};

//Dummy middleware/serializer types to allow this crate to be built without any
// middleware dependencies for crates that only need the data types by importing
// with 'default-features = false'
#[cfg_attr(feature = "python", pyclass)]
pub struct NoMiddleware {}

#[async_trait]
impl MiddlewareRaw for NoMiddleware {
    async fn publish_raw(
        &self,
        _message_type: &str,
        _topic: &str,
        _payload: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    async fn subscribe_raw(
        &self,
        _message_type: &str,
        _topic: &str,
        _callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    async fn subscribe_all_raw(
        &self,
        _topics: Vec<(String, String)>,
        _callback: CallbackClosureRaw,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[cfg(feature = "python")]
impl PyMiddleware for NoMiddleware {}

#[async_trait]
impl Middleware for NoMiddleware {
    fn get_serializer(&self) -> SerializerEnum {
        SerializerEnum::from(NoSerializer {})
    }
}

#[cfg_attr(feature = "python", pyclass)]
pub struct NoSerializer;

impl Serializer for NoSerializer {
    fn serializer(&self) -> SerializerEnum {
        SerializerEnum::from(Self {})
    }

    fn serialize<T: Serialize>(&self, _data: &T) -> Option<Vec<u8>> {
        None
    }

    fn deserialize<T: for<'de> Deserialize<'de>>(&self, _payload: &[u8]) -> Option<T> {
        None
    }
}
