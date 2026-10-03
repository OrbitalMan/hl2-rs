//! Valve KeyValues tokenization. Preserves duplicate entity output keys.
use anyhow::{bail, Context, Result};
use modkit_core::Entity;

#[derive(Clone, Debug)]
pub enum Value {
    Text(String),
    Block(Vec<Entry>),
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub key: String,
    pub value: Value,
}
impl Entry {
    pub fn text(&self) -> Option<&str> {
        if let Value::Text(s) = &self.value {
            Some(s)
        } else {
            None
        }
    }
    pub fn children(&self) -> &[Entry] {
        if let Value::Block(v) = &self.value {
            v
        } else {
            &[]
        }
    }
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.children()
            .iter()
            .find(|e| e.key.eq_ignore_ascii_case(name))
    }
}
/// Nested scripts retain duplicate keys, including all rndwave alternatives.
pub fn parse(text: &str) -> Result<Vec<Entry>> {
    fn block(t: &[String], i: &mut usize, depth: usize) -> Result<Vec<Entry>> {
        if depth > 64 {
            bail!("KeyValues nesting limit");
        }
        let mut out = Vec::new();
        while *i < t.len() && t[*i] != "}" {
            let key = t[*i].clone();
            *i += 1;
            if key == "{" || *i >= t.len() || t[*i] == "}" {
                bail!("invalid KeyValues entry");
            }
            let value = if t[*i] == "{" {
                *i += 1;
                let nested = block(t, i, depth + 1)?;
                if t.get(*i).map(String::as_str) != Some("}") {
                    bail!("unclosed KeyValues block");
                }
                *i += 1;
                Value::Block(nested)
            } else {
                let v = Value::Text(t[*i].clone());
                *i += 1;
                v
            };
            out.push(Entry { key, value });
        }
        Ok(out)
    }
    let t = tokens(text)?;
    let mut i = 0;
    let result = block(&t, &mut i, 0)?;
    if i != t.len() {
        bail!("unexpected KeyValues closing brace");
    }
    Ok(result)
}

/// Source scheme/localization resources may use UTF-16 with a byte-order mark.
/// Invalid encoding is reported rather than silently dropping an incomplete code unit.
pub fn decode_text(data: &[u8]) -> Result<String> {
    let decoded = if data.starts_with(&[0xff, 0xfe]) || data.starts_with(&[0xfe, 0xff]) {
        let (pairs, remainder) = data[2..].as_chunks::<2>();
        if !remainder.is_empty() {
            bail!("truncated UTF-16 resource code unit");
        }
        let little = data[0] == 0xff;
        let units = pairs
            .iter()
            .map(|p| {
                if little {
                    u16::from_le_bytes(*p)
                } else {
                    u16::from_be_bytes(*p)
                }
            })
            .collect::<Vec<_>>();
        String::from_utf16(&units).context("invalid UTF-16 resource text")?
    } else {
        std::str::from_utf8(data)
            .context("invalid UTF-8 resource text")?
            .trim_start_matches('\u{feff}')
            .to_owned()
    };
    Ok(decoded.trim_end_matches('\0').to_owned())
}

/// Published KeyValues::EvaluateConditional platform semantics for a desktop host.
/// `$WIN32` historically means IsPC, whereas `$WINDOWS` tests Windows itself.
/// `$DECK`/`$X360` are false here; unknown conditions stay false even when negated.
/// Source checks named platforms in order rather than evaluating a boolean grammar.
pub fn resource_condition(token: &str) -> Result<bool> {
    let token = token.trim();
    let expression = token
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .context("invalid KeyValues resource condition")?;
    let negate = expression.starts_with('!');
    let upper = expression.to_ascii_uppercase();
    let value = if upper.contains("$DECK") || upper.contains("$X360") {
        false
    } else if upper.contains("$WIN32") {
        true
    } else if upper.contains("$WINDOWS") {
        cfg!(target_os = "windows")
    } else if upper.contains("$OSX") {
        cfg!(target_os = "macos")
    } else if upper.contains("$LINUX") {
        cfg!(target_os = "linux")
    } else if upper.contains("$POSIX") {
        cfg!(unix)
    } else {
        return Ok(false);
    };
    Ok(value != negate)
}

/// Decode a resource and evaluate conditions before blocks or after values/blocks.
/// Duplicate active keys are preserved, including numbered font resolution ranges.
/// `#include` and `#base` remain entries; resolving their paths belongs to the VFS caller.
pub fn parse_resource(data: &[u8]) -> Result<Vec<Entry>> {
    fn block(t: &[Token], i: &mut usize, depth: usize) -> Result<Vec<Entry>> {
        if depth > 64 {
            bail!("KeyValues resource nesting limit");
        }
        let mut entries = Vec::new();
        while *i < t.len() && !t[*i].control("}") {
            let key = &t[*i];
            if key.control("{") || key.condition() {
                bail!("invalid KeyValues resource key");
            }
            let key = key.value.clone();
            *i += 1;
            let mut keep = true;
            while t.get(*i).is_some_and(Token::condition) {
                keep &= resource_condition(&t[*i].value)?;
                *i += 1;
            }
            let token = t.get(*i).context("missing KeyValues resource value")?;
            let value = if token.control("{") {
                *i += 1;
                let children = block(t, i, depth + 1)?;
                if !t.get(*i).is_some_and(|v| v.control("}")) {
                    bail!("unclosed KeyValues resource block");
                }
                *i += 1;
                Value::Block(children)
            } else {
                if token.control("}") {
                    bail!("invalid KeyValues resource value");
                }
                *i += 1;
                Value::Text(token.value.clone())
            };
            while t.get(*i).is_some_and(Token::condition) {
                keep &= resource_condition(&t[*i].value)?;
                *i += 1;
            }
            if keep {
                entries.push(Entry { key, value });
            }
        }
        Ok(entries)
    }
    let text = decode_text(data)?;
    let t = lexical_tokens(&text)?;
    let mut i = 0;
    let entries = block(&t, &mut i, 0)?;
    if i != t.len() {
        bail!("unexpected KeyValues resource closing brace");
    }
    Ok(entries)
}

struct Token {
    value: String,
    quoted: bool,
}
impl Token {
    fn control(&self, value: &str) -> bool {
        !self.quoted && self.value == value
    }
    fn condition(&self) -> bool {
        !self.quoted && self.value.starts_with('[')
    }
}

pub fn tokens(text: &str) -> Result<Vec<String>> {
    Ok(lexical_tokens(text)?.into_iter().map(|t| t.value).collect())
}
fn lexical_tokens(text: &str) -> Result<Vec<Token>> {
    let mut chars = text.trim_end_matches('\0').chars().peekable();
    let mut out = Vec::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for v in chars.by_ref() {
                if v == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '{' || c == '}' {
            out.push(Token {
                value: c.to_string(),
                quoted: false,
            });
            continue;
        }
        let mut s = String::new();
        if c == '"' {
            let mut closed = false;
            while let Some(v) = chars.next() {
                if v == '"' {
                    closed = true;
                    break;
                }
                if v == '\\' {
                    match chars.peek() {
                        Some('"') | Some('\\') => {
                            s.push(chars.next().unwrap());
                        }
                        _ => s.push(v),
                    }
                } else {
                    s.push(v);
                }
            }
            if !closed {
                bail!("unterminated quoted KeyValues string");
            }
        } else {
            s.push(c);
            while let Some(&v) = chars.peek() {
                if v.is_whitespace() || v == '{' || v == '}' {
                    break;
                }
                s.push(chars.next().unwrap());
            }
        }
        out.push(Token {
            value: s,
            quoted: c == '"',
        });
    }
    Ok(out)
}
pub fn entities(data: &[u8]) -> Result<Vec<Entity>> {
    let text = std::str::from_utf8(data)?.trim_end_matches('\0');
    let t = tokens(text)?;
    let mut i = 0;
    let mut out = Vec::new();
    while i < t.len() {
        if t[i] != "{" {
            bail!("expected entity opening brace");
        }
        i += 1;
        let mut p = Vec::new();
        while i < t.len() && t[i] != "}" {
            if i + 1 >= t.len() || t[i + 1] == "}" || t[i] == "{" {
                bail!("invalid entity key/value");
            }
            p.push((t[i].clone(), t[i + 1].clone()));
            i += 2;
        }
        if i >= t.len() {
            bail!("unclosed entity");
        }
        i += 1;
        out.push(Entity { properties: p });
    }
    Ok(out)
}
/// Base-texture lookup also accepts unquoted VMT keys. Patch includes are resolved by the VFS.
pub fn value(text: &str, key: &str) -> Result<Option<String>> {
    let t = tokens(text)?;
    Ok(t.windows(2)
        .find(|p| p[0].eq_ignore_ascii_case(key) && p[1] != "{" && p[1] != "}")
        .map(|p| p[1].clone()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_duplicates_and_slashes() {
        let e=entities(b"{\"classname\" \"logic_relay\" // hi\n\"OnTrigger\" \"a\" \"OnTrigger\" \"b\" \"model\" \"models\\props\\foo.mdl\"}\0").unwrap();
        assert_eq!(e[0].properties.len(), 4);
        assert_eq!(e[0].get("model"), Some("models\\props\\foo.mdl"));
    }
    #[test]
    fn nested_sound_variants_are_retained() {
        let entries =
            parse("Hit { channel CHAN_WEAPON rndwave { wave a.wav wave b.wav } }").unwrap();
        let waves = entries[0].get("rndwave").unwrap().children();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[1].text(), Some("b.wav"));
        assert!(parse("Hit { wave a.wav").is_err());
    }
    #[test]
    fn malformed_is_error() {
        assert!(entities(b"{\"x\"}").is_err());
        assert!(tokens("\"unterminated").is_err());
    }

    #[test]
    fn resource_conditions_select_desktop_variants_and_preserve_duplicate_ranges() {
        let entries = parse_resource(
            br#"Scheme {
            Fonts {
                Numbers { "1" { name HalfLife2 tall 70 [$DECK] tall 64 [!$DECK] } }
                Text { "1" { tall 8 yres "1 599" } "2" { tall 10 yres "600 767" } }
            }
            Hud [$DECK] { wide 200 }
            Hud [!$DECK] { wide 112 }
            Color "255 220 0 255" [$WIN32]
            Color "255 255 255 255" [$X360]
        }"#,
        )
        .unwrap();
        let scheme = &entries[0];
        assert_eq!(
            scheme.get("Hud").unwrap().get("wide").and_then(Entry::text),
            Some("112")
        );
        assert_eq!(
            scheme.get("Color").and_then(Entry::text),
            Some("255 220 0 255")
        );
        let fonts = scheme.get("Fonts").unwrap();
        assert_eq!(
            fonts.get("Numbers").unwrap().children()[0]
                .get("tall")
                .and_then(Entry::text),
            Some("64")
        );
        assert_eq!(fonts.get("Text").unwrap().children().len(), 2);
        assert!(resource_condition("[$WIN32]").unwrap());
        assert!(!resource_condition("[!$UNKNOWN]").unwrap());
        assert_eq!(
            resource_condition("[$WINDOWS]").unwrap(),
            cfg!(target_os = "windows")
        );
        assert_eq!(resource_condition("[$POSIX]").unwrap(), cfg!(unix));
    }

    #[test]
    fn resources_keep_quoted_brackets_and_braces_as_text() {
        let entries = parse_resource(br#"Root { literal "[$DECK]" brace "}" "{" value }"#).unwrap();
        assert_eq!(
            entries[0].get("literal").and_then(Entry::text),
            Some("[$DECK]")
        );
        assert_eq!(entries[0].get("brace").and_then(Entry::text), Some("}"));
        assert_eq!(entries[0].get("{").and_then(Entry::text), Some("value"));
    }

    #[test]
    fn utf16_resources_and_utf8_boms_preserve_non_ascii_localization() {
        let text = "lang { Tokens { HL2_Example \"Prisión 🦀\" } }\0";
        for little in [true, false] {
            let mut data = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for value in text.encode_utf16() {
                data.extend(if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            }
            let entries = parse_resource(&data).unwrap();
            assert_eq!(
                entries[0]
                    .get("Tokens")
                    .unwrap()
                    .get("HL2_Example")
                    .and_then(Entry::text),
                Some("Prisión 🦀")
            );
        }
        assert_eq!(decode_text(b"\xef\xbb\xbfHL2\0").unwrap(), "HL2");
        assert!(decode_text(&[0xff, 0xfe, 0x61]).is_err());
        assert!(decode_text(&[0xff, 0xfe, 0, 0xd8]).is_err());
        assert!(decode_text(&[0xff]).is_err());
    }

    #[test]
    fn malformed_resources_fail_even_in_excluded_platform_blocks() {
        for text in [
            "Root { key value",
            "Root { key }",
            "Root [$DECK] { key }",
            "Root { key value [$WIN32 }",
            "Root { } }",
        ] {
            assert!(parse_resource(text.as_bytes()).is_err(), "{text}");
        }
        let nested = "Root { ".repeat(66) + &"}".repeat(66);
        assert!(parse_resource(nested.as_bytes()).is_err());
    }
}
