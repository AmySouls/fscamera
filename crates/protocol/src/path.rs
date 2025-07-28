use nalgebra_glm as glm;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraFrame {
    pub time: f32,
    pub position: Vec3,
    pub rotation: Quat,
    pub fov: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraSample {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov: f32,
}

pub struct CameraTrack {
    keyframes: Vec<CameraFrame>,
}

impl CameraTrack {
    pub fn new(mut keyframes: Vec<CameraFrame>) -> Self {
        keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        Self { keyframes }
    }

    pub fn sample(&self, t: f32) -> CameraSample {
        let len = self.keyframes.len();
        if len == 0 {
            return CameraSample::default();
        }
        if len == 1 || t <= self.keyframes[0].time {
            return frame_to_sample(&self.keyframes[0]);
        }
        if t >= self.keyframes[len - 1].time {
            return frame_to_sample(&self.keyframes[len - 1]);
        }

        let (i1, i2) = find_segment(&self.keyframes, t);
        let (f0, f1, f2, f3) = get_catmull_neighbors(&self.keyframes, i1);

        let t0 = f1.time;
        let t1 = f2.time;
        let local_t = (t - t0) / (t1 - t0);

        // Position: Catmull-Rom spline
        let pos = catmull_rom(
            to_glm_vec3(f0.position),
            to_glm_vec3(f1.position),
            to_glm_vec3(f2.position),
            to_glm_vec3(f3.position),
            local_t,
        );

        // Rotation: SLERP
        let q1 = to_glm_quat(f1.rotation);
        let q2 = to_glm_quat(f2.rotation);
        let rot = glm::quat_slerp(&q1, &q2, local_t);

        // FOV: Linear
        let fov = f1.fov + (f2.fov - f1.fov) * local_t;

        CameraSample {
            position: from_glm_vec3(pos),
            rotation: from_glm_quat(rot),
            fov,
        }
    }
}

fn to_glm_vec3(v: Vec3) -> glm::Vec3 {
    glm::vec3(v.x, v.y, v.z)
}

fn from_glm_vec3(v: glm::Vec3) -> Vec3 {
    Vec3 { x: v.x, y: v.y, z: v.z }
}

fn to_glm_quat(q: Quat) -> glm::Quat {
    glm::quat(q.w, q.x, q.y, q.z)
}

fn from_glm_quat(q: glm::Quat) -> Quat {
    Quat { x: q.i, y: q.j, z: q.k, w: q.w }
}

fn frame_to_sample(f: &CameraFrame) -> CameraSample {
    CameraSample {
        position: f.position,
        rotation: f.rotation,
        fov: f.fov,
    }
}

fn find_segment(frames: &[CameraFrame], t: f32) -> (usize, usize) {
    for i in 0..frames.len() - 1 {
        if t >= frames[i].time && t <= frames[i + 1].time {
            return (i, i + 1);
        }
    }
    (frames.len() - 2, frames.len() - 1)
}

fn get_catmull_neighbors(frames: &[CameraFrame], i1: usize) -> (&CameraFrame, &CameraFrame, &CameraFrame, &CameraFrame) {
    let i0 = if i1 == 0 { 0 } else { i1 - 1 };
    let i2 = i1 + 1;
    let i3 = if i2 + 1 >= frames.len() { frames.len() - 1 } else { i2 + 1 };

    (&frames[i0], &frames[i1], &frames[i2], &frames[i3])
}

fn catmull_rom(p0: glm::Vec3, p1: glm::Vec3, p2: glm::Vec3, p3: glm::Vec3, t: f32) -> glm::Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;

    let a = 2.0 * p1;
    let b = p2 - p0;
    let c = 2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3;
    let d = -p0 + 3.0 * p1 - 3.0 * p2 + p3;

    (a + b * t + c * t2 + d * t3) * 0.5
}

impl Default for CameraSample {
    fn default() -> Self {
        Self {
            position: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
            rotation: Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
            fov: 60.0,
        }
    }
}
