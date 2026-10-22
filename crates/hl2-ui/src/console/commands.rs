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
    command(
        "npc_create",
        "npc_create <class> [name]: place an NPC at the crosshair (cheat; spawning not implemented yet)",
        &[Arg::Free, Arg::Free],
    ),
    command(
        "npc_create_aimed",
        "npc_create_aimed <class> [name]: like npc_create, facing the aim (cheat)",
        &[Arg::Free, Arg::Free],
    ),
    command(
        "npc_create_equipment",
        "npc_create_equipment <weapon>: weapon for the next npc_create",
        &[Arg::Free],
    ),
    command(
        "prop_physics_create",
        "prop_physics_create <model>: physics prop at the crosshair, model relative to models/ (cheat)",
        &[Arg::Free],
    ),
    command(
        "ent_create",
        "ent_create <class> [key value ...]: create an entity at the crosshair; NPC classes and prop_physics with a model only (cheat)",
        &[Arg::Free, Arg::Free],
    ),
    command(
        "ai_disable",
        "ai_disable: toggle all NPC AI off/on; disabled NPCs idle (cheat, Source command)",
        &[],
    ),
    command(
        "ai_enable",
        "ai_enable [0|1]: switch the NPC AI think loop (ours, default 1; Source uses ai_disable)",
        BOOLEAN,
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
        "snd_musicvolume",
        "snd_musicvolume [0..1]: query or set music volume",
        &[Arg::Free],
    ),
    cvar(
        "snd_mute_losefocus",
        "snd_mute_losefocus [0|1]: query or toggle muting audio when window loses focus",
        BOOLEAN,
    ),
    cvar(
        "closecaption",
        "closecaption [0|1]: query or toggle closed captioning",
        BOOLEAN,
    ),
    cvar(
        "cc_subtitles",
        "cc_subtitles [0|1]: query or toggle subtitles only (dialogue only, no sound effects)",
        BOOLEAN,
    ),
    cvar(
        "snd_surround_speakers",
        "snd_surround_speakers [0|2|4|5|7]: query or set speaker configuration (0=headphones, 2=stereo, 4=surround, 5=5.1, 7=7.1)",
        &[Arg::Values(&["0", "2", "4", "5", "7"])],
    ),
    cvar(
        "snd_pitchquality",
        "snd_pitchquality [0|1]: query or toggle sound pitch quality (0=low, 1=high)",
        BOOLEAN,
    ),
    cvar(
        "dsp_slow_cpu",
        "dsp_slow_cpu [0|1]: query or toggle reduced DSP CPU quality (1=slow CPU/low, 0=normal/high)",
        BOOLEAN,
    ),
    cvar(
        "sensitivity",
        "sensitivity [value]: query or set mouse sensitivity",
        &[Arg::Free],
    ),
    cvar(
        "m_pitch",
        "m_pitch [value]: query or set mouse pitch factor (negative inverts pitch)",
        &[Arg::Free],
    ),
    cvar(
        "m_filter",
        "m_filter [0|1]: query or toggle two-frame mouse movement averaging",
        BOOLEAN,
    ),
    cvar(
        "m_rawinput",
        "m_rawinput [0|1]: query or toggle raw mouse input",
        BOOLEAN,
    ),
    cvar(
        "m_customaccel",
        "m_customaccel [0|1|3]: query or toggle custom mouse acceleration",
        &[Arg::Values(&["0", "1", "3"])],
    ),
    cvar(
        "m_customaccel_exponent",
        "m_customaccel_exponent [value]: query or set mouse acceleration exponent",
        &[Arg::Free],
    ),
    cvar(
        "sk_autoaim_mode",
        "sk_autoaim_mode [0|1|2]: query or set autoaim mode (0=off, 1=basic, 2=enhanced)",
        &[Arg::Values(&["0", "1", "2"])],
    ),
    cvar(
        "m_autoaim_mode",
        "m_autoaim_mode [0|1|2]: query or set autoaim mode (alias for sk_autoaim_mode)",
        &[Arg::Values(&["0", "1", "2"])],
    ),
    cvar(
        "sk_autoaim_scale1",
        "sk_autoaim_scale1 [scale]: query or set autoaim deflection scale",
        &[Arg::Free],
    ),
    cvar(
        "m_autoaim_scale",
        "m_autoaim_scale [scale]: query or set autoaim deflection scale (alias for sk_autoaim_scale1)",
        &[Arg::Free],
    ),
    cvar(
        "fov",
        "fov [degrees]: query or set camera field of view",
        &[Arg::Free],
    ),
    cvar(
        "fov_desired",
        "fov_desired [degrees]: query or set desired field of view (alias for fov)",
        &[Arg::Free],
    ),
    cvar(
        "vertical_fov",
        "vertical_fov [degrees]: query or set vertical field of view (0 = automatic)",
        &[Arg::Free],
    ),
    cvar(
        "viewmodel_fov",
        "viewmodel_fov [degrees]: query or set viewmodel field of view",
        &[Arg::Free],
    ),
    cvar(
        "mat_colorcorrection",
        "mat_colorcorrection [0|1]: query or toggle color correction",
        BOOLEAN,
    ),
    cvar(
        "mat_hdr_level",
        "mat_hdr_level [0|1|2]: query or set HDR level (0=None, 1=Bloom, 2=Full)",
        &[Arg::Values(&["0", "1", "2"])],
    ),
    cvar(
        "mat_motion_blur_enabled",
        "mat_motion_blur_enabled [0|1]: query or toggle motion blur",
        BOOLEAN,
    ),
    cvar(
        "mat_queue_mode",
        "mat_queue_mode [-1|0|1|2]: query or set multicore rendering (-1=multicore, 0=single threaded)",
        &[Arg::Values(&["-1", "0", "1", "2"])],
    ),
    cvar(
        "mat_vsync",
        "mat_vsync [0|1]: query or toggle vertical sync",
        BOOLEAN,
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
