//! Screenshot readback for `--capture` and `--monitor-capture`: the observers that keep the captured images
//! in the run status, and saving them.
use crate::Status;
use bevy::{
    ecs::{observer::On, system::Res},
    image::Image,
    render::view::screenshot::ScreenshotCaptured,
};
use std::path::PathBuf;

pub(crate) fn capture_complete(event: On<ScreenshotCaptured>, status: Res<Status>) {
    let mut report = status.0.lock().expect("status lock");
    report.capture_completed = true;
    report.capture_size = Some([event.image.width(), event.image.height()]);
    report.captured_image = Some(event.image.clone());
}

pub(crate) fn monitor_capture_complete(event: On<ScreenshotCaptured>, status: Res<Status>) {
    let mut report = status.0.lock().expect("monitor capture");
    report.monitor_capture_completed = true;
    report.monitor_image = Some(event.image.clone());
}

pub(crate) fn save_capture(image: Option<Image>, path: Option<&PathBuf>) -> Option<String> {
    let path = path?;
    match image {
        Some(image) => match image.try_into_dynamic() {
            Ok(image) => image
                .to_rgb8()
                .save(path)
                .err()
                .map(|error| error.to_string()),
            Err(error) => Some(format!("screenshot conversion: {error}")),
        },
        None => Some("screenshot readback did not finish".into()),
    }
}
