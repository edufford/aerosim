use super::geometry::{Pose, Vector3};
use super::sensor::SensorType;
use super::vehicle::VehicleType;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyDict};

#[derive(Debug, Clone, Serialize, Deserialize)]
// PyO3 only supports unit variants for enums. Variant with data is not supported
pub enum ActorType {
    Vehicle(VehicleType),
    Sensor(SensorType),
}

// PyO3 only supports unit variants for enums so we need to use a struct instead
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyclass)]
pub struct Actor {
    pub uid: u64,
    pub actor_type: ActorType,
    pub state: ActorState,
    pub model: ActorModel,
    pub parent_actor_uid: Option<u64>,
}

#[cfg(feature = "python")]
#[pymethods]
impl Actor {
    #[getter]
    fn uid(&self) -> u64 {
        self.uid
    }

    #[setter]
    fn set_uid(&mut self, uid: u64) {
        self.uid = uid;
    }

    #[getter]
    fn state(&self) -> ActorState {
        self.state.clone()
    }

    #[setter]
    fn set_state(&mut self, state: ActorState) {
        self.state = state;
    }

    #[getter]
    fn model(&self) -> ActorModel {
        self.model.clone()
    }

    #[setter]
    fn set_model(&mut self, model: ActorModel) {
        self.model = model;
    }

    #[getter]
    fn parent_actor_uid(&self) -> Option<u64> {
        self.parent_actor_uid
    }

    #[setter]
    fn set_parent_actor_uid(&mut self, parent_actor_uid: Option<u64>) {
        self.parent_actor_uid = parent_actor_uid;
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyclass(get_all))]
pub struct ActorState {
    pub pose: Pose,
}

impl ActorState {
    pub fn new(pose: Pose) -> Self {
        ActorState { pose }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl ActorState {
    #[new]
    fn py_new(pose: Pose) -> Self {
        Self::new(pose)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("pose", self.pose.to_dict(py)?)?;
        Ok(dict.into())
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.pose == other.pose
    }

    pub fn __ne__(&self, other: &Self) -> bool {
        self.pose != other.pose
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct ActorModel {
    pub physical_properties: PhysicalProperties,
    pub asset_link: Option<String>,
}

impl ActorModel {
    pub fn new(physical_properties: PhysicalProperties, asset_link: Option<String>) -> Self {
        ActorModel {
            physical_properties,
            asset_link,
        }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl ActorModel {
    #[new]
    #[pyo3(signature = (physical_properties, asset_link=None))]
    fn py_new(physical_properties: PhysicalProperties, asset_link: Option<String>) -> Self {
        Self::new(physical_properties, asset_link)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("physical_properties", self.physical_properties.to_dict(py)?)?;
        dict.set_item("asset_link", self.asset_link.clone())?;
        Ok(dict.into())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct PhysicalProperties {
    pub mass: f64,
    pub inertia_tensor: Vector3,
    pub moment_of_inertia: Vector3,
}

impl PhysicalProperties {
    pub fn new(mass: f64, inertia_tensor: Vector3, moment_of_inertia: Vector3) -> Self {
        PhysicalProperties {
            mass,
            inertia_tensor,
            moment_of_inertia,
        }
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl PhysicalProperties {
    #[new]
    fn py_new(mass: f64, inertia_tensor: Vector3, moment_of_inertia: Vector3) -> Self {
        Self::new(mass, inertia_tensor, moment_of_inertia)
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("mass", self.mass)?;
        dict.set_item("inertia_tensor", self.inertia_tensor.to_dict(py)?)?;
        dict.set_item("moment_of_inertia", self.moment_of_inertia.to_dict(py)?)?;
        Ok(dict.into())
    }
}
