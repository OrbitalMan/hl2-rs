//! The console's implemented commands and cvars, described once: name, help text (`name args:
//! description`, the usage line `help` prints and the completion help line marks the argument
//! being typed in) and what each argument completes from.

/// What an argument completes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg {
    /// An installed map name.
    Map,
    /// A targetname of the loaded map's entities.
    EntityTarget,
    /// An input the loaded map's own connections fire.
    EntityInput,
    /// An implemented command or cvar name.
    Command,
    /// One of these literal values.
    Values(&'static [&'static str]),
    /// Free text or a number: nothing to complete.
    Free,
}

/// An implemented command or cvar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub name: &'static str,
    /// `name args: description`, as `help` prints it.
    pub help: &'static str,
    /// Completion for each argument, in order.
    pub args: &'static [Arg],
    /// A cvar: completion shows its current value.
    pub cvar: bool,
}

const fn command(name: &'static str, help: &'static str, args: &'static [Arg]) -> Spec {
    Spec {
        name,
        help,
        args,
        cvar: false,
    }
}

const fn cvar(name: &'static str, help: &'static str, args: &'static [Arg]) -> Spec {
    Spec {
        name,
        help,
        args,
        cvar: true,
    }
}

const BOOLEAN: &[Arg] = &[Arg::Values(&["0", "1"])];

pub const COMMANDS: &[Spec] = &[
    command(
        "help",
        "help [command]: describe implemented commands",
        &[Arg::Command],
    ),
    command(
        "find",
        "find <text>: search implemented command names and descriptions",
        &[Arg::Free],
    ),
    cvar(
        "sv_cheats",
        "sv_cheats [0|1]: query or enable local cheat commands",
        BOOLEAN,
    ),
    command(
        "impulse",
        "impulse 101: grant the six implemented weapons, ammunition and suit (cheat)",
        &[Arg::Values(&["101"])],
    ),
    command(
        "noclip",
        "noclip [0|1]: toggle collision-free flight (cheat)",
        BOOLEAN,
    ),
    command(
        "getpos",
        "getpos: print camera position and Source pitch/yaw/roll in degrees; getpos 2 is not implemented",
        &[],
    ),
    command(
        "setpos",
        "setpos <x> <y> [z]: move the player origin, preserving height if z is omitted (cheat)",
        &[Arg::Free, Arg::Free, Arg::Free],
    ),
    command(
        "setang",
        "setang <pitch> <yaw> [roll]: set Source view angles; roll must be zero (cheat)",
        &[Arg::Free, Arg::Free, Arg::Free],
    ),
    command(
        "map",
        "map <name>: load an installed map and reset the player inventory",
        &[Arg::Map],
    ),
    command("clear", "clear: clear console output", &[]),
    command(
        "ent_fire",
        "ent_fire <targetname> [input=Use] [parameter] [delay]: queue entity input; names/globs supported (cheat)",
        &[Arg::EntityTarget, Arg::EntityInput, Arg::Free, Arg::Free],
    ),
    command("echo", "echo <text>: print text", &[Arg::Free]),
    command(
        "toggleconsole",
        "toggleconsole: close or open the console",
        &[],
    ),
    command("quit", "quit: exit and save the runtime report", &[]),
    cvar(
        "volume",
        "volume [0..1]: query or set master audio volume",
        &[Arg::Free],
    ),
    cvar(
        "sensitivity",
        "sensitivity [value]: query or set mouse sensitivity",
        &[Arg::Free],
    ),
    cvar(
        "cl_software_cursor",
        crate::gameui::localize::placeholders::CMD_CL_SOFTWARE_CURSOR_HELP,
        BOOLEAN,
    ),
    cvar(
        "fov",
        "fov [degrees]: query or set camera field of view",
        &[Arg::Free],
    ),
    cvar(
        "mat_hdr_level",
        "mat_hdr_level [0|1|2]: query or set HDR level (0=None, 1=Bloom, 2=Full)",
        &[Arg::Values(&["0", "1", "2"])],
    ),
    command(
        "mat_setvideomode",
        "mat_setvideomode <width> <height> <windowed 0|1>: set the video mode",
        &[Arg::Free, Arg::Free, Arg::Values(&["0", "1"])],
    ),
    command("mainmenu", "mainmenu: show the main menu", &[]),
    cvar(
        "background_map",
        "background_map [name|default]: query or set the persistent main-menu background map",
        &[Arg::Map],
    ),
    command(
        "map_background",
        "map_background <name>: load an installed map as the main-menu background and persist it",
        &[Arg::Map],
    ),
    cvar(
        "con_filter_enable",
        "con_filter_enable [0|1|2]: filter console output by con_filter_text (1 hides other lines, 2 shows matches brighter)",
        &[Arg::Values(&["0", "1", "2"])],
    ),
    cvar(
        "con_filter_text",
        "con_filter_text [text]: the text con_filter_enable filters console output by",
        &[Arg::Free],
    ),
    cvar(
        "con_timestamp",
        "con_timestamp [0|1]: prefix console output with the time it was printed",
        BOOLEAN,
    ),
];

/// The implemented command or cvar named `name` (case-insensitive).
pub fn find(name: &str) -> Option<&'static Spec> {
    COMMANDS.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// A help text's argument words (after the name, before the `:`) and its description.
pub fn usage(spec: &Spec) -> (Vec<String>, String) {
    let (head, description) = spec.help.split_once(": ").unwrap_or((spec.help, ""));
    let args = head.split_whitespace().skip(1).map(str::to_owned).collect();
    (args, description.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_is_unique_and_its_help_starts_with_its_name() {
        for (i, spec) in COMMANDS.iter().enumerate() {
            assert!(spec.help.starts_with(spec.name), "{}", spec.name);
            assert!(
                COMMANDS[i + 1..]
                    .iter()
                    .all(|other| other.name != spec.name),
                "duplicate {}",
                spec.name
            );
        }
        assert_eq!(find("ENT_FIRE").map(|s| s.name), Some("ent_fire"));
    }

    #[test]
    fn usage_splits_arguments_from_the_description() {
        let (args, description) = usage(find("setpos").unwrap());
        assert_eq!(args, ["<x>", "<y>", "[z]"]);
        assert!(description.starts_with("move the player origin"));
        let (args, _) = usage(find("clear").unwrap());
        assert!(args.is_empty());
    }
}
