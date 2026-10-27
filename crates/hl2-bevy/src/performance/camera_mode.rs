//! Opt-in profiling experiment: configure direct drawing only at camera creation.
use bevy::{
    prelude::{App, Camera3d},
    render::view::NoIndirectDrawing,
};

fn requested(background: bool, value: Option<&str>) -> bool {
    background && value == Some("1")
}

pub(super) fn configure(app: &mut App) -> bool {
    let value = std::env::var("HL2RS_PROFILE_DIRECT_DRAW").ok();
    let direct = requested(
        !app.is_plugin_added::<bevy::winit::WinitPlugin>(),
        value.as_deref(),
    );
    if direct {
        // Bevy requires this mode to be present when cameras are initially spawned.
        app.register_required_components::<Camera3d, NoIndirectDrawing>();
    }
    direct
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_drawing_requires_an_explicit_background_profile_request() {
        assert!(requested(true, Some("1")));
        for background in [true, false] {
            for value in [None, Some("0"), Some("true"), Some("")] {
                assert!(!requested(background, value));
            }
        }
        assert!(!requested(false, Some("1")));
    }
}
