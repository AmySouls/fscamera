use nalgebra_glm as glm;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn to_glm(self) -> glm::Vec3 {
        glm::vec3(self.x, self.y, self.z)
    }

    pub fn from_glm(v: glm::Vec3) -> Self {
        Self { x: v.x, y: v.y, z: v.z }
    }
}

impl From<Vec3> for glm::Vec3 {
    fn from(v: Vec3) -> Self {
        glm::vec3(v.x, v.y, v.z)
    }
}

impl From<glm::Vec3> for Vec3 {
    fn from(v: glm::Vec3) -> Self {
        Vec3 { x: v.x, y: v.y, z: v.z }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quat(pub f32, pub f32, pub f32, pub f32);

impl Quat {
    pub const IDENTITY: Self = Self(0.0, 0.0, 0.0, 1.0);

    pub fn to_glm(self) -> glm::Quat {
        glm::quat(self.0, self.1, self.2, self.3)
    }

    pub fn from_glm(q: glm::Quat) -> Self {
        let qv = q.as_vector();
        Self(qv.x, qv.y, qv.z, qv.w)
    }
}

impl From<Quat> for glm::Quat {
    fn from(q: Quat) -> Self {
        glm::make_quat(&[q.0, q.1, q.2, q.3])
    }
}

impl From<glm::Quat> for Quat {
    fn from(q: glm::Quat) -> Self {
        let qv = q.as_vector();
        Quat(qv.x, qv.y, qv.z, qv.w)
    }
}

/// Orientation (pitch, yaw, roll)
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Orientation(pub f32, pub f32, pub f32);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f32,
    pub map_id: i32,
    pub position: Vec3,
    pub orientation: Orientation,
    pub fov: f32,
    pub tension: f32,
}
