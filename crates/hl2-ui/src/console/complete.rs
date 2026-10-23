//! Console completion: what the token being typed can become, ranked, and a help line for the
//! command being typed.
//!
//! Completion works on the line's last command (after the last `;` outside quotes). The first
//! word completes from every implemented command and cvar; later words from what that argument
//! takes ([`Arg`]): installed maps, the loaded map's entity names and the inputs its connections
//! fire, command names or fixed values. Matches rank by prefix, then substring, then the
//! letters in order (closest first); the retail console offers prefix matches only.

use super::commands::{self, Arg, Spec, COMMANDS};

/// Host-supplied names that arguments complete from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sources {
    /// Installed map names (all mounted games).
    pub maps: Vec<String>,
    /// Targetnames of the loaded map's entities.
    pub entity_targets: Vec<String>,
    /// Inputs the loaded map's connections fire.
    pub entity_inputs: Vec<String>,
}

/// A completion: the text it becomes and, for commands and cvars, their description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub detail: String,
}

/// The candidates for a line, and the line before the token they replace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completion {
    pub head: String,
    pub items: Vec<Candidate>,
}

/// Byte index where the line's last command starts (after the last `;` outside quotes).
pub fn last_command_start(line: &str) -> usize {
    let mut quoted = false;
    let mut start = 0;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => start = i + 1,
            _ => {}
        }
    }
    start
}

/// The command word and the index of the argument being typed in `command` (one command):
/// `None` while the command word itself is typed.
fn argument(command: &str) -> Option<(&str, usize)> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let ends_space = command.ends_with(char::is_whitespace);
    match (words.first(), words.len()) {
        (None, _) => None,
        (Some(_), 1) if !ends_space => None,
        (Some(name), n) => Some((name, if ends_space { n - 1 } else { n - 2 })),
    }
}

/// Fuzzy rank, lower is better: the exact name, then prefix matches (alphabetical among
/// themselves, as retail lists them), then substring matches by position, then the query's
/// letters in order by the letters skipped; `None` if no match.
pub fn score(name: &str, query: &str) -> Option<i32> {
    let (name, query) = (name.to_lowercase(), query.to_lowercase());
    if query.is_empty() {
        return Some(1000);
    }
    if name == query {
        return Some(0);
    }
    if name.starts_with(&query) {
        return Some(1);
    }
    if let Some(at) = name.find(&query) {
        return Some(100 + at as i32);
    }
    let mut letters = name.chars();
    let mut skipped = 0;
    for wanted in query.chars() {
        loop {
            match letters.next() {
                Some(c) if c == wanted => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
    }
    Some(300 + skipped)
}

fn names(list: &[String]) -> Vec<Candidate> {
    list.iter()
        .map(|name| Candidate {
            text: name.clone(),
            detail: String::new(),
        })
        .collect()
}

fn spec_candidate(spec: &Spec) -> Candidate {
    Candidate {
        text: spec.name.to_owned(),
        detail: commands::usage(spec).1,
    }
}

/// What the token being typed in `line` can become, best first.
pub fn candidates(line: &str, sources: &Sources) -> Completion {
    let start = last_command_start(line);
    let command = &line[start..];
    let lead = command.len() - command.trim_start().len();
    let (head, token, pool) = match argument(command) {
        None => (
            &line[..start + lead],
            command.trim(),
            COMMANDS.iter().map(spec_candidate).collect(),
        ),
        Some((name, index)) => {
            let token = if command.ends_with(char::is_whitespace) {
                ""
            } else {
                command.split_whitespace().last().unwrap_or("")
            };
            let pool = match commands::find(name).and_then(|spec| spec.args.get(index)) {
                Some(Arg::Map) => names(&sources.maps),
                Some(Arg::EntityTarget) => names(&sources.entity_targets),
                Some(Arg::EntityInput) => names(&sources.entity_inputs),
                Some(Arg::Command) => COMMANDS.iter().map(spec_candidate).collect(),
                Some(Arg::Values(values)) => values
                    .iter()
                    .map(|v| Candidate {
                        text: (*v).to_owned(),
                        detail: String::new(),
                    })
                    .collect(),
                Some(Arg::Free) | None => Vec::new(),
            };
            (&line[..line.len() - token.len()], token, pool)
        }
    };
    let mut ranked: Vec<(i32, Candidate)> = pool
        .into_iter()
        .filter_map(|c| score(&c.text, token).map(|s| (s, c)))
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.text.cmp(&b.1.text)));
    ranked.dedup_by(|a, b| a.1.text.eq_ignore_ascii_case(&b.1.text));
    Completion {
        head: head.to_owned(),
        items: ranked.into_iter().map(|(_, c)| c).collect(),
    }
}

/// The help line for the command being typed: its usage with the argument being typed marked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgHelp {
    pub name: &'static str,
    pub args: Vec<String>,
    /// The argument being typed, if the cursor is on one the command takes.
    pub current: Option<usize>,
    pub description: String,
    pub cvar: bool,
}

/// The help line for `line`'s last command, once its command word names an implemented one.
pub fn arg_help(line: &str) -> Option<ArgHelp> {
    let command = &line[last_command_start(line)..];
    let name = command.split_whitespace().next()?;
    let spec = commands::find(name)?;
    let (args, description) = commands::usage(spec);
    let typed = argument(command).map(|(_, index)| index);
    // Past the last argument: still on it when it takes the rest of the line (`echo <text>`).
    let current = typed.and_then(|index| match args.len() {
        0 => None,
        n if index < n => Some(index),
        n if args[n - 1].contains("text") => Some(n - 1),
        _ => None,
    });
    Some(ArgHelp {
        name: spec.name,
        args,
        current,
        description,
        cvar: spec.is_cvar(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(completion: &Completion) -> Vec<&str> {
        completion.items.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn ranking_prefers_prefix_then_substring_then_scattered_letters() {
        assert_eq!(score("map", "map"), Some(0));
        assert!(
            score("map", "map") < score("map_background", "map"),
            "exact first"
        );
        assert_eq!(
            score("map", "ma"),
            score("map_background", "ma"),
            "prefixes tie"
        );
        assert!(score("map_background", "back") > score("background_map", "back"));
        assert_eq!(score("background_map", "back"), Some(1));
        assert!(score("mat_setvideomode", "msv").is_some_and(|s| s >= 300));
        assert_eq!(score("fov", "xyz"), None);
        assert_eq!(score("Volume", "VOL"), Some(1), "case-insensitive");
    }

    #[test]
    fn the_first_word_completes_commands_and_later_words_their_arguments() {
        let sources = Sources {
            maps: vec!["d1_trainstation_01".into(), "background01".into()],
            entity_targets: vec!["kleiner".into(), "lab_door".into()],
            entity_inputs: vec!["Open".into(), "Close".into()],
        };
        let words = candidates("ma", &sources);
        assert_eq!(words.head, "");
        assert_eq!(
            &texts(&words)[..4],
            ["mainmenu", "map", "map_background", "mat_aaquality"]
        );
        assert!(words.items[1].detail.starts_with("load an installed map"));
        let maps = candidates("map d1", &sources);
        assert_eq!(maps.head, "map ");
        assert_eq!(
            texts(&maps),
            ["d1_trainstation_01", "background01"],
            "prefix first, then the letters in order"
        );
        assert_eq!(texts(&candidates("map ", &sources)).len(), 2);
        let targets = candidates("ent_fire lab", &sources);
        assert_eq!(texts(&targets), ["lab_door"]);
        let inputs = candidates("ent_fire lab_door o", &sources);
        assert_eq!(
            (inputs.head.as_str(), texts(&inputs)),
            ("ent_fire lab_door ", vec!["Open", "Close"])
        );
        assert_eq!(
            texts(&candidates("mat_hdr_level ", &sources)),
            ["0", "1", "2"]
        );
        assert!(
            candidates("setpos 1", &sources).items.is_empty(),
            "free argument"
        );
        assert!(candidates("unknown x", &sources).items.is_empty());
    }

    #[test]
    fn completion_follows_the_last_command_outside_quotes() {
        let sources = Sources::default();
        let line = "echo \"a;b\"; sv_ch";
        assert_eq!(last_command_start(line), 11);
        let completion = candidates(line, &sources);
        assert_eq!(completion.head, "echo \"a;b\"; ");
        assert_eq!(texts(&completion)[0], "sv_cheats");
        assert_eq!(texts(&candidates("fov 90;noc", &sources))[0], "noclip");
    }

    #[test]
    fn the_help_line_marks_the_argument_being_typed() {
        let help = arg_help("setpos 1 ").unwrap();
        assert_eq!((help.name, help.current), ("setpos", Some(1)));
        assert_eq!(help.args, ["<x>", "<y>", "[z]"]);
        assert_eq!(
            arg_help("setpos").unwrap().current,
            None,
            "still typing the name"
        );
        assert_eq!(
            arg_help("setpos 1 2 3 4").unwrap().current,
            None,
            "past the last"
        );
        assert_eq!(
            arg_help("echo a b c").unwrap().current,
            Some(0),
            "text takes the rest"
        );
        assert!(arg_help("volume").unwrap().cvar);
        assert_eq!(
            arg_help("fov 90; nocl"),
            None,
            "not a full command name yet"
        );
        assert_eq!(arg_help("nothing here"), None);
    }
}
