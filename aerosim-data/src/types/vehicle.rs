use crate::AerosimMessage;

use super::actor::ActorState;
use super::geometry::Vector3;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[cfg(feature = "python")]
use {
    crate::types::PyTypeSupport,
    pyo3::{
        prelude::*,
        types::{PyCapsule, PyDict},
    },
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, EnumString, Display, PartialEq)]
#[cfg_attr(feature = "python", pyclass(eq, eq_int))]
pub enum VehicleType {
    Ground,
    Aerial,
    Marine,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, AerosimMessage, JsonSchema)]
#[cfg_attr(feature = "python", pyclass(get_all))]
pub struct VehicleState {
    pub state: ActorState,
    pub velocity: Vector3,
    pub angular_velocity: Vector3,
    pub acceleration: Vector3,
    pub angular_acceleration: Vector3,
}

impl VehicleState {
    pub fn new(
        state: ActorState,
        velocity: Vector3,
        angular_velocity: Vector3,
        acceleration: Vector3,
        angular_acceleration: Vector3,
    ) -> Self {
        VehicleState {
            state,
            velocity,
            angular_velocity,
            acceleration,
            angular_acceleration,
        }
    }
}

// Python interface layer

#[cfg(feature = "python")]
#[pymethods]
impl VehicleType {
    #[staticmethod]
    #[pyo3(name = "from_str")]
    pub fn py_from_str(s: &str) -> PyResult<Self> {
        s.parse()
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid VehicleType"))
    }

    pub fn __str__(&self) -> String {
        self.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("VehicleType::{}", self)
    }

    #[staticmethod]
    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(py: Python) -> PyResult<PyObject> {
        let dict = pyo3::types::PyDict::new(py);
        for variant in [
            VehicleType::Ground,
            VehicleType::Aerial,
            VehicleType::Marine,
        ] {
            dict.set_item(variant.to_string(), variant.__repr__())?;
        }
        Ok(dict.into())
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl VehicleState {
    #[new]
    #[pyo3(signature = (state=ActorState::default(), velocity=Vector3::default(), angular_velocity=Vector3::default(), acceleration=Vector3::default(), angular_acceleration=Vector3::default()))]
    fn py_new(
        state: ActorState,
        velocity: Vector3,
        angular_velocity: Vector3,
        acceleration: Vector3,
        angular_acceleration: Vector3,
    ) -> Self {
        Self::new(state, velocity, angular_velocity, acceleration, angular_acceleration)
    }

    #[pyo3(name = "to_dict")]
    pub fn py_to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = PyDict::new(py);
        dict.set_item("state", self.state.py_to_dict(py)?)?;
        dict.set_item("velocity", self.velocity.py_to_dict(py)?)?;
        dict.set_item("angular_velocity", self.angular_velocity.py_to_dict(py)?)?;
        dict.set_item("acceleration", self.acceleration.py_to_dict(py)?)?;
        dict.set_item(
            "angular_acceleration",
            self.angular_acceleration.py_to_dict(py)?,
        )?;
        Ok(dict.into())
    }

    #[classattr]
    fn __type_support__() -> Py<PyCapsule> {
        PyTypeSupport::create::<Self>()
    }
}
