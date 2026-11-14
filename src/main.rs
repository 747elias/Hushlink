#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() -> eframe::Result {
    if hushlink::media::run_camera_helper_if_requested() {
        return Ok(());
    }
    hushlink::ui::run()
}
