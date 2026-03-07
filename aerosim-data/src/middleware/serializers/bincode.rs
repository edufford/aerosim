use bincode;
use serde::{Deserialize, Serialize};

use crate::middleware::{Serializer, SerializerEnum};

#[cfg(feature = "python")]
use {
    crate::middleware::{Metadata, PySerializer},
    pyo3::prelude::*,
};

#[cfg_attr(feature = "python", pyclass)]
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

#[cfg(feature = "python")]
impl PySerializer for BincodeSerializer {}

#[cfg(feature = "python")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::Metadata;
    use crate::types::{CameraInfo, Image, ImageEncoding, TimeStamp};

    fn create_test_camera_info(width: u32, height: u32) -> CameraInfo {
        CameraInfo::new(
            width,
            height,
            "plumb_bob".to_string(),
            vec![0.0; 5],
            [1.0, 0.0, 320.0, 0.0, 1.0, 240.0, 0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            [1.0, 0.0, 320.0, 0.0, 0.0, 1.0, 240.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        )
    }

    fn create_test_image(width: u32, height: u32, encoding: ImageEncoding) -> Image {
        let channels: u32 = match encoding {
            ImageEncoding::RGB8 | ImageEncoding::BGR8 => 3,
            ImageEncoding::RGBA8 | ImageEncoding::BGRA8 => 4,
            ImageEncoding::MONO8 => 1,
            ImageEncoding::MONO16 => 2,
            ImageEncoding::YUV422 => 2,
        };
        let step = width * channels;
        let data_size = (height * step) as usize;
        // Create gradient pixel data
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        Image::new(
            create_test_camera_info(width, height),
            height,
            width,
            encoding,
            0, // little-endian
            step,
            data,
        )
    }

    #[test]
    fn test_bincode_serialize_small_rgb8_image() {
        let serializer = BincodeSerializer;
        let image = create_test_image(4, 4, ImageEncoding::RGB8);

        let serialized = serializer.serialize(&image);
        assert!(serialized.is_some());

        let deserialized: Option<Image> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());

        let result = deserialized.unwrap();
        assert_eq!(result.width, 4);
        assert_eq!(result.height, 4);
        assert_eq!(result.encoding, ImageEncoding::RGB8);
        assert_eq!(result.data.len(), 48); // 4x4x3
    }

    #[test]
    fn test_bincode_serialize_rgba8_image() {
        let serializer = BincodeSerializer;
        let image = create_test_image(8, 8, ImageEncoding::RGBA8);

        let serialized = serializer.serialize(&image);
        assert!(serialized.is_some());

        let deserialized: Option<Image> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());

        let result = deserialized.unwrap();
        assert_eq!(result.width, 8);
        assert_eq!(result.height, 8);
        assert_eq!(result.encoding, ImageEncoding::RGBA8);
        assert_eq!(result.data.len(), 256); // 8x8x4
    }

    #[test]
    fn test_bincode_serialize_larger_bgra8_image() {
        let serializer = BincodeSerializer;
        // 64x64 BGRA image (16384 bytes of pixel data)
        let image = create_test_image(64, 64, ImageEncoding::BGRA8);

        let serialized = serializer.serialize(&image);
        assert!(serialized.is_some());

        let deserialized: Option<Image> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());

        let result = deserialized.unwrap();
        assert_eq!(result.width, 64);
        assert_eq!(result.height, 64);
        assert_eq!(result.encoding, ImageEncoding::BGRA8);
        assert_eq!(result.data.len(), 16384); // 64x64x4
    }

    #[test]
    fn test_bincode_serialize_mono8_image() {
        let serializer = BincodeSerializer;
        let image = create_test_image(16, 16, ImageEncoding::MONO8);

        let serialized = serializer.serialize(&image);
        assert!(serialized.is_some());

        let deserialized: Option<Image> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());

        let result = deserialized.unwrap();
        assert_eq!(result.encoding, ImageEncoding::MONO8);
        assert_eq!(result.data.len(), 256); // 16x16x1
    }

    #[test]
    fn test_bincode_image_with_metadata() {
        let serializer = BincodeSerializer;

        let metadata = Metadata::new(
            "camera/rgb",
            "aerosim::types::Image",
            Some(TimeStamp::new(10, 500000000)),
            None,
        );

        let image = create_test_image(4, 4, ImageEncoding::RGB8);

        let payload = serializer.serialize_message(&metadata, &image);
        assert!(payload.is_some());

        let (deserialized_meta, deserialized_image): (Metadata, Image) =
            serializer.deserialize_message(&payload.unwrap()).unwrap();

        assert_eq!(deserialized_meta.topic, "camera/rgb");
        assert_eq!(deserialized_meta.type_name, "aerosim::types::Image");
        assert_eq!(deserialized_image.width, 4);
        assert_eq!(deserialized_image.height, 4);
    }

    #[test]
    fn test_bincode_serializer_returns_self() {
        let serializer = BincodeSerializer;
        let returned = serializer.serializer();
        match returned {
            SerializerEnum::BincodeSerializer(_) => {}
            _ => panic!("Expected BincodeSerializer variant"),
        }
    }

    #[test]
    fn test_bincode_deserialize_corrupted_data() {
        let serializer = BincodeSerializer;
        // Corrupted/truncated data
        let corrupted: &[u8] = &[0x04, 0x00, 0x00];
        let result: Option<Image> = serializer.deserialize(corrupted);
        assert!(result.is_none());
    }

    #[test]
    fn test_bincode_raw_pixel_array() {
        let serializer = BincodeSerializer;

        // Raw u8 array representing pixel data
        let pixels: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();

        let serialized = serializer.serialize(&pixels);
        assert!(serialized.is_some());

        let deserialized: Option<Vec<u8>> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());
        assert_eq!(deserialized.unwrap(), pixels);
    }

    #[test]
    fn test_bincode_camera_info_roundtrip() {
        let serializer = BincodeSerializer;
        let camera_info = create_test_camera_info(640, 480);

        let serialized = serializer.serialize(&camera_info);
        assert!(serialized.is_some());

        let deserialized: Option<CameraInfo> = serializer.deserialize(&serialized.unwrap());
        assert!(deserialized.is_some());

        let result = deserialized.unwrap();
        assert_eq!(result.width, 640);
        assert_eq!(result.height, 480);
        assert_eq!(result.distortion_model, "plumb_bob");
    }
}
