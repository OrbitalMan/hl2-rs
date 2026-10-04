//! Isolated Bevy/wgpu migration; gameplay still lives in hl2-runtime.
mod assets;
mod rendering;
use anyhow::{Context, Result, bail};
use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::{
        renderer::RenderAdapterInfo,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::{
        CursorGrabMode, CursorOptions, MonitorSelection, PrimaryWindow, WindowMode,
        WindowResolution,
    },
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
                "--help" | "-h" => {
                    println!(
                        "HL2-RS Bevy migration: static owned-map viewer (no gameplay yet).\n--game PATH --map NAME --borderless --width N --height N\n--position X Y Z --yaw DEGREES --pitch DEGREES\n--frames N --capture PNG --report JSON\nClick to capture mouse; WASD fly, Space/Ctrl rise/fall, Shift fast. Esc releases mouse; F10 quits."
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
        if options.capture.is_some() && options.frames.is_none() {
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
}
#[derive(Clone, Resource, Default)]
struct Status(Arc<Mutex<RunStatus>>);
#[derive(Component)]
struct FlyCamera {
    yaw: f32,
    pitch: f32,
}
#[derive(Resource, Default)]
struct CaptureControl {
    frames: u64,
    requested: bool,
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
        "Loading owned {} for Bevy/wgpu static renderer...",
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
    });
    let spawn_sky_visibility = format!("{:?}", loaded.bsp.sky_visibility(loaded.world.spawn().0));
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
                        title: "HL2-RS | Bevy/wgpu migration | static map viewer".into(),
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
        .add_systems(Startup, setup)
        .add_systems(Update, fly_camera)
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
        "runtime": "Bevy 0.19.1 / wgpu migration static renderer", "map": options.map, "bsp_revision": revision,
        "render": &*status, "models": model_report, "textures": texture_summary, "texture_errors": texture_errors, "asset_warnings": warnings,
        "capture_file_exists": capture_exists, "capture_write_error": capture_write_error, "spawn_sky_visibility": spawn_sky_visibility,
        "limitations": ["No gameplay, player collision, AI, scene playback or audio in Bevy yet", "Models use static bind poses; brush entities use authored initial transforms", "No Source sky rendering, PVS/areaportals, material proxies, dynamic lighting or HDR", "Base textures and baked lightmaps use approximate legacy gamma multiplication; not full Source shader fidelity"]
    });
    std::fs::write(&options.report, serde_json::to_vec_pretty(&report)?)?;
    println!("Report: {}", options.report.display());
    if !matches!(exit, AppExit::Success) {
        bail!("Bevy runner exited with {exit:?}");
    }
    if options.capture.is_some()
        && (!status.capture_completed || !capture_exists || capture_write_error.is_some())
    {
        bail!("requested screenshot did not complete; inspect report/log");
    }
    Ok(())
}
fn setup(
    mut commands: Commands,
    mut prepared: ResMut<PreparedMap>,
    options: Res<Options>,
    status: Res<Status>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<rendering::SourceMaterial>>,
    mut images: ResMut<Assets<Image>>,
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
        FlyCamera {
            yaw,
            pitch: options.pitch,
        },
    ));
}
fn fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse: Res<AccumulatedMouseMotion>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut FlyCamera)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::F10) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::Escape) || !window.focused {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if buttons.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    let Ok((mut transform, mut camera)) = cameras.single_mut() else {
        return;
    };
    if cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    camera.yaw -= mouse.delta.x * 0.002;
    camera.pitch = (camera.pitch - mouse.delta.y * 0.002).clamp(-1.55, 1.55);
    let forward = source_to_bevy(source_direction(camera.yaw, camera.pitch));
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let mut motion = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        motion += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        motion -= forward;
    }
    if keys.pressed(KeyCode::KeyD) {
        motion += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        motion -= right;
    }
    if keys.pressed(KeyCode::Space) {
        motion += Vec3::Y;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        motion -= Vec3::Y;
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        900.
    } else {
        300.
    };
    transform.translation += motion.normalize_or_zero() * speed * time.delta_secs().min(0.05);
    transform.rotation = Transform::IDENTITY.looking_to(forward, Vec3::Y).rotation;
}
fn monitor(
    mut commands: Commands,
    options: Res<Options>,
    mut control: ResMut<CaptureControl>,
    status: Res<Status>,
    adapter: Option<Res<RenderAdapterInfo>>,
    cameras: Query<&Transform, With<FlyCamera>>,
    mut exit: MessageWriter<AppExit>,
) {
    control.frames += 1;
    let mut report = status.0.lock().expect("status lock");
    report.frames = control.frames;
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
    if let Some(limit) = options.frames {
        if control.frames >= limit && !control.requested {
            control.requested = true;
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
        if control.frames > limit + 600 {
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
