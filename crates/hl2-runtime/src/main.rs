mod entities;
mod gameplay;
mod hud;
mod impacts;
mod physics;
mod playback;
mod rendering;
mod sandbox;
mod selection;
mod sky;
mod sounds;
mod viewer;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use source_assets::{bsp::Bsp, install, vpk::Vfs, vtf};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Options {
    pub command: String,
    pub game: PathBuf,
    pub map: String,
    pub all: bool,
    pub capture: Option<PathBuf>,
    pub frames: Option<u32>,
    pub position: Option<glam::Vec3>,
    pub yaw: f32,
    pub pitch: f32,
    pub output: Option<PathBuf>,
    pub mods: PathBuf,
    pub input_script: Option<PathBuf>,
    pub time_scale: f32,
    pub smoke: bool,
    pub fly: bool,
}
fn options() -> Result<Options> {
    let mut a = std::env::args().skip(1);
    let command = a.next().unwrap_or("view".into());
    if command == "--help" || command == "help" {
        println!("HL2-RS experimental Rust runtime\n\nCommands: view (default), inspect, verify, export\nOptions: --game <HL2 folder> --map <map> --all (verify every map)\n         --capture <PNG> --frames <N> --position <x,y,z> --yaw <degrees> --pitch <degrees>\n         --output <JSON> --mods <sandbox.json> --smoke (scripted movement/mod test)\n         --input-script <JSON> (timed regression inputs) --time-scale <factor> (default 1)\n\nStandalone partial reconstruction using installed assets; campaign and NPC AI are incomplete.\nWASD move; click to capture mouse, Esc releases; arrows look; Q/E fly up/down;\nShift faster; F1 debug HUD; F2 fly/walk; Space jump; Tab entity markers; B place block; F5 reload mod; Ctrl crouch; E use; R reload; 1-6 slots/mousewheel weapon selection; Q previous weapon; F3 developer loadout; F4 reset; F12 screenshot; F10 quit.");
        std::process::exit(0);
    }
    if !["view", "inspect", "verify", "export"].contains(&command.as_str()) {
        bail!("unknown command {command}; use --help");
    }
    let mut o = Options {
        command,
        game: PathBuf::new(),
        map: "d1_trainstation_02".into(),
        all: false,
        capture: None,
        frames: None,
        position: None,
        yaw: 0.,
        pitch: 0.,
        output: None,
        mods: "mods/vanilla-preview.json".into(),
        input_script: None,
        time_scale: 1.,
        smoke: false,
        fly: false,
    };
    while let Some(key) = a.next() {
        if key == "--fly" {
            o.fly = true;
            continue;
        }
        if key == "--all" {
            o.all = true;
            continue;
        }
        if key == "--smoke" {
            o.smoke = true;
            continue;
        }
        let value = a
            .next()
            .with_context(|| format!("missing value for {key}"))?;
        match key.as_str() {
            "--game" => o.game = value.into(),
            "--map" => o.map = value,
            "--capture" => o.capture = Some(value.into()),
            "--frames" => o.frames = Some(value.parse()?),
            "--output" => o.output = Some(value.into()),
            "--mods" => o.mods = value.into(),
            "--input-script" => o.input_script = Some(value.into()),
            "--time-scale" => o.time_scale = value.parse()?,
            "--yaw" => o.yaw = value.parse::<f32>()?.to_radians(),
            "--pitch" => o.pitch = value.parse::<f32>()?.to_radians(),
            "--position" => {
                o.position = Some(
                    modkit_core::parse_vec3(&value.replace(',', " "))
                        .context("position must be finite x,y,z")?,
                )
            }
            _ => bail!("unknown option {key}"),
        }
    }
    if !o.yaw.is_finite() || !o.pitch.is_finite() {
        bail!("angles must be finite");
    }
    if !o.time_scale.is_finite() || !(0.01..=10.).contains(&o.time_scale) {
        bail!("time scale must be finite and between 0.01 and 10");
    }
    if o.map.contains('/') || o.map.contains('\\') || o.map.contains("..") || o.map.contains(':') {
        bail!("map must be a simple map name");
    }
    o.game = if o.game.as_os_str().is_empty() {
        install::discover()?
    } else {
        install::validate(o.game)?
    };
    Ok(o)
}
pub fn load(o: &Options) -> Result<(Bsp, modkit_core::World, Vfs)> {
    let path = o
        .game
        .join("hl2/maps")
        .join(format!("{}.bsp", o.map.trim_end_matches(".bsp")));
    let bsp =
        Bsp::parse(&std::fs::read(&path).with_context(|| format!("read {}", path.display()))?)?;
    let world = bsp.world(&o.map)?;
    let mut vfs = Vfs::mount(&o.game)?;
    vfs.mount_pak(bsp.lump(40))?;
    Ok((bsp, world, vfs))
}
#[derive(Serialize)]
struct Summary {
    map: String,
    version: u32,
    revision: u32,
    triangles: usize,
    materials: usize,
    solid_brushes: usize,
    entities: usize,
    model_instances: usize,
    spawn: [f32; 3],
    classes: BTreeMap<String, usize>,
    textures_loaded: usize,
    texture_errors: Vec<String>,
    warnings: Vec<String>,
}
fn summary(o: &Options, textures: bool) -> Result<Summary> {
    let (bsp, world, vfs) = load(o)?;
    let mut classes = BTreeMap::new();
    for e in &world.entities {
        *classes.entry(e.class().to_string()).or_insert(0) += 1;
    }
    let mut loaded = 0;
    let mut errors = Vec::new();
    if textures {
        for s in &world.surfaces {
            let result = (|| -> Result<()> {
                let texture = vfs
                    .base_texture(&s.material)?
                    .context("VMT/base texture missing")?;
                let path = format!("materials/{}.vtf", texture.trim_end_matches(".vtf"));
                let data = vfs.read(&path)?.context("VTF not found")?;
                vtf::decode(&data, 512)?;
                Ok(())
            })();
            match result {
                Ok(()) => loaded += 1,
                Err(e) => errors.push(format!("{}: {e}", s.material)),
            }
        }
    }
    Ok(Summary {
        map: o.map.clone(),
        version: bsp.version,
        revision: bsp.revision,
        triangles: world.surfaces.iter().map(|s| s.indices.len() / 3).sum(),
        materials: world.surfaces.len(),
        solid_brushes: world.brushes.len(),
        entities: world.entities.len(),
        model_instances: world.model_instances.len(),
        spawn: world.spawn().0.to_array(),
        classes,
        textures_loaded: loaded,
        texture_errors: errors,
        warnings: world.warnings.into_iter().chain(vfs.warnings).collect(),
    })
}
fn cli(o: &Options) -> Result<()> {
    match o.command.as_str() {
        "inspect" => {
            let s = summary(o, true)?;
            let json = serde_json::to_string_pretty(&s)?;
            println!("{json}");
            if let Some(out) = &o.output {
                write(out, json.as_bytes())?;
            }
        }
        "export" => {
            let (_, mut world, vfs) = load(o)?;
            let report = source_assets::models::append_models(&mut world, &vfs);
            println!("Model report: {}", serde_json::to_string(&report)?);
            let output = o
                .output
                .clone()
                .unwrap_or_else(|| PathBuf::from("artifacts/world.json"));
            write(&output, &serde_json::to_vec(&world)?)?;
            println!("Wrote local game-derived world to {}", output.display());
        }
        "verify" => {
            let mut maps = if o.all {
                install::maps(&o.game)?
                    .iter()
                    .filter_map(|p| p.file_stem().map(|n| n.to_string_lossy().to_string()))
                    .collect::<Vec<_>>()
            } else {
                vec![o.map.clone()]
            };
            maps.sort();
            let mut successes = Vec::new();
            let mut failures = Vec::new();
            for name in maps {
                let mut map_options = o.clone();
                map_options.map = name.clone();
                match summary(&map_options, false) {
                    Ok(s) => {
                        println!(
                            "OK {name}: {} triangles, {} entities",
                            s.triangles, s.entities
                        );
                        successes.push(s);
                    }
                    Err(e) => {
                        println!("FAIL {name}: {e:#}");
                        failures.push(serde_json::json!({"map":name,"error":format!("{e:#}")}));
                    }
                }
            }
            let report = serde_json::json!({"passed":successes.len(),"failed":failures.len(),"maps":successes,"errors":failures});
            write(
                &o.output
                    .clone()
                    .unwrap_or_else(|| "artifacts/verification.json".into()),
                &serde_json::to_vec_pretty(&report)?,
            )?;
            println!("{} maps passed, {} failed", successes.len(), failures.len());
            if !failures.is_empty() {
                bail!(
                    "map verification found {} errors (see report)",
                    failures.len()
                );
            }
        }
        _ => bail!("not a CLI command"),
    }
    Ok(())
}
pub fn write(path: &std::path::Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, data)?;
    Ok(())
}
fn main() {
    let o = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
    };
    if o.command == "view" {
        macroquad::Window::from_config(viewer::config(), async move {
            if let Err(e) = viewer::run(o).await {
                eprintln!("Runtime error: {e:#}");
                std::process::exit(1);
            }
        });
    } else if let Err(e) = cli(&o) {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
