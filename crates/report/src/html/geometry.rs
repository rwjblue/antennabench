use std::fmt::Write as _;

const GEOMETRY_STEPS: u16 = 1_000;

pub(super) fn geometry_class(percent: f64) -> String {
    let step = (percent.clamp(0.0, 100.0) * 10.0).round() as u16;
    format!("g{step}")
}

pub(super) fn write_geometry_styles(write: &mut impl FnMut(&str)) {
    write(".geometry-left{left:var(--g)}.geometry-width{width:var(--g)}");
    for step in 0..=GEOMETRY_STEPS {
        let whole = step / 10;
        let tenth = step % 10;
        let mut rule = String::with_capacity(24);
        if tenth == 0 {
            write!(rule, ".g{step}{{--g:{whole}%}}").expect("writing CSS to a string cannot fail");
        } else {
            write!(rule, ".g{step}{{--g:{whole}.{tenth}%}}")
                .expect("writing CSS to a string cannot fail");
        }
        write(&rule);
    }
}
