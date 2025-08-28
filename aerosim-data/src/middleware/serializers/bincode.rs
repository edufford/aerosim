use pyo3::prelude::*;

use bincode;
use serde::{Deserialize, Serialize};

use crate::middleware::{Metadata, PySerializer, Serializer, SerializerEnum};

#[pyclass]
pub struct BincodeSerializer;

impl Serializer for BincodeSerializer {
    fn serializer(&self) -> SerializerEnum {
        SerializerEnum::from(Self {})
    }

    fn serialize<T: Serialize>(&self, data: &T) -> Option<Vec<u8>> {
        bincode::serialize(data).ok()
    }

    fn deserialize<T: for<'de> Deserialize<'de>>(&self, payload: &[u8]) -> Option<T> {
        bincode::deserialize(payload).ok()
    }
}

impl PySerializer for BincodeSerializer {}

#[pymethods]
impl BincodeSerializer {
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
        let serializer = SerializerEnum::from(BincodeSerializer {});
        self.pyserialize_message_impl(py, &serializer, metadata, data)
    }

    #[pyo3(name = "deserialize_message")]
    fn pydeserialize_message(
        &self,
        py: Python<'_>,
        message_type: PyObject,
        payload: &[u8],
    ) -> Option<(Metadata, PyObject)> {
        let serializer = SerializerEnum::from(BincodeSerializer {});
        self.pydeserialize_message_impl(py, &serializer, message_type, payload)
    }

    #[pyo3(name = "deserialize_metadata")]
    fn pydeserialize_metadata(&self, py: Python<'_>, payload: &[u8]) -> Option<Metadata> {
        let serializer = SerializerEnum::from(BincodeSerializer {});
        self.pydeserialize_metadata_impl(py, &serializer, payload)
    }

    #[pyo3(name = "deserialize_data")]
    fn pydeserialize_data(
        &self,
        py: Python<'_>,
        message_type: PyObject,
        payload: &[u8],
    ) -> Option<PyObject> {
        let serializer = SerializerEnum::from(BincodeSerializer {});
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
        let serializer = SerializerEnum::from(BincodeSerializer {});
        self.pyserialize_from_json_impl(py, &serializer, type_name, metadata, data)
    }

    #[pyo3(name = "to_json")]
    fn pydeserialize_to_json(
        &self,
        py: Python<'_>,
        type_name: &str,
        payload: &[u8],
    ) -> Option<PyObject> {
        let serializer = SerializerEnum::from(BincodeSerializer {});
        self.pydeserialize_to_json_impl(py, &serializer, type_name, payload)
    }
}
