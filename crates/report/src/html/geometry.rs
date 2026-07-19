use std::fmt::Write as _;

use super::shared::CheckedHtmlWriter;

const GEOMETRY_STEPS: u16 = 1_000;

pub(super) fn geometry_class(percent: f64) -> String {
    let step = (percent.clamp(0.0, 100.0) * 10.0).round() as u16;
    format!("g{step}")
}

pub(super) fn render_geometry_styles(out: &mut CheckedHtmlWriter<'_>) {
    out.push_str(".geometry-left{left:var(--g)}.geometry-width{width:var(--g)}");
    for step in 0..=GEOMETRY_STEPS {
        let whole = step / 10;
        let tenth = step % 10;
        if tenth == 0 {
            write!(out, ".g{step}{{--g:{whole}%}}").expect("checked HTML writer records failures");
        } else {
            write!(out, ".g{step}{{--g:{whole}.{tenth}%}}")
                .expect("checked HTML writer records failures");
        }
    }
}
