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

pub struct FreeCam {
    pub translation: Vec3,
    /// Final rotation after combining level and roll rotations.
    pub rotation: Quat,
    /// Level rotation as to now have yaw and pitch deltas be contaminated by roll.
    pub level_rotation: Quat,
    /// Roll rotation apart from yaw and pitch
    pub roll_angle: f32,

    /// Base movement speed for 
    pub base_speed: f32,
    pub look_sensitivty: f32,
    pub roll_speed: f32,
}

impl Default for FreeCam {
    fn default() -> Self {
        Self::from(Vec3::ZERO, Quat::IDENTITY)
    }
}

impl FreeCam {
    pub fn from(translation: Vec3, orientation: Quat) -> Self {
        Self {
            translation,
            rotation: orientation,
            level_rotation: orientation,
            roll_angle: 0.0,

            base_speed: 2.0,
            look_sensitivty: 0.004,
            roll_speed: 2.0,
        }
    }

    pub fn update(&mut self, input: &FreeCamInput, delta: f32) {
        if input.mouse_delta != Vec2::ZERO {
            let dx = -input.mouse_delta.x * self.look_sensitivty;
            let dy = -input.mouse_delta.y * self.look_sensitivty;

            let q_yaw = Quat::from_axis_angle(Vec3::Y, dx);
            self.level_rotation = (q_yaw * self.level_rotation).normalize();

            let right = (Mat3::from_quat(self.level_rotation) * Vec3::X).normalize();
            let q_pitch = Quat::from_axis_angle(right, dy);
            self.level_rotation = (q_pitch * self.level_rotation).normalize();
        }

        if input.roll_delta != 0.0 {
            self.roll_angle += input.roll_delta * self.roll_speed * delta;
        }

        let forward = (Mat3::from_quat(self.level_rotation) * Vec3::Z).normalize();
        let roll_rotation = Quat::from_axis_angle(forward, self.roll_angle);
        self.rotation = (roll_rotation * self.level_rotation).normalize();

        let rot3 = Mat3::from_quat(self.rotation);
        let forward = rot3 * Vec3::Z;
        let right = rot3 * Vec3::X;
        let up = rot3 * Vec3::Y;

        let mut velocity = Vec3::ZERO;
        velocity += forward * input.forward;
        velocity += right * input.right;
        velocity += up * input.up;

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize();
        }

        let speed = self.base_speed * input.speed_multiplier;
        self.translation += velocity * speed * delta;
    }
}
