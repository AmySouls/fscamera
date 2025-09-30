use bevy::{input::mouse::MouseMotion, prelude::*, window::{CursorGrabMode, PrimaryWindow}};
use camera::{FreeCam, FreeCamInput};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(FreeCamRes(FreeCam::from(
            glam::Vec3::new(-2.5, 2.5, 9.0),
            glam::Quat::IDENTITY,
        )))
        .insert_resource(MouseState { captured: false })
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                toggle_mouse_capture,
                freecam_input_system,
                apply_freecam_to_bevy,
            ),
        )
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((Camera3d::default(), FreeCamRig));

    // Base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    // Cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    // Lighting
    commands.spawn((PointLight {
        shadows_enabled: true,
        ..Default::default()
    },));
}

fn freecam_input_system(
    time: Res<Time>,
    mut cam: ResMut<FreeCamRes>,
    keys: Res<ButtonInput<KeyCode>>,
    mut evr_motion: EventReader<MouseMotion>,
    ms: Res<MouseState>,
) {
    let mut input = FreeCamInput::default();

    keys.pressed(KeyCode::KeyW).then(|| input.forward += 1.0);
    keys.pressed(KeyCode::KeyS).then(|| input.forward -= 1.0);
    keys.pressed(KeyCode::KeyD).then(|| input.right += 1.0);
    keys.pressed(KeyCode::KeyA).then(|| input.right -= 1.0);
    keys.pressed(KeyCode::Space).then(|| input.up += 1.0);
    keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
        .then(|| input.up -= 1.0);

    keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        .then(|| input.speed_multiplier *= 4.0);
    keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
        .then(|| input.speed_multiplier /= 4.0);

    keys.pressed(KeyCode::KeyE).then(|| input.roll_delta += 1.0);
    keys.pressed(KeyCode::KeyQ).then(|| input.roll_delta -= 1.0);

    if ms.captured {
        input.mouse_delta = {
            let mut tmp = Vec2::ZERO;
            for ev in evr_motion.read() {
                tmp += ev.delta;
            }
            glam::Vec2::new(tmp.x, tmp.y)
        };
    } else {
        for _ev in evr_motion.read() {}
    }

    cam.0.update(&input, time.delta_secs());
}

fn apply_freecam_to_bevy(mut q_cam: Query<&mut Transform, With<FreeCamRig>>, cam: Res<FreeCamRes>) {
    if let Ok(mut t) = q_cam.get_single_mut() {
        t.translation = Vec3::new(
            cam.0.translation.x,
            cam.0.translation.y,
            cam.0.translation.z,
        );

        t.rotation = Quat::from_xyzw(
            cam.0.rotation.x,
            cam.0.rotation.y,
            cam.0.rotation.z,
            cam.0.rotation.w,
        );
    }
}

fn toggle_mouse_capture(
    keys: Res<ButtonInput<KeyCode>>,
    mut ms: ResMut<MouseState>,
    mut q_windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        ms.captured = !ms.captured;
        if let Ok(mut window) = q_windows.get_single_mut() {
            window.cursor_options.visible = !ms.captured;
            window.cursor_options.grab_mode = if ms.captured {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
        }
    }
}

#[derive(Component)]
struct FreeCamRig;

#[derive(Resource)]
struct FreeCamRes(FreeCam);

#[derive(Resource)]
struct MouseState {
    captured: bool,
}
