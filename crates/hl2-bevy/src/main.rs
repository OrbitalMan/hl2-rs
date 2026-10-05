//! Isolated Bevy/wgpu host reusing engine-independent Source simulation.
mod assets;
mod audio;
mod effects;
mod eyes;
mod gameplay;
mod hud;
mod movement;
mod rendering;
mod sky;
use anyhow::{Context, Result, bail};
use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        renderer::RenderAdapterInfo,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::{MonitorSelection, WindowMode, WindowResolution},
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Resource)]
struct Options {
    game: PathBuf,
    map: String,
    frames: Option<u64>,
    capture: Option<PathBuf>,
    report: PathBuf,
    position: Option<Vec3>,
    yaw: Option<f32>,
    pitch: f32,
    width: u32,
    height: u32,
    borderless: bool,
    fly: bool,
    movement_script: Option<PathBuf>,
}
impl Options {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut game = None;
        let mut options = Self {
            game: PathBuf::new(),
            map: "d1_trainstation_02".into(),
            frames: None,
            capture: None,
            report: "artifacts/bevy-report.json".into(),
            position: None,
            yaw: None,
            pitch: 0.,
            width: 1280,
            height: 720,
            borderless: false,
            fly: false,
            movement_script: None,
        };
        while let Some(arg) = args.next() {
            let next = |args: &mut std::iter::Skip<std::env::Args>| -> Result<String> {
                args.next()
                    .with_context(|| format!("missing value after {arg}"))
            };
            match arg.as_str() {
                "--game" => game = Some(PathBuf::from(next(&mut args)?)),
                "--map" => options.map = next(&mut args)?,
                "--frames" => options.frames = Some(next(&mut args)?.parse()?),
                "--capture" => options.capture = Some(next(&mut args)?.into()),
                "--report" => options.report = next(&mut args)?.into(),
                "--position" => {
                    options.position = Some(Vec3::new(
                        next(&mut args)?.parse()?,
                        next(&mut args)?.parse()?,
                        next(&mut args)?.parse()?,
                    ))
                }
                "--yaw" => options.yaw = Some(next(&mut args)?.parse::<f32>()?.to_radians()),
                "--pitch" => options.pitch = next(&mut args)?.parse::<f32>()?.to_radians(),
                "--width" => options.width = next(&mut args)?.parse()?,
                "--height" => options.height = next(&mut args)?.parse()?,
                "--borderless" => options.borderless = true,
                "--fly" => options.fly = true,
                "--movement-script" => options.movement_script = Some(next(&mut args)?.into()),
                "--help" | "-h" => {
                    println!(
                        "HL2-RS Bevy migration preview (campaign incomplete).\n--game PATH --map NAME --borderless --width N --height N\n--position X Y Z --yaw DEGREES --pitch DEGREES\n--frames N --capture PNG --report JSON\n--fly --movement-script JSON\nClick to capture mouse; WASD move, Space jump, Ctrl crouch, Shift sprint, Alt walk. F2 toggles fly. F3 gives weapons; slots/wheel select; mouse buttons fire/confirm; R reloads; Q last weapon; E uses. Esc cancels selection then pauses; click resumes. F10 quits."
                    );
                    std::process::exit(0);
                }
                _ => bail!("unknown option {arg}; use --help"),
            }
        }
        if !(320..=8192).contains(&options.width) || !(240..=8192).contains(&options.height) {
            bail!("window size is outside 320x240..8192x8192");
        }
        if options.frames.is_some_and(|n| !(30..=36000).contains(&n)) {
            bail!("--frames must be between 30 and 36000");
        }
        if !options.position.is_none_or(|p| p.is_finite())
            || !options.pitch.is_finite()
            || !options.yaw.is_none_or(f32::is_finite)
        {
            bail!("camera coordinates and angles must be finite");
        }
        if options
            .capture
            .as_ref()
            .is_some_and(|p| p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("png")))
        {
            bail!("--capture must be a .png path");
        }
        options.pitch = options.pitch.clamp(-1.55, 1.55);
        if options.capture.is_some()
            && options.frames.is_none()
            && options.movement_script.is_none()
        {
            options.frames = Some(120);
        }
        options.game = match game {
            Some(path) => source_assets::install::validate(path)?,
            None => source_assets::install::discover()?,
        };
        for path in [Some(&options.report), options.capture.as_ref()]
            .into_iter()
            .flatten()
        {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(options)
    }
}
#[derive(Resource)]
struct PreparedMap(Option<assets::LoadedMap>);
#[derive(Default, Serialize)]
struct RunStatus {
    #[serde(skip)]
    captured_image: Option<Image>,
    frames: u64,
    capture_completed: bool,
    capture_size: Option<[u32; 2]>,
    adapter: Option<String>,
    backend: Option<String>,
    meshes: usize,
    triangles: usize,
    materials: usize,
    skipped_background_surfaces: usize,
    camera_source: [f32; 3],
    simulation: serde_json::Value,
    presentation: serde_json::Value,
}
#[derive(Clone, Resource, Default)]
struct Status(Arc<Mutex<RunStatus>>);
#[derive(Component)]
struct FlyCamera;
#[derive(Resource, Default)]
struct CaptureControl {
    frames: u64,
    requested: bool,
    requested_frame: Option<u64>,
    completed_frame: Option<u64>,
}
/// Source right-handed Z-up coordinates become Bevy right-handed Y-up.
fn source_to_bevy(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, -p.y)
}
fn bevy_to_source(p: Vec3) -> Vec3 {
    Vec3::new(p.x, -p.z, p.y)
}
fn source_direction(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(
        pitch.cos() * yaw.cos(),
        pitch.cos() * yaw.sin(),
        pitch.sin(),
    )
}

fn main() -> Result<()> {
    let options = Options::parse()?;
    println!(
        "Loading owned {} for Bevy/wgpu migration host...",
        options.map
    );
    let loaded = assets::load(&options.game, &options.map)?;
    let revision = loaded.revision;
    let model_report = loaded.models.clone();
    let texture_errors = loaded.texture_errors.clone();
    let warnings = loaded.world.warnings.clone();
    let unique_textures: std::collections::BTreeMap<_, _> = loaded
        .materials
        .values()
        .filter_map(|material| {
            material
                .base_path
                .as_ref()
                .zip(material.base.as_ref())
                .map(|(key, image)| (key.clone(), image.rgba.len()))
        })
        .collect();
    let texture_summary = serde_json::json!({
        "source_materials": loaded.materials.len(), "unique_base_textures": unique_textures.len(),
        "decoded_base_bytes": unique_textures.values().sum::<usize>(), "decoded_base_budget": 512 * 1024 * 1024,
        "owned_eye_dx8_fallbacks":loaded.materials.iter().filter(|(_,m)|m.eye_fallback).map(|(name,_)|name).collect::<Vec<_>>(),
    });
    let spawn_sky_visibility = format!("{:?}", loaded.bsp.sky_visibility(loaded.world.spawn().0));
    let (spawn, spawn_yaw) = loaded.world.spawn();
    let movement_commands = options
        .movement_script
        .as_ref()
        .map(|p| movement::read_script(p))
        .transpose()?
        .unwrap_or_default();
    let simulation = movement::Simulation::new(
        &loaded.world,
        options
            .position
            .map(|p| glam::Vec3::from_array(p.to_array()))
            .unwrap_or(spawn),
        options.yaw.unwrap_or(spawn_yaw),
        options.pitch,
        options.fly,
        movement_commands,
    );
    let status = Status::default();
    let packaged_assets = std::env::current_exe()?
        .parent()
        .context("executable has no directory")?
        .join("bevy-assets");
    let asset_root = if packaged_assets.join("shaders/source.wgsl").is_file() {
        packaged_assets
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    };
    let mut app = App::new();
    app.insert_resource(options.clone())
        .insert_resource(status.clone())
        .insert_resource(PreparedMap(Some(loaded)))
        .insert_resource(simulation)
        .init_resource::<CaptureControl>()
        .insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.1)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root.to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "HL2-RS | Bevy/wgpu migration | gameplay migration preview".into(),
                        resolution: WindowResolution::new(options.width, options.height)
                            .with_scale_factor_override(1.),
                        mode: if options.borderless {
                            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                        } else {
                            WindowMode::Windowed
                        },
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(MaterialPlugin::<rendering::SourceMaterial>::default())
        .add_plugins(MaterialPlugin::<effects::EffectMaterial>::default())
        .add_plugins(bevy::sprite_render::Material2dPlugin::<hud::HudMaterial>::default())
        .add_plugins(movement::MovementPlugin)
        .add_systems(
            PostUpdate,
            (
                rendering::present_entities,
                rendering::present_weapons,
                effects::present,
                sky::present,
                hud::present,
            )
                .before(TransformSystems::Propagate),
        )
        .add_systems(
            PostUpdate,
            eyes::present
                .after(rendering::present_entities)
                .before(TransformSystems::Propagate),
        )
        .add_systems(Startup, setup)
        .add_systems(
            PostUpdate,
            audio::queue
                .after(hud::present)
                .before(TransformSystems::Propagate),
        )
        .add_systems(Last, effects::observe.before(monitor))
        .add_systems(Last, audio::observe.before(monitor))
        .add_systems(Last, monitor);
    let exit = app.run();
    let mut status = status
        .0
        .lock()
        .map_err(|_| anyhow::anyhow!("render status lock poisoned"))?;
    // Readback is retained in memory during the app. PNG/report writes happen
    // only after the host exits, so renderer systems perform no blocking I/O.
    let capture_write_error = if let Some(path) = &options.capture {
        match status.captured_image.take() {
            Some(image) => match image.try_into_dynamic() {
                Ok(image) => image
                    .to_rgb8()
                    .save(path)
                    .err()
                    .map(|error| error.to_string()),
                Err(error) => Some(format!("screenshot conversion: {error}")),
            },
            None => Some("screenshot readback did not finish".into()),
        }
    } else {
        None
    };
    let capture_exists = options.capture.as_ref().is_none_or(|path| path.is_file());
    let report = serde_json::json!({
        "runtime": "Bevy 0.19.1 / wgpu gameplay migration preview", "map": options.map, "bsp_revision": revision,
        "render": &*status, "models": model_report, "textures": texture_summary, "texture_errors": texture_errors, "asset_warnings": warnings,
        "capture_file_exists": capture_exists, "capture_write_error": capture_write_error, "spawn_sky_visibility": spawn_sky_visibility,
        "limitations": ["Pause menu/console and campaign transitions are not migrated yet; migrated effects retain incomplete native particles and studio decals", "Audio uses shared script selection/decoding and Bevy sinks; mixing is 2D without Source DSP, spatialization or soundscapes", "Retained incomplete scene/AI/weapon behavior; missing animation clips remain bind poses", "LDR sky uses owned faces/leaf visibility; sky polygon masks, PVS/areaportals, material proxies, dynamic lighting and HDR remain unfinished", "Base textures and baked lightmaps use approximate legacy gamma multiplication; not full Source shader fidelity"]
    });
    std::fs::write(&options.report, serde_json::to_vec_pretty(&report)?)?;
    println!("Report: {}", options.report.display());
    if !matches!(exit, AppExit::Success) {
        bail!("Bevy runner exited with {exit:?}");
    }
    if options.movement_script.is_some()
        && !status.simulation["script_finished"]
            .as_bool()
            .unwrap_or(false)
    {
        bail!("movement script did not finish; inspect report/log");
    }
    if options.capture.is_some()
        && (!status.capture_completed || !capture_exists || capture_write_error.is_some())
    {
        bail!("requested screenshot did not complete; inspect report/log");
    }
    Ok(())
}
type StartupAssets<'w> = (
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<rendering::SourceMaterial>>,
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<AudioSource>>,
    ResMut<'w, Assets<effects::EffectMaterial>>,
);
fn setup(
    mut commands: Commands,
    mut prepared: ResMut<PreparedMap>,
    options: Res<Options>,
    status: Res<Status>,
    (mut meshes, mut materials, mut images, mut sounds, mut effect_materials): StartupAssets,
) {
    let loaded = prepared.0.take().expect("startup map consumed once");
    let (spawn, yaw) = loaded.world.spawn();
    let position = options
        .position
        .unwrap_or_else(|| Vec3::from_array(spawn.to_array()));
    let yaw = options.yaw.unwrap_or(yaw);
    rendering::spawn_map(
        &loaded,
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        &status,
    );
    effects::install(
        &loaded,
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut effect_materials,
        &mut images,
    );
    commands.insert_resource(eyes::Eyes::new(&loaded));
    sky::install(
        &loaded,
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
    );
    commands.insert_resource(sky::Sky::new(
        loaded.bsp,
        loaded.world.background_camera.clone(),
        loaded.sky.as_ref(),
    ));
    effects::adopt(loaded.effects.sprites, &mut commands);
    commands.insert_resource(loaded.gameplay);
    commands.insert_resource(hud::Hud::new(loaded.hud));
    audio::install(&mut commands, &mut sounds, loaded.audio);
    commands.spawn((
        Camera2d,
        Camera {
            order: 2,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::None,
        Msaa::Off,
        bevy::camera::visibility::RenderLayers::layer(2),
    ));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        bevy::camera::visibility::RenderLayers::layer(1),
        Tonemapping::None,
        Msaa::Off,
        Projection::Perspective(PerspectiveProjection {
            fov: 2. * ((54f32.to_radians() / 2.).tan() / (4. / 3.)).atan(),
            near: 0.1,
            far: 1000.,
            ..default()
        }),
        Transform::IDENTITY.looking_to(Vec3::X, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Msaa::Off,
        Projection::Perspective(PerspectiveProjection {
            fov: 2. * ((75f32.to_radians() / 2.).tan() / (4. / 3.)).atan(),
            near: 1.,
            far: 32000.,
            ..default()
        }),
        Transform::from_translation(source_to_bevy(position)).looking_to(
            source_to_bevy(source_direction(yaw, options.pitch)),
            Vec3::Y,
        ),
        FlyCamera,
    ));
}
type EntityDrawQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static rendering::SourceEntity,
        &'static Transform,
        &'static Visibility,
    ),
>;
type HostResources<'w> = (
    Res<'w, Options>,
    Res<'w, movement::Simulation>,
    Res<'w, gameplay::Gameplay>,
    Res<'w, hud::Hud>,
    Res<'w, audio::Audio>,
    Res<'w, sky::Sky>,
    Res<'w, eyes::Eyes>,
    Res<'w, effects::Effects>,
);
fn monitor(
    mut commands: Commands,
    (options, simulation, game, hud, audio, sky, eyes, effects): HostResources,
    mut control: ResMut<CaptureControl>,
    status: Res<Status>,
    adapter: Option<Res<RenderAdapterInfo>>,
    (cameras, draws): (Query<&Transform, With<FlyCamera>>, EntityDrawQuery),
    mut exit: MessageWriter<AppExit>,
) {
    control.frames += 1;
    let mut report = status.0.lock().expect("status lock");
    report.frames = control.frames;
    report.simulation = simulation.report();
    report.simulation["gameplay"] = game.report(&simulation.physics);
    let mut mismatches = 0;
    let mut owned_meshes = 0;
    let mut doors = Vec::new();
    for (owner, transform, visibility) in &draws {
        owned_meshes += 1;
        let state = &game.scene.states[owner.0];
        let (origin, rotation) = simulation
            .physics
            .entity_pose(owner.0)
            .unwrap_or((state.origin, state.rotation));
        let expected = rendering::entity_transform(origin, rotation);
        let hidden = !state.visible || state.killed;
        if transform.translation.distance(expected.translation) > 0.001
            || transform.rotation.dot(expected.rotation).abs() < 0.99999
            || (*visibility == Visibility::Hidden) != hidden
        {
            mismatches += 1;
        }
        if game.world.entities[owner.0].get("targetname") == Some("station_entrance") {
            doors.push(serde_json::json!({"entity":owner.0,"origin":bevy_to_source(transform.translation).to_array(),
                "bevy_rotation":transform.rotation.to_array(),"hidden":hidden}));
        }
    }
    report.presentation = serde_json::json!({"owned_meshes":owned_meshes,"pose_or_visibility_mismatches":mismatches,"station_entrance_draws":doors,"hud":hud.report(),"audio":audio.report(),"sky":sky.report(),"eyes":eyes.report(),"effects":effects.report()});
    if let Some(adapter) = adapter {
        report.adapter = Some(adapter.name.clone());
        report.backend = Some(format!("{:?}", adapter.backend));
    }
    if let Ok(camera) = cameras.single() {
        report.camera_source = bevy_to_source(camera.translation).to_array();
    }
    if report.capture_completed && control.completed_frame.is_none() {
        control.completed_frame = Some(control.frames);
    }
    if options.frames.is_some() || options.movement_script.is_some() {
        let limit = options.frames.unwrap_or(u64::MAX - 600);
        if (control.frames >= limit || simulation.finished) && !control.requested {
            control.requested = true;
            control.requested_frame = Some(control.frames);
            if options.capture.is_some() {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(capture_complete);
            } else {
                exit.write(AppExit::Success);
            }
        }
        if control
            .completed_frame
            .is_some_and(|frame| control.frames >= frame + 5)
        {
            exit.write(AppExit::Success);
        }
        if control
            .requested_frame
            .is_some_and(|frame| control.frames > frame + 600)
        {
            exit.write(AppExit::error());
        }
    }
}
fn capture_complete(event: On<ScreenshotCaptured>, status: Res<Status>) {
    let mut report = status.0.lock().expect("status lock");
    report.capture_completed = true;
    report.capture_size = Some([event.image.width(), event.image.height()]);
    report.captured_image = Some(event.image.clone());
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_axes_round_trip_and_preserve_handedness() {
        let p = Vec3::new(12., -34., 56.);
        assert_eq!(bevy_to_source(source_to_bevy(p)), p);
        assert_eq!(
            source_to_bevy(Vec3::X).cross(source_to_bevy(Vec3::Y)),
            source_to_bevy(Vec3::Z)
        );
    }
    #[test]
    fn source_camera_positive_pitch_looks_up() {
        assert!(source_to_bevy(source_direction(0., 0.5)).y > 0.);
        assert!(source_to_bevy(source_direction(std::f32::consts::FRAC_PI_2, 0.)).z < -0.99);
    }
}
