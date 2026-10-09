//! Portable input-binding policy shared by UI hosts.
//!
//! The host translates its native key identifiers at the edge. This module
//! intentionally knows only the persisted Source-style binding names.

use crate::config::Config;

/// Returns the configured native-key name for an action, or its documented
/// default when the configuration has no binding.
pub fn binding<'a>(config: Option<&'a Config>, action: &str, default: &'a str) -> &'a str {
    config
        .and_then(|config| config.keys.get(action))
        .map(String::as_str)
        .unwrap_or(default)
}

/// Source-style action names and their default physical key names.
///
/// Hosts use this as the single source of truth when building a native input
/// frame; UI configuration serializes the same names.
pub const MOVEMENT_BINDINGS: &[(&str, &str)] = &[
    ("+forward", "KeyW"),
    ("+back", "KeyS"),
    ("+moveleft", "KeyA"),
    ("+moveright", "KeyD"),
    ("+jump", "Space"),
    ("+duck", "ControlLeft"),
    ("+speed", "ShiftLeft"),
    ("+walk", "AltLeft"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_binding_overrides_the_documented_default() {
        let mut config = Config::default();
        config.keys.insert("+jump".into(), "KeyJ".into());
        assert_eq!(binding(Some(&config), "+jump", "Space"), "KeyJ");
        assert_eq!(binding(Some(&config), "+forward", "KeyW"), "KeyW");
        assert_eq!(binding(None, "+jump", "Space"), "Space");
    }
}
