use std::ops;

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg_attr(feature = "python", pyclass(subclass))]
#[derive(Clone, Copy, Debug)]
pub struct Rotator {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
}

impl Default for Rotator {
    fn default() -> Self {
        Self {  roll: 0.0, yaw: 0.0, pitch: 0.0 }
    }
}

impl Rotator {
    pub fn new(roll: f64, pitch: f64, yaw: f64) -> Self {
        Self { roll, pitch, yaw }
    }

    pub const fn to_tuple(&self) -> (f64, f64, f64) {
        (self.roll, self.pitch, self.yaw)
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl Rotator {
    #[new]
    #[pyo3(signature = (roll = 0.0, pitch = 0.0, yaw = 0.0))]
    pub fn py_new(roll: f64, pitch: f64, yaw: f64) -> Self {
        Self::new(roll, pitch, yaw)
    }

    #[getter]
    fn roll(&self) -> f64 { self.roll }
    #[setter]
    fn set_roll(&mut self, val: f64) { self.roll = val; }

    #[getter]
    fn pitch(&self) -> f64 { self.pitch }
    #[setter]
    fn set_pitch(&mut self, val: f64) { self.pitch = val; }

    #[getter]
    fn yaw(&self) -> f64 { self.yaw }
    #[setter]
    fn set_yaw(&mut self, val: f64) { self.yaw = val; }

    pub const fn to_python_tuple(&self) -> (f64, f64, f64) {
        self.to_tuple()
    }
}

impl ops::Add<Rotator> for Rotator {
    type Output = Rotator;

    fn add(self, _rhs: Rotator) -> Rotator {
        Rotator {
            roll: self.roll + _rhs.roll,
            pitch: self.pitch + _rhs.pitch,
            yaw: self.yaw + _rhs.yaw,
        }
    }
}

impl ops::AddAssign<Rotator> for Rotator {
    fn add_assign(&mut self, _rhs: Rotator) {
        self.roll += _rhs.roll;
        self.pitch += _rhs.pitch;
        self.yaw += _rhs.yaw;
    }
}

impl ops::Sub<Rotator> for Rotator {
    type Output = Rotator;

    fn sub(self, _rhs: Rotator) -> Rotator {
        Rotator {
            roll: self.roll - _rhs.roll,
            pitch: self.pitch - _rhs.pitch,
            yaw: self.yaw - _rhs.yaw,
        }
    }
}

impl ops::SubAssign<Rotator> for Rotator {
    fn sub_assign(&mut self, _rhs: Rotator) {
        self.roll -= _rhs.roll;
        self.pitch -= _rhs.pitch;
        self.yaw -= _rhs.yaw;
    }
}

impl ops::Mul<f64> for Rotator {
    type Output = Rotator;

    fn mul(self, _rhs: f64) -> Rotator {
        Rotator {
            roll: self.roll * _rhs,
            pitch: self.pitch * _rhs,
            yaw: self.yaw * _rhs,
        }
    }
}

impl ops::Mul<Rotator> for f64 {
    type Output = Rotator;

    fn mul(self, _rhs: Rotator) -> Rotator {
        Rotator {
            roll: self * _rhs.roll,
            pitch: self * _rhs.pitch,
            yaw: self * _rhs.yaw,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let rotator1 = Rotator::new(1.0, 2.0, 3.0);
        let rotator2 = Rotator::new(4.0, 5.0, 6.0);
        let result = rotator1 + rotator2;
        assert_eq!(result.roll, 5.0);
        assert_eq!(result.pitch, 7.0);
        assert_eq!(result.yaw, 9.0);
    }

    #[test]
    fn test_add_assign() {
        let mut rotator1 = Rotator::new(1.0, 2.0, 3.0);
        let rotator2 = Rotator::new(4.0, 5.0, 6.0);
        rotator1 += rotator2;
        assert_eq!(rotator1.roll, 5.0);
        assert_eq!(rotator1.pitch, 7.0);
        assert_eq!(rotator1.yaw, 9.0);
    }

    #[test]
    fn test_sub() {
        let rotator1 = Rotator::new(1.0, 2.0, 3.0);
        let rotator2 = Rotator::new(4.0, 5.0, 6.0);
        let result = rotator1 - rotator2;
        assert_eq!(result.roll, -3.0);
        assert_eq!(result.pitch, -3.0);
        assert_eq!(result.yaw, -3.0);
    }

    #[test]
    fn test_sub_assign() {
        let mut rotator1 = Rotator::new(1.0, 2.0, 3.0);
        let rotator2 = Rotator::new(4.0, 5.0, 6.0);
        rotator1 -= rotator2;
        assert_eq!(rotator1.roll, -3.0);
        assert_eq!(rotator1.pitch, -3.0);
        assert_eq!(rotator1.yaw, -3.0);
    }

    #[test]
    fn test_mul() {
        let rotator = Rotator::new(1.0, 2.0, 3.0);
        let result = rotator * 2.0;
        assert_eq!(result.roll, 2.0);
        assert_eq!(result.pitch, 4.0);
        assert_eq!(result.yaw, 6.0);
    }

    #[test]
    fn test_mul_reverse() {
        let rotator = Rotator::new(1.0, 2.0, 3.0);
        let result = 2.0 * rotator;
        assert_eq!(result.roll, 2.0);
        assert_eq!(result.pitch, 4.0);
        assert_eq!(result.yaw, 6.0);
    }

    #[test]
    fn test_to_tuple() {
        let rotator = Rotator::new(1.0, 2.0, 3.0);
        let result = rotator.to_tuple();
        assert_eq!(result, (1.0, 2.0, 3.0));
    }

    #[test]
    fn test_default() {
        let rotator = Rotator::default();
        assert_eq!(rotator.roll, 0.0);
        assert_eq!(rotator.pitch, 0.0);
        assert_eq!(rotator.yaw, 0.0);
    }

}
