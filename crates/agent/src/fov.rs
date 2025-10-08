use crate::game::CSCamera;

pub struct FovOverride {
    // What FoV to apply
    pub applied_fov: f32,
    // Are we supposed to be applying the FoV at all?
    pub active: bool,
}

impl Default for FovOverride {
    fn default() -> Self {
        Self {
            applied_fov: 48.0f32.to_radians(),
            active: false,
        }
    }
}

impl FovOverride {
    pub fn apply(&self, camera: &mut CSCamera) {
        if self.active {
            camera.pers_cam_1.fov = self.applied_fov;
        }
    }
}
