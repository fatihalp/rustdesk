pub(crate) fn mobile_wheel_delta(xy: (i32, i32), reverse_mouse_wheel: &str) -> (i32, i32) {
    if reverse_mouse_wheel == "Y" {
        (xy.0.saturating_neg(), xy.1.saturating_neg())
    } else {
        xy
    }
}
