//! On-demand asynchronous late precaching for player weapons and optional attachments.
//! Mirrors [`crate::runtime_npcs`]: lightweight script metadata stays loaded everywhere, while
//! heavy player viewmodel meshes, animations, materials, projectiles, and audio load on a
//! background worker when granted mid-play (e.g. `F3` / `impulse 101`) or carried into play.
//! NPC world models and physics (`w_*.mdl`, `.phy`) stay split into a shared world-weapon helper
//! reused by map loading and [`crate::runtime_npcs`].
use crate::{
    assets::{MaterialData, load_material},
    effects::Effects,
    gameplay::Gameplay,
    rendering::{
        AnimatedMesh, Batch, ModelCache, ModelLit, SourceMaterial, WeaponMesh, WeaponSkin,
        alpha_mode, gpu_image_filtered, lod, runtime::draw_material,
    },
    ui_host::Console,
};
use anyhow::Result;
use bevy::{
    asset::Assets,
    camera::visibility::RenderLayers,
    ecs::{
        resource::Resource,
        system::{Commands, Res, ResMut, SystemParam},
    },
    image::Image,
    math::{Mat4, Vec4},
    mesh::Mesh,
    pbr::MeshMaterial3d,
    prelude::{AlphaMode, Mesh3d, Name, Transform, Visibility},
};
use hl2_simulation::gameplay::Weapon;
use modkit_core::Surface;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Instant,
};

/// One optional bone-merged or attached visual accessory on a weapon bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentSpec {
    pub id: String,
    pub model: String,
    pub skin: usize,
}

/// Generic asset bundle description for any stock or mod player weapon plus optional attachments.
#[derive(Clone, Debug)]
pub struct WeaponBundleSpec {
    pub weapon_id: String,
    pub viewmodel: String,
    pub projectile_models: Vec<String>,
    pub attachments: Vec<AttachmentSpec>,
    pub sound_cues: Vec<String>,
}

impl WeaponBundleSpec {
    pub fn from_weapon(weapon: &Weapon, attachments: Vec<AttachmentSpec>) -> Self {
        let mut sound_cues: Vec<String> = weapon.sounds.values().cloned().collect();
        sound_cues.extend(
            hl2_simulation::gameplay::extra_sound_cues(&weapon.class)
                .iter()
                .copied()
                .map(str::to_owned),
        );
        Self {
            weapon_id: weapon.class.clone(),
            viewmodel: weapon.viewmodel.clone(),
            projectile_models: hl2_simulation::gameplay::projectile_models(&weapon.class)
                .iter()
                .map(|m| (*m).to_owned())
                .collect(),
            attachments,
            sound_cues,
        }
    }
}

/// Queued mid-play weapon acquisition waiting on asynchronous asset preparation.
#[derive(Clone, Debug)]
pub struct PendingWeaponGrant {
    pub weapon_id: String,
    pub equip: bool,
    pub refill_ammo: bool,
    pub priority: bool,
    pub attachments: Vec<AttachmentSpec>,
}

type Slot = Arc<Mutex<Option<Result<PreparedWeaponBundle, String>>>>;

/// Mounted VFS, shared audio cache, and in-flight worker jobs for late-precached weapons.
#[derive(Resource)]
pub struct RuntimeWeapons {
    vfs: Arc<source_assets::vpk::Vfs>,
    audio_reuse: Arc<crate::audio::reuse::AudioReuse>,
    jobs: BTreeMap<String, Job>,
    loaded: BTreeSet<String>,
}

struct Job {
    started: Instant,
    priority: bool,
    waiters: Vec<PendingWeaponGrant>,
    result: Slot,
}

impl RuntimeWeapons {
    pub(crate) fn new(
        vfs: Arc<source_assets::vpk::Vfs>,
        audio_reuse: Arc<crate::audio::reuse::AudioReuse>,
        precached_weapons: BTreeSet<String>,
    ) -> Self {
        Self {
            vfs,
            audio_reuse,
            jobs: BTreeMap::new(),
            loaded: precached_weapons,
        }
    }
}

pub(crate) struct PrebuiltWeaponDraw {
    name: String,
    mesh: Mesh,
    animation: Option<AnimatedMesh>,
    skin: Option<u8>,
    model_lit: bool,
}

pub(crate) struct PreparedWeaponBundle {
    spec: WeaponBundleSpec,
    world: modkit_core::World,
    gibs: BTreeMap<String, Vec<Surface>>,
    prebuilt_images: Vec<(String, Image)>,
    material_defs: BTreeMap<String, MaterialData>,
    draws: Vec<PrebuiltWeaponDraw>,
    audio: crate::audio::late::PreparedLateAudio,
}

struct WorkerInput {
    vfs: Arc<source_assets::vpk::Vfs>,
    audio_reuse: Arc<crate::audio::reuse::AudioReuse>,
    weapon: Weapon,
    spec: WeaponBundleSpec,
    known_defs: BTreeMap<String, MaterialData>,
    known_textures: BTreeSet<String>,
    existing_models: BTreeSet<String>,
    existing_physics: BTreeSet<String>,
    loaded_waves: BTreeSet<String>,
    config: hl2_ui::config::Config,
    cpu_skinning: bool,
    allow_bc: bool,
}

fn spawn_background(work: impl FnOnce() + Send + 'static) {
    if let Some(pool) = bevy::tasks::AsyncComputeTaskPool::try_get() {
        pool.spawn(async move { work() }).detach();
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("hl2-weapon-precache".into())
        .spawn(move || {
            crate::priority::set_current_thread(crate::priority::Priority::Background);
            work();
        });
    if let Err(error) = spawned {
        bevy::log::warn!("failed to spawn weapon precache worker: {error}");
    }
}

fn load_extra_models(input: &WorkerInput, mini_world: &mut modkit_core::World) {
    let root_lod = input.config.video.root_lod();
    for att in &input.spec.attachments {
        let key = format!("{}#{}", att.model.to_lowercase(), att.skin);
        if input.existing_models.contains(&key) || mini_world.model_assets.contains_key(&key) {
            continue;
        }
        if let Ok(surfaces) =
            source_assets::models::read_model_lod(&input.vfs, &att.model, att.skin, root_lod)
        {
            mini_world.model_assets.insert(key, surfaces);
        }
        for err in source_assets::models::append_rigs(
            mini_world,
            &input.vfs,
            std::slice::from_ref(&att.model),
        ) {
            mini_world.warnings.push(err);
        }
    }
}

/// Loads NPC weapon world models (`w_*.mdl#0`), rigs, and `.phy` physics bodies for `weapon_classes`.
/// Shared between map load (`assets::loading`) and late async NPC precaching (`runtime_npcs`).
pub(crate) fn load_world_weapon_models(
    world: &mut modkit_core::World,
    vfs: &source_assets::vpk::Vfs,
    weapon_classes: &BTreeSet<String>,
    root_lod: usize,
    existing_models: &BTreeSet<String>,
) -> Vec<String> {
    if weapon_classes.is_empty() {
        return Vec::new();
    }
    let Ok(defs) = hl2_simulation::npc_weapons::definitions(vfs) else {
        return Vec::new();
    };
    let weapon_models: Vec<String> = defs
        .into_iter()
        .filter(|(class, _)| weapon_classes.contains(&class.to_ascii_lowercase()))
        .map(|(_, d)| d.world_model)
        .filter(|m| !m.is_empty())
        .filter(|m| !existing_models.contains(&format!("{}#0", m.to_lowercase())))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for failure in source_assets::models::append_rigs(world, vfs, &weapon_models) {
        world.warnings.push(failure);
    }
    for model in &weapon_models {
        let key = format!("{}#0", model.to_lowercase());
        if let std::collections::btree_map::Entry::Vacant(entry) = world.model_assets.entry(key) {
            match source_assets::models::read_model_lod(vfs, model, 0, root_lod) {
                Ok(surfaces) => {
                    entry.insert(surfaces);
                }
                Err(error) => world.warnings.push(format!("{model}: {error:#}")),
            }
        }
        let _ = source_assets::models::append_model_physics(world, vfs, model);
    }
    weapon_models
}

/// Loads `w_grenade.phy` for thrown frags when enabled and not already present.
pub(crate) fn load_frag_projectile_physics(
    world: &mut modkit_core::World,
    vfs: &source_assets::vpk::Vfs,
    existing_physics: &BTreeSet<String>,
) {
    let frag = hl2_simulation::projectiles::FRAG_MODEL;
    if !crate::assets::FRAG_RIGID_BODY.load(std::sync::atomic::Ordering::Relaxed)
        || existing_physics.contains(&format!("{frag}#0"))
    {
        return;
    }
    if let Err(error) = source_assets::models::append_model_physics(world, vfs, frag) {
        world.warnings.push(format!("{frag}: {error:#}"));
    }
}

/// Loads `w_grenade.mdl` render surfaces and physics for late Combine soldiers (`npc_combine_s`).
pub(crate) fn load_frag_projectile(
    world: &mut modkit_core::World,
    vfs: &source_assets::vpk::Vfs,
    root_lod: usize,
    existing_models: &BTreeSet<String>,
    gibs: &mut BTreeMap<String, Vec<Surface>>,
) {
    let frag = hl2_simulation::projectiles::FRAG_MODEL;
    let key = hl2_simulation::gibs::render_key(frag, 0);
    if !existing_models.contains(&key)
        && !gibs.contains_key(&key)
        && let Ok(surfaces) = crate::effects::read_effect_model_lod(vfs, frag, 0, root_lod)
    {
        gibs.insert(key, surfaces);
    }
    load_frag_projectile_physics(world, vfs, &BTreeSet::new());
}

fn load_projectile_models(
    input: &WorkerInput,
    mini_world: &mut modkit_core::World,
) -> BTreeMap<String, Vec<Surface>> {
    let root_lod = input.config.video.root_lod();
    let mut gibs = BTreeMap::new();
    for path in &input.spec.projectile_models {
        let key = hl2_simulation::gibs::render_key(path, 0);
        if input.existing_models.contains(&key) || gibs.contains_key(&key) {
            continue;
        }
        match crate::effects::read_effect_model_lod(&input.vfs, path, 0, root_lod) {
            Ok(surfaces) => {
                gibs.insert(key, surfaces);
            }
            Err(e) => mini_world.warnings.push(format!("{path}: {e:#}")),
        }
    }
    if input.spec.weapon_id == "weapon_frag" {
        load_frag_projectile_physics(mini_world, &input.vfs, &input.existing_physics);
    }
    gibs
}

fn decode_bundle_materials(
    input: &WorkerInput,
    mini_world: &mut modkit_core::World,
    gibs: &BTreeMap<String, Vec<Surface>>,
) -> (Vec<(String, Image)>, BTreeMap<String, MaterialData>) {
    let names: BTreeSet<String> = mini_world
        .model_assets
        .values()
        .chain(gibs.values())
        .flatten()
        .map(|s| s.material.clone())
        .filter(|m| !input.known_defs.contains_key(m))
        .collect();
    let (picmip, hdr, allow_bc) = (
        input.config.video.picmip(),
        input.config.video.hdr.is_hdr(),
        input.allow_bc,
    );
    let filtering = input.config.video.texture_sampling().0;
    let (mut tex_cache, mut frame_cache, mut decoded, mut errors) =
        (BTreeMap::new(), BTreeMap::new(), 0usize, Vec::new());
    let mut material_defs = BTreeMap::new();
    let mut seen_textures = input.known_textures.clone();
    let mut prebuilt_images = Vec::new();
    for name in names {
        let mut material = load_material(
            &input.vfs,
            &name,
            &mut tex_cache,
            &mut frame_cache,
            &mut decoded,
            &mut errors,
            (picmip, hdr, allow_bc),
        );
        let mut convert =
            |image: Option<Arc<source_assets::vtf::GpuImage>>, path: &Option<String>, repeat| {
                let (Some(image), Some(path)) = (image, path) else {
                    return;
                };
                if !seen_textures.insert(path.clone()) {
                    return;
                }
                let owned = Arc::try_unwrap(image).unwrap_or_else(|a| (*a).clone());
                prebuilt_images.push((
                    path.clone(),
                    gpu_image_filtered(owned, repeat, false, filtering),
                ));
            };
        convert(material.base.take(), &material.base_path, true);
        convert(material.iris.take(), &material.iris_path, false);
        convert(
            material.envmap_mask.take(),
            &material.envmap_mask_path,
            true,
        );
        material_defs.insert(name, material);
    }
    mini_world.warnings.extend(errors);
    (prebuilt_images, material_defs)
}

fn build_weapon_draws(
    input: &WorkerInput,
    mini_world: &modkit_core::World,
    material_defs: &BTreeMap<String, MaterialData>,
) -> Vec<PrebuiltWeaponDraw> {
    let model_lower = input.spec.viewmodel.to_lowercase();
    let vm_key = format!("{model_lower}#0");
    let Some(surfaces) = mini_world.model_assets.get(&vm_key) else {
        return Vec::new();
    };
    let lookup_def = |name: &str| {
        material_defs
            .get(name)
            .or_else(|| input.known_defs.get(name))
    };
    let mut batches: BTreeMap<(String, usize), Batch> = BTreeMap::new();
    let mut transparent_id = 0usize;
    let mut append_surface = |surface: &Surface| {
        if surface.indices.is_empty() {
            return;
        }
        let definition = lookup_def(&surface.material);
        let draw_id = if definition
            .is_some_and(|m| matches!(alpha_mode(m), AlphaMode::Blend | AlphaMode::Add))
        {
            transparent_id += 1;
            transparent_id
        } else {
            0
        };
        batches
            .entry((surface.material.clone(), draw_id))
            .or_default()
            .append(surface, Mat4::IDENTITY, definition);
    };
    for surface in surfaces {
        append_surface(surface);
    }
    let (mut skin0, mut skin1) = (BTreeSet::new(), BTreeSet::new());
    if let Some(alternate) = mini_world.model_assets.get(&format!("{model_lower}#1")) {
        for (a, b) in surfaces.iter().zip(alternate) {
            if a.material != b.material {
                skin0.insert(a.material.clone());
                skin1.insert(b.material.clone());
                append_surface(b);
            }
        }
    }
    for att in &input.spec.attachments {
        let key = format!("{}#{}", att.model.to_lowercase(), att.skin);
        if let Some(att_surfaces) = mini_world.model_assets.get(&key) {
            for surface in att_surfaces {
                append_surface(surface);
            }
        }
    }
    let joint_count = if !input.cpu_skinning {
        mini_world
            .rigs
            .get(&vm_key)
            .filter(|rig| !rig.bones.is_empty() && rig.bones.len() < 256)
            .map(|rig| rig.bones.len() + 1)
    } else {
        None
    };
    batches
        .into_iter()
        .map(|((name, _), mut batch)| {
            let definition = lookup_def(&name);
            let model_lit = batch.model && definition.is_none_or(|d| !d.unlit && !d.camera);
            let animation = batch.skin.iter().any(|(_, w)| w.is_some()).then(|| {
                AnimatedMesh::from_batch(
                    &mut batch,
                    (None, Some(input.spec.weapon_id.clone())),
                    (vm_key.clone(), 1.),
                    false,
                )
            });
            let mut mesh = batch.mesh();
            if let (Some(anim), Some(joints)) = (animation.as_ref(), joint_count) {
                lod::apply_skin_attributes(&mut mesh, &anim.vertices.bind, joints);
            }
            let skin = if skin1.contains(&name) {
                Some(1)
            } else if skin0.contains(&name) {
                Some(0)
            } else {
                None
            };
            PrebuiltWeaponDraw {
                name,
                mesh,
                animation,
                skin,
                model_lit,
            }
        })
        .collect()
}

fn prepare_weapon_bundle(input: WorkerInput) -> Result<PreparedWeaponBundle> {
    let mut mini_world = modkit_core::World::default();
    hl2_simulation::actors::prepare_weapon(&mut mini_world, &input.vfs, &input.weapon)?;
    load_extra_models(&input, &mut mini_world);
    let gibs = load_projectile_models(&input, &mut mini_world);
    let (prebuilt_images, material_defs) = decode_bundle_materials(&input, &mut mini_world, &gibs);
    let draws = build_weapon_draws(&input, &mini_world, &material_defs);
    let mut cues: BTreeSet<String> = input.spec.sound_cues.iter().cloned().collect();
    for rig in mini_world.rigs.values() {
        cues.extend(hl2_simulation::sounds::animation_sound_names(rig));
    }
    let audio = crate::audio::late::prepare_npc_audio(
        &input.vfs,
        &input.audio_reuse,
        &input.loaded_waves,
        &cues,
    );
    Ok(PreparedWeaponBundle {
        spec: input.spec,
        world: mini_world,
        gibs,
        prebuilt_images,
        material_defs,
        draws,
        audio,
    })
}

fn complete_grant(game: &mut Gameplay, pending: &PendingWeaponGrant) {
    if pending.equip {
        game.inventory
            .give(&pending.weapon_id, &game.weapons, game.scene.time);
    } else {
        game.inventory
            .grant_without_switch(&pending.weapon_id, &game.weapons);
    }
    if pending.refill_ammo {
        game.inventory.refill_ammo(&game.weapons);
    }
}

#[derive(SystemParam)]
pub struct HostState<'w> {
    game: ResMut<'w, Gameplay>,
    console: ResMut<'w, Console>,
    options: Option<Res<'w, crate::Options>>,
    texture_caps: Option<Res<'w, crate::TextureCaps>>,
    audio: Option<ResMut<'w, crate::audio::Audio>>,
}

#[derive(SystemParam)]
pub struct RenderAssets<'w> {
    cache: ResMut<'w, ModelCache>,
    effects: Option<ResMut<'w, Effects>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<SourceMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    inverse_binds: ResMut<'w, Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
}

fn enqueue_requests(
    weapons: &mut RuntimeWeapons,
    host: &mut HostState<'_>,
    render: &RenderAssets<'_>,
) {
    let mut requests = std::mem::take(&mut host.game.weapon_requests);
    for class in host.game.inventory.owned.keys() {
        if !host.game.precached_weapons.contains(class)
            && !weapons.loaded.contains(class)
            && !weapons.jobs.contains_key(class)
            && !requests.iter().any(|r| &r.weapon_id == class)
        {
            requests.push(PendingWeaponGrant {
                weapon_id: class.clone(),
                equip: false,
                refill_ammo: false,
                priority: false,
                attachments: Vec::new(),
            });
        }
    }
    for pending in requests {
        if host.game.precached_weapons.contains(&pending.weapon_id)
            || weapons.loaded.contains(&pending.weapon_id)
        {
            host.game
                .precached_weapons
                .insert(pending.weapon_id.clone());
            complete_grant(&mut host.game, &pending);
            continue;
        }
        if let Some(job) = weapons.jobs.get_mut(&pending.weapon_id) {
            job.priority |= pending.priority;
            job.waiters.push(pending);
            continue;
        }
        let Some(weapon) = host.game.weapons.get(&pending.weapon_id).cloned() else {
            host.console
                .source
                .log(format!("Unknown weapon: {}", pending.weapon_id));
            continue;
        };
        let spec = WeaponBundleSpec::from_weapon(&weapon, pending.attachments.clone());
        let input = WorkerInput {
            vfs: weapons.vfs.clone(),
            audio_reuse: weapons.audio_reuse.clone(),
            weapon,
            spec,
            known_defs: render.cache.definitions.clone(),
            known_textures: render.cache.textures.keys().cloned().collect(),
            existing_models: host.game.world.model_assets.keys().cloned().collect(),
            existing_physics: host.game.world.model_physics.keys().cloned().collect(),
            loaded_waves: host
                .audio
                .as_deref()
                .map(crate::audio::Audio::loaded_wave_paths)
                .unwrap_or_default(),
            config: host.console.source.config.clone(),
            cpu_skinning: host.options.as_deref().is_some_and(|o| o.cpu_skinning),
            allow_bc: host.texture_caps.as_deref().is_some_and(|c| c.bc),
        };
        let slot: Slot = Arc::default();
        let out = slot.clone();
        let priority = pending.priority;
        let id = pending.weapon_id.clone();
        spawn_background(move || {
            let result = prepare_weapon_bundle(input).map_err(|e| format!("{e:#}"));
            *out.lock().expect("weapon precache slot") = Some(result);
        });
        weapons.jobs.insert(
            id,
            Job {
                started: Instant::now(),
                priority,
                waiters: vec![pending],
                result: slot,
            },
        );
    }
}

fn spawn_installed_draws(
    commands: &mut Commands,
    spec: WeaponBundleSpec,
    draws: Vec<PrebuiltWeaponDraw>,
    game: &Gameplay,
    render: &mut RenderAssets<'_>,
    cpu_skinning: bool,
) {
    let weapon_id = spec.weapon_id;
    let vm_lower = spec.viewmodel.to_lowercase();
    let vm_key = format!("{vm_lower}#0");
    let gpu_skeleton = if !cpu_skinning {
        game.world
            .rigs
            .get(&vm_key)
            .filter(|rig| !rig.bones.is_empty() && rig.bones.len() < 256)
            .map(|rig| {
                let root = commands
                    .spawn((
                        crate::campaign::MapOwned,
                        Transform::IDENTITY,
                        Visibility::Inherited,
                        WeaponMesh(weapon_id.clone()),
                    ))
                    .id();
                crate::gpu_skinning::Skeleton::spawn(
                    commands,
                    &mut render.inverse_binds,
                    rig,
                    1.,
                    root,
                )
            })
    } else {
        None
    };
    let fallback = MaterialData::default();
    for item in draws {
        let definition = render
            .cache
            .definitions
            .get(&item.name)
            .unwrap_or(&fallback);
        let material = render.materials.add(draw_material(
            &render.cache,
            definition,
            item.model_lit,
            Vec4::ONE,
        ));
        let mut draw = commands.spawn((
            crate::campaign::MapOwned,
            Name::new(item.name),
            Mesh3d(render.meshes.add(item.mesh)),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            WeaponMesh(weapon_id.clone()),
            RenderLayers::layer(1),
            Visibility::Hidden,
        ));
        if let Some(skin) = item.skin {
            draw.insert(WeaponSkin(skin));
        }
        if item.model_lit {
            draw.insert(
                ModelLit::new(None, vm_key.clone(), 1., definition.half_lambert)
                    .reflecting(render.cache.reflects(definition)),
            );
        }
        if let Some(mut animation) = item.animation {
            animation.gpu = gpu_skeleton.clone();
            if let Some(skeleton) = &animation.gpu {
                draw.insert((
                    skeleton.component(),
                    bevy::camera::visibility::DynamicSkinnedMeshBounds,
                ));
            }
            animation.insert_into(&mut draw);
        }
    }
}

fn install_bundle(
    commands: &mut Commands,
    weapons: &mut RuntimeWeapons,
    host: &mut HostState<'_>,
    render: &mut RenderAssets<'_>,
    bundle: PreparedWeaponBundle,
    waiters: Vec<PendingWeaponGrant>,
) {
    let cpu_skinning = host.options.as_deref().is_some_and(|o| o.cpu_skinning);
    if let Some(audio) = host.audio.as_deref_mut()
        && audio.is_ready()
    {
        let game = &mut *host.game;
        audio.merge_late(bundle.audio, &mut game.scene.lipsync);
    }
    render.cache.add_prebuilt_materials(
        &mut render.images,
        bundle.prebuilt_images,
        bundle.material_defs,
    );
    if let Some(effects) = render.effects.as_deref_mut() {
        effects.add_models(
            bundle.gibs,
            &render.cache,
            &mut render.meshes,
            &mut render.materials,
        );
    }
    let weapon_id = bundle.spec.weapon_id.clone();
    host.game.merge_model(bundle.world);
    spawn_installed_draws(
        commands,
        bundle.spec,
        bundle.draws,
        &host.game,
        render,
        cpu_skinning,
    );
    weapons.loaded.insert(weapon_id.clone());
    host.game.precached_weapons.insert(weapon_id);
    for pending in waiters {
        complete_grant(&mut host.game, &pending);
    }
}

/// Queue late weapon loads and install at most one completed bundle per frame, prioritized first.
pub fn poll(
    mut commands: Commands,
    mut weapons: ResMut<RuntimeWeapons>,
    mut host: HostState<'_>,
    mut render: RenderAssets<'_>,
) {
    enqueue_requests(&mut weapons, &mut host, &render);
    if weapons.jobs.is_empty() {
        return;
    }
    let ready_key = take_ready_key(&weapons);
    let Some(key) = ready_key else {
        return;
    };
    let job = weapons.jobs.remove(&key).expect("ready weapon job");
    let load_ms = job.started.elapsed().as_secs_f64() * 1000.;
    let result = match job.result.lock() {
        Ok(mut slot) => slot.take().unwrap_or_else(|| Err("missing result".into())),
        Err(_) => Err("weapon precache worker stopped".into()),
    };
    match result {
        Ok(bundle) => install_bundle(
            &mut commands,
            &mut weapons,
            &mut host,
            &mut render,
            bundle,
            job.waiters,
        ),
        Err(error) => {
            host.console
                .source
                .log(format!("Can't precache {} ({load_ms:.0} ms): {error}", key));
        }
    }
}

fn take_ready_key(weapons: &RuntimeWeapons) -> Option<String> {
    let mut fallback = None;
    for (id, job) in &weapons.jobs {
        let ready = job.result.lock().is_ok_and(|slot| slot.is_some());
        if !ready {
            continue;
        }
        if job.priority {
            return Some(id.clone());
        }
        if fallback.is_none() {
            fallback = Some(id.clone());
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Simulation;
    use bevy::{app::Update, ecs::schedule::IntoScheduleConfigs, prelude::App};
    use modkit_core::{Surface, Vertex, World};

    fn test_app(weapons_map: BTreeMap<String, Weapon>) -> App {
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<SourceMaterial>::default());
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(Assets::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>::default());
        app.insert_resource(ModelCache::default());
        let mut game = Gameplay::synthetic(World::default());
        game.weapons = weapons_map;
        app.insert_resource(game);
        app.insert_resource(Simulation::new(
            &World::default(),
            glam::Vec3::ZERO,
            0.,
            0.,
            false,
            Vec::new(),
        ));
        app.insert_resource(Console::new(Default::default()));
        app.insert_resource(RuntimeWeapons::new(
            Arc::new(source_assets::vpk::Vfs::default()),
            Arc::default(),
            BTreeSet::new(),
        ));
        app.add_systems(Update, poll.before(crate::rendering::present_weapons));
        app.add_systems(Update, crate::rendering::present_weapons);
        app
    }

    fn weapon_def(class: &str) -> Weapon {
        Weapon {
            class: class.into(),
            viewmodel: format!(
                "models/weapons/v_{}.mdl",
                class.trim_start_matches("weapon_")
            ),
            magazine: 18,
            default_clip: 18,
            ammo_type: if class == "weapon_crowbar" {
                "None".into()
            } else {
                "Pistol".into()
            },
            ammo_max: 150,
            ..Default::default()
        }
    }

    fn synthetic_bundle(weapon: &Weapon) -> PreparedWeaponBundle {
        let vm_key = format!("{}#0", weapon.viewmodel.to_lowercase());
        let surface = Surface {
            background: false,
            material: "models/weapons/test".into(),
            lightmap: None,
            flex_source: None,
            vertices: [
                glam::Vec3::new(0., 0., 0.),
                glam::Vec3::new(4., 0., 0.),
                glam::Vec3::new(0., 4., 0.),
            ]
            .into_iter()
            .map(|position| Vertex {
                position,
                normal: glam::Vec3::Z,
                uv: glam::Vec2::ZERO,
                light_uv: glam::Vec2::ZERO,
                color: [255; 4],
                skin: None,
            })
            .collect(),
            indices: vec![0, 1, 2],
        };
        let mut batch = Batch::default();
        batch.append(&surface, Mat4::IDENTITY, None);
        let mut world = World::default();
        world.model_assets.insert(vm_key, vec![surface]);
        PreparedWeaponBundle {
            spec: WeaponBundleSpec::from_weapon(weapon, Vec::new()),
            world,
            gibs: BTreeMap::new(),
            prebuilt_images: Vec::new(),
            material_defs: BTreeMap::new(),
            draws: vec![PrebuiltWeaponDraw {
                name: "models/weapons/test".into(),
                mesh: batch.mesh(),
                animation: None,
                skin: None,
                model_lit: false,
            }],
            audio: crate::audio::late::PreparedLateAudio::default(),
        }
    }

    #[test]
    fn amortized_poll_installs_priority_weapon_first_and_one_bundle_per_frame() {
        let crowbar = weapon_def("weapon_crowbar");
        let pistol = weapon_def("weapon_pistol");
        let defs = [
            ("weapon_crowbar".into(), crowbar.clone()),
            ("weapon_pistol".into(), pistol.clone()),
        ]
        .into_iter()
        .collect();
        let mut app = test_app(defs);
        {
            let mut runtime = app.world_mut().resource_mut::<RuntimeWeapons>();
            for (weapon, priority, equip) in [(&pistol, false, false), (&crowbar, true, true)] {
                let slot: Slot = Arc::default();
                *slot.lock().unwrap() = Some(Ok(synthetic_bundle(weapon)));
                runtime.jobs.insert(
                    weapon.class.clone(),
                    Job {
                        started: Instant::now(),
                        priority,
                        waiters: vec![PendingWeaponGrant {
                            weapon_id: weapon.class.clone(),
                            equip,
                            refill_ammo: false,
                            priority,
                            attachments: Vec::new(),
                        }],
                        result: slot,
                    },
                );
            }
        }

        // Frame 1: priority weapon_crowbar installs first while weapon_pistol waits.
        app.update();
        let game = app.world().resource::<Gameplay>();
        assert_eq!(game.inventory.active, "weapon_crowbar");
        assert!(game.precached_weapons.contains("weapon_crowbar"));
        assert!(!game.precached_weapons.contains("weapon_pistol"));

        // Frame 2: second weapon installs without switching active away from crowbar.
        app.update();
        let game = app.world().resource::<Gameplay>();
        assert_eq!(game.inventory.active, "weapon_crowbar");
        assert!(game.precached_weapons.contains("weapon_pistol"));
        assert!(game.inventory.owned.contains_key("weapon_pistol"));
        assert!(app.world().resource::<RuntimeWeapons>().jobs.is_empty());

        let mut draws = app.world_mut().query::<(&WeaponMesh, &Visibility)>();
        let shown: Vec<_> = draws
            .iter(app.world())
            .filter(|(_, v)| **v != Visibility::Hidden)
            .map(|(w, _)| w.0.clone())
            .collect();
        assert_eq!(shown, ["weapon_crowbar"]);

        let mut world = World::default();
        let empty = BTreeSet::new();
        assert!(
            load_world_weapon_models(
                &mut world,
                &source_assets::vpk::Vfs::default(),
                &empty,
                0,
                &empty,
            )
            .is_empty()
        );
        assert!(world.model_assets.is_empty());
    }

    #[test]
    #[ignore = "requires owned HL2 installation"]
    fn owned_unarmed_station02_skips_viewmodels_until_async_loadout_equips_crowbar_first() {
        let root = source_assets::install::discover().expect("owned install");
        let loaded = crate::assets::load(&root, "hl2", "d1_trainstation_02").expect("load map");
        assert!(
            loaded.gameplay.precached_weapons.is_empty(),
            "unarmed station 02 must not preload player viewmodels"
        );
        assert!(
            !loaded
                .world
                .model_assets
                .keys()
                .any(|k| k.starts_with("models/weapons/v_")),
            "no v_*.mdl should be loaded on unarmed station 02"
        );

        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<SourceMaterial>::default());
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(Assets::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>::default());
        app.insert_resource(ModelCache::default());
        app.insert_resource(RuntimeWeapons::new(
            loaded.vfs.clone(),
            Arc::default(),
            loaded.gameplay.precached_weapons.clone(),
        ));
        app.insert_resource(Simulation::new(
            &loaded.world,
            glam::Vec3::ZERO,
            0.,
            0.,
            false,
            Vec::new(),
        ));
        app.insert_resource(Console::new(Default::default()));
        app.insert_resource(loaded.gameplay);
        app.add_systems(Update, (poll, crate::rendering::present_weapons).chain());

        assert!(!app.world_mut().resource_mut::<Gameplay>().request_loadout());
        let deadline = Instant::now() + std::time::Duration::from_secs(15);
        while app.world().resource::<Gameplay>().inventory.active != "weapon_crowbar" {
            assert!(Instant::now() < deadline, "timeout waiting for crowbar");
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.update();
        }
        assert_eq!(
            app.world().resource::<Gameplay>().inventory.active,
            "weapon_crowbar"
        );
        while app.world().resource::<Gameplay>().precached_weapons.len()
            < hl2_simulation::gameplay::IMPLEMENTED.len()
        {
            assert!(
                Instant::now() < deadline,
                "timeout waiting for all 11 weapons"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
            app.update();
        }
        assert_eq!(
            app.world().resource::<Gameplay>().inventory.active,
            "weapon_crowbar",
            "streaming the remaining weapons must keep crowbar equipped"
        );
    }
}
