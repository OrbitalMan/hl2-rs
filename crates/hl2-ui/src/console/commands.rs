//! The console's implemented commands and cvars, described once: name, help text (`name args:
//! description`, the usage line `help` prints and the completion help line marks the argument
//! being typed in), what each argument completes from, the handler that runs it and, for a
//! cvar, its current value. The handlers live in submodules by area.

mod audio;
mod cheats;
mod developer;
mod map;
mod mouse;
mod video;

use super::{Console, Effect};

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

/// What a command returns: an effect for the host, or the error line the console prints.
pub type Outcome = Result<Option<Effect>, String>;

/// Runs a command with its arguments (the words after its name).
pub type Handler = fn(&mut Console, &'static Spec, &[String]) -> Outcome;

/// A cvar's current value, as the console prints it (`None` while unset).
pub type Value = fn(&Console) -> Option<String>;

/// An implemented command or cvar.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub name: &'static str,
    /// `name args: description`, as `help` prints it.
    pub help: &'static str,
    /// Completion for each argument, in order.
    pub args: &'static [Arg],
    pub run: Handler,
    /// A cvar's current value: completion shows it.
    pub value: Option<Value>,
}

impl Spec {
    /// A cvar: completion shows its current value.
    pub fn is_cvar(&self) -> bool {
        self.value.is_some()
    }

    /// The error line for wrong arguments: the help text.
    pub fn usage(&self) -> String {
        self.help.to_string()
    }
}

const fn command(
    name: &'static str,
    help: &'static str,
    args: &'static [Arg],
    run: Handler,
) -> Spec {
    Spec {
        name,
        help,
        args,
        run,
        value: None,
    }
}

const fn cvar(
    name: &'static str,
    help: &'static str,
    args: &'static [Arg],
    run: Handler,
    value: Value,
) -> Spec {
    Spec {
        name,
        help,
        args,
        run,
        value: Some(value),
    }
}

const BOOLEAN: &[Arg] = &[Arg::Values(&["0", "1"])];

pub const COMMANDS: &[Spec] = &[
    command(
        "help",
        "help [command]: describe implemented commands",
        &[Arg::Command],
        developer::help,
    ),
    command(
        "find",
        "find <text>: search implemented command names and descriptions",
        &[Arg::Free],
        developer::find,
    ),
    cvar(
        "sv_cheats",
        "sv_cheats [0|1]: query or enable local cheat commands",
        BOOLEAN,
        cheats::sv_cheats,
        cheats::sv_cheats_value,
    ),
    command(
        "impulse",
        "impulse 101: grant the six implemented weapons, ammunition and suit (cheat)",
        &[Arg::Values(&["101"])],
        cheats::impulse,
    ),
    command(
        "noclip",
        "noclip [0|1]: toggle collision-free flight (cheat)",
        BOOLEAN,
        cheats::noclip,
    ),
    command(
        "getpos",
        "getpos: print camera position and Source pitch/yaw/roll in degrees; getpos 2 is not implemented",
        &[],
        cheats::getpos,
    ),
    command(
        "setpos",
        "setpos <x> <y> [z]: move the player origin, preserving height if z is omitted (cheat)",
        &[Arg::Free, Arg::Free, Arg::Free],
        cheats::setpos,
    ),
    command(
        "setang",
        "setang <pitch> <yaw> [roll]: set Source view angles; roll must be zero (cheat)",
        &[Arg::Free, Arg::Free, Arg::Free],
        cheats::setang,
    ),
    command(
        "map",
        "map <name>: load an installed map and reset the player inventory",
        &[Arg::Map],
        map::map,
    ),
    command(
        "npc_create",
        "npc_create <class> [name]: place an NPC at the crosshair (cheat; spawning not implemented yet)",
        &[Arg::Free, Arg::Free],
        cheats::create,
    ),
    command(
        "npc_create_aimed",
        "npc_create_aimed <class> [name]: like npc_create, facing the aim (cheat)",
        &[Arg::Free, Arg::Free],
        cheats::create,
    ),
    command(
        "npc_create_equipment",
        "npc_create_equipment <weapon>: weapon for the next npc_create",
        &[Arg::Free],
        cheats::npc_create_equipment,
    ),
    command(
        "prop_physics_create",
        "prop_physics_create <model>: physics prop at the crosshair, model relative to models/ (cheat)",
        &[Arg::Free],
        cheats::create,
    ),
    command(
        "ent_create",
        "ent_create <class> [key value ...]: create an entity at the crosshair; NPC classes and prop_physics with a model only (cheat)",
        &[Arg::Free, Arg::Free],
        cheats::create,
    ),
    command(
        "ai_disable",
        "ai_disable: toggle all NPC AI off/on; disabled NPCs idle (cheat, Source command)",
        &[],
        cheats::ai_disable,
    ),
    command(
        "ai_enable",
        "ai_enable [0|1]: switch the NPC AI think loop (ours, default 1; Source uses ai_disable)",
        BOOLEAN,
        cheats::ai_enable,
    ),
    command(
        "clear",
        "clear: clear console output",
        &[],
        developer::clear,
    ),
    command(
        "ent_fire",
        "ent_fire <targetname> [input=Use] [parameter] [delay]: queue entity input; names/globs supported (cheat)",
        &[Arg::EntityTarget, Arg::EntityInput, Arg::Free, Arg::Free],
        cheats::ent_fire,
    ),
    command(
        "echo",
        "echo <text>: print text",
        &[Arg::Free],
        developer::echo,
    ),
    command(
        "toggleconsole",
        "toggleconsole: close or open the console",
        &[],
        developer::toggleconsole,
    ),
    command(
        "quit",
        "quit: exit and save the runtime report",
        &[],
        developer::quit,
    ),
    cvar(
        "volume",
        "volume [0..1]: query or set master audio volume",
        &[Arg::Free],
        audio::volume,
        audio::volume_value,
    ),
    cvar(
        "snd_musicvolume",
        "snd_musicvolume [0..1]: query or set music volume",
        &[Arg::Free],
        audio::snd_musicvolume,
        audio::snd_musicvolume_value,
    ),
    cvar(
        "snd_mute_losefocus",
        "snd_mute_losefocus [0|1]: query or toggle muting audio when window loses focus",
        BOOLEAN,
        audio::snd_mute_losefocus,
        audio::snd_mute_losefocus_value,
    ),
    cvar(
        "closecaption",
        "closecaption [0|1]: query or toggle closed captioning",
        BOOLEAN,
        audio::closecaption,
        audio::closecaption_value,
    ),
    cvar(
        "cc_subtitles",
        "cc_subtitles [0|1]: query or toggle subtitles only (dialogue only, no sound effects)",
        BOOLEAN,
        audio::cc_subtitles,
        audio::cc_subtitles_value,
    ),
    cvar(
        "snd_surround_speakers",
        "snd_surround_speakers [0|2|4|5|7]: query or set speaker configuration (0=headphones, 2=stereo, 4=surround, 5=5.1, 7=7.1)",
        &[Arg::Values(&["0", "2", "4", "5", "7"])],
        audio::snd_surround_speakers,
        audio::snd_surround_speakers_value,
    ),
    cvar(
        "snd_pitchquality",
        "snd_pitchquality [0|1]: query or toggle sound pitch quality (0=low, 1=high)",
        BOOLEAN,
        audio::snd_pitchquality,
        audio::snd_pitchquality_value,
    ),
    cvar(
        "dsp_slow_cpu",
        "dsp_slow_cpu [0|1]: query or toggle reduced DSP CPU quality (1=slow CPU/low, 0=normal/high)",
        BOOLEAN,
        audio::dsp_slow_cpu,
        audio::dsp_slow_cpu_value,
    ),
    cvar(
        "sensitivity",
        "sensitivity [value]: query or set mouse sensitivity",
        &[Arg::Free],
        mouse::sensitivity,
        mouse::sensitivity_value,
    ),
    cvar(
        "m_pitch",
        "m_pitch [value]: query or set mouse pitch factor (negative inverts pitch)",
        &[Arg::Free],
        mouse::m_pitch,
        mouse::m_pitch_value,
    ),
    cvar(
        "m_filter",
        "m_filter [0|1]: query or toggle two-frame mouse movement averaging",
        BOOLEAN,
        mouse::m_filter,
        mouse::m_filter_value,
    ),
    cvar(
        "m_rawinput",
        "m_rawinput [0|1]: query or toggle raw mouse input",
        BOOLEAN,
        mouse::m_rawinput,
        mouse::m_rawinput_value,
    ),
    cvar(
        "m_customaccel",
        "m_customaccel [0|1|3]: query or toggle custom mouse acceleration",
        &[Arg::Values(&["0", "1", "3"])],
        mouse::m_customaccel,
        mouse::m_customaccel_value,
    ),
    cvar(
        "m_customaccel_exponent",
        "m_customaccel_exponent [value]: query or set mouse acceleration exponent",
        &[Arg::Free],
        mouse::m_customaccel_exponent,
        mouse::m_customaccel_exponent_value,
    ),
    cvar(
        "sk_autoaim_mode",
        "sk_autoaim_mode [0|1|2]: query or set autoaim mode (0=off, 1=basic, 2=enhanced)",
        &[Arg::Values(&["0", "1", "2"])],
        mouse::autoaim_mode,
        mouse::autoaim_mode_value,
    ),
    cvar(
        "m_autoaim_mode",
        "m_autoaim_mode [0|1|2]: query or set autoaim mode (alias for sk_autoaim_mode)",
        &[Arg::Values(&["0", "1", "2"])],
        mouse::autoaim_mode,
        mouse::autoaim_mode_value,
    ),
    cvar(
        "sk_autoaim_scale1",
        "sk_autoaim_scale1 [scale]: query or set autoaim deflection scale",
        &[Arg::Free],
        mouse::autoaim_scale,
        mouse::autoaim_scale_value,
    ),
    cvar(
        "m_autoaim_scale",
        "m_autoaim_scale [scale]: query or set autoaim deflection scale (alias for sk_autoaim_scale1)",
        &[Arg::Free],
        mouse::autoaim_scale,
        mouse::autoaim_scale_value,
    ),
    cvar(
        "fov",
        "fov [degrees]: query or set camera field of view",
        &[Arg::Free],
        video::fov,
        video::fov_value,
    ),
    cvar(
        "fov_desired",
        "fov_desired [degrees]: query or set desired field of view (alias for fov)",
        &[Arg::Free],
        video::fov,
        video::fov_value,
    ),
    cvar(
        "vertical_fov",
        "vertical_fov [degrees]: query or set vertical field of view (0 = automatic)",
        &[Arg::Free],
        video::vertical_fov,
        video::vertical_fov_value,
    ),
    cvar(
        "viewmodel_fov",
        "viewmodel_fov [degrees]: query or set viewmodel field of view",
        &[Arg::Free],
        video::viewmodel_fov,
        video::viewmodel_fov_value,
    ),
    cvar(
        "mat_aaquality",
        "mat_aaquality [quality]: query or set antialiasing quality",
        &[Arg::Values(&["0"])],
        video::mat_aaquality,
        video::mat_aaquality_value,
    ),
    cvar(
        "mat_antialias",
        "mat_antialias [0|1|2|4|8]: query or set multisample antialiasing mode",
        &[Arg::Values(&["0", "1", "2", "4", "8"])],
        video::mat_antialias,
        video::mat_antialias_value,
    ),
    cvar(
        "mat_colorcorrection",
        "mat_colorcorrection [0|1]: query or toggle color correction",
        BOOLEAN,
        video::mat_colorcorrection,
        video::mat_colorcorrection_value,
    ),
    cvar(
        "mat_disable_d3d9ex",
        "mat_disable_d3d9ex [0|1]: query or toggle disabling Direct3D 9Ex extensions (0=enabled, 1=disabled)",
        BOOLEAN,
        video::mat_disable_d3d9ex,
        video::mat_disable_d3d9ex_value,
    ),
    cvar(
        "mat_forceaniso",
        "mat_forceaniso [1|2|4|8|16]: query or set anisotropic texture filtering mode",
        &[Arg::Values(&["1", "2", "4", "8", "16"])],
        video::mat_forceaniso,
        video::mat_forceaniso_value,
    ),
    cvar(
        "mat_hdr_level",
        "mat_hdr_level [0|1|2]: query or set HDR level (0=None, 1=Bloom, 2=Full)",
        &[Arg::Values(&["0", "1", "2"])],
        video::mat_hdr_level,
        video::mat_hdr_level_value,
    ),
    cvar(
        "mat_motion_blur_enabled",
        "mat_motion_blur_enabled [0|1]: query or toggle motion blur",
        BOOLEAN,
        video::mat_motion_blur_enabled,
        video::mat_motion_blur_enabled_value,
    ),
    cvar(
        "mat_picmip",
        "mat_picmip [-1|0|1|2]: query or set texture detail (-1=very high, 0=high, 1=medium, 2=low)",
        &[Arg::Values(&["-1", "0", "1", "2"])],
        video::mat_picmip,
        video::mat_picmip_value,
    ),
    cvar(
        "mat_queue_mode",
        "mat_queue_mode [-1|0|1|2]: query or set multicore rendering (-1=multicore, 0=single threaded)",
        &[Arg::Values(&["-1", "0", "1", "2"])],
        video::mat_queue_mode,
        video::mat_queue_mode_value,
    ),
    cvar(
        "mat_reducefillrate",
        "mat_reducefillrate [0|1]: query or toggle reduced shader fillrate (0=high, 1=low)",
        BOOLEAN,
        video::mat_reducefillrate,
        video::mat_reducefillrate_value,
    ),
    cvar(
        "mat_trilinear",
        "mat_trilinear [0|1]: query or toggle trilinear texture filtering",
        BOOLEAN,
        video::mat_trilinear,
        video::mat_trilinear_value,
    ),
    cvar(
        "mat_vsync",
        "mat_vsync [0|1]: query or toggle vertical sync",
        BOOLEAN,
        video::mat_vsync,
        video::mat_vsync_value,
    ),
    cvar(
        "r_classic_blood",
        "r_classic_blood [0|1]: query or toggle classic blood effects",
        BOOLEAN,
        video::r_classic_blood,
        video::classic_effects_value,
    ),
    cvar(
        "r_classic_fire",
        "r_classic_fire [0|1]: query or toggle classic fire effects",
        BOOLEAN,
        video::r_classic_fire,
        video::classic_effects_value,
    ),
    cvar(
        "r_flashlightdepthtexture",
        "r_flashlightdepthtexture [0|1]: query or toggle depth texture flashlight shadows (0=medium, 1=high)",
        BOOLEAN,
        video::r_flashlightdepthtexture,
        video::r_flashlightdepthtexture_value,
    ),
    cvar(
        "r_rootlod",
        "r_rootlod [0|1|2]: query or set model detail (0=high, 1=medium, 2=low)",
        &[Arg::Values(&["0", "1", "2"])],
        video::r_rootlod,
        video::r_rootlod_value,
    ),
    cvar(
        "r_shadowrendertotexture",
        "r_shadowrendertotexture [0|1]: query or toggle dynamic render-to-texture shadows (0=low, 1=medium/high)",
        BOOLEAN,
        video::r_shadowrendertotexture,
        video::r_shadowrendertotexture_value,
    ),
    cvar(
        "r_waterforceexpensive",
        "r_waterforceexpensive [0|1]: query or toggle expensive water reflections (0=simple, 1=world/all)",
        BOOLEAN,
        video::r_waterforceexpensive,
        video::r_waterforceexpensive_value,
    ),
    cvar(
        "r_waterforcereflectentities",
        "r_waterforcereflectentities [0|1]: query or toggle water entity reflections (0=world only, 1=all)",
        BOOLEAN,
        video::r_waterforcereflectentities,
        video::r_waterforcereflectentities_value,
    ),
    command(
        "mat_setvideomode",
        "mat_setvideomode <width> <height> <windowed 0|1>: set the video mode",
        &[Arg::Free, Arg::Free, Arg::Values(&["0", "1"])],
        video::mat_setvideomode,
    ),
    command(
        "mainmenu",
        "mainmenu: show the main menu",
        &[],
        map::mainmenu,
    ),
    cvar(
        "background_map",
        "background_map [name|default]: query or set the persistent main-menu background map",
        &[Arg::Map],
        map::background_map,
        map::background_map_value,
    ),
    command(
        "map_background",
        "map_background <name>: load an installed map as the main-menu background and persist it",
        &[Arg::Map],
        map::map_background,
    ),
    cvar(
        "con_filter_enable",
        "con_filter_enable [0|1|2]: filter console output by con_filter_text (1 hides other lines, 2 shows matches brighter)",
        &[Arg::Values(&["0", "1", "2"])],
        developer::con_filter_enable,
        developer::con_filter_enable_value,
    ),
    cvar(
        "con_filter_text",
        "con_filter_text [text]: the text con_filter_enable filters console output by",
        &[Arg::Free],
        developer::con_filter_text,
        developer::con_filter_text_value,
    ),
    cvar(
        "con_timestamp",
        "con_timestamp [0|1]: prefix console output with the time it was printed",
        BOOLEAN,
        developer::con_timestamp,
        developer::con_timestamp_value,
    ),
    cvar(
        "skill",
        "skill [1|2|3]: query or set game skill level (1=easy, 2=normal, 3=hard)",
        &[Arg::Values(&["1", "2", "3"])],
        map::skill,
        map::skill_value,
    ),
    command(
        "migrate_episodic_data",
        "migrate_episodic_data: migrate save data from standalone episodes (placeholder)",
        &[],
        map::migrate_episodic_data,
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

    /// Every command is one `COMMANDS` entry, so every spec has a handler by construction, and the
    /// handler modules are private: a handler no spec names fails the build as dead code. This
    /// checks that each spec runs its own handler: any spelling of its name dispatches, a cvar's
    /// query prints that cvar with its own value, the NPC console commands pass their own name
    /// on, and arguments no command takes answer with the spec's usage.
    #[test]
    fn every_spec_runs_its_own_handler() {
        let mut console = Console {
            cheats: true,
            ..Console::default()
        };
        console.config.gameplay.background_map = Some("background01".into());
        let unknown = |name: &str| Err(format!("Unknown or unimplemented command: {name}"));
        assert_eq!(console.interpret(&["nope".into()]), unknown("nope"));
        // Commands that take any arguments: the rest of the line or the host's NPC console.
        let free = [
            "echo",
            "npc_create",
            "npc_create_aimed",
            "npc_create_equipment",
            "prop_physics_create",
            "ent_create",
        ];
        let junk = vec!["x".to_owned(); 9];
        for spec in COMMANDS {
            let bare = console.interpret(&[spec.name.to_uppercase()]);
            assert_ne!(bare, unknown(&spec.name.to_uppercase()), "{}", spec.name);
            if let Some(value) = spec.value {
                // `fov_desired` answers as `fov`, like retail's alias.
                let label = if spec.name == "fov_desired" {
                    "fov"
                } else {
                    spec.name
                };
                let value = value(&console).unwrap_or_else(|| panic!("{} unset", spec.name));
                let printed = console.output.back().map(|line| line.text.as_str());
                assert_eq!(printed, Some(format!("\"{label}\" = \"{value}\"").as_str()));
            }
            let tokens: Vec<String> = std::iter::once(spec.name.to_owned())
                .chain(junk.iter().cloned())
                .collect();
            match console.interpret(&tokens) {
                Ok(Some(Effect::NpcCommand(line))) if free.contains(&spec.name) => {
                    assert!(line.starts_with(&format!("{} x", spec.name)), "{line}");
                }
                Ok(None) if spec.name == "echo" => {}
                other => assert_eq!(other, Err(spec.usage()), "{}", spec.name),
            }
        }
    }

    #[test]
    fn usage_splits_arguments_from_the_description() {
        let (args, description) = usage(find("setpos").unwrap());
        assert_eq!(args, ["<x>", "<y>", "[z]"]);
        assert!(description.starts_with("move the player origin"));
        let (args, _) = usage(find("clear").unwrap());
        assert!(args.is_empty());
    }

    /// A value each cvar accepts, different from its default where it can be: the first listed
    /// value that is not the current one, else a typical value for free arguments.
    fn golden_value(spec: &Spec, current: &str) -> String {
        if let Some(Arg::Values(values)) = spec.args.first() {
            return values
                .iter()
                .find(|v| **v != current)
                .unwrap_or(&values[0])
                .to_string();
        }
        match spec.name {
            "volume" => "0.5",
            "snd_musicvolume" => "0.25",
            "sensitivity" => "2.5",
            "m_pitch" => "-0.022",
            "m_customaccel_exponent" => "1.2",
            "sk_autoaim_scale1" | "m_autoaim_scale" => "1.5",
            "fov" | "fov_desired" => "100",
            "vertical_fov" => "60",
            "viewmodel_fov" => "70",
            "con_filter_text" => "bridge",
            "background_map" => "background02",
            other => panic!("{other}: no golden value"),
        }
        .to_owned()
    }

    /// The settings that differ between two serialized configurations, `path: before -> after`.
    fn config_changes(before: &serde_json::Value, after: &serde_json::Value) -> Vec<String> {
        fn leaves(value: &serde_json::Value, path: String, out: &mut Vec<(String, String)>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, value) in map {
                        let path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        };
                        leaves(value, path, out);
                    }
                }
                other => out.push((path, other.to_string())),
            }
        }
        let (mut a, mut b) = (Vec::new(), Vec::new());
        leaves(before, String::new(), &mut a);
        leaves(after, String::new(), &mut b);
        let a: std::collections::BTreeMap<_, _> = a.into_iter().collect();
        let b: std::collections::BTreeMap<_, _> = b.into_iter().collect();
        a.keys()
            .chain(b.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|key| a.get(*key) != b.get(*key))
            .map(|key| {
                let side = |m: &std::collections::BTreeMap<String, String>| {
                    m.get(key).cloned().unwrap_or_else(|| "-".into())
                };
                format!("{key}: {} -> {}", side(&a), side(&b))
            })
            .collect()
    }

    /// Every cvar on a fresh console: a query, a valid set, an invalid value, an out-of-range
    /// number and too many arguments, with the lines each logs, its error or effect (for
    /// `ApplyConfig`, the settings it changed) and the value afterwards. The transcript is recorded in `cvars.golden`;
    /// `UPDATE_GOLDEN=1` rewrites it after a reviewed change.
    #[test]
    fn every_cvar_matches_its_golden_transcript() {
        let mut transcript = String::new();
        for spec in COMMANDS.iter().filter(|spec| spec.is_cvar()) {
            let value = spec.value.expect("a cvar");
            let current = value(&Console::default()).unwrap_or_default();
            let mut cases = vec![
                Vec::new(),
                vec![golden_value(spec, &current)],
                vec!["x".to_owned()],
                vec!["9".to_owned()],
                vec!["1".to_owned(), "1".to_owned()],
            ];
            if spec.name == "background_map" {
                // `default` resets the background instead of naming a map.
                cases.insert(2, vec!["default".to_owned()]);
            }
            transcript += &format!("{}\n", spec.name);
            for args in cases {
                let mut console = Console::default();
                let before = serde_json::to_value(&console.config).unwrap();
                let printed = console.output.len();
                let tokens: Vec<String> = std::iter::once(spec.name.to_owned())
                    .chain(args.iter().cloned())
                    .collect();
                let outcome = match console.interpret(&tokens) {
                    Ok(None) => "no effect".to_owned(),
                    Ok(Some(Effect::ApplyConfig(config))) => {
                        assert_eq!(
                            *config, console.config,
                            "{}: applies the stored settings",
                            spec.name
                        );
                        let after = serde_json::to_value(&*config).unwrap();
                        format!(
                            "ApplyConfig [{}]",
                            config_changes(&before, &after).join(", ")
                        )
                    }
                    Ok(Some(effect)) => format!("{effect:?}"),
                    Err(error) => format!("error {error:?}"),
                };
                let call = if args.is_empty() {
                    "query".to_owned()
                } else {
                    args.join(" ")
                };
                transcript += &format!("  {call}: {outcome}\n");
                for line in console.output.iter().skip(printed) {
                    transcript += &format!("    | {}\n", line.text);
                }
                transcript += &format!("    value {:?}\n", value(&console));
            }
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/console/commands/cvars.golden");
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &transcript).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            transcript == expected,
            "cvar transcript differs from {} (rerun with UPDATE_GOLDEN=1 after review):\n{transcript}",
            path.display()
        );
    }
}
