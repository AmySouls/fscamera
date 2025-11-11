use glam::{Mat3, Quat, Vec3};

pub mod freecam;
pub mod playback;

/// Basic camera we can place in the world.
pub struct Camera {
    /// Describes how the camera should deal with its coordinate system.
    pub space: Mat3,
    /// Position of the camera.
    pub translation: Vec3,
    /// Final rotation after combining level and roll rotations.
    pub rotation: Quat,
    /// Field of vision in radians.
    pub fov: f32,
}

impl Camera {
    pub fn new(space: Mat3, translation: Vec3, rotation: Quat, fov: f32) -> Self {
        Self {
            space,
            translation,
            rotation,
            fov,
        }
    }
}
