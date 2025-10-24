use crate::{Camera, Space};
use protocol::keyframe::Keyframe;
use glam::{Vec3, Quat};

/// Camera that can play a path made up of keyframes by smoothly interpolating between them.
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
    pub fn new(space: Space, translation: Vec3, orientation: Quat, fov: f32) -> Self {
        Self {
            camera: Camera::new(space, translation, orientation, fov),
            playing: false,
            time: 0.0,
            keyframes: vec![],
        }
    }

    pub fn set_keyframes(&mut self, mut keyframes: Vec<Keyframe>) {
        keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));

        // For (s)lerping to work properly we need to ensure that all quats are have their
        // directional vector on the same hemisphere.
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

        let frame = interpolate_frame(&self.keyframes, self.time);
        self.camera.translation = frame.translation;
        self.camera.rotation = frame.rotation;
        self.camera.fov = frame.fov;
    }
}

fn hemifix(q: Quat) -> Quat {
    let mut r = q;
    if r.length_squared() == 0.0 {
        r = Quat::IDENTITY;
    }

    r.normalize()
}

/// Intermediate state of the camera produced by interpolating between the keyframes. 
#[derive(Default)]
pub struct PlaybackFrame {
    pub translation: Vec3,
    pub rotation: Quat,
    pub fov: f32,
}

fn interpolate_frame(frames: &[Keyframe], t: f32) -> PlaybackFrame {
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

    let k0r = hemifix(Quat::from_xyzw(k0.orientation.0, k0.orientation.1, k0.orientation.2, k0.orientation.3));
    let mut k1r = hemifix(Quat::from_xyzw(k1.orientation.0, k1.orientation.1, k1.orientation.2, k1.orientation.3));
    let mut k2r = hemifix(Quat::from_xyzw(k2.orientation.0, k2.orientation.1, k2.orientation.2, k2.orientation.3));
    let mut k3r = hemifix(Quat::from_xyzw(k3.orientation.0, k3.orientation.1, k3.orientation.2, k3.orientation.3));

    const ANTI_EPS: f32 = -0.9999;

    if k1r.dot(k0r) < 0.0 && k1r.dot(k0r) > ANTI_EPS { k1r = -k1r; }
    if k2r.dot(k1r) < 0.0 && k2r.dot(k1r) > ANTI_EPS { k2r = -k2r; }
    if k3r.dot(k2r) < 0.0 && k3r.dot(k2r) > ANTI_EPS { k3r = -k3r; }

    // Determine the blending factor between k1 and k2 from t.
    let u = if k2.time > k1.time {
        ((t - k1.time) / (k2.time - k1.time)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let a = squad_tangent(k0r, k1r, k2r);
    let b = squad_tangent(k1r, k2r, k3r);
    let rotation = squad(k1r, k2r, a, b, u).normalize();

    // let s1 = slerp_stable(k0r, k1r, u);
    // let s2 = slerp_stable(a, b, u);
    // let rotation = slerp_stable(s1, s2, 2.0 * u (1.0 - u)).normalize();

    // u = u*u*(3.0 - 2.0 * u);

    // let translation = catmull_rom_centripetal_vec3(k0t, k1t, k2t, k3t, u); 
    let translation = pos_tcb_hermite(
        k0t, k0.time,
        k1t, k1.time,
        k2t, k2.time,
        k3t, k3.time,
        u,
        0.0,
        0.0,
        0.3,
    ); 

    // let rotation = squad_with_neighbors(k0r, k1r, k2r, k3r, u, dt0, dt1, dt2); 
    let fov = catmull_rom_centripetal_scalar(k0.fov, k1.fov, k2.fov, k3.fov, u); 

    PlaybackFrame {
        translation,
        rotation,
        fov,
    }
}

/// Interpolate position on the span [k1, k2] with Kochanek–Bartels (TCB) Hermite.
/// T in [0,1]  : 0 = loose (Catmull-Rom-like), 1 = straight lines (no curvature)
/// C in [-1,1] : -1 = more corner, +1 = smoother join
/// B in [-1,1] : -1 = favor incoming, +1 = favor outgoing (set ~0.2–0.4 to *preserve momentum*)
fn pos_tcb_hermite(
    p0: Vec3, t0: f32,
    p1: Vec3, t1: f32,
    p2: Vec3, t2: f32,
    p3: Vec3, t3: f32,
    u: f32,               // normalized local time in [0,1] over [t1, t2]
    tension: f32,         // T
    continuity: f32,      // C
    bias: f32,            // B
) -> Vec3 {
    let eps = 1e-6;
    let dt0 = (t1 - t0).max(eps);
    let dt1 = (t2 - t1).max(eps);
    let dt2 = (t3 - t2).max(eps);

    // Finite differences with real time (non-uniform-friendly)
    let d1 = (p1 - p0) / dt0;
    let d2 = (p2 - p1) / dt1;
    let d3 = (p3 - p2) / dt2;

    let t = tension.clamp(0.0, 1.0);
    let c = continuity.clamp(-1.0, 1.0);
    let b = bias.clamp(-1.0, 1.0);

    // Incoming/outgoing tangents at p1 and p2 (Kochanek–Bartels)
    // See: Kochanek & Bartels 1984; also used in many DCC tools.
    let m1_out = (1.0 - t) * (
        (1.0 - c) * (1.0 + b) * 0.5 * d1 +
        (1.0 + c) * (1.0 - b) * 0.5 * d2
    );
    let m2_in  = (1.0 - t) * (
        (1.0 + c) * (1.0 + b) * 0.5 * d2 +
        (1.0 - c) * (1.0 - b) * 0.5 * d3
    );

    // Cubic Hermite basis (u in [0,1]); scale tangents by real span length
    let h = dt1;

    let u2 = u * u;
    let u3 = u2 * u;

    let h00 =  2.0*u3 - 3.0*u2 + 1.0;
    let h10 =      u3 - 2.0*u2 + u;
    let h01 = -2.0*u3 + 3.0*u2;
    let h11 =      u3 -     u2;

    h00 * p1 + h10 * (m1_out * h) + h01 * p2 + h11 * (m2_in * h)
}

fn slerp_stable(q_from: Quat, q_to: Quat, t: f32) -> Quat {
    let qf = q_from.normalize();
    let qt = q_to.normalize();

    let delta = qf.conjugate() * qt;
    let v = delta.xyz();
    let w = delta.w;

    // Angle in [0, PI]
    let v_len = v.length();
    let angle = 2.0 * v_len.atan2(w);

    if angle < 1e-6 {
        return (qf * (1.0 - t) + qt * t).normalize();
    }

    let axis = if v_len > 1e-8 { v / v_len } else { Vec3::X };

    qf * Quat::from_axis_angle(axis, angle * t)
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

fn squad_tangent(q0: Quat, q1: Quat, q2: Quat) -> Quat {
    let inv_q1 = q1.conjugate();
    let l1 = quat_log(inv_q1 * q0);
    let l2 = quat_log(inv_q1 * q2);
    q1 * quat_exp((l1 + l2) * (-0.25))
}

fn squad(q1: Quat, q2: Quat, a: Quat, b: Quat, u: f32) -> Quat {
    let s1 = q1.slerp(q2, u);
    let s2 = a.slerp(b, u);
    s1.slerp(s2, 2.0 * u * (1.0 - u))
}

fn quat_log(q: Quat) -> glam::Vec3 {
    let v = q.xyz();
    let w = q.w;
    let v_len = v.length();
    if v_len < 1e-8 {
        glam::Vec3::ZERO
    } else {
        let angle = v_len.atan2(w);
        v * (angle / v_len)
    }
}

fn quat_exp(v: glam::Vec3) -> Quat {
    let theta = v.length();
    if theta < 1e-8 {
        Quat::from_xyzw(v.x, v.y, v.z, 1.0).normalize()
    } else {
        let s = theta.sin() / theta;
        Quat::from_xyzw(v.x * s, v.y * s, v.z * s, theta.cos()).normalize()
    }
}

/// Ensure all quats are on the same hemisphere otherwise we run into an ouchie when lerping
fn fix_quaternion_hemispheres(frames: &mut [Keyframe]) {
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
