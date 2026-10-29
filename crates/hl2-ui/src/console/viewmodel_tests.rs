//! The viewmodel debug commands only ask the host; it decides whether they apply.

use super::{Console, Effect};

#[test]
fn viewmodel_debug_and_reset_commands_reach_the_host() {
    let mut console = Console::default();
    assert_eq!(
        console.submit("viewmodel_debug_mode 1; viewmodel_tweak_reset; viewmodel_debug_mode 0"),
        vec![
            Effect::ViewmodelDebugMode(true),
            Effect::ViewmodelTweakReset,
            Effect::ViewmodelDebugMode(false)
        ]
    );
    assert!(console
        .submit("viewmodel_debug_mode; viewmodel_debug_mode 2; viewmodel_tweak_reset 1")
        .is_empty());
}

#[test]
fn viewmodel_debug_name_is_registered_and_old_mode_name_is_not() {
    let names: Vec<_> = super::commands::COMMANDS.iter().map(|s| s.name).collect();
    assert!(names.contains(&"viewmodel_debug_mode"));
    assert!(!names.contains(&"viewmodel_tweak_mode"));
}
