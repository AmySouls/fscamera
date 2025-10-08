use crate::{Camera, Space};
use protocol::keyframe::Keyframe;
use glam::{Vec3, Quat};

pub struct PlaybackCam {
    pub camera: Camera,
    /// Is the path currently playing?
    pub playing: bool,
    /// Current playback time.
    pub time: f32,
    /// Current camera path.
    keyframes: Vec<Keyframe>,
}

impl PlaybackCam {
    pub fn new(space: Space, translation: Vec3, orientation: Quat) -> Self {
        Self {
            camera: Camera::new(space, translation, orientation),
            playing: false,
            time: 0.0,
            keyframes: vec![],
        }
    }

    pub fn set_keyframes(&mut self, mut keyframes: Vec<Keyframe>) {
        keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
        fix_quaternion_hemispheres(keyframes.as_mut_slice());
        self.keyframes = keyframes;
    }

    pub fn update(&mut self, delta: f32) {
        if self.keyframes.is_empty() {
            return;
        }

        if self.playing {
            self.time += delta;
        }

        // TODO: interpolation code updating the camera
        let frame = interpolate_frame(&self.keyframes, self.time);
        self.camera.translation = frame.translation;
        self.camera.rotation = frame.rotation;
    }
}

#[derive(Default)]
pub struct PlaybackFrame {
    pub translation: Vec3,
    pub rotation: Quat,
    pub fov: f32,
}

pub fn interpolate_frame(frames: &[Keyframe], t: f32) -> PlaybackFrame {
    // If there is only a single frame keep it in place on that one frame.
    if frames.len() == 1 && let Some(f) = frames.first() {
        let protocol::keyframe::Vec3 { x, y, z } = f.position;
        let protocol::keyframe::Quat(qx, qy, qz, qw) = f.orientation;

        return PlaybackFrame {
            translation: Vec3::new(x, y, z),
            rotation: Quat::from_xyzw(qx, qy, qz, qw),
            fov: f.fov,
        }
    }

    // Find the segment containing t
    let (i1, i2) = find_segment(frames, t);
    // Retrieve neighbor indices and duplicate ends when missing.
    let i0 = i1.saturating_sub(1);
    let i3 = (i2 + 1).min(frames.len() - 1);

    let k0 = &frames[i0];
    let k1 = &frames[i1];
    let k2 = &frames[i2];
    let k3 = &frames[i3];

    let k0t = Vec3::new(k0.position.x, k0.position.y, k0.position.z);
    let k1t = Vec3::new(k1.position.x, k1.position.y, k1.position.z);
    let k2t = Vec3::new(k2.position.x, k2.position.y, k2.position.z);
    let k3t = Vec3::new(k3.position.x, k3.position.y, k3.position.z);

    let k0r = Quat::from_xyzw(k0.orientation.0, k0.orientation.1, k0.orientation.2, k0.orientation.3);
    let k1r = Quat::from_xyzw(k1.orientation.0, k1.orientation.1, k1.orientation.2, k1.orientation.3);
    let k2r = Quat::from_xyzw(k2.orientation.0, k2.orientation.1, k2.orientation.2, k2.orientation.3);
    let k3r = Quat::from_xyzw(k3.orientation.0, k3.orientation.1, k3.orientation.2, k3.orientation.3);

    let dt0 = (k1.time - k0.time).max(1e-6);
    let dt1 = (k2.time - k1.time).max(1e-6);
    let dt2 = (k3.time - k2.time).max(1e-6);

    // Determine the blending factor between k1 and k2 from t.
    let mut u = if k2.time > k1.time {
        ((t - k1.time) / (k2.time - k1.time)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    u = u*u*(3.0 - 2.0 * u);

    let translation = catmull_rom_centripetal_vec3(k0t, k1t, k2t, k3t, u); 
    let rotation = squad_with_neighbors(k0r, k1r, k2r, k3r, u, dt0, dt1, dt2); 
    let fov = catmull_rom_centripetal_scalar(k0.fov, k1.fov, k2.fov, k3.fov, u); 

    PlaybackFrame {
        translation,
        rotation,
        fov,
    }
}

fn find_segment(frames: &[Keyframe], t: f32) -> (usize, usize) {
    // If t is before the first keyframe, use the first two.
    if t <= frames[0].time { return (0, 1); }
    // If t is part the last keyframe, use the last two.
    if t >= frames[frames.len() - 1].time { return (frames.len() - 2, frames.len() -1); }

    // Binary search our way to the right frames.
    let mut lo = 0;
    let mut hi = frames.len() - 1;
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if t < frames[mid].time { hi = mid; } else { lo = mid; }
    }

    (lo, hi)
}

fn catmull_rom_centripetal_vec3(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, u01: f32) -> Vec3 {
    let (t0, t1, t2, t3) = chord_params_vec3(p0, p1, p2, p3);
    // Map u in [0,1] to the inner interval [t1, t2]
    let t = lerp_f32(t1, t2, u01);
    let a1 = lerp_vec3(p0, p1, (t - t0) / (t1 - t0).max(1e-6));
    let a2 = lerp_vec3(p1, p2, (t - t1) / (t2 - t1).max(1e-6));
    let a3 = lerp_vec3(p2, p3, (t - t2) / (t3 - t2).max(1e-6));

    let b1 = lerp_vec3(a1, a2, (t - t0) / (t2 - t0).max(1e-6));
    let b2 = lerp_vec3(a2, a3, (t - t1) / (t3 - t1).max(1e-6));

    lerp_vec3(b1, b2, (t - t1) / (t2 - t1).max(1e-6))
}

fn catmull_rom_centripetal_scalar(p0: f32, p1: f32, p2: f32, p3: f32, u01: f32) -> f32 {
    let (t0, t1, t2, t3) = chord_params_scalar(p0, p1, p2, p3);
    let t = lerp_f32(t1, t2, u01);

    let a1 = lerp_f32(p0, p1, (t - t0) / (t1 - t0).max(1e-6));
    let a2 = lerp_f32(p1, p2, (t - t1) / (t2 - t1).max(1e-6));
    let a3 = lerp_f32(p2, p3, (t - t2) / (t3 - t2).max(1e-6));

    let b1 = lerp_f32(a1, a2, (t - t0) / (t2 - t0).max(1e-6));
    let b2 = lerp_f32(a2, a3, (t - t1) / (t3 - t1).max(1e-6));

    lerp_f32(b1, b2, (t - t1) / (t2 - t1).max(1e-6))
}

fn chord_params_vec3(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3) -> (f32, f32, f32, f32) {
    let alpha = 0.5;
    let t0 = 0.0;
    let t1 = t0 + (p1 - p0).length().sqrt().powf(alpha);
    let t2 = t1 + (p2 - p1).length().sqrt().powf(alpha);
    let t3 = t2 + (p3 - p2).length().sqrt().powf(alpha);
    (t0, t1.max(t0 + 1e-6), t2.max(t1 + 1e-6), t3.max(t2 + 1e-6))
}

fn chord_params_scalar(p0: f32, p1: f32, p2: f32, p3: f32) -> (f32, f32, f32, f32) {
    let alpha = 0.5;
    let t0 = 0.0;
    let t1 = t0 + (p1 - p0).abs().sqrt().powf(alpha);
    let t2 = t1 + (p2 - p1).abs().sqrt().powf(alpha);
    let t3 = t2 + (p3 - p2).abs().sqrt().powf(alpha);
    (t0, (t1.max(t0 + 1e-6)), (t2.max(t1 + 1e-6)), (t3.max(t2 + 1e-6)))
}

#[inline]
fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn squad_with_neighbors(
    q0: Quat,
    q1: Quat,
    q2: Quat,
    q3: Quat,
    u: f32,
    dt0: f32,
    dt1: f32,
    dt2: f32,
) -> Quat {
    // dt0 = t1 - t0, dt1 = t2 - t1, dt2 = t3 - t2  (clamped to small eps > 0)
    let q0 = q0.normalize();
    let q1 = q1.normalize();
    let q2 = q2.normalize();
    let q3 = q3.normalize();

    let s1 = squad_control_weighted(q0, q1, q2, dt0, dt1);
    let s2 = squad_control_weighted(q1, q2, q3, dt1, dt2);

    // Standard SQUAD blend (Shoemake)
    let q12 = q1.slerp(q2, u);
    let s12 = s1.slerp(s2, u);
    q12.slerp(s12, 2.0 * u * (1.0 - u)).normalize()
}

// Time-weighted tangent (Grassia/Shoemake style)
fn squad_control_weighted(q_prev: Quat, q_curr: Quat, q_next: Quat, dt_prev: f32, dt_next: f32) -> Quat {
    let eps = 1e-6;
    let dtp = dt_prev.max(eps);
    let dtn = dt_next.max(eps);
    let w_prev = dtn / (dtp + dtn); // more weight to the farther side
    let w_next = dtp / (dtp + dtn);

    // s_i = q_i * exp(-0.5 * ( w_next*log(q_i^{-1} q_{i+1}) + w_prev*log(q_i^{-1} q_{i-1}) ) / 2)
    // The -0.25 factor is the usual unweighted form; here we keep the same scale with weights.
    let inv = q_curr.conjugate();
    let a = quat_log(inv * q_next);
    let b = quat_log(inv * q_prev);
    q_curr * quat_exp( (a * w_next + b * w_prev) * (-0.25) )
}

fn quat_log(q: Quat) -> Vec3 {
    let w = q.w;
    let v = Vec3::new(q.x, q.y, q.z);
    let v_len = v.length();
    let eps = 1e-8;
    if v_len < eps { Vec3::ZERO } else { v * (w.acos() / v_len) }
}

fn quat_exp(v: Vec3) -> Quat {
    let t = v.length();
    if t < 1e-8 { Quat::from_xyzw(v.x, v.y, v.z, 1.0).normalize() }
    else {
        let s = t.sin() / t;
        Quat::from_xyzw(v.x * s, v.y * s, v.z * s, t.cos()).normalize()
    }
}

pub fn fix_quaternion_hemispheres(frames: &mut [Keyframe]) {
    if frames.is_empty() { return; }
    for i in 1..frames.len() {
        let a = Quat::from_xyzw(
            frames[i-1].orientation.0,
            frames[i-1].orientation.1,
            frames[i-1].orientation.2,
            frames[i-1].orientation.3,
        );

        let b = Quat::from_xyzw(
            frames[i].orientation.0,
            frames[i].orientation.1,
            frames[i].orientation.2,
            frames[i].orientation.3,
        );

        if a.dot(b) < 0.0 {
            let inverted = -b;
            frames[i].orientation = protocol::keyframe::Quat(
                inverted.x,
                inverted.y,
                inverted.z,
                inverted.w,
            );
        }
    }
}
