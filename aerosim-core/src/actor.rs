#[cfg(feature = "python")]
use pyo3::prelude::*;

use crate::math::rotator::Rotator;
use aerosim_data::types::geometry::Vector3;

#[cfg_attr(feature = "python", pyclass)]
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Actor {
    uid: u64, // unique id identifier for this actor
    actor_name: String, // actor name
    actor_type: u64, // identifier for actor type
    semantics: String, // Semantics of this actor
    latitude: f64, // latitude for this actor // We maybe do not want this and we have to remove it, we will need to discuss how are we gonna manage internally poses
    longitude: f64, // longitude for this actor // We maybe do not want this and we have to remove it, we will need to discuss how are we gonna manage internally poses
    altitude: f64, // altitude for this actor // We maybe do not want this and we have to remove it, we will need to discuss how are we gonna manage internally poses
    position: Vector3, // Position of this actor x y z meters from world origin
    rotation: Rotator, // Rotation of this actor roll pitch yaw
    velocity_linear: Vector3, // Velocity vector of this actor in m/s
    velocity_angular: Vector3, // Velocity vector of this actor in rad/s
    mass: f64,  // Mass of this actor in kg
    parent_uid: u64,  // ID of the parent actor
}

impl Actor {
    pub fn new(actor_name: String, actor_type: u64, semantics: String, latitude: f64, longitude: f64, altitude: f64, position: Vector3, rotation: Rotator, velocity: Vector3, velocity_angular:Vector3, mass: f64, parent_uid: u64) -> Self {
        let actor = Actor {
            uid: u64::MAX,
            actor_name: actor_name,
            actor_type: actor_type,
            semantics: semantics,
            latitude: latitude,
            longitude: longitude,
            altitude: altitude,
            position: position,
            rotation: rotation,
            velocity_linear: velocity,
            velocity_angular: velocity_angular,
            mass: mass,
            parent_uid: parent_uid,
        };
        return actor;
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Actor {
    #[new]
    #[pyo3(signature = (actor_name, actor_type, semantics, latitude=0.0, longitude=0.0, altitude=0.0, position=Vector3{x: 0.0, y: 0.0, z: 0.0}, rotation=Rotator{yaw: 0.0, pitch: 0.0, roll: 0.0}, velocity=Vector3{x: 0.0, y: 0.0, z: 0.0}, velocity_angular=Vector3{x: 0.0, y: 0.0, z: 0.0}, mass=0.0, parent_uid=u64::MAX))]
    pub fn py_new(actor_name: String, actor_type: u64, semantics: String, latitude: f64, longitude: f64, altitude: f64, position: Vector3, rotation: Rotator, velocity: Vector3, velocity_angular:Vector3, mass: f64, parent_uid: u64) -> Self {
        Self::new(actor_name, actor_type, semantics, latitude, longitude, altitude, position, rotation, velocity, velocity_angular, mass, parent_uid)
    }

    #[getter]
    fn uid(&self) -> u64 { self.uid }
    #[setter]
    fn set_uid(&mut self, val: u64) { self.uid = val; }

    #[getter]
    fn actor_name(&self) -> String { self.actor_name.clone() }
    #[setter]
    fn set_actor_name(&mut self, val: String) { self.actor_name = val; }

    #[getter]
    fn actor_type(&self) -> u64 { self.actor_type }
    #[setter]
    fn set_actor_type(&mut self, val: u64) { self.actor_type = val; }

    #[getter]
    fn semantics(&self) -> String { self.semantics.clone() }
    #[setter]
    fn set_semantics(&mut self, val: String) { self.semantics = val; }

    #[getter]
    fn latitude(&self) -> f64 { self.latitude }
    #[setter]
    fn set_latitude(&mut self, val: f64) { self.latitude = val; }

    #[getter]
    fn longitude(&self) -> f64 { self.longitude }
    #[setter]
    fn set_longitude(&mut self, val: f64) { self.longitude = val; }

    #[getter]
    fn altitude(&self) -> f64 { self.altitude }
    #[setter]
    fn set_altitude(&mut self, val: f64) { self.altitude = val; }

    #[getter]
    fn position(&self) -> Vector3 { self.position }
    #[setter]
    fn set_position(&mut self, val: Vector3) { self.position = val; }

    #[getter]
    fn rotation(&self) -> Rotator { self.rotation }
    #[setter]
    fn set_rotation(&mut self, val: Rotator) { self.rotation = val; }

    #[getter]
    fn velocity_linear(&self) -> Vector3 { self.velocity_linear }
    #[setter]
    fn set_velocity_linear(&mut self, val: Vector3) { self.velocity_linear = val; }

    #[getter]
    fn velocity_angular(&self) -> Vector3 { self.velocity_angular }
    #[setter]
    fn set_velocity_angular(&mut self, val: Vector3) { self.velocity_angular = val; }

    #[getter]
    fn mass(&self) -> f64 { self.mass }
    #[setter]
    fn set_mass(&mut self, val: f64) { self.mass = val; }

    #[getter]
    fn parent_uid(&self) -> u64 { self.parent_uid }
    #[setter]
    fn set_parent_uid(&mut self, val: u64) { self.parent_uid = val; }
}
