use crate::{Camera, Space};
use glam::{Mat3, Quat, Vec2, Vec3};

pub struct FreeCamInput {
    pub forward: f32,
    pub right: f32,
    pub up: f32,
    pub speed_multiplier: f32,
    pub mouse_delta: Vec2,
    pub roll_delta: f32,
}

impl Default for FreeCamInput {
    fn default() -> Self {
        Self {
            forward: 0.0,
            right: 0.0,
            up: 0.0,
            speed_multiplier: 1.0,
            mouse_delta: Vec2::ZERO,
            roll_delta: 0.0,
        }
    }
}

/// Freecam meant for looking around and setting up paths.
pub struct FreeCam {
    pub camera: Camera,
    pub target_translation: Vec3,
    /// Level rotation as to now have yaw and pitch deltas be contaminated by roll.
    pub level_rotation: Quat,
    /// Target level rotation to interpolate to.
    pub target_level_rotation: Quat,
    /// Roll rotation apart from yaw and pitch.
    pub roll_angle: f32,
    /// Target roll rotation to interpolate to.
    pub target_roll_angle: f32,

    pub movement_speed: f32,
    pub rotation_speed: f32,
    pub roll_speed: f32,
    pub movement_speed_multiplier: f32,
    pub rotation_speed_multiplier: f32,
    pub roll_speed_multiplier: f32,

    translation_smooth_time: f32,
    orientation_smooth_time: f32,
    roll_smooth_time: f32,
}

impl FreeCam {
    pub fn from(space: Space, translation: Vec3, orientation: Quat) -> Self {
        Self {
            camera: Camera::new(space, translation, orientation),
            target_translation: translation,

            level_rotation: orientation,
            target_level_rotation: orientation,
            roll_angle: 0.0,
            target_roll_angle: 0.0,

            movement_speed: 6.0,
            rotation_speed: 0.002,
            roll_speed: 2.0,

            movement_speed_multiplier: 1.0,
            rotation_speed_multiplier: 1.0,
            roll_speed_multiplier: 1.0,

            translation_smooth_time: 0.10,
            orientation_smooth_time: 0.12,
            roll_smooth_time: 0.10,
        }
    }

    fn relevel(mut q: Quat, space: &Space) -> Quat {
        let f = (Mat3::from_quat(q) * space.forward).normalize();
        let mut r = space.up.cross(f);
        if r.length_squared() < 1e-6 {
            r = (space.right - f * f.dot(space.right)).normalize();
            if r.length_squared() < 1e-6 {
                let alt = if f.abs().x < 0.99 { Vec3::X } else { Vec3::Y };
                r = (alt - f * f.dot(alt)).normalize();
            }
        } else {
            r = r.normalize();
        }

        let u = f.cross(r).normalize();
        Quat::from_mat3(&Mat3::from_cols(r, u, f)).normalize()
    }

    #[inline]
    fn exp_alpha(delta: f32, tau: f32) -> f32 {
        let tau = tau.max(1e-4);
        1.0 - (-delta / tau).exp()
    }

    pub fn update(&mut self, input: &FreeCamInput, delta: f32) {
        if input.mouse_delta != Vec2::ZERO {
            let dx = input.mouse_delta.x * self.rotation_speed * self.rotation_speed_multiplier;
            let dy = input.mouse_delta.y * self.rotation_speed * self.rotation_speed_multiplier;

            let q_yaw = Quat::from_axis_angle(self.camera.space.up, dx);
            self.target_level_rotation = (q_yaw * self.target_level_rotation).normalize();

            let right =
                (Mat3::from_quat(self.target_level_rotation) * self.camera.space.right).normalize();
            let q_pitch = Quat::from_axis_angle(right, dy);
            self.target_level_rotation = (q_pitch * self.target_level_rotation).normalize();
        }

        if input.roll_delta != 0.0 {
            self.target_roll_angle += input.roll_delta * self.roll_speed * delta;
        }

        // Blend orientation towards targets
        let a_orientation = Self::exp_alpha(delta, self.orientation_smooth_time);
        self.level_rotation = self.level_rotation
                .slerp(self.target_level_rotation, a_orientation)
                .normalize();

        // self.level_rotation = Self::relevel(
        //     self.level_rotation
        //         .slerp(self.target_level_rotation, a_orientation)
        //         .normalize(),
        //     &self.camera.space,
        // );

        let a_roll = Self::exp_alpha(delta, self.roll_smooth_time);
        self.roll_angle = self.roll_angle + (self.target_roll_angle - self.roll_angle) * a_roll;

        let forward =
            (Mat3::from_quat(self.level_rotation) * self.camera.space.forward).normalize();
        let roll_rotation = Quat::from_axis_angle(forward, self.roll_angle);
        self.camera.rotation = (roll_rotation * self.level_rotation).normalize();

        // Figure up directions for camera positional movement
        let rot3 = Mat3::from_quat(self.camera.rotation);
        let forward = rot3 * self.camera.space.forward;
        let right = rot3 * self.camera.space.right;
        let up = rot3 * self.camera.space.up;

        let mut velocity = Vec3::ZERO;
        velocity += forward * input.forward;
        velocity += right * input.right;
        velocity += up * input.up;

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize();
        }

        let speed = self.movement_speed * self.movement_speed_multiplier * input.speed_multiplier;
        self.target_translation += velocity * speed * delta;

        let a_translation = Self::exp_alpha(delta, self.translation_smooth_time);
        self.camera.translation +=
            (self.target_translation - self.camera.translation) * a_translation;
    }
}
