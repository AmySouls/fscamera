use eframe::egui::{DragValue, Response, RichText, Slider, Ui};

pub(crate) fn drag_fov(ui: &mut Ui, label: &str, radians: &mut f32) -> Response {
    let mut degrees = radians.to_degrees();
    let mut response = ui.add(
        DragValue::new(&mut degrees)
            .prefix(label)
            .speed(1.0)
            .suffix("°")
            .range(1.0..=180.0),
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

#[inline]
pub(crate) fn panel_header(ui: &mut Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .heading()
            .size(16.0)
    );
    ui.separator();
}

#[inline]
pub(crate) fn labeled_control<F>(ui: &mut Ui, label: &str, mut build: F)
where
    F: FnMut(&mut Ui) {

    ui.vertical(|ui| {
        ui.label(label);

        build(ui);
    });
}
