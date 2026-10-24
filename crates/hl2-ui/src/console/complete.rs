//! HL2's console completion: the toolkit's completion ([`source_ui::console::complete`]) over
//! HL2's commands ([`COMMANDS`]) and the names the host supplies for their arguments (installed
//! maps, the loaded map's entity names and the inputs its connections fire).

use super::commands::{Names, COMMANDS};
use source_ui::console::complete::{self, ArgHelp, Completer, Completion};

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

impl Sources {
    /// The names an argument of kind `names` completes from.
    fn list(&self, names: Names) -> &[String] {
        match names {
            Names::Map => &self.maps,
            Names::EntityTarget => &self.entity_targets,
            Names::EntityInput => &self.entity_inputs,
        }
    }
}

impl Completer for Sources {
    fn complete(&self, line: &str) -> Completion {
        candidates(line, self)
    }
}

/// What the token being typed in `line` can become, best first.
pub fn candidates(line: &str, sources: &Sources) -> Completion {
    complete::candidates(line, COMMANDS, |names| sources.list(names))
}

/// The help line for `line`'s last command, once its command word names an implemented one.
pub fn arg_help(line: &str) -> Option<ArgHelp> {
    complete::arg_help(line, COMMANDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_ui::console::complete::last_command_start;
    use source_ui::console::editor::{LineEditor, HISTORY_MAX};

    fn texts(completion: &Completion) -> Vec<&str> {
        completion.items.iter().map(|c| c.text.as_str()).collect()
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

    // The line editor with HL2's completion.

    fn typed(text: &str) -> LineEditor {
        let mut editor = LineEditor::default();
        editor.insert(text);
        editor.refresh(&Sources::default());
        editor
    }

    #[test]
    fn editing_keys_work_on_characters_and_words() {
        let mut e = typed("ent_fire lab_door Open");
        assert_eq!(e.cursor(), 22);
        e.delete_word_back();
        assert_eq!(e.text(), "ent_fire lab_door ");
        e.word_left();
        assert_eq!(e.cursor(), 9);
        e.delete_word_forward();
        assert_eq!(e.text(), "ent_fire  ");
        e.home();
        e.insert("é");
        assert_eq!((e.text(), e.cursor()), ("éent_fire  ", 1));
        e.right();
        e.kill_to_start();
        assert_eq!((e.text(), e.cursor()), ("nt_fire  ", 0));
        e.word_right();
        e.kill_to_end();
        assert_eq!(e.text(), "nt_fire  ");
        e.end();
        e.backspace();
        assert_eq!(e.text(), "nt_fire ");
        e.home();
        e.delete();
        assert_eq!(e.text(), "t_fire ");
        e.paste("a\r\nb\n");
        assert_eq!(e.text(), "a; bt_fire ");
    }

    #[test]
    fn tab_steps_the_list_and_typing_rebuilds_it() {
        let mut e = typed("ma");
        let (items, index) = e.popup().unwrap();
        assert_eq!((items[0].text.as_str(), index), ("mainmenu", None));
        e.step(false, true);
        assert_eq!(e.text(), "mainmenu");
        e.step(false, true);
        assert_eq!(e.text(), "map");
        e.step(true, true);
        assert_eq!(e.text(), "mainmenu", "Shift+Tab steps back");
        e.step(false, true);
        e.refresh(&Sources::default());
        assert_eq!(e.popup().unwrap().1, Some(1), "stepping keeps the list");
        e.insert(" d");
        e.refresh(&Sources {
            maps: vec!["d1_trainstation_01".into()],
            ..Sources::default()
        });
        e.step(false, true);
        assert_eq!(
            e.text(),
            "map d1_trainstation_01 ",
            "one match: taken with a space"
        );
        assert!(e.popup().is_none());
    }

    #[test]
    fn an_empty_line_steps_through_the_history_and_submit_dedupes() {
        let mut e = LineEditor::default();
        for line in ["fov 90", "map x", "fov 90", "  "] {
            e.insert(line);
            e.submit();
        }
        assert_eq!(
            e.history(),
            ["map x", "fov 90"],
            "repeat moves to the end; blanks skipped"
        );
        assert!(e.take_history_changed());
        assert!(!e.take_history_changed());
        e.step(true, false);
        assert_eq!(e.text(), "fov 90", "Up starts at the newest");
        e.refresh(&Sources::default());
        e.step(true, false);
        assert_eq!(e.text(), "map x");
        let (items, index) = e.popup().unwrap();
        assert_eq!((items.len(), index), (2, Some(0)));
        e.step(false, false);
        assert_eq!(e.text(), "fov 90");
        let mut many = LineEditor::default();
        many.set_history((0..150).map(|i| i.to_string()).collect());
        assert_eq!(
            (many.history().len(), many.history()[0].as_str()),
            (HISTORY_MAX, "50")
        );
    }

    #[test]
    fn escape_ends_a_search_then_hides_the_list() {
        let mut e = LineEditor::default();
        for line in ["volume 0.5", "fov 90", "volume 1"] {
            e.insert(line);
            e.submit();
        }
        e.search_back();
        e.insert("vol");
        assert_eq!(e.search(), Some(("vol", Some("volume 1"))));
        e.search_back();
        assert_eq!(e.search(), Some(("vol", Some("volume 0.5"))));
        e.search_back();
        assert_eq!(
            e.search().unwrap().1,
            Some("volume 0.5"),
            "stays on the oldest match"
        );
        assert!(e.dismiss(), "Escape ends the search");
        assert_eq!(e.text(), "");
        e.search_back();
        e.insert("fov");
        assert_eq!(e.submit(), "fov 90", "Enter runs the match");
        e.insert("vo");
        e.refresh(&Sources::default());
        assert!(e.popup().is_some());
        assert!(e.dismiss(), "Escape hides the list");
        assert!(e.popup().is_none());
        assert!(!e.dismiss(), "then Escape closes the console");
        e.insert("l");
        e.refresh(&Sources::default());
        assert!(e.popup().is_some(), "typing brings it back");
    }
}
