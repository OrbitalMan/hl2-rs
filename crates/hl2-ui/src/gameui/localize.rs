//! Localized GameUI strings from the installed `*_english.txt` / `merged_loc_all.txt` files.
//!
//! Source merges every localization file it loads into one token table; the first file to define
//! a token wins (the search path order). Lookups are case-insensitive and accept an optional
//! leading `#`, matching how `.res` files and code reference tokens.
//!
//! Fallback policy (owner requirement): an unresolved token renders as its **key name** (without
//! the `#`), never as empty text, so missing translations stay visible and diagnosable.
//!
//! Text that only HL2RS shows (no installed token) uses the temporary [`placeholders`].

pub mod placeholders;

use source_assets::keyvalues::{self, Entry};
use std::collections::HashMap;

/// Merged, case-insensitive localization table.
#[derive(Clone, Debug, Default)]
pub struct Localization {
    tokens: HashMap<String, String>,
    /// Files that contributed tokens, in load order (diagnostics/reporting).
    sources: Vec<String>,
}

impl Localization {
    /// Adds every token of a localization resource. Existing tokens are kept (first file wins).
    ///
    /// Accepts both layouts found in the install:
    /// `"lang" { "Language" "English" "Tokens" { ... } }` (UTF-16 `*_english.txt`) and
    /// `"lang" { english { Tokens { ... } } }` (UTF-8 `hl2_complete/resource/merged_loc_all.txt`).
    /// Returns the number of new tokens, or `None` if the bytes are not a localization file.
    pub fn add_resource(&mut self, name: &str, bytes: &[u8]) -> Option<usize> {
        let roots = keyvalues::parse_resource(bytes).ok()?;
        let root = roots
            .into_iter()
            .find(|e| e.key.eq_ignore_ascii_case("lang"))?;
        let tokens = token_block(&root)?;
        let mut added = 0;
        for entry in tokens.children() {
            let Some(text) = entry.text() else { continue };
            if let std::collections::hash_map::Entry::Vacant(slot) =
                self.tokens.entry(entry.key.to_ascii_lowercase())
            {
                slot.insert(unescape(text));
                added += 1;
            }
        }
        self.sources.push(name.to_owned());
        Some(added)
    }

    /// Inserts a token if absent (tests and synthetic resources).
    pub fn insert(&mut self, token: &str, text: &str) {
        self.tokens
            .entry(token.trim_start_matches('#').to_ascii_lowercase())
            .or_insert_with(|| unescape(text));
    }

    /// The translated text of a token, if defined in loaded resources, the temporary
    /// [`placeholders`] or the central fallback table.
    pub fn lookup(&self, token: &str) -> Option<&str> {
        let key = token.trim().trim_start_matches('#');
        if let Some(text) = self.tokens.get(&key.to_ascii_lowercase()) {
            return Some(text.as_str());
        }
        placeholders::lookup(key).or_else(|| fallback_lookup(key))
    }

    /// The translated text of `token`, else the token's key name (never empty for a non-empty token).
    pub fn text(&self, token: &str) -> String {
        let key = token.trim().trim_start_matches('#');
        match self.lookup(key) {
            Some(text) if !text.is_empty() => text.to_owned(),
            _ => key.to_owned(),
        }
    }

    /// Resolves `#Token` references; other text (e.g. `"1280 x 720"`) is returned as-is.
    pub fn localize(&self, text: &str) -> String {
        if text.trim_start().starts_with('#') {
            self.text(text)
        } else {
            text.to_owned()
        }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn sources(&self) -> &[String] {
        &self.sources
    }
}

/// Finds `Tokens` directly under `lang`, or under its first language block.
fn token_block(root: &Entry) -> Option<&Entry> {
    root.get("Tokens").or_else(|| {
        root.children()
            .iter()
            .filter(|e| !e.children().is_empty())
            .find_map(|language| language.get("Tokens"))
    })
}

/// Unescapes common Source localization escape sequences (`\n`, `\r`, `\t`, `\"`, `\\`),
/// leaving non-escape backslashes intact.
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('r') => {
                    chars.next();
                    out.push('\r');
                }
                Some('t') => {
                    chars.next();
                    out.push('\t');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                Some('"') => {
                    chars.next();
                    out.push('"');
                }
                _ => {
                    out.push('\\');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Central English fallback table for installed-game tokens, used when game resources are missing
/// or unresolved. HL2RS-only text belongs in [`placeholders`] instead.
pub const FALLBACK_STRINGS: &[(&str, &str)] = &[
    ("GameUI_Console", "Console"),
    ("Console_Title", "Console"),
    ("Console_Submit", "Submit"),
    ("GameUI_ConfirmMainMenu_Title", "RETURN TO MAIN MENU"),
    (
        "GameUI_MainMenuWarning",
        "Are you sure?\nAll unsaved progress will be lost.",
    ),
    ("GameUI_QuitConfirmationTitle", "QUIT GAME"),
    (
        "GameUI_QuitConfirmationText",
        "Do you wish to stop playing now?",
    ),
    (
        "GameUI_Console_QuitWarning",
        "Are you sure you want to quit?\nAll unsaved progress will be lost.",
    ),
    ("GameUI_Quit", "Quit"),
    ("GameUI_Confirm", "CONFIRM"),
    ("Spec_Auto", "Auto"),
    ("vgui_TimeLeftSecondsRemaining", "%s1 seconds remaining"),
    ("vgui_TimeLeftSecondRemaining", "%s1 second remaining"),
    ("GameUI_OK", "OK"),
    ("GameUI_Cancel", "Cancel"),
    ("GameUI_Apply", "Apply"),
    ("GameUI_Close", "Close"),
    ("GameUI_Options", "OPTIONS"),
    ("GameUI_Loading", "LOADING..."),
    ("GameUI_LoadingFilename", "Loading %s1 ..."),
    (
        "LoadingProgress_SpawningServer",
        "Starting local game server...",
    ),
    ("LoadingProgress_LoadMap", "Loading world..."),
    ("LoadingProgress_PrecacheWorld", "Initializing world..."),
    (
        "LoadingProgress_SignonDataLocal",
        "Initializing game data...",
    ),
    ("LoadingProgress_LoadResources", "Loading resources..."),
    ("LoadingProgress_SignonLocal", "Initializing resources..."),
    ("GameUI_HalfLife2", "Half-Life 2"),
    ("GameUI_EpisodeOne", "Episode One"),
    ("GameUI_EpisodeTwo", "Episode Two"),
    ("GameUI_LostCoast", "Lost Coast"),
    ("GameUI_HalfLife2LostCoast", "Half-Life 2: Lost Coast"),
    ("GameUI_Worskhop_MenuName", "Workshop"),
    ("GameUI_Workshop_MenuName", "Workshop"),
    ("GameUI_Gameplay", "Gameplay"),
    ("GameUI_Keyboard", "Keyboard"),
    ("GameUI_Mouse", "Mouse"),
    ("GameUI_Gamepad", "Gamepad"),
    ("GameUI_Audio", "Audio"),
    ("GameUI_Video", "Video"),
    ("GameUI_HDR", "High Dynamic Range"),
    ("GameUI_hdr_level0", "None"),
    ("GameUI_hdr_level1", "Bloom (if available)"),
    ("GameUI_hdr_level2", "Full (if available)"),
    (
        "GameUI_hdr_Hint",
        "High Dynamic Range will adjust camera exposure based on lighting. \"Release Setting\" will use what each level originally shipped.",
    ),
    ("GameUI_Action", "Action"),
    ("GameUI_KeyButton", "Key / Button"),
    ("GameUI_AdjustGamma_Title", "BRIGHTNESS"),
    (
        "GameUI_ThirdPartyTechCredits",
        "3RD PARTY TECHNOLOGY CREDITS",
    ),
    ("GameUI_KeyboardSettings", "Keyboard Settings"),
    (
        "GameUI_KeyboardSettingsText",
        "Reset key bindings to default?",
    ),
    (
        "GameUI_Miles_Voice",
        "Uses Miles Sound System. Copyright © 1991-2001 by RAD Game Tools, Inc.",
    ),
    (
        "GameUI_Miles_Audio",
        "MPEG Layer-3 playback supplied with the Miles Sound System from RAD Game Tools, Inc. MPEG Layer-3 audio compression technology licensed by Fraunhofer IIS and THOMSON multimedia.",
    ),
    (
        "GameUI_Bink",
        "Uses Bink Video. Copyright © 1997-2007 by RAD Game Tools, Inc.",
    ),
    ("Valve_Hud_HEALTH", "HEALTH"),
    ("Valve_Hud_SUIT", "SUIT"),
    ("Valve_Hud_AMMO", "AMMO"),
    ("Valve_Hud_AMMO_ALT", "ALT"),
];

/// Looks up an authentic English fallback string for the given token name if defined in the central table.
pub fn fallback_lookup(token: &str) -> Option<&'static str> {
    let key = token.trim().trim_start_matches('#');
    FALLBACK_STRINGS
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xfe];
        for unit in text.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn unescape_handles_linebreaks_tabs_quotes_and_preserves_literal_backslashes() {
        assert_eq!(unescape("No escapes here"), "No escapes here");
        assert_eq!(unescape("Line 1\\nLine 2"), "Line 1\nLine 2");
        assert_eq!(unescape("Line 1\\r\\nLine 2"), "Line 1\r\nLine 2");
        assert_eq!(unescape("Col 1\\tCol 2"), "Col 1\tCol 2");
        assert_eq!(unescape("He said \\\"hi\\\""), "He said \"hi\"");
        assert_eq!(unescape("Path\\\\to\\\\file"), "Path\\to\\file");
        // Literal backslashes that are not escape sequences stay intact:
        assert_eq!(unescape("C:\\Games\\Half-Life"), "C:\\Games\\Half-Life");
        assert_eq!(unescape("Trailing backslash\\"), "Trailing backslash\\");
        assert_eq!(unescape("Single \\ in middle"), "Single \\ in middle");
    }

    #[test]
    fn localized_tokens_unescape_line_breaks() {
        let mut loc = Localization::default();
        loc.insert("GameUI_CustomWarning", "Warning line 1\\nWarning line 2");
        assert_eq!(
            loc.text("#GameUI_CustomWarning"),
            "Warning line 1\nWarning line 2"
        );
        let resource =
            b"\"lang\" { english { Tokens { \"GameUI_Multiline\" \"One\\nTwo\\nThree\" } } }";
        loc.add_resource("test.txt", resource);
        assert_eq!(loc.text("#GameUI_Multiline"), "One\nTwo\nThree");
    }

    #[test]
    fn central_fallback_table_resolves_standard_tokens_when_resources_absent() {
        let loc = Localization::default();
        assert_eq!(
            loc.text("#GameUI_ConfirmMainMenu_Title"),
            "RETURN TO MAIN MENU"
        );
        assert_eq!(
            loc.text("#GameUI_MainMenuWarning"),
            "Are you sure?\nAll unsaved progress will be lost."
        );
        assert_eq!(loc.text("#GameUI_Console"), "Console");
        assert_eq!(loc.text("#GameUI_Loading"), "LOADING...");
        assert_eq!(loc.text("#LoadingProgress_LoadMap"), "Loading world...");
        assert_eq!(loc.text("#GameUI_HDR"), "High Dynamic Range");
        assert_eq!(loc.text("#GameUI_hdr_level0"), "None");
        assert_eq!(loc.text("#GameUI_hdr_level1"), "Bloom (if available)");
        assert_eq!(loc.text("#GameUI_hdr_level2"), "Full (if available)");
        assert_eq!(loc.text("#Valve_Hud_HEALTH"), "HEALTH");
        assert_eq!(loc.text("#Valve_Hud_SUIT"), "SUIT");
        assert_eq!(loc.text("#Valve_Hud_AMMO"), "AMMO");
        assert_eq!(loc.text("#Valve_Hud_AMMO_ALT"), "ALT");
    }

    #[test]
    fn loaded_game_resources_take_precedence_over_central_fallbacks() {
        let mut loc = Localization::default();
        loc.insert("GameUI_Console", "Custom Console");
        assert_eq!(loc.text("#GameUI_Console"), "Custom Console");
    }

    #[test]
    fn utf16_gameui_layout_and_merged_layout_both_load() {
        let mut loc = Localization::default();
        let gameui = utf16(
            "\"lang\" { \"Language\" \"English\" \"Tokens\" { \"GameUI_Back\" \"Back\" \"GameUI_Subtitles\" \"Subtitles (dialog only)\" [$WIN32] \"GameUI_Subtitles\" \"Subtitles\" [$X360] } }",
        );
        assert_eq!(loc.add_resource("gameui_english.txt", &gameui), Some(2));
        let merged = b"\"lang\" { english { Tokens { \"HL2_GET_CROWBAR_NAME\" \"Trusty Hardware\" \"GameUI_Back\" \"Other\" } } }";
        assert_eq!(loc.add_resource("merged_loc_all.txt", merged), Some(1));
        assert_eq!(loc.text("#GameUI_Back"), "Back", "first file wins");
        assert_eq!(loc.text("gameui_subtitles"), "Subtitles (dialog only)");
        assert_eq!(loc.text("HL2_GET_CROWBAR_NAME"), "Trusty Hardware");
        assert_eq!(loc.sources().len(), 2);
    }

    #[test]
    fn unresolved_tokens_render_as_their_key_name_never_empty() {
        let mut loc = Localization::default();
        loc.insert("GameUI_Empty", "");
        assert_eq!(loc.text("#GameUI_NoWindowBorder"), "GameUI_NoWindowBorder");
        assert_eq!(loc.text("GameUI_NoWindowBorder"), "GameUI_NoWindowBorder");
        assert_eq!(
            loc.text("#GameUI_Empty"),
            "GameUI_Empty",
            "empty translations fall back too"
        );
        assert_eq!(loc.localize("#Missing_Token"), "Missing_Token");
        assert!(!loc.localize("#Missing_Token").is_empty());
        assert_eq!(loc.localize("1280 x 720"), "1280 x 720");
    }

    #[test]
    fn non_localization_files_are_rejected() {
        let mut loc = Localization::default();
        assert_eq!(loc.add_resource("x", b"\"GameMenu\" { }"), None);
        assert_eq!(loc.add_resource("x", b"{ broken"), None);
        assert!(loc.is_empty());
    }

    #[test]
    fn audit_no_hardcoded_user_visible_strings_in_ui_code_outside_fallback_constants() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let src_dir = manifest_dir.join("src");
        let ui_files = [
            "console.rs",
            "gameui/loading.rs",
            "gameui/menu.rs",
            "gameui/options.rs",
            "gameui/resources.rs",
            "hud.rs",
        ];
        let banned = [
            "Are you sure?\nAll unsaved progress will be lost.",
            "Are you sure you want to quit?",
            "RETURN TO MAIN MENU",
            "QUIT GAME",
            "Starting local game server...",
            "Loading world...",
            "Initializing world...",
            "Initializing game data...",
            "Loading resources...",
            "Initializing resources...",
            "3RD PARTY TECHNOLOGY CREDITS",
            "Reset key bindings to default?",
        ];
        for rel in ui_files {
            let path = src_dir.join(rel);
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            // Check non-test code
            let non_test_content = content.split("#[cfg(test)]").next().unwrap_or(&content);
            for b in banned {
                assert!(
                    !non_test_content.contains(b),
                    "Found banned user-visible literal {:?} in {}",
                    b,
                    path.display()
                );
            }
        }

        // Specifically check that console.rs non-test code does not hardcode "Console" title
        let console_src = std::fs::read_to_string(src_dir.join("console.rs")).unwrap();
        let console_non_test = console_src.split("#[cfg(test)]").next().unwrap();
        assert!(
            !console_non_test.contains("text(\n                \"Console\""),
            "console.rs should not hardcode Console title string"
        );

        // Check that hud.rs non-test code does not hardcode numeric panel labels
        let hud_src = std::fs::read_to_string(src_dir.join("hud.rs")).unwrap();
        let hud_non_test = hud_src.split("#[cfg(test)]").next().unwrap();
        for label in ["\"HEALTH\"", "\"SUIT\"", "\"AMMO\"", "\"ALT\""] {
            assert!(
                !hud_non_test.contains(&format!(
                    "numeric_panel(\n            &self.health_panel,\n            {label}"
                )),
                "hud.rs should not pass literal {label} to numeric_panel"
            );
        }
    }
}
