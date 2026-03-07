use crate::{coordinate_system::conversion_utils::*, Ellipsoid};
use serde::{Deserialize, Serialize};

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg_attr(feature = "python", pyclass)]
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct WorldCoordinate {
    // TODO Temporarily set variables as pub access for scene graph development. Change
    // access to private after changing structure from tuples to named fields for better
    // serialization.
    pub ned: (f64, f64, f64),
    pub lla: (f64, f64, f64),
    pub ecef: (f64, f64, f64),
    pub cartesian: (f64, f64, f64),
    pub origin_lla: (f64, f64, f64),
    pub ellipsoid: Ellipsoid,
}

// TODO - Change cartesian axes to x=N y=E z=D at origin
// TODO - Add methdos to convert cartesian-ECEF
// TODO - Check accuracy of conversions and fix if necessary (especially NED-LLA)
impl WorldCoordinate {
    pub fn new(origin_lat: f64, origin_lon: f64, origin_alt: f64, ellipsoid: Ellipsoid) -> Self {
        let ned = (0.0, 0.0, 0.0);
        WorldCoordinate {
            ned: ned,
            lla: ned_to_lla(
                ned.0, ned.1, ned.2, origin_lat, origin_lon, origin_alt, ellipsoid,
            ),
            ecef: ned_to_ecef(
                ned.0, ned.1, ned.2, origin_lat, origin_lon, origin_alt, ellipsoid,
            ),
            cartesian: ned_to_cartesian(
                ned.0, ned.1, ned.2, origin_lat, origin_lon, origin_alt, ellipsoid,
            ),
            origin_lla: (origin_lat, origin_lon, origin_alt),
            ellipsoid: ellipsoid,
        }
    }

    pub fn from_ned(
        north: f64,
        east: f64,
        down: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        let mut sim_coordinate =
            WorldCoordinate::new(origin_lat, origin_lon, origin_alt, ellipsoid);
        sim_coordinate.set_ned(north, east, down);
        sim_coordinate
    }

    pub fn from_lla(
        lat: f64,
        lon: f64,
        alt: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        let mut sim_coordinate =
            WorldCoordinate::new(origin_lat, origin_lon, origin_alt, ellipsoid);
        sim_coordinate.set_lla(lat, lon, alt);
        sim_coordinate
    }

    pub fn from_ecef(
        x: f64,
        y: f64,
        z: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        let mut sim_coordinate =
            WorldCoordinate::new(origin_lat, origin_lon, origin_alt, ellipsoid);
        sim_coordinate.set_ecef(x, y, z);
        sim_coordinate
    }

    pub fn from_cartesian(
        x: f64,
        y: f64,
        z: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        let mut sim_coordinate =
            WorldCoordinate::new(origin_lat, origin_lon, origin_alt, ellipsoid);
        sim_coordinate.set_cartesian(x, y, z);
        sim_coordinate
    }

    pub fn set_ned(&mut self, north: f64, east: f64, down: f64) {
        self.ned = (north, east, down);

        self.lla = ned_to_lla(
            north,
            east,
            down,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );

        self.ecef = ned_to_ecef(
            north,
            east,
            down,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );

        self.cartesian = ned_to_cartesian(
            north,
            east,
            down,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );
    }

    pub fn ned(&self) -> (f64, f64, f64) {
        self.ned
    }

    pub fn set_lla(&mut self, lat: f64, lon: f64, alt: f64) {
        self.lla = (lat, lon, alt);

        self.ned = lla_to_ned(
            lat,
            lon,
            alt,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );

        self.ecef = lla_to_ecef(lat, lon, alt, self.ellipsoid);

        self.cartesian = lla_to_cartesian(
            lat,
            lon,
            alt,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );
    }

    pub fn lla(&self) -> (f64, f64, f64) {
        self.lla
    }

    pub fn set_ecef(&mut self, x: f64, y: f64, z: f64) {
        self.ecef = (x, y, z);

        self.ned = ecef_to_ned(
            x,
            y,
            z,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );

        self.lla = ecef_to_lla(x, y, z, self.ellipsoid);

        // self.cartesian = ecef_to_cartesian(
        //     x,
        //     y,
        //     z,
        //     self.origin_lla.0,
        //     self.origin_lla.1,
        //     self.origin_lla.2,
        //     self.ellipsoid,
        // );
    }

    pub fn ecef(&self) -> (f64, f64, f64) {
        self.ecef
    }

    pub fn set_cartesian(&mut self, x: f64, y: f64, z: f64) {
        self.cartesian = (x, y, z);

        self.ned = cartesian_to_ned(
            x,
            y,
            z,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );

        self.lla = cartesian_to_lla(
            x,
            y,
            z,
            self.origin_lla.0,
            self.origin_lla.1,
            self.origin_lla.2,
            self.ellipsoid,
        );

        // self.ecef = cartesian_to_ecef(
        //     x,
        //     y,
        //     z,
        //     self.origin_lla.0,
        //     self.origin_lla.1,
        //     self.origin_lla.2,
        //     self.ellipsoid,
        // );
    }

    pub fn cartesian(&self) -> (f64, f64, f64) {
        self.cartesian
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl WorldCoordinate {
    #[new]
    #[pyo3(signature = (origin_lat=0.0, origin_lon=0.0, origin_alt=0.0, ellipsoid=Ellipsoid::wgs84()))]
    pub fn py_new(origin_lat: f64, origin_lon: f64, origin_alt: f64, ellipsoid: Ellipsoid) -> Self {
        Self::new(origin_lat, origin_lon, origin_alt, ellipsoid)
    }

    #[staticmethod]
    #[pyo3(name = "from_ned")]
    pub fn py_from_ned(
        north: f64,
        east: f64,
        down: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        Self::from_ned(north, east, down, origin_lat, origin_lon, origin_alt, ellipsoid)
    }

    #[staticmethod]
    #[pyo3(name = "from_lla")]
    pub fn py_from_lla(
        lat: f64,
        lon: f64,
        alt: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        Self::from_lla(lat, lon, alt, origin_lat, origin_lon, origin_alt, ellipsoid)
    }

    #[staticmethod]
    #[pyo3(name = "from_ecef")]
    pub fn py_from_ecef(
        x: f64,
        y: f64,
        z: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        Self::from_ecef(x, y, z, origin_lat, origin_lon, origin_alt, ellipsoid)
    }

    #[staticmethod]
    #[pyo3(name = "from_cartesian")]
    pub fn py_from_cartesian(
        x: f64,
        y: f64,
        z: f64,
        origin_lat: f64,
        origin_lon: f64,
        origin_alt: f64,
        ellipsoid: Ellipsoid,
    ) -> Self {
        Self::from_cartesian(x, y, z, origin_lat, origin_lon, origin_alt, ellipsoid)
    }

    #[pyo3(name = "set_ned")]
    pub fn py_set_ned(&mut self, north: f64, east: f64, down: f64) {
        self.set_ned(north, east, down)
    }

    #[pyo3(name = "ned")]
    pub fn py_ned(&self) -> (f64, f64, f64) {
        self.ned()
    }

    #[pyo3(name = "set_lla")]
    pub fn py_set_lla(&mut self, lat: f64, lon: f64, alt: f64) {
        self.set_lla(lat, lon, alt)
    }

    #[pyo3(name = "lla")]
    pub fn py_lla(&self) -> (f64, f64, f64) {
        self.lla()
    }

    #[pyo3(name = "set_ecef")]
    pub fn py_set_ecef(&mut self, x: f64, y: f64, z: f64) {
        self.set_ecef(x, y, z)
    }

    #[pyo3(name = "ecef")]
    pub fn py_ecef(&self) -> (f64, f64, f64) {
        self.ecef()
    }

    #[pyo3(name = "set_cartesian")]
    pub fn py_set_cartesian(&mut self, x: f64, y: f64, z: f64) {
        self.set_cartesian(x, y, z)
    }

    #[pyo3(name = "cartesian")]
    pub fn py_cartesian(&self) -> (f64, f64, f64) {
        self.cartesian()
    }
}
