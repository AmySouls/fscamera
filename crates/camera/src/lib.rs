use glam::{Quat, Vec3};

pub mod freecam;
pub mod playback;

/// Describes the worlds coordinate system
#[derive(Clone)]
pub struct Space {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

/// Basic camera we can place in the world.
pub struct Camera {
    /// Describes how the camera should deal with its coordinate system.
    pub space: Space,
    /// Position of the camera.
    pub translation: Vec3,
    /// Final rotation after combining level and roll rotations.
    pub rotation: Quat,
    /// Field of vision in radians.
    pub fov: f32,
}

impl Camera {
    pub fn new(space: Space, translation: Vec3, rotation: Quat, fov: f32) -> Self {
        Self {
            space,
            translation,
            rotation,
            fov,
        }
    }
}
