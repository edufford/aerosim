use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use turbojpeg;

use crate::{types::downlink_format::DownlinkFormat, AerosimMessage};

use super::Vector3;

#[cfg(feature = "python")]
use {
    crate::types::PyTypeSupport,
    pyo3::{
        exceptions::PyRuntimeError,
        prelude::*,
        types::{PyCapsule, PyDict},
    },
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, EnumString, Display)]
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
pub enum SensorType {
    Camera,
    GNSS,
    ADSB,
    IMU,
    LIDAR,
    RADAR,
}

#[cfg(feature = "python")]
#[pymethods]
impl SensorType {
    #[staticmethod]
    pub fn from_str(s: &str) -> PyResult<Self> {
        s.parse()
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid SensorType"))
    }

    pub fn __str__(&self) -> String {
        self.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("SensorType::{}", self)
    }

    #[staticmethod]
    pub fn to_dict(py: Python) -> PyResult<PyObject> {
        let dict = pyo3::types::PyDict::new(py);
        for variant in [
            SensorType::Camera,
            SensorType::GNSS,
            SensorType::ADSB,
            SensorType::IMU,
            SensorType::LIDAR,
            SensorType::RADAR,
        ] {
            dict.set_item(variant.to_string(), variant.__repr__())?;
        }
        Ok(dict.into())
    }
}

// Image types

#[derive(Clone, Debug, Serialize, Deserialize, EnumString, Display, PartialEq)]
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
pub enum ImageEncoding {
    RGB8,
    RGBA8,
    BGR8,
    BGRA8,
    MONO8,
    MONO16,
    YUV422,
    // Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, EnumString, Display, PartialEq)]
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
pub enum ImageFormat {
    JPEG,
    PNG,
}

#[cfg(feature = "python")]
#[pymethods]
impl ImageEncoding {
    #[staticmethod]
    pub fn from_str(s: &str) -> PyResult<Self> {
        s.parse()
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid ImageEncoding"))
    }

    pub fn __str__(&self) -> String {
        self.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("ImageEncoding::{}", self)
    }

    #[staticmethod]
    pub fn to_dict(py: Python) -> PyResult<PyObject> {
        let dict = pyo3::types::PyDict::new(py);
        for variant in [
            ImageEncoding::RGB8,
            ImageEncoding::RGBA8,
            ImageEncoding::BGR8,
            ImageEncoding::BGRA8,
            ImageEncoding::MONO8,
            ImageEncoding::MONO16,
            ImageEncoding::YUV422,
        ] {
            dict.set_item(variant.to_string(), variant.__repr__())?;
        }
        Ok(dict.into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, AerosimMessage)]
#[cfg_attr(feature = "python", pyclass)]
pub struct Image {
    pub camera_info: CameraInfo,
    pub height: u32,
    pub width: u32,
    pub encoding: ImageEncoding,
    pub is_bigendian: u8,
    pub step: u32,

    #[serde(
        serialize_with = "serialize_pixels",
        deserialize_with = "deserialize_pixels"
    )]
    pub data: Cow<'static, [u8]>,
}

impl Image {
    pub fn new(
        camera_info: CameraInfo,
        height: u32,
        width: u32,
        encoding: ImageEncoding,
        is_bigendian: u8,
        step: u32,
        data: Vec<u8>,
    ) -> Self {
        Image {
            camera_info,
            height,
            width,
            encoding,
            is_bigendian,
            step,
            data: Cow::Owned(data),
        }
    }

    pub fn compress(&self) -> Result<CompressedImage, String> {
        let pixel_format = match self.encoding {
            ImageEncoding::RGB8 => turbojpeg::PixelFormat::RGB,
            ImageEncoding::RGBA8 => turbojpeg::PixelFormat::RGBA,
            ImageEncoding::BGR8 => turbojpeg::PixelFormat::BGR,
            ImageEncoding::BGRA8 => turbojpeg::PixelFormat::BGRA,
            _ => turbojpeg::PixelFormat::BGRA, // fallback for non-RGB formats
        };
        let raw_img = turbojpeg::Image {
            pixels: self.data.as_ref(),
            width: self.width as usize,
            pitch: self.step as usize,
            height: self.height as usize,
            format: pixel_format,
        };

        let mut compressor = turbojpeg::Compressor::new()
            .map_err(|e| format!("Could not create turbojpeg compressor: {}", e))?;
        let _ = compressor.set_quality(80);
        let _ = compressor.set_subsamp(turbojpeg::Subsamp::Sub2x2);
        let data = compressor
            .compress_to_vec(raw_img)
            .map_err(|e| format!("Could not compress raw image to jpeg: {}", e))?;

        Ok(CompressedImage {
            format: ImageFormat::JPEG,
            data: Cow::Owned(data),
        })
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Image {
    #[new]
    fn py_new(
        camera_info: CameraInfo,
        height: u32,
        width: u32,
        encoding: ImageEncoding,
        is_bigendian: u8,
        step: u32,
        data: Vec<u8>,
    ) -> Self {
        Self::new(camera_info, height, width, encoding, is_bigendian, step, data)
    }

    #[getter]
    fn camera_info(&self) -> CameraInfo {
        self.camera_info.clone()
    }

    #[setter]
    fn set_camera_info(&mut self, camera_info: CameraInfo) {
        self.camera_info = camera_info;
    }

    #[getter]
    fn height(&self) -> u32 {
        self.height
    }

    #[setter]
    fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    #[getter]
    fn width(&self) -> u32 {
        self.width
    }

    #[setter]
    fn set_width(&mut self, width: u32) {
        self.width = width;
    }

    #[getter]
    fn encoding(&self) -> ImageEncoding {
        self.encoding.clone()
    }

    #[setter]
    fn set_encoding(&mut self, encoding: ImageEncoding) {
        self.encoding = encoding;
    }

    #[getter]
    fn is_bigendian(&self) -> u8 {
        self.is_bigendian
    }

    #[setter]
    fn set_is_bigendian(&mut self, is_bigendian: u8) {
        self.is_bigendian = is_bigendian;
    }

    #[getter]
    fn step(&self) -> u32 {
        self.step
    }

    #[setter]
    fn set_step(&mut self, step: u32) {
        self.step = step;
    }

    #[getter]
    fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    #[pyo3(name = "compress")]
    pub fn py_compress(&self) -> PyResult<CompressedImage> {
        self.compress().map_err(PyRuntimeError::new_err)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("camera_info", self.camera_info.to_dict(py)?)?;
        dict.set_item("height", self.height)?;
        dict.set_item("width", self.width)?;
        dict.set_item(
            "encoding",
            match self.encoding {
                ImageEncoding::RGB8 => "RGB8",
                ImageEncoding::RGBA8 => "RGBA8",
                ImageEncoding::BGR8 => "BGR8",
                ImageEncoding::BGRA8 => "BGRA8",
                ImageEncoding::MONO8 => "MONO8",
                ImageEncoding::MONO16 => "MONO16",
                ImageEncoding::YUV422 => "YUV422",
                // ImageEncoding::Custom(ref s) => s,
            },
        )?;
        dict.set_item("is_bigendian", self.is_bigendian)?;
        dict.set_item("step", self.step)?;
        dict.set_item("data", self.data.clone())?;
        Ok(dict.into())
    }

    #[classattr]
    fn __type_support__() -> Py<PyCapsule> {
        PyTypeSupport::create::<Self>()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, AerosimMessage)]
#[cfg_attr(feature = "python", pyclass)]
pub struct CompressedImage {
    pub format: ImageFormat,

    #[serde(
        serialize_with = "serialize_pixels",
        deserialize_with = "deserialize_pixels"
    )]
    pub data: Cow<'static, [u8]>,
}

impl CompressedImage {
    pub fn new(format: ImageFormat, data: Vec<u8>) -> Self {
        CompressedImage {
            format,
            data: Cow::Owned(data),
        }
    }

    pub fn decompress(&self) -> Result<Image, String> {
        let mut decompressor = turbojpeg::Decompressor::new()
            .map_err(|e| format!("Could not create turbojpeg decompresor: {}", e))?;
        let header = decompressor
            .read_header(self.data.as_ref())
            .map_err(|e| format!("Could not read header from jpeg image: {}", e))?;

        // FIXME: Currently hardcoded to BGRA as used in the renderer
        let pitch = header.width * 4;
        let mut image = turbojpeg::Image {
            pixels: vec![0; header.height * pitch],
            width: header.width,
            pitch,
            height: header.height,
            format: turbojpeg::PixelFormat::BGRA,
        };
        decompressor
            .decompress(self.data.as_ref(), image.as_deref_mut())
            .map_err(|e| format!("Could not decompress jpeg image: {}", e))?;

        // FIXME: Currently hardcoding some values as in the renderer.
        // TODO: Some parameters (e.g., CameraInfo) cannot be derived from the compressed image.
        // Consider using a single data type for consistency?
        let d: Vec<f64> = vec![0.0];
        let k: [f64; 9] = [0.0; 9];
        let r: [f64; 9] = [0.0; 9];
        let p: [f64; 12] = [0.0; 12];
        Ok(Image {
            camera_info: CameraInfo::new(
                header.width as u32,
                header.height as u32,
                "none".to_string(),
                d,
                k,
                r,
                p,
            ),
            height: header.height as u32,
            width: header.width as u32,
            encoding: ImageEncoding::BGRA8,
            is_bigendian: 0,
            step: pitch as u32,
            data: Cow::Owned(image.pixels),
        })
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl CompressedImage {
    #[new]
    fn py_new(format: ImageFormat, data: Vec<u8>) -> Self {
        Self::new(format, data)
    }

    #[getter]
    fn format(&self) -> ImageFormat {
        self.format.clone()
    }

    #[setter]
    fn set_format(&mut self, format: ImageFormat) {
        self.format = format;
    }

    #[getter]
    fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    #[pyo3(name = "decompress")]
    fn py_decompress(&self) -> PyResult<Image> {
        self.decompress().map_err(PyRuntimeError::new_err)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item(
            "format",
            match self.format {
                ImageFormat::JPEG => "JPEG",
                ImageFormat::PNG => "PNG",
            },
        )?;
        dict.set_item("data", self.data.clone())?;
        Ok(dict.into())
    }

    #[classattr]
    fn __type_support__() -> Py<PyCapsule> {
        PyTypeSupport::create::<Self>()
    }
}

fn serialize_pixels<S>(data: &Cow<'_, [u8]>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde_bytes::serialize(data.as_ref(), serializer)
}

fn deserialize_pixels<'de, D>(deserializer: D) -> Result<Cow<'static, [u8]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes = serde_bytes::ByteBuf::deserialize(deserializer)?;
    let bytes = bytes.into_vec();
    Ok(Cow::Owned(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct CameraInfo {
    pub width: u32,
    pub height: u32,
    pub distortion_model: String,
    pub d: Vec<f64>,
    pub k: [f64; 9],
    pub r: [f64; 9],
    pub p: [f64; 12],
}

impl CameraInfo {
    pub fn new(
        width: u32,
        height: u32,
        distortion_model: String,
        d: Vec<f64>,
        k: [f64; 9],
        r: [f64; 9],
        p: [f64; 12],
    ) -> Self {
        CameraInfo {
            width,
            height,
            distortion_model,
            d,
            k,
            r,
            p,
        }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl CameraInfo {
    #[new]
    fn py_new(
        width: u32,
        height: u32,
        distortion_model: String,
        d: Vec<f64>,
        k: [f64; 9],
        r: [f64; 9],
        p: [f64; 12],
    ) -> Self {
        Self::new(width, height, distortion_model, d, k, r, p)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("width", self.width)?;
        dict.set_item("height", self.height)?;
        dict.set_item("distortion_model", self.distortion_model.clone())?;
        dict.set_item("d", self.d.clone())?;
        dict.set_item("k", self.k)?;
        dict.set_item("r", self.r)?;
        dict.set_item("p", self.p)?;
        Ok(dict.into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, AerosimMessage, JsonSchema)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct ADSB {
    pub message: DownlinkFormat,
}

impl Default for ADSB {
    fn default() -> Self {
        ADSB {
            message: DownlinkFormat::GNSSPositionData(
                crate::types::adsb::gnss_position_data::GNSSPositionData::default(),
            ),
        }
    }
}

impl ADSB {
    pub fn new(message: DownlinkFormat) -> Self {
        ADSB { message }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl ADSB {
    #[new]
    #[pyo3(signature = (message=DownlinkFormat::GNSSPositionData(crate::types::adsb::gnss_position_data::GNSSPositionData::default())))]
    fn py_new(message: DownlinkFormat) -> Self {
        Self::new(message)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        let _ = dict.set_item("message", self.message.to_dict(py)?);
        Ok(dict.into())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, AerosimMessage, JsonSchema)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct GNSS {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub velocity: Vector3,
    pub heading: f64,
}

impl GNSS {
    pub fn new(
        latitude: f64,
        longitude: f64,
        altitude: f64,
        velocity: Vector3,
        heading: f64,
    ) -> Self {
        GNSS {
            latitude,
            longitude,
            altitude,
            velocity,
            heading,
        }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl GNSS {
    #[new]
    #[pyo3(signature = (latitude=0.0, longitude=0.0, altitude=0.0, velocity=Vector3::default(), heading=0.0))]
    fn py_new(
        latitude: f64,
        longitude: f64,
        altitude: f64,
        velocity: Vector3,
        heading: f64,
    ) -> Self {
        Self::new(latitude, longitude, altitude, velocity, heading)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("latitude", self.latitude)?;
        dict.set_item("longitude", self.longitude)?;
        dict.set_item("altitude", self.altitude)?;
        dict.set_item("velocity", self.velocity.to_dict(py)?)?;
        dict.set_item("heading", self.heading)?;
        Ok(dict.into())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, AerosimMessage, JsonSchema)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct IMU {
    pub acceleration: Vector3,
    pub gyroscope: Vector3,
    pub magnetic_field: Vector3,
}

impl IMU {
    pub fn new(acceleration: Vector3, gyroscope: Vector3, magnetic_field: Vector3) -> Self {
        IMU {
            acceleration,
            gyroscope,
            magnetic_field,
        }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl IMU {
    #[new]
    #[pyo3(signature = (acceleration=Vector3::default(), gyroscope=Vector3::default(), magnetic_field=Vector3::default()))]
    fn py_new(acceleration: Vector3, gyroscope: Vector3, magnetic_field: Vector3) -> Self {
        Self::new(acceleration, gyroscope, magnetic_field)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("acceleration", self.acceleration.to_dict(py)?)?;
        dict.set_item("gyroscope", self.gyroscope.to_dict(py)?)?;
        dict.set_item("magnetic_field", self.magnetic_field.to_dict(py)?)?;
        Ok(dict.into())
    }
}
