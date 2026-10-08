//! Soundscapes: env_soundscape selection (SDK server soundscape.cpp / soundscape_system.cpp)
//! and script playback (SDK client c_soundscape.cpp). Hosts apply [`Command`]s to sinks.
use glam::Vec3;
use modkit_core::World;
use source_assets::{keyvalues, keyvalues::Entry, vpk::Vfs};
use std::collections::BTreeSet;

/// soundscape_fadetime: seconds for a full 0..1 loop volume change.
pub const FADE_TIME: f32 = 3.;
/// playsoundscape nesting limit (MAX_SOUNDSCAPE_RECURSION).
const MAX_RECURSION: u32 = 8;
/// "position random" places a play this far from the eye (DEFAULT_SOUND_RADIUS).
const RANDOM_RADIUS: f32 = 36.;
/// MASK_SOLID_BRUSHONLY | MASK_WATER for the visibility trace.
const TRACE_CONTENTS: u32 = 0x403b;
const MANIFEST: &str = "scripts/soundscapes_manifest.txt";

/// Every top-level soundscape section, in load order.
#[derive(Default)]
pub struct Definitions {
    sections: Vec<Entry>,
    pub errors: Vec<String>,
}
impl Definitions {
    /// The manifest's files in order, then `scripts/soundscapes_<map>.txt` if it is not listed.
    pub fn load(vfs: &Vfs, map: &str) -> Self {
        let mut files = Vec::new();
        let mut errors = Vec::new();
        let map_file = format!("scripts/soundscapes_{}.txt", map.to_ascii_lowercase());
        let mut listed = BTreeSet::new();
        match vfs.read(MANIFEST) {
            Ok(Some(data)) => match keyvalues::parse(&String::from_utf8_lossy(&data)) {
                Ok(entries) => {
                    for entry in entries.iter().flat_map(Entry::children) {
                        if let (true, Some(path)) =
                            (entry.key.eq_ignore_ascii_case("file"), entry.text())
                        {
                            listed.insert(path.to_ascii_lowercase());
                            files.push(path.to_owned());
                        }
                    }
                }
                Err(e) => errors.push(format!("{MANIFEST}: {e:#}")),
            },
            Ok(None) => errors.push(format!("{MANIFEST}: absent")),
            Err(e) => errors.push(format!("{MANIFEST}: {e:#}")),
        }
        if !listed.contains(&map_file) && matches!(vfs.read(&map_file), Ok(Some(_))) {
            files.push(map_file);
        }
        let mut texts = Vec::new();
        for file in files {
            match vfs.read(&file) {
                Ok(Some(data)) => texts.push((file, String::from_utf8_lossy(&data).into_owned())),
                Ok(None) => errors.push(format!("{file}: absent")),
                Err(e) => errors.push(format!("{file}: {e:#}")),
            }
        }
        let mut definitions = Self::parse(&texts);
        errors.append(&mut definitions.errors);
        definitions.errors = errors;
        definitions
    }
    pub fn parse(files: &[(String, String)]) -> Self {
        let mut definitions = Self::default();
        for (name, text) in files {
            match keyvalues::parse(text) {
                Ok(entries) => definitions
                    .sections
                    .extend(entries.into_iter().filter(|e| !e.children().is_empty())),
                Err(e) => definitions.errors.push(format!("{name}: {e:#}")),
            }
        }
        definitions
    }
    /// Case-insensitive; the first section wins (client FindSoundscapeByName).
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.sections
            .iter()
            .find(|s| s.key.eq_ignore_ascii_case(name))
    }
    pub fn len(&self) -> usize {
        self.sections.len()
    }
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
    /// Reachable loop waves and random waves, as playback paths.
    pub fn waves_by_kind<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> (BTreeSet<String>, BTreeSet<String>) {
        let names: Vec<&str> = names.into_iter().collect();
        let all = self.waves(names.iter().copied());
        let loops = self.loop_waves(names);
        let randoms = all.difference(&loops).cloned().collect();
        (loops, randoms)
    }
    fn loop_waves<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
        fn visit(d: &Definitions, section: &Entry, depth: u32, out: &mut BTreeSet<String>) {
            if depth > MAX_RECURSION {
                return;
            }
            for command in section.children() {
                if command.key.eq_ignore_ascii_case("playlooping") {
                    out.extend(
                        command
                            .get("wave")
                            .and_then(Entry::text)
                            .map(crate::sounds::playback_path),
                    );
                } else if command.key.eq_ignore_ascii_case("playsoundscape") {
                    if let Some(inner) = command
                        .get("name")
                        .and_then(Entry::text)
                        .and_then(|n| d.get(n))
                    {
                        visit(d, inner, depth + 1, out);
                    }
                }
            }
        }
        let mut out = BTreeSet::new();
        for name in names {
            if let Some(section) = self.get(name) {
                visit(self, section, 0, &mut out);
            }
        }
        out
    }
    /// Playback paths of every wave reachable from these soundscapes, for preloading.
    pub fn waves<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
        fn visit(
            definitions: &Definitions,
            section: &Entry,
            depth: u32,
            out: &mut BTreeSet<String>,
        ) {
            if depth > MAX_RECURSION {
                return;
            }
            for command in section.children() {
                match command.key.to_ascii_lowercase().as_str() {
                    "playlooping" => out.extend(
                        command
                            .get("wave")
                            .and_then(Entry::text)
                            .map(crate::sounds::playback_path),
                    ),
                    "playrandom" => {
                        for wave in command.get("rndwave").map_or(&[][..], Entry::children) {
                            out.extend(wave.text().map(crate::sounds::playback_path));
                        }
                    }
                    "playsoundscape" => {
                        if let Some(inner) = command
                            .get("name")
                            .and_then(Entry::text)
                            .and_then(|n| definitions.get(n))
                        {
                            visit(definitions, inner, depth + 1, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = BTreeSet::new();
        for name in names {
            if let Some(section) = self.get(name) {
                visit(self, section, 0, &mut out);
            }
        }
        out
    }
}

/// What the active soundscape entity sends the player (SDK audioparams_t).
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub entity: usize,
    pub soundscape: String,
    /// position0..7 entity origins; missing names leave the bit clear.
    pub positions: [Option<Vec3>; 8],
}

#[derive(Clone, Debug)]
struct Candidate {
    entity: usize,
    origin: Vec3,
    /// -1 means unlimited.
    radius: f32,
    soundscape: String,
    positions: [Option<String>; 8],
}

/// Server-side soundscape selection for the local player.
#[derive(Default)]
pub struct Selector {
    candidates: Vec<Candidate>,
    current: Option<usize>,
    pub params: Option<Params>,
    pub takeovers: u64,
    pub traces: u64,
}
impl Selector {
    pub fn new(world: &World) -> Self {
        let mut candidates = Vec::new();
        for (entity, e) in world.entities.iter().enumerate() {
            let class = e.class();
            if !matches!(
                class,
                "env_soundscape" | "env_soundscape_triggerable" | "env_soundscape_proxy"
            ) {
                continue;
            }
            let number = |key: &str| e.get(key).and_then(|v| v.trim().parse::<f32>().ok());
            let mut soundscape = e.get("soundscape").unwrap_or("").to_owned();
            let mut positions: [Option<String>; 8] = Default::default();
            for (i, slot) in positions.iter_mut().enumerate() {
                *slot = e
                    .get(&format!("position{i}"))
                    .filter(|n| !n.is_empty())
                    .map(str::to_owned);
            }
            if class == "env_soundscape_proxy" {
                // CEnvSoundscapeProxy::Activate copies the main soundscape and positions.
                let Some(main) = e.get("MainSoundscapeName").and_then(|name| {
                    world.entities.iter().find(|m| {
                        m.get("targetname") == Some(name) && m.class().starts_with("env_soundscape")
                    })
                }) else {
                    continue;
                };
                soundscape = main.get("soundscape").unwrap_or("").to_owned();
                for (i, slot) in positions.iter_mut().enumerate() {
                    *slot = main
                        .get(&format!("position{i}"))
                        .filter(|n| !n.is_empty())
                        .map(str::to_owned);
                }
            }
            candidates.push(Candidate {
                entity,
                origin: e.origin(),
                radius: number("radius").unwrap_or(0.),
                soundscape,
                positions,
            });
        }
        Self {
            candidates,
            ..Self::default()
        }
    }
    /// One FrameUpdatePostEntityThink for the player ear (eye) position. Returns the entity
    /// that became active this tick, for its OnPlay output.
    pub fn update(
        &mut self,
        world: &World,
        ear: Vec3,
        enabled: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let mut traces = 0;
        let mut visible = |from: Vec3| {
            traces += 1;
            let trace = modkit_core::trace_brushes(
                &world.brushes,
                from,
                ear,
                Vec3::ZERO,
                Vec3::ZERO,
                TRACE_CONTENTS,
            );
            trace.fraction >= 1. && !trace.start_solid
        };
        let in_radius = |c: &Candidate, range: f32| c.radius > range || c.radius == -1.;
        // UpdateForPlayer on the current soundscape first.
        let mut current = self.current;
        let mut in_range = false;
        let mut current_distance = 0.;
        if let Some(index) = current {
            let c = &self.candidates[index];
            if !enabled(c.entity) {
                current = None;
            } else {
                let range = ear.distance(c.origin);
                current_distance = range;
                in_range = in_radius(c, range) && visible(c.origin);
            }
        }
        let mut took_over = None;
        for (index, c) in self.candidates.iter().enumerate() {
            if Some(index) == current || Some(index) == self.current || !enabled(c.entity) {
                continue;
            }
            let range = ear.distance(c.origin);
            if (!in_range || range < current_distance) && in_radius(c, range) && visible(c.origin) {
                current = Some(index);
                in_range = true;
                current_distance = range;
                took_over = Some(index);
            }
        }
        self.traces += traces;
        let index = took_over?;
        // WriteAudioParamsTo: the player's params now name this soundscape.
        let c = &self.candidates[index];
        let mut positions = [None; 8];
        for (slot, name) in positions.iter_mut().zip(&c.positions) {
            *slot = name.as_deref().and_then(|name| {
                world
                    .entities
                    .iter()
                    .find(|e| e.get("targetname") == Some(name))
                    .map(modkit_core::Entity::origin)
            });
        }
        self.current = Some(index);
        self.params = Some(Params {
            entity: c.entity,
            soundscape: c.soundscape.clone(),
            positions,
        });
        self.takeovers += 1;
        Some(c.entity)
    }
}

/// SDK interval_t: start plus a uniform draw in [0, range].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Interval {
    start: f32,
    range: f32,
}
/// C atof: the longest numeric prefix, 0 when there is none.
fn atof(text: &str) -> f32 {
    let text = text.trim_start();
    let end = text
        .char_indices()
        .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0))
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    (0..=end)
        .rev()
        .find_map(|n| text[..n].parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(0.)
}
/// ReadInterval: "a" or "a,b" (strtok skips empty fields).
fn read_interval(text: &str) -> Interval {
    let mut fields = text.split(',').filter(|f| !f.is_empty());
    let Some(first) = fields.next() else {
        return Interval::default();
    };
    let start = atof(first);
    let range = fields.next().map_or(0., |b| atof(b) - start);
    Interval { start, range }
}
fn attn_to_sndlvl(attenuation: f32) -> f32 {
    if attenuation == 0. {
        0.
    } else {
        (50. + 20. / attenuation).trunc()
    }
}

/// Host operations, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Start a looping sound. `spatial` is (origin, soundlevel); None plays unspatialized.
    StartLoop {
        handle: u64,
        wave: String,
        volume: f32,
        pitch: f32,
        spatial: Option<(Vec3, f32)>,
    },
    LoopVolume {
        handle: u64,
        volume: f32,
    },
    StopLoop {
        handle: u64,
    },
    /// A one-shot from a playrandom.
    Play {
        wave: String,
        volume: f32,
        pitch: f32,
        spatial: Option<(Vec3, f32)>,
    },
}

#[derive(Clone, Debug)]
struct Loop {
    handle: u64,
    wave: String,
    pitch: i32,
    generation: u64,
    ambient: bool,
    position: Vec3,
    soundlevel: f32,
    current: f32,
    target: f32,
}
#[derive(Clone, Debug)]
struct Random {
    next: f64,
    time: Interval,
    volume: Interval,
    pitch: Interval,
    soundlevel: Interval,
    master: f32,
    waves: Vec<String>,
    ambient: bool,
    random_position: bool,
    position: Vec3,
}
#[derive(Clone, Copy)]
struct SubParams {
    depth: u32,
    master: f32,
    start: i32,
    position_override: i32,
    ambient_override: i32,
}

/// Client-side soundscape playback state.
pub struct Playback {
    active: Option<(usize, String)>,
    loops: Vec<Loop>,
    randoms: Vec<Random>,
    generation: u64,
    next_random: f64,
    rng: u64,
    next_handle: u64,
    commands: Vec<Command>,
    positions: [Option<Vec3>; 8],
    view: (Vec3, Vec3, Vec3),
    pub started: u64,
    pub unknown: BTreeSet<String>,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            active: None,
            loops: Vec::new(),
            randoms: Vec::new(),
            generation: 0,
            next_random: 0.,
            rng: 0x5eed_50da_c4a9_e001,
            next_handle: 1,
            commands: Vec::new(),
            positions: [None; 8],
            view: (Vec3::ZERO, Vec3::X, Vec3::NEG_Y),
            started: 0,
            unknown: BTreeSet::new(),
        }
    }
}
impl Playback {
    /// Names and target volumes of the loops now playing.
    pub fn loops(&self) -> Vec<(String, f32, f32)> {
        self.loops
            .iter()
            .map(|l| (l.wave.clone(), l.current, l.target))
            .collect()
    }
    pub fn active(&self) -> Option<&(usize, String)> {
        self.active.as_ref()
    }
    fn uniform(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
    fn draw(&mut self, interval: Interval) -> f32 {
        if interval.range == 0. {
            interval.start
        } else {
            interval.start + self.uniform() * interval.range
        }
    }
    /// One client frame: params from the server, then loop fades and random sounds.
    /// `view` is (eye, forward, right); `dt` is 0 while paused.
    pub fn update(
        &mut self,
        definitions: &Definitions,
        params: Option<&Params>,
        now: f64,
        dt: f32,
        view: (Vec3, Vec3, Vec3),
    ) -> Vec<Command> {
        self.view = view;
        if let Some(params) = params {
            let key = (params.entity, params.soundscape.to_ascii_lowercase());
            if self.active.as_ref() != Some(&key) {
                self.active = Some(key);
                self.positions = params.positions;
                match definitions.get(&params.soundscape) {
                    Some(section) => self.start(definitions, section, now),
                    // A bad index keeps the old soundscape playing.
                    None => {
                        self.unknown.insert(params.soundscape.clone());
                    }
                }
            }
        }
        self.fade(dt);
        self.randoms(now);
        std::mem::take(&mut self.commands)
    }
    fn start(&mut self, definitions: &Definitions, section: &Entry, now: f64) {
        for l in &mut self.loops {
            l.target = 0.;
        }
        self.generation += 1;
        self.randoms.clear();
        self.next_random = now;
        self.started += 1;
        self.sub(
            definitions,
            section,
            SubParams {
                depth: 0,
                master: 1.,
                start: 0,
                position_override: -1,
                ambient_override: -1,
            },
            now,
        );
    }
    fn sub(&mut self, definitions: &Definitions, section: &Entry, params: SubParams, now: f64) {
        for command in section.children() {
            match command.key.to_ascii_lowercase().as_str() {
                "playlooping" => self.play_looping(command, params),
                "playrandom" => self.play_random(command, params, now),
                "playsoundscape" => {
                    let mut inner = params;
                    inner.depth += 1;
                    if inner.depth > MAX_RECURSION {
                        continue;
                    }
                    let mut name = None;
                    for key in command.children() {
                        let text = key.text().unwrap_or("");
                        match key.key.to_ascii_lowercase().as_str() {
                            "volume" => {
                                inner.master = params.master * self.draw(read_interval(text))
                            }
                            "position" => inner.start = params.start + atof(text) as i32,
                            "positionoverride" if params.position_override < 0 => {
                                inner.position_override = params.start + atof(text) as i32;
                                inner.ambient_override = inner.position_override;
                            }
                            "ambientpositionoverride" if params.ambient_override < 0 => {
                                inner.ambient_override = params.start + atof(text) as i32;
                            }
                            "name" => name = Some(text),
                            _ => {}
                        }
                    }
                    if let Some(inner_section) = name.and_then(|n| definitions.get(n)) {
                        self.sub(definitions, inner_section, inner, now);
                    }
                }
                // DSP, dsp_volume and the sound mixer are not modeled.
                _ => {}
            }
        }
    }
    fn position(&self, index: i32) -> Option<Vec3> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.positions.get(i).copied().flatten())
    }
    fn play_looping(&mut self, command: &Entry, params: SubParams) {
        let mut volume = 0.;
        let mut soundlevel = attn_to_sndlvl(0.8);
        let mut wave = None;
        let mut pitch = 100;
        let mut position = -1;
        for key in command.children() {
            let text = key.text().unwrap_or("");
            match key.key.to_ascii_lowercase().as_str() {
                "volume" => volume = params.master * self.draw(read_interval(text)),
                "pitch" => pitch = self.draw(read_interval(text)) as i32,
                "wave" => wave = Some(text.to_owned()),
                "position" => position = params.start + atof(text) as i32,
                "attenuation" => soundlevel = attn_to_sndlvl(self.draw(read_interval(text))),
                "soundlevel" => {
                    soundlevel = if text.to_ascii_uppercase().starts_with("SNDLVL_") {
                        crate::sounds::soundlevel_value(text).unwrap_or(0.)
                    } else {
                        self.draw(read_interval(text)).trunc()
                    }
                }
                _ => {}
            }
        }
        if position < 0 {
            position = params.ambient_override;
        } else if params.position_override >= 0 {
            position = params.position_override;
        }
        let Some(wave) = wave.filter(|_| volume != 0.) else {
            return;
        };
        if position < 0 {
            self.add_loop(wave, true, volume, 75., pitch, Vec3::ZERO);
        } else if let Some(origin) = self.position(position) {
            self.add_loop(wave, false, volume, soundlevel, pitch, origin);
        }
    }
    fn add_loop(
        &mut self,
        wave: String,
        ambient: bool,
        volume: f32,
        soundlevel: f32,
        pitch: i32,
        position: Vec3,
    ) {
        let generation = self.generation;
        let reuse = self.loops.iter().rposition(|l| {
            l.generation != generation
                && l.pitch == pitch
                && l.wave.eq_ignore_ascii_case(&wave)
                && l.ambient == ambient
        });
        let index = match reuse {
            Some(i) if ambient || self.loops[i].position.abs_diff_eq(position, 0.1) => i,
            Some(i) => {
                // A positional loop moving elsewhere is stopped and started at the new place
                // with its current volume (SND_CHANGE_VOL starts a sound that is not playing).
                let old = self.loops[i].handle;
                self.commands.push(Command::StopLoop { handle: old });
                let handle = self.next_handle;
                self.next_handle += 1;
                self.commands.push(Command::StartLoop {
                    handle,
                    wave: wave.clone(),
                    volume: self.loops[i].current,
                    pitch: pitch as f32,
                    spatial: Some((position, soundlevel)),
                });
                self.loops[i].handle = handle;
                i
            }
            None => {
                let handle = self.next_handle;
                self.next_handle += 1;
                // Ambients start silent; positional loops at 0.05 so they are not culled.
                let current = if ambient { 0. } else { 0.05 };
                self.commands.push(Command::StartLoop {
                    handle,
                    wave: wave.clone(),
                    volume: current,
                    pitch: pitch as f32,
                    spatial: (!ambient).then_some((position, soundlevel)),
                });
                self.loops.push(Loop {
                    handle,
                    wave: wave.clone(),
                    pitch,
                    generation,
                    ambient,
                    position,
                    soundlevel,
                    current,
                    target: volume,
                });
                self.loops.len() - 1
            }
        };
        let l = &mut self.loops[index];
        l.wave = wave;
        l.target = volume;
        l.pitch = pitch;
        l.generation = generation;
        l.ambient = ambient;
        l.position = position;
        l.soundlevel = soundlevel;
    }
    fn play_random(&mut self, command: &Entry, params: SubParams, now: f64) {
        let mut sound = Random {
            next: 0.,
            time: Interval::default(),
            volume: Interval::default(),
            pitch: Interval::default(),
            soundlevel: Interval::default(),
            master: params.master,
            waves: Vec::new(),
            ambient: false,
            random_position: false,
            position: Vec3::ZERO,
        };
        let mut position = -1;
        for key in command.children() {
            let text = key.text().unwrap_or("");
            match key.key.to_ascii_lowercase().as_str() {
                "volume" => sound.volume = read_interval(text),
                "pitch" => sound.pitch = read_interval(text),
                "attenuation" => {
                    let a = read_interval(text);
                    let start = attn_to_sndlvl(a.start);
                    sound.soundlevel = Interval {
                        start,
                        range: attn_to_sndlvl(a.start + a.range) - start,
                    };
                }
                "soundlevel" => {
                    sound.soundlevel = if text.to_ascii_uppercase().starts_with("SNDLVL_") {
                        Interval {
                            start: crate::sounds::soundlevel_value(text).unwrap_or(0.),
                            range: 0.,
                        }
                    } else {
                        read_interval(text)
                    }
                }
                "time" => sound.time = read_interval(text),
                "rndwave" => {
                    sound.waves = key
                        .children()
                        .iter()
                        .filter_map(|w| w.text().map(str::to_owned))
                        .collect()
                }
                "position" if text.eq_ignore_ascii_case("random") => sound.random_position = true,
                "position" => position = params.start + atof(text) as i32,
                _ => {}
            }
        }
        if position < 0 {
            position = params.ambient_override;
        } else if params.position_override >= 0 {
            position = params.position_override;
            sound.random_position = false;
        }
        if sound.waves.is_empty() {
            return;
        }
        if position < 0 && !sound.random_position {
            sound.ambient = true;
        } else if !sound.random_position {
            let Some(origin) = self.position(position) else {
                return;
            };
            sound.position = origin;
        }
        sound.next = now + 0.5 * f64::from(self.draw(sound.time));
        self.randoms.push(sound);
    }
    fn fade(&mut self, dt: f32) {
        let amount = dt / FADE_TIME;
        let mut index = self.loops.len();
        while index > 0 {
            index -= 1;
            let l = &mut self.loops[index];
            if l.current == l.target {
                continue;
            }
            // SDK Approach: step by `amount`, snapping once within it.
            let delta = l.target - l.current;
            let next = if delta > amount {
                l.current + amount
            } else if delta < -amount {
                l.current - amount
            } else {
                l.target
            };
            if next == l.current {
                continue;
            }
            l.current = next;
            if l.target == 0. && l.current == 0. {
                let handle = l.handle;
                self.commands.push(Command::StopLoop { handle });
                self.loops.swap_remove(index);
            } else {
                let (handle, volume) = (l.handle, l.current);
                self.commands.push(Command::LoopVolume { handle, volume });
            }
        }
    }
    fn randoms(&mut self, now: f64) {
        if now < self.next_random {
            return;
        }
        self.next_random = now + 3600.;
        for index in (0..self.randoms.len()).rev() {
            if now >= self.randoms[index].next {
                self.play_random_sound(index);
                let time = self.randoms[index].time;
                self.randoms[index].next = now + f64::from(self.draw(time));
            }
            self.next_random = self.next_random.min(self.randoms[index].next);
        }
    }
    fn play_random_sound(&mut self, index: usize) {
        let count = self.randoms[index].waves.len();
        let choice = ((self.uniform() * count as f32) as usize).min(count - 1);
        let sound = self.randoms[index].clone();
        let volume = sound.master * self.draw(sound.volume);
        let pitch = self.draw(sound.pitch).trunc();
        let wave = sound.waves[choice].clone();
        if sound.ambient {
            self.commands.push(Command::Play {
                wave,
                volume,
                pitch,
                spatial: None,
            });
            return;
        }
        let soundlevel = self.draw(sound.soundlevel).trunc();
        let position = if sound.random_position {
            // GenerateRandomSoundPosition passes a degree range to a radian SinCos.
            let angle = -180. + self.uniform() * 360.;
            let (eye, forward, right) = self.view;
            eye + RANDOM_RADIUS * (angle.cos() * right + angle.sin() * forward)
        } else {
            sound.position
        };
        self.commands.push(Command::Play {
            wave,
            volume,
            pitch,
            spatial: Some((position, soundlevel)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modkit_core::{Brush, Entity, Plane};

    fn entity(class: &str, pairs: &[(&str, &str)]) -> Entity {
        let mut properties = vec![("classname".to_owned(), class.to_owned())];
        properties.extend(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        );
        Entity { properties }
    }
    fn definitions(text: &str) -> Definitions {
        Definitions::parse(&[("test".into(), text.into())])
    }
    const SCRIPT: &str = r#"
        "room" { "dsp" "1" "playlooping" { "volume" "0.2" "wave" "*a/room.wav" "pitch" "100" "attenuation" "0" }
                 "playsoundscape" { "name" "computer" "volume" "1.0" "positionoverride" "0" } }
        "hall" { "playlooping" { "volume" "0.5" "wave" "a/hall.wav" "pitch" "100" }
                 "playsoundscape" { "name" "city" "volume" "0.5" } }
        "computer" { "playlooping" { "volume" "0.4" "wave" "a/terminal.wav" "soundlevel" "SNDLVL_65db" }
                     "playrandom" { "time" "5,15" "volume" "0.3,0.5" "rndwave" { "wave" "a/beep.wav" } } }
        "city" { "playrandom" { "time" "10,30" "volume" "0.2,0.4" "pitch" "100" "position" "random"
                 "soundlevel" "SNDLVL_140db" "rndwave" { "wave" "a/plane.wav" "wave" "a/truck.wav" } } }
    "#;

    #[test]
    fn intervals_follow_read_interval_and_atof() {
        assert_eq!(
            read_interval("5,15"),
            Interval {
                start: 5.,
                range: 10.
            }
        );
        assert_eq!(
            read_interval("0.2"),
            Interval {
                start: 0.2,
                range: 0.
            }
        );
        assert_eq!(
            read_interval(",3"),
            Interval {
                start: 3.,
                range: 0.
            }
        );
        assert_eq!(atof("12abc"), 12.);
        assert_eq!(atof("x"), 0.);
        assert_eq!(attn_to_sndlvl(0.8), 75.);
    }

    #[test]
    fn reachable_waves_include_sub_soundscapes() {
        let d = definitions(SCRIPT);
        let waves = d.waves(["room"]);
        assert_eq!(
            waves.into_iter().collect::<Vec<_>>(),
            ["a/beep.wav", "a/room.wav", "a/terminal.wav"]
        );
        assert!(d.get("ROOM").is_some());
    }

    #[test]
    fn missing_position_suppresses_overridden_sub_soundscape() {
        let d = definitions(SCRIPT);
        let mut p = Playback::default();
        let params = Params {
            entity: 1,
            soundscape: "room".into(),
            positions: [None; 8],
        };
        let view = (Vec3::ZERO, Vec3::X, Vec3::NEG_Y);
        let commands = p.update(&d, Some(&params), 0., 0., view);
        // Only the ambient loop: the computer loop and its random need position 0.
        assert_eq!(
            commands,
            [Command::StartLoop {
                handle: 1,
                wave: "*a/room.wav".into(),
                volume: 0.,
                pitch: 100.,
                spatial: None
            }]
        );
        assert!(p.randoms.is_empty());
        // With position 0 the terminal loop is positional at 65 dB, starting at 0.05.
        let mut p = Playback::default();
        let mut positions = [None; 8];
        positions[0] = Some(Vec3::new(10., 0., 0.));
        let params = Params {
            positions,
            ..params
        };
        let commands = p.update(&d, Some(&params), 0., 0., view);
        assert!(commands.contains(&Command::StartLoop {
            handle: 2,
            wave: "a/terminal.wav".into(),
            volume: 0.05,
            pitch: 100.,
            spatial: Some((Vec3::new(10., 0., 0.), 65.))
        }));
        assert_eq!(p.randoms.len(), 1);
        assert!(!p.randoms[0].ambient);
    }

    #[test]
    fn loops_crossfade_over_three_seconds_and_reuse_same_wave() {
        let d = definitions(
            r#""a" { "playlooping" { "volume" "0.6" "wave" "x.wav" } "playlooping" { "volume" "0.3" "wave" "y.wav" } }
               "b" { "playlooping" { "volume" "0.3" "wave" "X.WAV" } }"#,
        );
        let mut p = Playback::default();
        let view = (Vec3::ZERO, Vec3::X, Vec3::NEG_Y);
        let a = Params {
            entity: 1,
            soundscape: "a".into(),
            positions: [None; 8],
        };
        p.update(&d, Some(&a), 0., 0., view);
        p.update(&d, Some(&a), 1.5, 1.5, view);
        let loops = p.loops();
        assert!((loops[0].1 - 0.5).abs() < 1e-6, "{loops:?}");
        assert!((loops[1].1 - 0.3).abs() < 1e-6, "{loops:?}");
        // Switching keeps x.wav playing (fading 0.5 -> 0.3) and fades y.wav out.
        let b = Params {
            entity: 2,
            soundscape: "b".into(),
            positions: [None; 8],
        };
        let commands = p.update(&d, Some(&b), 1.5, 0., view);
        assert!(!commands
            .iter()
            .any(|c| matches!(c, Command::StartLoop { .. })));
        let commands = p.update(&d, Some(&b), 3., 1., view);
        assert!(
            commands.contains(&Command::StopLoop { handle: 2 }),
            "{commands:?}"
        );
        let loops = p.loops();
        assert_eq!(loops.len(), 1);
        assert!((loops[0].1 - 0.3).abs() < 1e-6);
        // Paused frames (dt 0) do not fade.
        p.update(&d, Some(&a), 3., 0., view);
        assert!((p.loops()[0].1 - 0.3).abs() < 1e-6);
    }

    #[test]
    fn random_sounds_wait_half_a_draw_then_repeat() {
        let d = definitions(SCRIPT);
        let mut p = Playback::default();
        let params = Params {
            entity: 1,
            soundscape: "hall".into(),
            positions: [None; 8],
        };
        let view = (Vec3::new(0., 0., 64.), Vec3::X, Vec3::NEG_Y);
        p.update(&d, Some(&params), 0., 0., view);
        let first = p.randoms[0].next;
        assert!((5. ..=15.).contains(&first), "{first}");
        let mut plays = Vec::new();
        let mut t = 0.;
        while t < 200. {
            t += 0.5;
            for c in p.update(&d, Some(&params), t, 0.5, view) {
                if let Command::Play {
                    volume, spatial, ..
                } = c
                {
                    plays.push((t, volume, spatial));
                }
            }
        }
        assert!(plays.len() >= 6 && plays.len() <= 21, "{}", plays.len());
        for (_, volume, spatial) in &plays {
            assert!((0.1..=0.2).contains(volume));
            let (origin, soundlevel) = spatial.unwrap();
            assert_eq!(soundlevel, 140.);
            assert!((origin.distance(view.0) - 36.).abs() < 1e-3);
        }
        for pair in plays.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= 10. - 0.5);
        }
    }

    fn wall(x: f32) -> Brush {
        let plane = |normal: Vec3, distance: f32| Plane { normal, distance };
        Brush {
            planes: vec![
                plane(Vec3::X, x + 8.),
                plane(Vec3::NEG_X, -(x - 8.)),
                plane(Vec3::Y, 1000.),
                plane(Vec3::NEG_Y, 1000.),
                plane(Vec3::Z, 1000.),
                plane(Vec3::NEG_Z, 1000.),
            ],
            contents: 1,
        }
    }

    #[test]
    fn selection_follows_range_sight_and_closer_takeover() {
        let mut world = World {
            entities: vec![
                entity("worldspawn", &[]),
                entity(
                    "env_soundscape",
                    &[
                        ("origin", "0 0 0"),
                        ("radius", "100"),
                        ("soundscape", "room"),
                    ],
                ),
                entity(
                    "env_soundscape",
                    &[
                        ("origin", "300 0 0"),
                        ("radius", "300"),
                        ("soundscape", "hall"),
                    ],
                ),
                entity(
                    "env_soundscape",
                    &[
                        ("origin", "600 0 0"),
                        ("radius", "-1"),
                        ("soundscape", "city"),
                        ("position0", "marker"),
                    ],
                ),
                entity(
                    "info_target",
                    &[("origin", "1 2 3"), ("targetname", "marker")],
                ),
            ],
            ..World::default()
        };
        world.brushes.push(wall(450.));
        let mut s = Selector::new(&world);
        let on = |_| true;
        assert_eq!(s.update(&world, Vec3::new(50., 0., 0.), on), Some(1));
        // Out of range stays current while nothing else qualifies (the wall hides "city").
        assert_eq!(s.update(&world, Vec3::new(-150., 0., 0.), on), None);
        assert_eq!(s.params.as_ref().unwrap().soundscape, "room");
        assert_eq!(s.update(&world, Vec3::new(120., 0., 0.), on), Some(2));
        // Past the wall the hall is hidden; only the unlimited one is visible.
        assert_eq!(s.update(&world, Vec3::new(500., 0., 0.), on), Some(3));
        assert_eq!(
            s.params.as_ref().unwrap().positions[0],
            Some(Vec3::new(1., 2., 3.))
        );
        // Back in front of the wall the current one is hidden, so the hall takes over.
        assert_eq!(s.update(&world, Vec3::new(330., 0., 0.), on), Some(2));
        // A closer in-range soundscape replaces an in-range current one.
        assert_eq!(s.update(&world, Vec3::new(60., 0., 0.), on), Some(1));
        // Disabled ones are skipped, and a disabled current only drops out of the comparison.
        let only_hall = |entity| entity == 2;
        assert_eq!(
            s.update(&world, Vec3::new(230., 0., 0.), only_hall),
            Some(2)
        );
        assert_eq!(s.update(&world, Vec3::new(10., 0., 0.), |_| false), None);
        assert_eq!(s.params.as_ref().unwrap().soundscape, "hall");
    }

    #[test]
    #[ignore = "requires an owned installed Half-Life 2 copy"]
    fn installed_trainstation_soundscapes_resolve_and_waves_exist() {
        let root = source_assets::install::discover().unwrap();
        let vfs = Vfs::mount(&root).unwrap();
        let d = Definitions::load(&vfs, "d1_trainstation_02");
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        let names = [
            "d1_trainstation.Interrogation",
            "d1_trainstation.Turnstyle",
            "d1_trainstation.TerminalSquare",
            "d1_trainstation.QuietCourtyard",
            "d1_trainstation.AppartmentCourtyard",
            "d1_trainstation.Appartments",
        ];
        for name in names {
            assert!(d.get(name).is_some(), "{name}");
        }
        let (loops, randoms) = d.waves_by_kind(names);
        assert!(loops.contains("ambient/atmosphere/station_ambience_loop2.wav"));
        assert!(loops.contains("ambient/atmosphere/plaza_amb.wav"));
        assert!(randoms.contains("ambient/machines/heli_pass1.wav"));
        for wave in loops.iter().chain(&randoms) {
            assert!(
                vfs.read(&format!("sound/{wave}")).unwrap().is_some(),
                "{wave}"
            );
        }
    }
}
