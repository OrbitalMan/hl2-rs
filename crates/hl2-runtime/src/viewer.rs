use crate::playback::{Action, AttackSuppression};
use crate::{entities::Scene, load, physics::Physics, sandbox::Sandbox, write, Options};
use anyhow::{Context, Result};
use hl2_simulation::actors::{
    prepare_choreography_animations, prepare_npcs, prepare_weapons, tick_npcs,
};
use macroquad::prelude::*;
use modkit_core::movement::{Input as MoveInput, Player, TICK};
use modkit_core::{Block, ModContext, ModPlugin, World};
use source_assets::{vpk::Vfs, vtf};
use std::{collections::HashMap, path::Path};

pub fn config(o: &Options) -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: Conf {
            window_title: "HL2-RS | Experimental Rust runtime".into(),
            window_width: o.width,
            window_height: o.height,
            fullscreen: o.borderless,
            // Explicit pixel-size comparisons should avoid Windows bitmap scaling.
            high_dpi: o.borderless || o.width != 1280 || o.height != 720,
            sample_count: 1,
            ..Default::default()
        },
        draw_call_vertex_capacity: 16384,
        draw_call_index_capacity: 32768,
        ..Default::default()
    }
}
fn v3(p: glam::Vec3) -> Vec3 {
    vec3(p.x, p.y, p.z)
}
struct DrawBatch {
    background: bool,
    mesh: Mesh,
    center: Vec3,
    radius: f32,
    lightmap: Option<usize>,
    kind: usize,
    secondary: Option<Texture2D>,
    scroll: Vec2,
    skin: Vec<(glam::Vec3, Option<modkit_core::animation::Weights>)>,
}
pub(crate) fn texture(vfs: &Vfs, name: &str) -> Result<Texture2D> {
    let base = vfs
        .base_texture(name)?
        .or(vfs.material_value(name, "$refracttinttexture")?)
        .context("VMT base texture not found")?;
    texture_file(vfs, &base)
}
fn texture_file(vfs: &Vfs, base: &str) -> Result<Texture2D> {
    let file = format!(
        "materials/{}.vtf",
        base.trim_start_matches("materials/")
            .trim_end_matches(".vtf")
    );
    let image = vtf::decode(&vfs.read(&file)?.context("VTF not found")?, 512)?;
    let texture = Texture2D::from_rgba8(image.width, image.height, &image.rgba);
    texture.set_filter(FilterMode::Linear);
    // Main-thread-only GPU access; no alias is retained across a frame.
    unsafe {
        get_internal_gl().quad_context.texture_set_wrap(
            texture.raw_miniquad_id(),
            miniquad::TextureWrap::Repeat,
            miniquad::TextureWrap::Repeat,
        );
    }
    Ok(texture)
}
fn make_batch(
    mesh: Mesh,
    skin: Vec<(glam::Vec3, Option<modkit_core::animation::Weights>)>,
    lightmap: Option<usize>,
    kind: usize,
    secondary: Option<Texture2D>,
    scroll: Vec2,
) -> DrawBatch {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        min = min.min(v.position);
        max = max.max(v.position);
    }
    DrawBatch {
        background: false,
        center: (min + max) * 0.5,
        radius: (max - min).length() * 0.5,
        mesh,
        lightmap,
        kind,
        secondary,
        scroll,
        skin,
    }
}
fn meshes(
    world: &World,
    vfs: &Vfs,
    cache: &mut HashMap<String, Option<Texture2D>>,
) -> (Vec<DrawBatch>, usize, Vec<String>) {
    let mut batches = Vec::new();
    let mut loaded = 0;
    let mut missing = Vec::new();
    for s in &world.surfaces {
        let batch_start = batches.len();
        let kind = crate::rendering::kind(vfs, &s.material);
        let secondary_name = vfs.material_value(&s.material, "$texture2").ok().flatten();
        let secondary = secondary_name.as_ref().and_then(|name| {
            cache
                .entry(format!("texture2:{name}"))
                .or_insert_with(|| texture_file(vfs, name).ok())
                .clone()
        });
        let rate = vfs
            .material_value(&s.material, "texturescrollrate")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.);
        let angle = vfs
            .material_value(&s.material, "texturescrollangle")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.)
            .to_radians();
        let scroll = vec2(angle.cos(), angle.sin()) * rate;
        let texture = cache
            .entry(s.material.clone())
            .or_insert_with(|| match texture(vfs, &s.material) {
                Ok(t) => Some(t),
                Err(e) => {
                    missing.push(format!("{}: {e:#}", s.material));
                    None
                }
            })
            .clone();
        if texture.is_some() {
            loaded += 1;
        }
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: texture.clone(),
        };
        let mut remap = HashMap::new();
        let mut skin = Vec::new();
        for tri in s.indices.as_chunks::<3>().0 {
            if mesh.indices.len() + 3 > 30000 || mesh.vertices.len() + 3 > 15000 {
                batches.push(make_batch(
                    mesh,
                    skin,
                    s.lightmap,
                    kind,
                    secondary.clone(),
                    scroll,
                ));
                mesh = Mesh {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                    texture: texture.clone(),
                };
                remap.clear();
                skin = Vec::new();
            }
            for &id in tri {
                let next = mesh.vertices.len() as u16;
                let index = *remap.entry(id).or_insert_with(|| {
                    let p = &s.vertices[id as usize];
                    skin.push((p.position, p.skin.clone()));
                    mesh.vertices.push(Vertex {
                        position: v3(p.position),
                        uv: vec2(p.uv.x, p.uv.y),
                        color: if texture.is_some() {
                            p.color
                        } else {
                            [210, 120, 210, 255]
                        },
                        normal: vec4(p.light_uv.x, p.light_uv.y, 0., 0.),
                    });
                    next
                });
                mesh.indices.push(index);
            }
        }
        if !mesh.indices.is_empty() {
            batches.push(make_batch(
                mesh,
                skin,
                s.lightmap,
                kind,
                secondary.clone(),
                scroll,
            ));
        }
        for batch in &mut batches[batch_start..] {
            batch.background = s.background;
        }
    }
    (batches, loaded, missing)
}
fn entity_meshes(
    world: &World,
    vfs: &Vfs,
    cache: &mut HashMap<String, Option<Texture2D>>,
) -> Vec<(usize, Vec<DrawBatch>, f32)> {
    let mut result = Vec::new();
    for (id, e) in world.entities.iter().enumerate() {
        if e.class().starts_with("trigger_") || e.class().starts_with("func_areaportal") {
            continue;
        }
        let surfaces = e
            .get("model")
            .and_then(|s| s.strip_prefix('*'))
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|i| world.brush_models.iter().find(|m| m.id == i))
            .map(|m| &m.surfaces);
        if let Some(surfaces) = surfaces {
            let local = World {
                surfaces: surfaces.clone(),
                ..Default::default()
            };
            result.push((id, meshes(&local, vfs, cache).0, 1.));
        }
    }
    for instance in &world.model_instances {
        if let Some(id) = instance.entity {
            if let Some(surfaces) = world.model_assets.get(&instance.asset_key()) {
                let local = World {
                    surfaces: surfaces.clone(),
                    ..Default::default()
                };
                result.push((id, meshes(&local, vfs, cache).0, instance.scale));
            }
        }
    }
    result
}
fn weapon_meshes(
    world: &World,
    vfs: &Vfs,
    weapons: &std::collections::BTreeMap<String, crate::gameplay::Weapon>,
    cache: &mut HashMap<String, Option<Texture2D>>,
) -> std::collections::BTreeMap<String, Vec<DrawBatch>> {
    weapons
        .iter()
        .filter_map(|(name, w)| {
            world
                .model_assets
                .get(&format!("{}#0", w.viewmodel.to_lowercase()))
                .map(|surfaces| {
                    let local = World {
                        surfaces: surfaces.clone(),
                        ..Default::default()
                    };
                    (name.clone(), meshes(&local, vfs, cache).0)
                })
        })
        .collect()
}
fn capture(path: &Path) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    get_screen_data().export_png(path.to_str().context("screenshot path is not UTF8")?);
    Ok(())
}
fn projectile_meshes(
    vfs: &Vfs,
    cache: &mut HashMap<String, Option<Texture2D>>,
) -> (HashMap<String, Vec<DrawBatch>>, Vec<String>) {
    let mut models = HashMap::new();
    let mut errors = Vec::new();
    {
        let path = crate::projectiles::GRENADE_MODEL;
        match source_assets::models::read_model(vfs, path, 0) {
            Ok(surfaces) => {
                let local = World {
                    surfaces,
                    ..Default::default()
                };
                let (batches, _, missing) = meshes(&local, vfs, cache);
                errors.extend(missing);
                models.insert(path.into(), batches);
            }
            Err(error) => errors.push(format!("{path}: {error:#}")),
        }
    }
    (models, errors)
}

fn apply_selection(
    result: crate::selection::SelectionResult,
    inventory: &mut crate::gameplay::Inventory,
    weapons: &std::collections::BTreeMap<String, crate::gameplay::Weapon>,
    scene: &mut Scene,
) {
    if let Some(weapon) = result.weapon {
        inventory.give(&weapon, weapons, scene.time);
    }
    scene.sounds.extend(
        result
            .sounds
            .into_iter()
            .map(crate::sounds::SoundRequest::from),
    );
}
fn confirm_held_selection(
    selection: &mut crate::selection::Selection,
    inventory: &mut crate::gameplay::Inventory,
    weapons: &std::collections::BTreeMap<String, crate::gameplay::Weapon>,
    scene: &mut Scene,
    suppression: &mut AttackSuppression,
    held: (bool, bool),
    queued: (&mut bool, &mut bool),
) -> bool {
    if selection.pending.is_none()
        || !(held.0 && suppression.primary_allowed() || held.1 && suppression.secondary_allowed())
    {
        return false;
    }
    // Source ProcessInput tests held IN_ATTACK|IN_ATTACK2, including the frame
    // a slot/wheel opens selection, and clears each input button independently.
    let result = selection.confirm(inventory, weapons, scene.time);
    if result.weapon.is_none() {
        apply_selection(result, inventory, weapons, scene);
        return false;
    }
    apply_selection(result, inventory, weapons, scene);
    suppression.consume();
    suppression.update(held.0, held.1);
    *queued.0 = false;
    *queued.1 = false;
    true
}

struct ConsoleContext<'a> {
    inventory: &'a mut crate::gameplay::Inventory,
    weapons: &'a std::collections::BTreeMap<String, crate::gameplay::Weapon>,
    scene: &'a mut Scene,
    player: &'a mut Player,
    position: &'a mut glam::Vec3,
    yaw: &'a mut f32,
    pitch: &'a mut f32,
    fly: &'a mut bool,
    vfs: &'a Vfs,
    map: &'a mut Option<String>,
    quit: &'a mut bool,
}
fn apply_console_effects(
    console: &mut crate::console::Console,
    effects: Vec<crate::console::Effect>,
    context: ConsoleContext<'_>,
) {
    use crate::console::Effect;
    for effect in effects {
        match effect {
            Effect::Loadout => {
                for name in context.weapons.keys() {
                    context
                        .inventory
                        .give(name, context.weapons, context.scene.time);
                }
                context.inventory.refill_ammo(context.weapons);
                context.inventory.suit = true;
                context
                    .inventory
                    .give("weapon_crowbar", context.weapons, context.scene.time);
                console.log("Granted the six implemented weapons, ammunition and suit. Remaining HL2 weapons are not implemented.");
            }
            Effect::Noclip(value) => {
                *context.fly = value.unwrap_or(!*context.fly);
                *context.player = Player::new(*context.position);
                console.log(format!("noclip {}", u8::from(*context.fly)));
            }
            Effect::Getpos => {
                // Retail GetPos defaults to the rendered camera origin, not feet.
                let feet = *context.position;
                console.log(format!(
                    "setpos {:.6} {:.6} {:.6}; setang {:.6} {:.6} 0",
                    feet.x,
                    feet.y,
                    feet.z,
                    -context.pitch.to_degrees(),
                    context.yaw.to_degrees()
                ));
            }
            Effect::Setpos { x, y, z } => {
                let feet = glam::Vec3::new(x, y, z.unwrap_or(context.player.feet.z));
                context.player.feet = feet;
                // Retail setpos calls SetAbsOrigin: it does not clear velocity.
                *context.position = context.player.eye();
                console.log(format!("Player origin: {} {} {}", feet.x, feet.y, feet.z));
            }
            Effect::Setang(angles) => {
                *context.pitch = -angles.x.to_radians().clamp(-1.53, 1.53);
                *context.yaw = angles.y.to_radians();
                console.log(format!(
                    "View angles: {} {} 0",
                    -context.pitch.to_degrees(),
                    context.yaw.to_degrees()
                ));
            }
            Effect::Map(map) => match context.vfs.read(&format!("maps/{map}.bsp")) {
                Ok(Some(bytes)) if !bytes.is_empty() => {
                    *context.map = Some(map);
                }
                Ok(_) => console.log(format!(
                    "Map not found or empty in mounted owned content: {map}"
                )),
                Err(error) => console.log(format!("Map lookup failed: {error:#}")),
            },
            Effect::Fire {
                target,
                input,
                parameter,
                delay,
            } => {
                if context.scene.send_named(&target, &input, &parameter, delay) {
                    console.log(format!("Queued {target}.{input} after {delay} seconds."));
                }
            }
            Effect::Quit => *context.quit = true,
        }
    }
}

pub async fn run(mut o: Options) -> Result<()> {
    if o.borderless {
        // The vendored Windows backend uses a desktop-sized popup, without a
        // display-mode switch. Correct its initial default window position too.
        macroquad::miniquad::window::set_fullscreen(true);
        next_frame().await;
    }
    clear_background(BLACK);
    draw_text("Reading your HL2 map and textures...", 35., 75., 30., WHITE);
    next_frame().await;
    let (mut bsp, mut world, mut vfs) = load(&o)?;
    let mut model_report = source_assets::models::append_models(&mut world, &vfs);
    let weapons = crate::gameplay::definitions(&vfs)?;
    let mut selection = crate::selection::Selection::new();
    let hud = crate::hud::WeaponHud::load(&vfs)?;
    let mut impacts = crate::impacts::Impacts::new(&vfs, &world);
    let mut attack_suppression = AttackSuppression::default();
    let mut inventory = crate::gameplay::Inventory::default();
    prepare_weapons(&mut world, &vfs, &weapons)?;
    println!("Model report: {}", serde_json::to_string(&model_report)?);
    let mut texture_cache = HashMap::new();
    let (mut batches, mut loaded, mut missing) = meshes(&world, &vfs, &mut texture_cache);
    let mut dynamic = entity_meshes(&world, &vfs, &mut texture_cache);
    let mut viewmodels = weapon_meshes(&world, &vfs, &weapons, &mut texture_cache);
    let (mut projectile_models, mut projectile_model_errors) =
        projectile_meshes(&vfs, &mut texture_cache);
    let mut projectiles = crate::projectiles::Projectiles::default();
    let mut projectile_visuals = crate::projectile_rendering::ProjectileVisuals::new(&vfs);
    let mut physics = Physics::new(&world);
    let mut scene = Scene::new(&world);
    scene.load_choreography(&world, &vfs)?;
    prepare_choreography_animations(&mut world, &vfs, &scene);
    let mut npcs = prepare_npcs(&mut world, &vfs, &o.map, bsp.revision);
    let mut navigation = crate::navigation_report(&vfs, &o.map, bsp.revision);
    let mut materials = crate::rendering::Materials::new(&world)?;
    let (mut sky_background, mut sky_asset_error) = crate::sky::prepare(&vfs, &world.entities);

    let mut audio = crate::sounds::Audio::new(&vfs);
    audio.ambient(&vfs, &world).await?;
    println!(
        "Collision: {} colliders, {} rigid bodies, {} skipped",
        physics.colliders.len(),
        physics.bodies.len(),
        physics.skipped
    );
    let mut triangles: usize = world.surfaces.iter().map(|s| s.indices.len() / 3).sum();
    let (mut spawn, mut spawn_yaw) = world.spawn();
    let mut position = o.position.unwrap_or(spawn);
    let mut yaw = if o.position.is_some() {
        o.yaw
    } else {
        spawn_yaw
    };
    let mut pitch = o.pitch;
    let mut sandbox = Sandbox::read(&o.mods).unwrap_or_else(|e| {
        eprintln!("Mod config: {e}");
        Sandbox::empty()
    });
    sandbox.on_load(&world);
    let mut placed = Vec::<Block>::new();
    let mut fly = o.fly;
    let mut player = Player::new(position);
    let mut accumulator = 0f32;
    let mut replay = Vec::new();
    let mut markers = false;
    let mut grabbed = false;
    let mut queued_attack = false;
    let mut queued_secondary = false;
    let mut playback = crate::playback::Playback::read(o.input_script.as_deref())?;
    let playback_enabled = o.input_script.is_some();
    let mut input_events = Vec::new();
    let mut capture_snapshots = Vec::new();
    let mut playback_movement: Option<MoveInput> = None;
    let mut animation_events = Vec::new();
    let mut weapon_sounds = hl2_simulation::sounds::WeaponAnimationSounds::default();
    let mut debug_hud = false;
    let mut sky_2d_frames = 0u64;
    let mut sky_3d_frames = 0u64;
    let mut sky_visibility_error: Option<String> = None;
    let mut console = crate::console::Console::load(&vfs);
    let mut playback_time = 0f64;
    let mut jump_suppressed = false;
    let mut pending_console_map: Option<String> = None;
    let input_trace = std::env::var_os("HL2_RS_INPUT_TRACE").is_some();

    let mut frame = 0u32;
    let mut message = String::new();
    let mut smoke_start = None;
    let mut smoke_end = None;
    let mins = glam::Vec3::new(-16., -16., -64.);
    let maxs = glam::Vec3::new(16., 16., 8.);
    println!(
        "Loaded {}: {triangles} triangles, {loaded}/{} textures, {} solid brushes",
        o.map,
        world.surfaces.len(),
        world.brushes.len()
    );
    for e in &missing {
        eprintln!("Texture fallback: {e}");
    }
    loop {
        let dt = (get_frame_time() * o.time_scale).clamp(0.0001, 0.05);
        playback_time += f64::from(dt);
        frame += 1;
        if input_trace {
            for key in get_keys_pressed() {
                println!("Input frame {frame}: key {key:?}");
            }
            if is_mouse_button_pressed(MouseButton::Left) {
                println!("Input frame {frame}: left click");
            }
            if is_mouse_button_pressed(MouseButton::Right) {
                println!("Input frame {frame}: right click");
            }
        }
        selection.tick(&inventory, &weapons, scene.time);
        // Playback uses a host clock so a script can resume a paused simulation.
        let mut actions = playback.poll(playback_time);
        let mut requested_captures = Vec::new();
        let mut requested_quit = false;
        let mut requested_use = false;
        attack_suppression.update(
            is_mouse_button_down(MouseButton::Left) || playback.held,
            is_mouse_button_down(MouseButton::Right) || playback.secondary_held,
        );
        let mode_before_input = console.mode;
        let cancel_selection =
            is_key_pressed(KeyCode::Escape) && !console.paused() && selection.pending.is_some();
        if cancel_selection {
            actions.push(Action::Cancel);
        }
        if is_key_pressed(KeyCode::Escape) && !cancel_selection {
            console.escape();
        }
        if is_key_pressed(KeyCode::GraveAccent) {
            console.toggle();
        }
        let mode_key_changed = console.mode != mode_before_input;
        let mut console_effects = if mode_key_changed {
            while get_char_pressed().is_some() {}
            Vec::new()
        } else {
            console.input()
        };
        // Drain character events in gameplay too: they must not reappear in a later console.
        if console.mode != crate::console::Mode::Console {
            while get_char_pressed().is_some() {}
        }
        let mut ui_transition = console.mode != mode_before_input;
        if ui_transition {
            grabbed = !console.paused();
            queued_attack = false;
            queued_secondary = false;
            attack_suppression.consume();
            jump_suppressed = true;
            set_cursor_grab(grabbed);
            show_mouse(!grabbed);
        }
        if !is_key_down(KeyCode::Space) && !playback_movement.is_some_and(|input| input.jump) {
            jump_suppressed = false;
        }
        let capturing = !console.paused()
            && !ui_transition
            && is_mouse_button_pressed(MouseButton::Left)
            && !grabbed;
        if capturing {
            grabbed = true;
            set_cursor_grab(true);
            show_mouse(false);
        }
        if grabbed
            && !capturing
            && !console.paused()
            && !ui_transition
            && is_mouse_button_pressed(MouseButton::Left)
            && selection.pending.is_none()
            && attack_suppression.primary_allowed()
        {
            queued_attack = true;
        }
        if grabbed
            && !capturing
            && !console.paused()
            && !ui_transition
            && is_mouse_button_pressed(MouseButton::Right)
            && selection.pending.is_none()
            && attack_suppression.secondary_allowed()
        {
            queued_secondary = true;
        }
        if grabbed && !capturing && !console.paused() && !ui_transition {
            let d = mouse_delta_position();
            if input_trace && d.length_squared() > 0. {
                println!("Input frame {frame}: motion {d:?}");
            }
            yaw += d.x * screen_width() * 0.5 * 0.001152;
            // macroquad returns previous - current: moving up is a positive Y delta.
            pitch += d.y * screen_height() * 0.5 * 0.001152;
        }
        if !console.paused() && is_key_down(KeyCode::Left) {
            yaw += dt * 1.5;
        }
        if !console.paused() && is_key_down(KeyCode::Right) {
            yaw -= dt * 1.5;
        }
        if !console.paused() && is_key_down(KeyCode::Up) {
            pitch += dt;
        }
        if !console.paused() && is_key_down(KeyCode::Down) {
            pitch -= dt;
        }
        pitch = pitch.clamp(-1.53, 1.53);
        if !console.paused() && is_key_pressed(KeyCode::F2) {
            fly = !fly;
            player = Player::new(position);
            message = if fly {
                "Fly mode"
            } else {
                "Walk mode: fixed-tick movement and collision"
            }
            .into();
        }
        if !console.paused() && is_key_pressed(KeyCode::Tab) {
            markers = !markers;
        }
        if !console.paused() && is_key_pressed(KeyCode::F1) {
            debug_hud = !debug_hud;
        }
        if !console.paused() && is_key_pressed(KeyCode::F4) {
            position = spawn;
            yaw = spawn_yaw;
            pitch = 0.;
            player = Player::new(position);
        }
        if !console.paused() && is_key_pressed(KeyCode::F5) {
            match Sandbox::read(&o.mods) {
                Ok(mut s) => {
                    s.on_load(&world);
                    sandbox = s;
                    message = "Mod reloaded".into();
                }
                Err(e) => message = format!("Mod error: {e}"),
            }
        }
        let direction = glam::Vec3::new(
            yaw.cos() * pitch.cos(),
            yaw.sin() * pitch.cos(),
            pitch.sin(),
        );
        let forward = if fly {
            direction
        } else {
            glam::Vec3::new(yaw.cos(), yaw.sin(), 0.)
        };
        let right = glam::Vec3::new(yaw.sin(), -yaw.cos(), 0.);
        let mut movement = glam::Vec3::ZERO;
        if is_key_down(KeyCode::W) {
            movement += forward;
        }
        if is_key_down(KeyCode::S) {
            movement -= forward;
        }
        if is_key_down(KeyCode::D) {
            movement += right;
        }
        if is_key_down(KeyCode::A) {
            movement -= right;
        }
        if fly {
            if is_key_down(KeyCode::Q) {
                movement += glam::Vec3::Z;
            }
            if is_key_down(KeyCode::E) {
                movement -= glam::Vec3::Z;
            }
        }
        if fly && !console.paused() && !ui_transition {
            let speed = if is_key_down(KeyCode::LeftShift) {
                600.
            } else {
                190.
            };
            position += movement.normalize_or_zero() * speed * dt;
            player = Player::new(position);
        }
        let mut input = MoveInput {
            forward: f32::from(is_key_down(KeyCode::W)) - f32::from(is_key_down(KeyCode::S)),
            side: f32::from(is_key_down(KeyCode::D)) - f32::from(is_key_down(KeyCode::A)),
            yaw,
            jump: !jump_suppressed && is_key_down(KeyCode::Space),
            crouch: is_key_down(KeyCode::LeftControl),
            sprint: is_key_down(KeyCode::LeftShift),
            slow: is_key_down(KeyCode::LeftAlt),
        };
        if !console.paused() && is_key_pressed(KeyCode::F3) {
            actions.push(Action::Loadout);
        }
        let wheel = mouse_wheel().1;
        if !console.paused() && wheel != 0. {
            actions.push(Action::Wheel {
                delta: if wheel > 0. { -1 } else { 1 },
            });
        }
        for (slot, key) in [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
        ]
        .into_iter()
        .enumerate()
        {
            if !console.paused() && is_key_pressed(key) {
                actions.push(Action::Slot { slot });
            }
        }
        if !console.paused() && !fly && is_key_pressed(KeyCode::Q) {
            actions.push(Action::Previous);
        }
        if !console.paused() && is_key_pressed(KeyCode::R) {
            actions.push(Action::Reload);
        }
        for action in actions {
            let before = serde_json::json!({"active":inventory.active,"shots":inventory.shots,"pending":selection.pending,"ui":console.mode,"scene_time":scene.time,"player":player});
            let previous_mode = console.mode;
            // UI focus consumes gameplay actions, but releases still rearm held buttons.
            let blocked = console.paused()
                && !matches!(
                    action,
                    Action::Escape
                        | Action::ToggleConsole
                        | Action::Resume
                        | Action::Console { .. }
                        | Action::Capture { .. }
                        | Action::Quit
                        | Action::FireUp
                        | Action::SecondaryUp
                );
            if blocked {
                if matches!(action, Action::FireDown) {
                    playback.held = true;
                    attack_suppression.consume();
                }
                if matches!(action, Action::SecondaryDown) {
                    playback.secondary_held = true;
                    attack_suppression.consume();
                }
                input_events.push(serde_json::json!({"time":scene.time,"playback_time":playback_time,"action":action,"blocked_by_ui":true,"before":before,"after":before}));
                continue;
            }
            match &action {
                Action::Move {
                    forward,
                    side,
                    jump,
                    crouch,
                    sprint,
                    slow,
                } => {
                    playback_movement = Some(MoveInput {
                        forward: *forward,
                        side: *side,
                        yaw,
                        jump: *jump,
                        crouch: *crouch,
                        sprint: *sprint,
                        slow: *slow,
                    });
                }
                Action::Escape => {
                    if !console.paused() && selection.pending.is_some() {
                        let result = selection.cancel();
                        apply_selection(result, &mut inventory, &weapons, &mut scene);
                    } else {
                        console.escape();
                    }
                }
                Action::ToggleConsole => console.toggle(),
                Action::Resume => console.mode = crate::console::Mode::Gameplay,
                Action::Console { command } => console_effects.extend(console.submit(command)),
                Action::Loadout => {
                    for name in weapons.keys() {
                        inventory.give(name, &weapons, scene.time);
                    }
                    inventory.refill_ammo(&weapons);
                    inventory.suit = true;
                    inventory.give("weapon_crowbar", &weapons, scene.time);
                    message = "Developer loadout: six primary weapons".into();
                }
                Action::Slot { slot } => {
                    let result = selection.slot(&inventory, &weapons, *slot, scene.time);
                    apply_selection(result, &mut inventory, &weapons, &mut scene);
                    queued_attack = false;
                    queued_secondary = false;
                }
                Action::Wheel { delta } => {
                    let result = selection.wheel(&inventory, &weapons, *delta, scene.time);
                    apply_selection(result, &mut inventory, &weapons, &mut scene);
                    queued_attack = false;
                    queued_secondary = false;
                }
                Action::Confirm => {
                    if selection.pending.is_some() {
                        let result = selection.confirm(&inventory, &weapons, scene.time);
                        apply_selection(result, &mut inventory, &weapons, &mut scene);
                        attack_suppression.consume();
                        queued_attack = false;
                        queued_secondary = false;
                    }
                }
                Action::Cancel => {
                    let result = selection.cancel();
                    apply_selection(result, &mut inventory, &weapons, &mut scene);
                }
                Action::Previous => {
                    let result = selection.last(&inventory, &weapons);
                    apply_selection(result, &mut inventory, &weapons, &mut scene);
                }
                Action::Fire | Action::FireDown | Action::Secondary | Action::SecondaryDown => {
                    if matches!(action, Action::FireDown) {
                        playback.held = true;
                    }
                    let secondary = matches!(action, Action::Secondary | Action::SecondaryDown);
                    if matches!(action, Action::SecondaryDown) {
                        playback.secondary_held = true;
                    }
                    let allowed = if secondary {
                        attack_suppression.secondary_allowed()
                    } else {
                        attack_suppression.primary_allowed()
                    };
                    if allowed && selection.pending.is_some() {
                        let result = selection.confirm(&inventory, &weapons, scene.time);
                        apply_selection(result, &mut inventory, &weapons, &mut scene);
                        attack_suppression.consume();
                        queued_attack = false;
                        queued_secondary = false;
                    } else if allowed && secondary {
                        queued_secondary = true;
                    } else if allowed {
                        queued_attack = true;
                    }
                }
                Action::FireUp => {
                    playback.held = false;
                }
                Action::SecondaryUp => {
                    playback.secondary_held = false;
                }
                Action::Look {
                    yaw: new_yaw,
                    pitch: new_pitch,
                } => {
                    yaw = new_yaw.to_radians();
                    pitch = new_pitch.to_radians().clamp(-1.53, 1.53);
                }
                Action::ActorPose {
                    target,
                    origin,
                    yaw,
                } => {
                    if !scene.fixture_actor_pose(
                        &world,
                        target,
                        glam::Vec3::from_array(*origin),
                        *yaw,
                    ) {
                        world
                            .warnings
                            .push(format!("fixture actor pose target missing: {target}"));
                    }
                }
                Action::Reload => inventory.reload(&weapons, &mut scene, &world),
                Action::Capture { name } => requested_captures.push(name.clone()),
                Action::Use => requested_use = true,
                Action::Quit => {
                    requested_quit = true;
                }
            }
            if console.mode != previous_mode {
                ui_transition = true;
                grabbed = !console.paused();
                queued_attack = false;
                queued_secondary = false;
                attack_suppression.consume();
                jump_suppressed = true;
                set_cursor_grab(grabbed);
                show_mouse(!grabbed);
            }
            apply_console_effects(
                &mut console,
                std::mem::take(&mut console_effects),
                ConsoleContext {
                    inventory: &mut inventory,
                    weapons: &weapons,
                    scene: &mut scene,
                    player: &mut player,
                    position: &mut position,
                    yaw: &mut yaw,
                    pitch: &mut pitch,
                    fly: &mut fly,
                    vfs: &vfs,
                    map: &mut pending_console_map,
                    quit: &mut requested_quit,
                },
            );
            // Observe each ordered script transition, so an up/down pair in one
            // poll rearms the button before its new press is dispatched.
            let held = (
                is_mouse_button_down(MouseButton::Left) || playback.held,
                is_mouse_button_down(MouseButton::Right) || playback.secondary_held,
            );
            attack_suppression.update(held.0, held.1);
            if !console.paused() && !ui_transition {
                confirm_held_selection(
                    &mut selection,
                    &mut inventory,
                    &weapons,
                    &mut scene,
                    &mut attack_suppression,
                    held,
                    (&mut queued_attack, &mut queued_secondary),
                );
            }
            input_events.push(serde_json::json!({"time":scene.time,"playback_time":playback_time,"action":action,"before":before,"after":{"active":inventory.active,"shots":inventory.shots,"pending":selection.pending,"clips":inventory.owned,"reloading":inventory.is_reloading(),"ui":console.mode,"sv_cheats":console.cheats,"scene_time":scene.time,"player":player,"fly":fly}}));
        }
        apply_console_effects(
            &mut console,
            console_effects,
            ConsoleContext {
                inventory: &mut inventory,
                weapons: &weapons,
                scene: &mut scene,
                player: &mut player,
                position: &mut position,
                yaw: &mut yaw,
                pitch: &mut pitch,
                fly: &mut fly,
                vfs: &vfs,
                map: &mut pending_console_map,
                quit: &mut requested_quit,
            },
        );
        if let Some(command) = playback_movement {
            input = MoveInput {
                yaw,
                jump: command.jump && !jump_suppressed,
                ..command
            };
        }
        if !console.paused() && !ui_transition {
            confirm_held_selection(
                &mut selection,
                &mut inventory,
                &weapons,
                &mut scene,
                &mut attack_suppression,
                (
                    is_mouse_button_down(MouseButton::Left) || playback.held,
                    is_mouse_button_down(MouseButton::Right) || playback.secondary_held,
                ),
                (&mut queued_attack, &mut queued_secondary),
            );
        }
        // Playback look commands enter the same camera state before that frame's simulation.
        let direction = glam::Vec3::new(
            yaw.cos() * pitch.cos(),
            yaw.sin() * pitch.cos(),
            pitch.sin(),
        );
        let simulation_dt = if console.paused() || ui_transition {
            0.
        } else {
            dt
        };
        if simulation_dt == 0. {
            accumulator = 0.;
            queued_attack = false;
            queued_secondary = false;
        }
        accumulator += simulation_dt;
        while accumulator >= TICK {
            scene.tick(&world, player.feet, TICK);
            inventory.advance_projectile_fire(&world, &mut scene, &weapons, position, direction);
            let input_allowed = (grabbed || playback_enabled)
                && (!capturing || playback_enabled)
                && selection.pending.is_none();
            let primary = input_allowed
                && attack_suppression.primary_allowed()
                && (is_mouse_button_down(MouseButton::Left) || queued_attack || playback.held);
            let secondary = input_allowed
                && attack_suppression.secondary_allowed()
                && matches!(
                    inventory.active.as_str(),
                    "weapon_shotgun" | "weapon_smg1" | "weapon_ar2"
                )
                && (is_mouse_button_down(MouseButton::Right)
                    || queued_secondary
                    || playback.secondary_held);
            inventory.set_attack_input(primary, secondary);
            let attacking = primary
                || secondary
                || input_allowed
                    && (attack_suppression.primary_allowed() && inventory.delayed_attack
                        || attack_suppression.secondary_allowed()
                            && inventory.delayed_secondary_attack);
            inventory.tick(&world, &mut scene, &weapons, player.feet, attacking, TICK);
            if input_allowed
                && attack_suppression.secondary_allowed()
                && (secondary || inventory.delayed_secondary_attack)
            {
                inventory.secondary_attack(
                    &weapons,
                    &world,
                    &mut scene,
                    &mut physics,
                    position,
                    direction,
                );
            } else if input_allowed
                && attack_suppression.primary_allowed()
                && (primary || inventory.delayed_attack)
            {
                inventory.attack(
                    &weapons,
                    &world,
                    &mut scene,
                    &mut physics,
                    position,
                    direction,
                );
            }
            queued_attack = false;
            queued_secondary = false;
            for (id, state) in scene.states.iter().enumerate() {
                physics.set_entity(id, state.origin, state.rotation, state.collides());
            }
            for launch in inventory.projectile_spawns.drain(..) {
                projectiles.spawn(launch, &mut scene);
            }
            physics.refresh_entity_queries();
            tick_npcs(&mut npcs, &mut scene, &world, &mut physics, &player, fly);
            let damage = projectiles.tick(
                &world,
                &mut scene,
                &mut physics,
                player.feet,
                player.crouched,
                TICK,
            );
            inventory.apply_projectile_damage(damage, &world, &mut scene, &mut physics);
            physics.tick(TICK);
            if !fly {
                player.step(input, &physics, TICK);
                position = player.eye();
            }
            if o.smoke {
                replay.push(serde_json::json!({"tick":player.ticks,"position":position.to_array(),"velocity":player.velocity.to_array(),"grounded":player.grounded}));
            }
            accumulator -= TICK;
        }
        for (hit, melee) in inventory.impacts.drain(..) {
            impacts.add(hit, melee, &world, &physics, &mut scene, &vfs);
        }
        for event in weapon_sounds.events(&world, &inventory, &weapons, scene.time) {
            scene.sounds.push(event.options.clone().into());
            animation_events.push(serde_json::json!({"time":scene.time,"weapon":inventory.active,"clip":inventory.animation,"event":event}));
        }
        for sound in scene.sounds.drain(..) {
            audio.play_request(&vfs, &sound, false, 0.4).await?;
        }
        if !console.paused()
            && !ui_transition
            && (is_key_pressed(KeyCode::E) || requested_use)
            && !fly
        {
            if let Some((id, _)) = physics.ray(position, direction, 96.) {
                scene.use_entity_at(&world, id, player.feet);
            }
        }
        if !console.paused()
            && !ui_transition
            && is_mouse_button_pressed(MouseButton::Middle)
            && grabbed
        {
            if let Some((id, _)) = physics.ray(position, direction, 128.) {
                physics.impulse(id, direction, 6.);
                message = "Applied a prop impulse".into();
            }
        }
        let console_map = pending_console_map.take();
        let direct_map = console_map.is_some();
        if let Some((map, landmark)) = console_map
            .map(|map| (map, String::new()))
            .or_else(|| scene.transition.take())
        {
            let previous_time = scene.time;
            let previous = position;
            let mut next = o.clone();
            next.map = map;
            match load(&next) {
                Ok((new_bsp, mut new_world, new_vfs)) => {
                    if direct_map {
                        inventory = crate::gameplay::Inventory::default();
                    }
                    selection.pending = None;
                    model_report = source_assets::models::append_models(&mut new_world, &new_vfs);
                    prepare_weapons(&mut new_world, &new_vfs, &weapons)?;
                    (spawn, spawn_yaw) = new_world.spawn();
                    position = if direct_map {
                        spawn
                    } else {
                        hl2_simulation::campaign::arrival(&world, &new_world, previous, &landmark)
                    };
                    if !direct_map {
                        inventory.rebase_clock(previous_time, 0.);
                    }
                    player = Player::new(position);
                    audio.stop();
                    world = new_world;
                    bsp = new_bsp;
                    triangles = world.surfaces.iter().map(|s| s.indices.len() / 3).sum();
                    vfs = new_vfs;
                    texture_cache.clear();
                    (batches, loaded, missing) = meshes(&world, &vfs, &mut texture_cache);
                    dynamic = entity_meshes(&world, &vfs, &mut texture_cache);
                    viewmodels = weapon_meshes(&world, &vfs, &weapons, &mut texture_cache);
                    (projectile_models, projectile_model_errors) =
                        projectile_meshes(&vfs, &mut texture_cache);
                    projectiles = crate::projectiles::Projectiles::default();
                    projectile_visuals = crate::projectile_rendering::ProjectileVisuals::new(&vfs);
                    physics = Physics::new(&world);
                    scene = Scene::with_campaign(&world, false);
                    scene.load_choreography(&world, &vfs)?;
                    prepare_choreography_animations(&mut world, &vfs, &scene);
                    npcs = prepare_npcs(&mut world, &vfs, &next.map, bsp.revision);
                    impacts = crate::impacts::Impacts::new(&vfs, &world);
                    navigation = crate::navigation_report(&vfs, &next.map, bsp.revision);
                    materials = crate::rendering::Materials::new(&world)?;
                    (sky_background, sky_asset_error) = crate::sky::prepare(&vfs, &world.entities);
                    sky_2d_frames = 0;
                    sky_3d_frames = 0;
                    sky_visibility_error = None;
                    audio = crate::sounds::Audio::new(&vfs);
                    audio.ambient(&vfs, &world).await?;
                    o = next;
                    message = format!("Loaded {}", o.map);
                    if direct_map {
                        console.log(message.clone());
                        console.mode = crate::console::Mode::Gameplay;
                        fly = false;
                        yaw = spawn_yaw;
                        pitch = 0.;
                        playback_movement = None;
                        jump_suppressed = true;
                        grabbed = true;
                        attack_suppression.consume();
                        queued_attack = false;
                        queued_secondary = false;
                        accumulator = 0.;
                        set_cursor_grab(true);
                        show_mouse(false);
                    }
                }
                Err(e) => {
                    message = format!("Map transition failed: {e:#}");
                    if direct_map {
                        console.log(message.clone());
                    }
                }
            }
        }
        if !console.paused() && !ui_transition && is_key_pressed(KeyCode::B) {
            if let Some(hit) = world.raycast(position, direction, 4096.) {
                let p = ((hit - direction * 20.) / 32.).round() * 32.;
                placed.push(Block {
                    position: p,
                    size: 32.,
                    color: [60, 210, 220, 255],
                });
                message = "Placed a portable mod block".into();
            }
        }
        if !console.paused() && !ui_transition && is_key_pressed(KeyCode::Backspace) {
            placed.pop();
        }
        sandbox.tick(
            simulation_dt,
            &mut ModContext {
                world: &world,
                blocks: &mut placed,
                player: position,
            },
        );
        if o.smoke {
            if frame == 20 {
                smoke_start = Some(position);
                let end = world.slide(position, glam::Vec3::new(24., 12., 0.), mins, maxs);
                position = end;
                player = Player::new(position);
                smoke_end = Some(end);
                placed.push(Block {
                    position: position + direction * 96.,
                    size: 24.,
                    color: [50, 200, 220, 255],
                });
            }
            if frame == 35 {
                let mut reload = Sandbox::read(&o.mods)?;
                reload.on_load(&world);
                sandbox = reload;
            }
        }
        clear_background(Color::new(0.32, 0.42, 0.53, 1.));
        let world_camera = Camera3D {
            position: v3(position),
            target: v3(position + direction),
            up: vec3(0., 0., 1.),
            fovy: 2. * (75f32.to_radians().mul_add(0.5, 0.).tan() / (4. / 3.)).atan(),
            z_near: 1.,
            z_far: 32000.,
            ..Default::default()
        };
        set_camera(&world_camera);
        let mut animated_vertices = 0usize;
        for (id, meshes, _) in &mut dynamic {
            let Some(instance) = world.model_instances.iter().find(|i| i.entity == Some(*id))
            else {
                continue;
            };
            let Some(rig) = world.rigs.get(&instance.asset_key()) else {
                continue;
            };
            let state = &scene.states[*id];
            let name = &state.animation;
            if !rig.clips.contains_key(name) {
                continue;
            }
            let matrices = scene.actor_matrices(rig, *id);
            for batch in meshes {
                for (v, (bind, weights)) in batch.mesh.vertices.iter_mut().zip(&batch.skin) {
                    if let Some(weights) = weights {
                        v.position = v3(modkit_core::animation::skin(*bind, weights, &matrices));
                        animated_vertices += 1;
                    }
                }
            }
        }
        // Opaque geometry first; transparent world and entity surfaces share a back-to-front pass.
        let mut draws = Vec::new();
        let sky_visibility = match bsp.sky_visibility(position) {
            Ok(visibility) => visibility,
            Err(error) => {
                if sky_visibility_error.is_none() {
                    let message = format!("Sky visibility: {error:#}");
                    eprintln!("{message}");
                    sky_visibility_error = Some(message);
                }
                source_assets::bsp::SkyVisibility::default()
            }
        };
        let sky_2d_visible = sky_visibility.background_visible() && sky_background.is_some();
        let sky_3d_visible = sky_visibility.sky_3d && world.background_camera.is_some();
        if let Some(sky) = sky_background.as_mut().filter(|_| sky_2d_visible) {
            sky.draw(&world_camera, v3(direction));
            sky_2d_frames += 1;
            set_camera(&world_camera);
        }
        if let Some(sky) = world.background_camera.as_ref().filter(|_| sky_3d_visible) {
            sky_3d_frames += 1;
            let origin = position / sky.scale + sky.origin;
            set_camera(&Camera3D {
                position: v3(origin),
                target: v3(origin + direction),
                up: vec3(0., 0., 1.),
                fovy: 2. * (75f32.to_radians().mul_add(0.5, 0.).tan() / (4. / 3.)).atan(),
                z_near: 0.1,
                z_far: 32000.,
                ..Default::default()
            });
            for batch in batches.iter().filter(|b| b.background) {
                materials.select(
                    batch.lightmap,
                    batch.kind,
                    batch.secondary.as_ref(),
                    batch.scroll * scene.time as f32,
                    vec4(1., 1., 1., 1.),
                );
                draw_mesh(&batch.mesh);
            }
            for (id, meshes, scale) in &dynamic {
                if !world.background_entities.contains(id)
                    || !scene.states[*id].visible
                    || scene.states[*id].killed
                {
                    continue;
                }
                let state = &scene.states[*id];
                let transform = Mat4::from_scale_rotation_translation(
                    Vec3::splat(*scale),
                    state.rotation,
                    v3(state.origin),
                );
                for batch in meshes {
                    materials.select(
                        batch.lightmap,
                        batch.kind,
                        batch.secondary.as_ref(),
                        batch.scroll * scene.time as f32,
                        vec4(1., 1., 1., 1.),
                    );
                    unsafe {
                        get_internal_gl().quad_gl.push_model_matrix(transform);
                    }
                    draw_mesh(&batch.mesh);
                    unsafe {
                        get_internal_gl().quad_gl.pop_model_matrix();
                    }
                }
            }
            unsafe {
                let mut gl = get_internal_gl();
                gl.flush();
                gl.quad_context.clear(None, Some(1.), None);
            }
            set_camera(&world_camera);
        }
        for batch in &batches {
            if batch.background {
                continue;
            }
            let to = batch.center - v3(position);
            if to.length() > batch.radius + 100. && to.dot(v3(direction)) + batch.radius < 0. {
                continue;
            }
            draws.push((
                batch,
                Mat4::IDENTITY,
                to.length_squared(),
                vec4(1., 1., 1., 1.),
            ));
        }
        for (id, meshes, scale) in &dynamic {
            if world.background_entities.contains(id) {
                continue;
            }
            let Some(state) = scene.states.get(*id) else {
                continue;
            };
            if state.killed || !state.visible {
                continue;
            }
            let (origin, rotation) = physics
                .entity_pose(*id)
                .unwrap_or((state.origin, state.rotation));
            let transform =
                Mat4::from_scale_rotation_translation(Vec3::splat(*scale), rotation, v3(origin));
            let e = &world.entities[*id];
            let color = modkit_core::parse_vec3(e.get("rendercolor").unwrap_or("255 255 255"))
                .unwrap_or(glam::Vec3::splat(255.))
                / 255.;
            let alpha = e
                .get("renderamt")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(255.)
                / 255.;
            let tint = vec4(color.x, color.y, color.z, alpha);
            for batch in meshes {
                let distance =
                    (transform.transform_point3(batch.center) - v3(position)).length_squared();
                draws.push((batch, transform, distance, tint));
            }
        }
        for projectile in &projectiles.active {
            if projectile.kind == crate::projectiles::ProjectileKind::CombineBall {
                continue;
            }
            let Some(meshes) = projectile_models.get(projectile.model()) else {
                continue;
            };
            let scale = if projectile.kind == crate::projectiles::ProjectileKind::CombineBall {
                projectile.physical_radius() / 10.
            } else {
                1.
            };
            let transform = Mat4::from_scale_rotation_translation(
                Vec3::splat(scale),
                crate::physics::angles(projectile.angles),
                v3(projectile.position),
            );
            for batch in meshes {
                let distance =
                    (transform.transform_point3(batch.center) - v3(position)).length_squared();
                draws.push((batch, transform, distance, vec4(1., 1., 1., 1.)));
            }
        }
        draws.sort_by(|(a, _, da, _), (b, _, db, _)| {
            (a.kind >= 2).cmp(&(b.kind >= 2)).then_with(|| {
                if a.kind >= 2 {
                    db.total_cmp(da)
                } else {
                    a.lightmap.cmp(&b.lightmap)
                }
            })
        });
        for (batch, transform, _, tint) in draws {
            materials.select(
                batch.lightmap,
                batch.kind,
                batch.secondary.as_ref(),
                batch.scroll * scene.time as f32,
                tint,
            );
            unsafe {
                get_internal_gl().quad_gl.push_model_matrix(transform);
            }
            draw_mesh(&batch.mesh);
            unsafe {
                get_internal_gl().quad_gl.pop_model_matrix();
            }
        }
        impacts.draw(&scene, &physics, &materials);
        projectile_visuals.draw(
            &projectiles,
            v3(position),
            v3(direction),
            scene.time,
            simulation_dt == 0.,
            &materials,
            &physics,
        );
        gl_use_default_material();
        for b in sandbox.blocks.iter().chain(&placed) {
            draw_cube(
                v3(b.position),
                Vec3::splat(b.size),
                None,
                Color::from_rgba(b.color[0], b.color[1], b.color[2], b.color[3]),
            );
            draw_cube_wires(v3(b.position), Vec3::splat(b.size), DARKBLUE);
        }
        if markers {
            for e in &world.entities {
                let p = e.origin();
                if p.distance(position) < 1500. && p != glam::Vec3::ZERO {
                    draw_cube_wires(
                        v3(p),
                        vec3(12., 12., 20.),
                        if e.class().starts_with("npc_") {
                            ORANGE
                        } else {
                            GREEN
                        },
                    );
                }
            }
        }
        if let Some(w) = weapons.get(&inventory.active) {
            if let Some(meshes) = viewmodels.get_mut(&inventory.active) {
                let rig = world.rigs.get(&format!("{}#0", w.viewmodel.to_lowercase()));
                let elapsed = (scene.time - inventory.animation_at) as f32;
                let clip = if rig
                    .and_then(|r| r.clips.get(&inventory.animation))
                    .is_some_and(|c| elapsed < c.duration())
                {
                    inventory.animation.as_str()
                } else {
                    inventory.idle_animation()
                };
                if let Some(rig) = rig {
                    let matrices = rig.matrices(
                        clip,
                        if clip == inventory.idle_animation() {
                            scene.time as f32
                        } else {
                            elapsed
                        },
                    );
                    for batch in meshes.iter_mut() {
                        for (vertex, (bind, weights)) in
                            batch.mesh.vertices.iter_mut().zip(&batch.skin)
                        {
                            if let Some(weights) = weights {
                                vertex.position =
                                    v3(modkit_core::animation::skin(*bind, weights, &matrices));
                            }
                        }
                    }
                }
                unsafe {
                    let mut gl = get_internal_gl();
                    gl.flush();
                    gl.quad_context.clear(None, Some(1.), None);
                }
                set_camera(&Camera3D {
                    position: Vec3::ZERO,
                    target: vec3(1., 0., 0.),
                    up: vec3(0., 0., 1.),
                    fovy: 2. * (54f32.to_radians().mul_add(0.5, 0.).tan() / (4. / 3.)).atan(),
                    z_near: 0.1,
                    z_far: 1000.,
                    ..Default::default()
                });
                for batch in meshes.iter() {
                    materials.select(
                        None,
                        batch.kind,
                        batch.secondary.as_ref(),
                        Vec2::ZERO,
                        vec4(1., 1., 1., 1.),
                    );
                    draw_mesh(&batch.mesh);
                }
                gl_use_default_material();
                set_camera(&Camera3D {
                    position: v3(position),
                    target: v3(position + direction),
                    up: vec3(0., 0., 1.),
                    fovy: 2. * (75f32.to_radians().mul_add(0.5, 0.).tan() / (4. / 3.)).atan(),
                    z_near: 1.,
                    z_far: 32000.,
                    ..Default::default()
                });
            }
        }
        set_default_camera();
        if debug_hud {
            draw_rectangle(
                0.,
                0.,
                screen_width(),
                105.,
                Color::new(0.025, 0.045, 0.07, 0.88),
            );
            draw_text(
                format!("HL2-RS  /  Rust runtime  /  {}", o.map),
                22.,
                30.,
                27.,
                WHITE,
            );
            draw_text(
                format!(
                "{triangles} triangles  |  {loaded}/{} textures  |  {} entities  |  {}  |  {} fps",
                world.surfaces.len(),
                world.entities.len(),
                if fly { "FLY" } else { "WALK" },
                get_fps()
            ),
                22.,
                55.,
                19.,
                LIGHTGRAY,
            );
            draw_text("WASD / click: mouse / Esc pause / tilde console / F2 fly / Q,E vertical in fly / Tab entities / B block",22.,78.,17.,LIGHTGRAY);
            draw_text(
            "E use / Ctrl crouch / Space jump / R reload / 1-6 / wheel: weapon menu / Q last / F3 dev loadout / F4 reset / F2 fly",
            22.,
            98.,
            16.,
            Color::from_rgba(240, 180, 90, 255),
        );
            draw_rectangle(
                0.,
                screen_height() - 38.,
                screen_width(),
                38.,
                Color::new(0.025, 0.045, 0.07, 0.85),
            );
            draw_text(
                format!(
                    "{}  |  position {:.1}, {:.1}, {:.1}  |  {}",
                    sandbox.name(),
                    position.x,
                    position.y,
                    position.z,
                    message
                ),
                20.,
                screen_height() - 13.,
                18.,
                WHITE,
            );
        }
        if !console.paused() {
            hud.draw_status(&inventory, &weapons, &selection, scene.time);
            scene.sounds.extend(
                hud.drain_sounds()
                    .into_iter()
                    .map(crate::sounds::SoundRequest::from),
            );
            hud.draw_selection(&selection, &inventory, &weapons, scene.time);
            hud.draw_crosshair(&inventory);
        }
        console.draw();
        if is_key_pressed(KeyCode::F12) {
            capture(Path::new("artifacts/manual-capture.png"))?;
            write(
                Path::new("artifacts/manual-camera.json"),
                &serde_json::to_vec_pretty(
                    &serde_json::json!({"position":position.to_array(),"yaw":yaw,"pitch":pitch}),
                )?,
            )?;
            message = "Saved artifacts/manual-capture.png".into();
        }
        for name in requested_captures {
            capture(&Path::new("artifacts").join(format!("{name}.png")))?;
            let doors: Vec<_> = world.entities.iter().enumerate().filter(|(_, entity)| entity.class().contains("door")).filter_map(|(id, entity)| scene.states.get(id).map(|state| serde_json::json!({"entity":id,"targetname":entity.get("targetname").unwrap_or(""),"origin":state.origin.to_array(),"rotation":state.rotation.to_array(),"locked":state.locked}))).collect();
            capture_snapshots.push(serde_json::json!({"name":name,"scene_time":scene.time,"playback_time":playback_time,"player":player,"position":position.to_array(),"yaw":yaw,"pitch":pitch,"ui":console.mode,"shots":inventory.shots,"active":inventory.active,"clips":inventory.owned,"reserve_ammo":inventory.reserve_ammo,"suit":inventory.suit,"fly":fly,"pending":selection.pending,"doors":doors}));
            let snapshot = capture_snapshots.last_mut().unwrap();
            snapshot["projectiles"] = serde_json::json!(projectiles.active);
            snapshot["effects"] = serde_json::json!(projectiles.effects);
            snapshot["projectile_diagnostics"] = serde_json::json!(projectiles.diagnostics);
            snapshot["health"] = serde_json::json!(inventory.health);
            snapshot["armor"] = serde_json::json!(inventory.armor);
            snapshot["ar2_charge_until"] = serde_json::json!(inventory.charge_until());
            snapshot["choreography"] = serde_json::json!(scene.choreography_states(&world));
            snapshot["look_targets"] = serde_json::json!(scene.look_targets.report());
            snapshot["actor_animations"] = serde_json::json!(scene.animation_states(&world));
            snapshot["navigation"] = serde_json::json!(navigation);
            snapshot["npc_movement"] = serde_json::json!(npcs.snapshots());
        }
        if o.frames.is_some_and(|n| frame >= n) || is_key_pressed(KeyCode::F10) || requested_quit {
            if let Some(path) = &o.capture {
                capture(path)?;
            }
            let mut report = serde_json::json!({"map":o.map,"time_scale":o.time_scale,"frames":frame,"triangles":triangles,"textures_loaded":loaded,"materials":world.surfaces.len(),"models":model_report,"animated_vertices":animated_vertices,"simulation":scene.diagnostics,"inventory":inventory,"colliders":physics.colliders.len(),"rigid_bodies":physics.bodies.len(),"skipped_colliders":physics.skipped,"player":player,"replay":replay,"input_events":input_events,"animation_events":animation_events,"rig_warnings":world.rigs.iter().filter(|(_,r)| !r.warnings.is_empty()).map(|(n,r)|(n.clone(),r.warnings.clone())).collect::<std::collections::BTreeMap<_,_>>(),"impact_decals":impacts.created,"unclippable_impacts":impacts.unclippable,"background_surfaces":batches.iter().filter(|b| b.background).count(),"background_entities":world.background_entities.len(),"sky_3d_visible":sky_3d_visible,"sky_3d_frames":sky_3d_frames,"sky_visibility_error":sky_visibility_error,"audio_variants":audio.variants_played,"audio_played":audio.played,"audio_errors":audio.errors,"lightmap_pages":world.lightmaps.len(),"texture_errors":missing,"position":position.to_array(),"yaw":yaw,"pitch":pitch,"mod":sandbox.name(),"mod_blocks":sandbox.blocks.len()+placed.len(),"smoke_start":smoke_start.map(|p|p.to_array()),"smoke_end":smoke_end.map(|p|p.to_array()),"warnings":world.warnings});
            report["sky_2d"] = serde_json::json!(sky_background.as_ref().map(|sky| sky.report()));
            report["sky_asset_error"] = serde_json::json!(sky_asset_error);
            report["sky_visibility"] = serde_json::to_value(sky_visibility)?;
            report["sky_2d_visible"] = serde_json::json!(sky_2d_visible);
            report["sky_2d_frames"] = serde_json::json!(sky_2d_frames);
            report["console"] = serde_json::json!({"mode":console.mode,"sv_cheats":console.cheats,"output":console.output,"playback_time":playback_time});
            report["capture_snapshots"] = serde_json::json!(capture_snapshots);
            report["choreography"] = serde_json::json!(scene.choreography_states(&world));
            report["look_targets"] = serde_json::json!(scene.look_targets.report());
            report["actor_animations"] = serde_json::json!(scene.animation_states(&world));
            report["navigation"] = serde_json::json!(navigation);
            report["npc_movement"] = serde_json::json!(npcs.snapshots());
            report["scene_time"] = serde_json::json!(scene.time);
            report["audio_decoded"] = serde_json::json!(audio.decoded);
            report["ar2_charge_until"] = serde_json::json!(inventory.charge_until());
            report["projectiles"] = serde_json::json!({"active":projectiles.active,"effects":projectiles.effects,"diagnostics":projectiles.diagnostics,"model_errors":projectile_model_errors});
            report["projectile_visuals"] = serde_json::json!({"errors":projectile_visuals.errors,"ball_frames":projectile_visuals.ball_frames,"effect_frames":projectile_visuals.effect_frames,"particle_emitters":projectile_visuals.particles.diagnostics,"missing_particle_draws":projectile_visuals.missing_particle_draws});
            report["native_static_collision"] = serde_json::json!({"convex_colliders":physics.native_shape_count,"hull_fallbacks":physics.native_shape_fallbacks});
            write(
                Path::new("artifacts/runtime-report.json"),
                &serde_json::to_vec_pretty(&report)?,
            )?;
            set_cursor_grab(false);
            show_mouse(true);
            break;
        }
        next_frame().await;
    }
    Ok(())
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use crate::gameplay::{Inventory, Weapon};
    use crate::selection::Selection;
    use std::collections::BTreeMap;

    fn inventory() -> (Inventory, BTreeMap<String, Weapon>, Scene) {
        let weapons = [("weapon_pistol", 1), ("weapon_shotgun", 3)]
            .into_iter()
            .map(|(class, slot)| {
                (
                    class.into(),
                    Weapon {
                        slot,
                        magazine: 6,
                        default_clip: 6,
                        ammo_type: "Buckshot".into(),
                        ..Default::default()
                    },
                )
            })
            .collect();
        let mut inv = Inventory::default();
        inv.suit = true;
        inv.give("weapon_pistol", &weapons, 0.);
        inv.give("weapon_shotgun", &weapons, 0.);
        let mut scene = Scene::new(&World::default());
        scene.time = 1.;
        (inv, weapons, scene)
    }

    #[test]
    fn opening_selection_with_either_attack_held_confirms_and_consumes_it() {
        for held in [(true, false), (false, true)] {
            let (mut inv, weapons, mut scene) = inventory();
            let mut selection = Selection::new();
            let mut suppression = AttackSuppression::default();
            let mut queued_primary = true;
            let mut queued_secondary = true;
            let result = selection.slot(&inv, &weapons, 1, scene.time);
            apply_selection(result, &mut inv, &weapons, &mut scene);
            assert!(confirm_held_selection(
                &mut selection,
                &mut inv,
                &weapons,
                &mut scene,
                &mut suppression,
                held,
                (&mut queued_primary, &mut queued_secondary),
            ));
            assert_eq!(inv.active, "weapon_pistol");
            assert!(selection.pending.is_none());
            assert!(!queued_primary && !queued_secondary);
            assert_eq!(suppression.primary_allowed(), !held.0);
            assert_eq!(suppression.secondary_allowed(), !held.1);
            assert_eq!(inv.shots, 0);
        }
    }

    #[test]
    fn consumed_secondary_cannot_reconfirm_but_a_new_primary_press_can() {
        let (mut inv, weapons, mut scene) = inventory();
        let mut selection = Selection::new();
        let mut suppression = AttackSuppression::default();
        let mut queued_primary = false;
        let mut queued_secondary = false;
        selection.slot(&inv, &weapons, 1, scene.time);
        confirm_held_selection(
            &mut selection,
            &mut inv,
            &weapons,
            &mut scene,
            &mut suppression,
            (false, true),
            (&mut queued_primary, &mut queued_secondary),
        );
        selection.slot(&inv, &weapons, 3, scene.time);
        assert!(!confirm_held_selection(
            &mut selection,
            &mut inv,
            &weapons,
            &mut scene,
            &mut suppression,
            (false, true),
            (&mut queued_primary, &mut queued_secondary),
        ));
        assert_eq!(inv.active, "weapon_pistol");
        assert!(confirm_held_selection(
            &mut selection,
            &mut inv,
            &weapons,
            &mut scene,
            &mut suppression,
            (true, true),
            (&mut queued_primary, &mut queued_secondary),
        ));
        assert_eq!(inv.active, "weapon_shotgun");
        assert!(selection.pending.is_none());
        assert!(!suppression.primary_allowed() && !suppression.secondary_allowed());
    }
}
