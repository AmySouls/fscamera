// use nalgebra_glm as glm;
// use protocol::keyframe::{Orientation, Quat, Vec3};
//
// #[derive(Clone, Debug)]
// pub struct PlaybackFrame {
//     pub time: f32,
//     pub position: Vec3,
//     pub orientation: Orientation,
//     pub fov: f32,
//     pub tension: f32,
// }
//
// pub trait KeyframeInterpolate {
//     fn interpolate(frames: &[PlaybackFrame], time: f32) -> Option<(Vec3, Quat, f32)>;
// }
//
// impl KeyframeInterpolate for PlaybackFrame {
//     fn interpolate(frames: &[PlaybackFrame], time: f32) -> Option<(Vec3, Quat, f32)> {
//         let len = frames.len();
//         if len < 2 {
//             return None;
//         }
//
//         // Handle before/after range
//         if time <= frames[0].time {
//             let f = &frames[0];
//             return Some((f.position, f.orientation, f.fov));
//         }
//
//         if time >= frames[len - 1].time {
//             let f = &frames[len - 1];
//             return Some((f.position, f.orientation, f.fov));
//         }
//
//         // Find segment
//         let (i1, i2) = frames.windows(2).enumerate().find_map(|(i, w)| {
//             if time >= w[0].time && time <= w[1].time {
//                 Some((i, i + 1))
//             } else {
//                 None
//             }
//         })?;
//
//         let f0 = if i1 > 0 { &frames[i1 - 1] } else { &frames[i1] };
//         let f1 = &frames[i1];
//         let f2 = &frames[i2];
//         let f3 = if i2 + 1 < len {
//             &frames[i2 + 1]
//         } else {
//             &frames[i2]
//         };
//
//         let t = (time - f1.time) / (f2.time - f1.time);
//
//         let pos = hermite_position(f0, f1, f2, f3, t, f1.tension);
//         let rot = if len > 3 {
//             interpolate_rotation(f0, f1, f2, f3, t)
//         } else {
//             Quat::from_glm(glm::quat_slerp(
//                 &f1.orientation.to_glm(),
//                 &f2.orientation.to_glm(),
//                 t,
//             ))
//         };
//         let fov = lerp(f1.fov, f2.fov, t);
//
//         Some((Vec3::from_glm(pos), rot, fov))
//     }
// }
//
// fn hermite_position(
//     f0: &PlaybackFrame,
//     f1: &PlaybackFrame,
//     f2: &PlaybackFrame,
//     f3: &PlaybackFrame,
//     t: f32,
//     tension: f32,
// ) -> glm::Vec3 {
//     let p0 = f0.position.to_glm();
//     let p1 = f1.position.to_glm();
//     let p2 = f2.position.to_glm();
//     let p3 = f3.position.to_glm();
//
//     // Tangents (can adjust tension)
//     let m1 = (p2 - p0) * 0.5 * (1.0 - tension);
//     let m2 = (p3 - p1) * 0.5 * (1.0 - tension);
//
//     let t2 = t * t;
//     let t3 = t2 * t;
//
//     let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
//     let h10 = t3 - 2.0 * t2 + t;
//     let h01 = -2.0 * t3 + 3.0 * t2;
//     let h11 = t3 - t2;
//
//     h00 * p1 + h10 * m1 + h01 * p2 + h11 * m2
// }
//
// fn lerp(a: f32, b: f32, t: f32) -> f32 {
//     a * (1.0 - t) + b * t
// }
//
// fn squad_tangent(q_prev: glm::Quat, q: glm::Quat, q_next: glm::Quat) -> glm::Quat {
//     let inv_q = glm::quat_inverse(&q);
//     let log1 = glm::quat_log(&(inv_q * q_prev));
//     let log2 = glm::quat_log(&(inv_q * q_next));
//     q * glm::quat_exp(&((-0.25) * (log1 + log2)))
// }
//
// fn squad(q1: glm::Quat, q2: glm::Quat, s1: glm::Quat, s2: glm::Quat, t: f32) -> glm::Quat {
//     let slerp_1 = glm::quat_slerp(&q1, &q2, t);
//     let slerp_2 = glm::quat_slerp(&s1, &s2, t);
//     glm::quat_slerp(&slerp_1, &slerp_2, 2.0 * t * (1.0 - t))
// }
//
// fn interpolate_rotation(
//     f0: &PlaybackFrame,
//     f1: &PlaybackFrame,
//     f2: &PlaybackFrame,
//     f3: &PlaybackFrame,
//     t: f32,
// ) -> Quat {
//     let q1 = f1.orientation.to_glm();
//     let mut q2 = f2.orientation.to_glm();
//
//     // Always ensure q2 is in the same hemisphere as q1
//     q2 = ensure_shortest_path(q1, q2);
//
//     // If q0 == q1 or we're at the start, fallback to SLERP
//     let use_slerp = f0.time == f1.time;
//
//     if use_slerp {
//         let rot = glm::quat_slerp(&q1, &q2, t);
//         return Quat::from_glm(rot);
//     }
//
//     // Normal case: use SQUAD
//     let mut q0 = f0.orientation.to_glm();
//     let mut q3 = f3.orientation.to_glm();
//     q0 = ensure_shortest_path(q1, q0);
//     q3 = ensure_shortest_path(q2, q3);
//
//     let s1 = squad_tangent(q0, q1, q2);
//     let s2 = squad_tangent(q1, q2, q3);
//
//     let result = squad(q1, q2, s1, s2, t);
//     Quat::from_glm(result)
// }
//
// fn ensure_shortest_path(q1: glm::Quat, q2: glm::Quat) -> glm::Quat {
//     if glm::quat_dot(&q1, &q2) < 0.0 {
//         -q2
//     } else {
//         q2
//     }
// }
