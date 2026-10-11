//! Localized GameUI strings from the installed `*_english.txt` / `merged_loc_all.txt` files.
//!
//! Source merges every localization file it loads into one token table; the first file to define
//! a token wins (the search path order). Lookups are case-insensitive and accept an optional
//! leading `#`, matching how `.res` files and code reference tokens.
//!
//! Fallback policy (owner requirement): an unresolved token renders as its **key name** (without
//! the `#`), never as empty text, so missing translations stay visible and diagnosable.

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
                slot.insert(text.to_owned());
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
            .or_insert_with(|| text.to_owned());
    }

    /// The translated text of a token, if the install defines it.
    pub fn lookup(&self, token: &str) -> Option<&str> {
        let key = token.trim().trim_start_matches('#');
        self.tokens
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// The translated text of `token`, else the token's key name (never empty for a non-empty token).
    pub fn text(&self, token: &str) -> String {
        let key = token.trim().trim_start_matches('#');
        match self.lookup(key) {
            Some(text) if !text.is_empty() => text.to_owned(),
            _ => key.to_owned(),
        }
    }

    /// Resolves `#Token` references; other text (e.g. `"1280 x 720"`) is returned unchanged.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xfe];
        for unit in text.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        bytes
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
}
