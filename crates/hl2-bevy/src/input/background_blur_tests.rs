use super::{ButtonInput, DebugRepeats, DebugTuningInput, KeyCode, process_debug_tuning_keys};

#[test]
fn minus_toggles_once_per_press_only_in_background_debug_outside_console() {
    let mut sim = crate::simulation::Simulation::new(
        &modkit_core::World::default(),
        glam::Vec3::ZERO,
        0.,
        0.,
        false,
        vec![],
    );
    sim.background_level = true;
    sim.background_debug.enabled = true;
    let mut ui = crate::ui_host::Console::new(Default::default());
    ui.source.canvas.resize(1280., 720.);
    ui.source.background_debug_mode = true;
    let mut repeats = DebugRepeats::default();
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Minus);
    let mut run = |keys: &ButtonInput<KeyCode>,
                   sim: &mut crate::simulation::Simulation,
                   ui: &mut crate::ui_host::Console| {
        process_debug_tuning_keys(
            sim,
            None,
            Some(ui),
            DebugTuningInput {
                keys,
                time: Some(0.),
                aspect: 16. / 9.,
            },
            &mut repeats,
        );
    };
    run(&keys, &mut sim, &mut ui);
    assert!(ui.source.config.video.background_force_default_blur);
    assert_eq!(sim.background_debug.blur_texture_name, "background01");
    keys.clear();
    assert!(keys.pressed(KeyCode::Minus));
    run(&keys, &mut sim, &mut ui);
    assert!(
        ui.source.config.video.background_force_default_blur,
        "held key must not repeat"
    );
    keys.release(KeyCode::Minus);
    keys.press(KeyCode::Minus);
    run(&keys, &mut sim, &mut ui);
    assert!(!ui.source.config.video.background_force_default_blur);
    assert_eq!(
        sim.background_debug.blur_texture_name,
        "background01_widescreen"
    );
    ui.source.set_mode(hl2_ui::console::Mode::Console);
    keys.release(KeyCode::Minus);
    keys.press(KeyCode::Minus);
    run(&keys, &mut sim, &mut ui);
    assert!(!ui.source.config.video.background_force_default_blur);
    ui.source.set_mode(hl2_ui::console::Mode::MainMenu);
    sim.background_debug.enabled = false;
    run(&keys, &mut sim, &mut ui);
    assert!(!ui.source.config.video.background_force_default_blur);
}
