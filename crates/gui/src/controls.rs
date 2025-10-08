use eframe::egui::{DragValue, Response, Slider, Ui};

pub(crate) fn drag_angle(ui: &mut Ui, label: &str, radians: &mut f32) -> Response {
    let mut degrees = radians.to_degrees();
    let mut response = ui.add(
        DragValue::new(&mut degrees)
            .prefix(label)
            .speed(1.0)
            .suffix("°")
            .range(0.0..=360.0),
    );

    // only touch `*radians` if we actually changed the degree value
    if degrees != radians.to_degrees() {
        *radians = degrees.to_radians();
        response.mark_changed();
    }

    response
}

pub(crate) fn drag_angle_signed(ui: &mut Ui, label: &str, radians: &mut f32) -> Response {
    let mut degrees = radians.to_degrees();
    let mut response = ui.add(
        DragValue::new(&mut degrees)
            .prefix(label)
            .speed(1.0)
            .suffix("°")
            .range(-360.0..=360.0),
    );

    // only touch `*radians` if we actually changed the degree value
    if degrees != radians.to_degrees() {
        *radians = degrees.to_radians();
        response.mark_changed();
    }

    response
}

pub(crate) fn drag_pitch(ui: &mut Ui, label: &str, radians: &mut f32) -> Response {
    let mut degrees = radians.to_degrees();
    let mut response = ui.add(
        DragValue::new(&mut degrees)
            .prefix(label)
            .speed(1.0)
            .suffix("°")
            .range(-90.0..=90.0),
    );

    // only touch `*radians` if we actually changed the degree value
    if degrees != radians.to_degrees() {
        *radians = degrees.to_radians();
        response.mark_changed();
    }

    response
}

pub(crate) fn drag_percentage(ui: &mut Ui, label: &str, multiple: &mut f32, log: bool) -> Response {
    let mut percentage = *multiple * 100.0;

    let response = ui.add(
        Slider::new(&mut percentage, 0.1..=1000.0)
            .logarithmic(log)
            .suffix("%")
            .text(label)
    );

    *multiple = (percentage / 100.0).clamp(0.001, 10.0);

    response
}

pub(crate) fn drag_percentage_delta(ui: &mut Ui, label: &str, multiple: &mut f32, log: bool) -> Response {
    let mut percentage = *multiple * 100.0;

    let response = ui.add(
        Slider::new(&mut percentage, -200.0..=200.0)
            .logarithmic(log)
            .suffix("%")
            .text(label)
    );

    *multiple = (percentage / 100.0).clamp(-10.0, 10.0);

    response
}
