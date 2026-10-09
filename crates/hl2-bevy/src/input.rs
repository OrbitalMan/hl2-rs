//! Bevy event adaptation for gameplay and character input.
//!
//! UI event handling lives in `ui`; this module only turns an already captured
//! keyboard/mouse frame into gameplay actions and a Source movement intent.

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use modkit_core::movement::Input;

type InputDevices<'w> = (
    Res<'w, ButtonInput<KeyCode>>,
    Res<'w, ButtonInput<MouseButton>>,
    Res<'w, AccumulatedMouseMotion>,
    Option<Res<'w, bevy::input::mouse::AccumulatedMouseScroll>>,
);

/// Adapts persisted UI binding names to Bevy's platform key enum.
pub fn parse_keycode(name: &str) -> Option<KeyCode> {
    match name {
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        "Digit0" => Some(KeyCode::Digit0),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "Space" => Some(KeyCode::Space),
        "ControlLeft" | "Ctrl" => Some(KeyCode::ControlLeft),
        "ControlRight" => Some(KeyCode::ControlRight),
        "ShiftLeft" | "Shift" => Some(KeyCode::ShiftLeft),
        "ShiftRight" => Some(KeyCode::ShiftRight),
        "AltLeft" | "Alt" => Some(KeyCode::AltLeft),
        "AltRight" => Some(KeyCode::AltRight),
        "ArrowUp" | "Up" => Some(KeyCode::ArrowUp),
        "ArrowDown" | "Down" => Some(KeyCode::ArrowDown),
        "ArrowLeft" | "Left" => Some(KeyCode::ArrowLeft),
        "ArrowRight" | "Right" => Some(KeyCode::ArrowRight),
        "Enter" => Some(KeyCode::Enter),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        "Tab" => Some(KeyCode::Tab),
        "Escape" => Some(KeyCode::Escape),
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),
        _ => None,
    }
}

pub(crate) fn action_pressed(
    action: &str,
    default_key: KeyCode,
    keys: &ButtonInput<KeyCode>,
    config: Option<&hl2_ui::config::Config>,
) -> bool {
    keys.pressed(
        parse_keycode(hl2_ui::input::binding(
            config,
            action,
            default_key_name(default_key),
        ))
        .unwrap_or(default_key),
    )
}

fn action_just_pressed(
    action: &str,
    default_key: KeyCode,
    keys: &ButtonInput<KeyCode>,
    config: Option<&hl2_ui::config::Config>,
) -> bool {
    keys.just_pressed(
        parse_keycode(hl2_ui::input::binding(
            config,
            action,
            default_key_name(default_key),
        ))
        .unwrap_or(default_key),
    )
}

fn default_key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::Space => "Space",
        KeyCode::ControlLeft => "ControlLeft",
        KeyCode::ShiftLeft => "ShiftLeft",
        KeyCode::AltLeft => "AltLeft",
        KeyCode::KeyW => "KeyW",
        KeyCode::KeyS => "KeyS",
        KeyCode::KeyA => "KeyA",
        KeyCode::KeyD => "KeyD",
        KeyCode::KeyE => "KeyE",
        KeyCode::KeyR => "KeyR",
        KeyCode::KeyQ => "KeyQ",
        KeyCode::KeyG => "KeyG",
        KeyCode::F3 => "F3",
        KeyCode::Digit1 => "Digit1",
        KeyCode::Digit2 => "Digit2",
        KeyCode::Digit3 => "Digit3",
        KeyCode::Digit4 => "Digit4",
        KeyCode::Digit5 => "Digit5",
        KeyCode::Digit6 => "Digit6",
        _ => "",
    }
}

pub(crate) fn controls(
    (keys, buttons, mouse, scroll): InputDevices,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut sim: ResMut<crate::movement::Simulation>,
    mut game: Option<ResMut<crate::gameplay::Gameplay>>,
    ui: Option<Res<crate::console::Console>>,
) {
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    if keys.just_pressed(KeyCode::F1) && window.focused {
        sim.dev_overlay = !sim.dev_overlay;
    }
    if !sim.commands.is_empty() {
        return;
    }
    let cfg = ui.as_ref().map(|u| &u.source.config);
    if !action_pressed("+jump", KeyCode::Space, &keys, cfg) {
        sim.jump_suppressed = false;
    }
    if let Some(game) = game.as_deref_mut() {
        game.buttons(
            buttons.pressed(MouseButton::Left),
            buttons.pressed(MouseButton::Right),
        );
    }
    sim.input = Input::default();
    if sim.paused() || sim.transition || cursor.grab_mode == CursorGrabMode::None {
        if let Some(game) = game.as_deref_mut() {
            game.consume_attacks();
        }
        return;
    }
    if let Some(game) = game.as_deref_mut() {
        use crate::gameplay::Action;
        for (name, key, action) in [
            ("loadout", KeyCode::F3, Action::Loadout),
            ("+use", KeyCode::KeyE, Action::Use),
            ("+reload", KeyCode::KeyR, Action::Reload),
            ("lastinv", KeyCode::KeyQ, Action::Previous),
            ("impulse", KeyCode::KeyG, Action::Impulse),
        ] {
            if action_just_pressed(name, key, &keys, cfg) {
                game.actions.push(action);
            }
        }
        for (slot, (name, key)) in [
            ("slot1", KeyCode::Digit1),
            ("slot2", KeyCode::Digit2),
            ("slot3", KeyCode::Digit3),
            ("slot4", KeyCode::Digit4),
            ("slot5", KeyCode::Digit5),
            ("slot6", KeyCode::Digit6),
        ]
        .into_iter()
        .enumerate()
        {
            if action_just_pressed(name, key, &keys, cfg) {
                game.actions.push(Action::Slot { slot });
            }
        }
        if let Some(scroll) = scroll.as_deref().filter(|scroll| scroll.delta.y != 0.) {
            game.actions.push(Action::Wheel {
                delta: if scroll.delta.y > 0. { -1 } else { 1 },
            });
        }
    }
    let (sensitivity, invert) = cfg
        .map(|c| (c.mouse.sensitivity, c.mouse.invert))
        .unwrap_or((3.0, false));
    let scale = (sensitivity / 3.0).clamp(0.01, 10.0);
    sim.yaw -= mouse.delta.x * 0.001152 * scale;
    sim.pitch = (sim.pitch - mouse.delta.y * 0.001152 * scale * if invert { -1.0 } else { 1.0 })
        .clamp(-1.55, 1.55);
    if keys.just_pressed(KeyCode::F2) {
        let fly = !sim.flying();
        sim.set_fly(fly);
    }
    sim.input = Input {
        forward: f32::from(action_pressed("+forward", KeyCode::KeyW, &keys, cfg))
            - f32::from(action_pressed("+back", KeyCode::KeyS, &keys, cfg)),
        side: f32::from(action_pressed("+moveright", KeyCode::KeyD, &keys, cfg))
            - f32::from(action_pressed("+moveleft", KeyCode::KeyA, &keys, cfg)),
        yaw: sim.yaw,
        jump: action_pressed("+jump", KeyCode::Space, &keys, cfg) && !sim.jump_suppressed,
        crouch: action_pressed("+duck", KeyCode::ControlLeft, &keys, cfg),
        sprint: action_pressed("+speed", KeyCode::ShiftLeft, &keys, cfg),
        slow: action_pressed("+walk", KeyCode::AltLeft, &keys, cfg),
    };
}
