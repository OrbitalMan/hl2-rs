//! Dynamic shadows of studio-model entities on the world, as Options > Video > Advanced > Shadow
//! detail sets them (`r_shadowrendertotexture`; Source SDK `game/client/clientshadowmgr.cpp`). At
//! Low every caster gets a blob: the installed `decals/simpleshadow` projected straight down from
//! its render bounds ([`Frame::blob`]). The geometry is [`hl2_simulation::shadows`]: the map's
//! receivers are gathered on the map worker ([`receivers`]); each caster keeps one decal mesh,
//! rewritten in place only when the caster moved by a quarter shadow texel, turned or changed
//! sequence, and drawn by [`ShadowMaterial`] (multiply blend).
//!
//! Casters (`C_BaseAnimating::ShadowCastType`, `ShouldUseParentShadow`): entities with a studio
//! model and its rig, without `disableshadows`, drawn (not hidden or killed), and with no caster up
//! their parent chain (a parent's shadow stands for its children). Bounds are
//! `C_BaseAnimating::GetRenderBounds`: the model's view box (or hull) with the current sequence's
//! box, times the model scale.
pub mod material;

use crate::{
    assets::{LoadedMap, MaterialData},
    campaign::MapOwned,
    gameplay::Gameplay,
    simulation::Simulation,
    ui_host::Console,
};
use bevy::{
    app::{App, Plugin, PostUpdate},
    asset::{Assets, Handle, RenderAssetUsages},
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::Visibility,
    },
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        resource::Resource,
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res, ResMut},
    },
    image::Image,
    math::{Vec3, Vec4},
    mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology, VertexAttributeValues},
    pbr::{MaterialPlugin, MeshMaterial3d},
    transform::{TransformSystems, components::Transform},
};
use hl2_simulation::shadows::{
    ShadowControl, TEXELS_PER_UNIT,
    frame::{Frame, Placement},
    linear_color,
    receivers::{Cast, Decal, Receivers, Scratch},
};
use hl2_ui::config::ShadowDetail;
pub use material::ShadowMaterial;
use material::ShadowParameters;
use modkit_core::World;
use std::{collections::BTreeMap, sync::Arc};

/// The blob shadows' material.
pub const BLOB_MATERIAL: &str = "decals/simpleshadow";
/// Shadow geometry sits this far off its receivers (units): above impact decals (0.06), which it
/// darkens as the engine's shadow pass does.
const LIFT: f32 = 0.1;
/// A shadow is rebuilt once its caster moved a quarter of a shadow texel...
const MOVE: f32 = 0.25 / TEXELS_PER_UNIT;
/// ...or turned this far (radians; a quarter texel at 64 units from the origin).
const TURN: f32 = MOVE / 64.;

/// The receivers among the world's surfaces: not translucent, alpha-tested, additive or flagged
/// `SURF_NOSHADOWS` (the engine's surface flags for these keep shadows off them).
pub fn receivers(world: &World, materials: &BTreeMap<String, MaterialData>) -> Receivers {
    Receivers::new(&world.surfaces, |name| {
        !world.no_shadow_materials.contains(name)
            && materials.get(name).is_none_or(|m| {
                !(m.translucent || m.additive || m.opacity < 1. || m.alpha_cutoff.is_some())
            })
    })
}

/// The shadow material and the per-frame update, after entities are posed.
pub struct ShadowPlugin;

impl Plugin for ShadowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShadowMaterial>::default())
            .add_systems(
                PostUpdate,
                update
                    .after(crate::rendering::present_entities)
                    .before(TransformSystems::Propagate),
            );
    }
}

/// A caster's shadow draw.
#[derive(Component)]
pub struct ShadowDraw;

struct Caster {
    id: usize,
    key: String,
    scale: f32,
    /// The sequence the bounds were taken for and those bounds (model space, scaled).
    sequence: Option<(String, (glam::Vec3, glam::Vec3))>,
    /// Where the decal was last built for (origin, rotation).
    placed: Option<(glam::Vec3, glam::Quat)>,
    decal: Decal,
    draw: Option<(Entity, Handle<Mesh>)>,
}

/// The map's shadows: its receivers and settings, each caster's state and the GPU material.
#[derive(Resource)]
pub struct MapShadows {
    receivers: Arc<Receivers>,
    control: ShadowControl,
    blob: Option<Arc<source_assets::vtf::Image>>,
    material: Option<Handle<ShadowMaterial>>,
    casters: Vec<Caster>,
    /// Model instances already looked at (runtime spawns append to the list).
    seen: usize,
    scratch: Scratch,
    detail: Option<ShadowDetail>,
    /// Decals rebuilt since the map was installed.
    rebuilds: u64,
}

impl MapShadows {
    pub fn new(loaded: &LoadedMap) -> Self {
        Self {
            receivers: loaded.shadow_receivers.clone(),
            control: ShadowControl::from_world(&loaded.world),
            blob: loaded
                .materials
                .get(BLOB_MATERIAL)
                .and_then(|m| m.base.clone()),
            material: None,
            casters: Vec::new(),
            seen: 0,
            scratch: Scratch::default(),
            detail: None,
            rebuilds: 0,
        }
    }

    /// Receivers, casters, shadows with geometry and their triangles, rebuilds.
    pub fn report(&self) -> serde_json::Value {
        let built = self.casters.iter().filter(|c| !c.decal.is_empty());
        serde_json::json!({
            "detail": self.detail.map(|d| format!("{d:?}")),
            "receiver_triangles": self.receivers.len(),
            "casters": self.casters.len(),
            "with_geometry": built.clone().count(),
            "decal_triangles": built.map(|c| c.decal.indices.len() / 3).sum::<usize>(),
            "rebuilds": self.rebuilds,
            "control": {"direction": self.control.direction.to_array(), "color": self.control.color,
                "distance": self.control.distance},
        })
    }

    /// Adds a caster for every model instance new since the last call.
    fn find_casters(&mut self, world: &World) {
        for instance in &world.model_instances[self.seen.min(world.model_instances.len())..] {
            let Some(id) = instance.entity else {
                continue;
            };
            let casts = crate::assets::visible_entity(world, id)
                && world.rigs.contains_key(&instance.asset_key())
                && world.entities[id]
                    .get("disableshadows")
                    .is_none_or(|v| v.trim().parse::<i32>().unwrap_or(0) == 0);
            if casts {
                self.casters.push(Caster {
                    id,
                    key: instance.asset_key(),
                    scale: instance.scale,
                    sequence: None,
                    placed: None,
                    decal: Decal::default(),
                    draw: None,
                });
            }
        }
        self.seen = world.model_instances.len();
    }

    /// The blob material, made once.
    fn blob_material(
        &mut self,
        materials: &mut Assets<ShadowMaterial>,
        images: &mut Assets<Image>,
    ) -> Option<Handle<ShadowMaterial>> {
        if self.material.is_none() {
            let blob = self.blob.as_ref()?;
            let texture = images.add(crate::rendering::image(
                blob.width,
                blob.height,
                blob.rgba.clone(),
                false,
            ));
            let color = linear_color(self.control.color);
            self.material = Some(materials.add(ShadowMaterial {
                texture,
                parameters: ShadowParameters {
                    color: Vec4::new(color[0], color[1], color[2], 1.),
                    jitter: Vec4::new(
                        1. / f32::from(blob.width),
                        1. / f32::from(blob.height),
                        0.,
                        0.,
                    ),
                    channel: Vec4::W,
                },
            }));
        }
        self.material.clone()
    }
}

/// The caster's render bounds for its current sequence (`GetRenderBounds`), cached per sequence.
fn bounds(caster: &mut Caster, world: &World, sequence: &str) -> Option<(glam::Vec3, glam::Vec3)> {
    if let Some((name, bounds)) = &caster.sequence
        && name == sequence
    {
        return Some(*bounds);
    }
    let rig = world.rigs.get(&caster.key)?;
    let (mut lo, mut hi) = rig.view_bounds;
    if let Some(s) = rig
        .sequences
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(sequence))
    {
        lo = lo.min(s.bounds.0);
        hi = hi.max(s.bounds.1);
    }
    let bounds = (lo * caster.scale, hi * caster.scale);
    caster.sequence = Some((sequence.to_owned(), bounds));
    caster.placed = None;
    Some(bounds)
}

/// Whether a caster's parent chain holds another caster (`ShouldUseParentShadow` /
/// `WillParentRenderBlobbyShadow`: the child projects nothing of its own).
fn under_caster(game: &Gameplay, casters: &[Caster], id: usize) -> bool {
    let mut at = game.scene.states[id].parent.as_ref().map(|p| p.entity);
    let mut depth = 0;
    while let Some(parent) = at
        && depth < 16
    {
        if casters.iter().any(|c| c.id == parent) {
            return true;
        }
        at = game
            .scene
            .states
            .get(parent)
            .and_then(|s| s.parent.as_ref().map(|p| p.entity));
        depth += 1;
    }
    false
}

type ShadowAssets<'w> = (
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<ShadowMaterial>>,
    ResMut<'w, Assets<Image>>,
);

/// Each caster's shadow follows it: rebuilt where it moved, hidden while it is.
pub(crate) fn update(
    mut commands: Commands,
    ui: Option<Res<Console>>,
    (game, sim): (Res<Gameplay>, Res<Simulation>),
    shadows: Option<ResMut<MapShadows>>,
    (mut meshes, mut materials, mut images): ShadowAssets,
    mut draws: Query<(&mut Visibility, &mut Aabb), With<ShadowDraw>>,
    performance: Option<Res<crate::performance::Performance>>,
) {
    let Some(mut shadows) = shadows else {
        return;
    };
    let _timing = crate::performance::scope(performance.as_deref(), "shadows");
    let detail = ui.map(|ui| ui.source.config.video.advanced.shadow_detail);
    if detail != shadows.detail {
        shadows.detail = detail;
        for caster in &mut shadows.casters {
            caster.placed = None;
        }
    }
    shadows.find_casters(&game.world);
    let material = shadows.blob_material(&mut materials, &mut images);
    let blobs = detail == Some(ShadowDetail::Low);
    let MapShadows {
        receivers,
        control,
        casters,
        scratch,
        rebuilds,
        ..
    } = &mut *shadows;
    for index in 0..casters.len() {
        let id = casters[index].id;
        let hidden = !blobs
            || material.is_none()
            || crate::rendering::draw_hidden(&game, id, None)
            || under_caster(&game, casters, id);
        let caster = &mut casters[index];
        match if hidden {
            Need::Hide
        } else {
            need(caster, (&game, &sim), control)
        } {
            Need::Hide => {
                show(caster, &mut draws, false);
                continue;
            }
            Need::Keep => {}
            Need::Build(frame) => {
                receivers.decal(&frame, BLOB_CAST, scratch, &mut caster.decal);
                *rebuilds += 1;
                if !caster.decal.is_empty() {
                    write(
                        caster,
                        &mut commands,
                        &mut meshes,
                        material.clone(),
                        &mut draws,
                    );
                }
            }
        }
        let shown = !caster.decal.is_empty();
        show(caster, &mut draws, shown);
    }
}

/// How blob shadows are laid: on the world only, unfaded by the caster.
const BLOB_CAST: Cast = Cast {
    static_props: false,
    bias: 0,
    lift: LIFT,
};

/// What a shown caster's shadow needs this frame.
enum Need {
    Hide,
    Keep,
    Build(Frame),
}

/// A caster's shadow frame when it moved, turned or changed bounds since its last build.
fn need(
    caster: &mut Caster,
    (game, sim): (&Gameplay, &Simulation),
    control: &ShadowControl,
) -> Need {
    let state = &game.scene.states[caster.id];
    let (origin, rotation) = sim
        .physics
        .entity_pose(caster.id)
        .unwrap_or((state.origin, state.rotation));
    let Some(box_) = bounds(caster, &game.world, &state.animation) else {
        return Need::Hide;
    };
    if caster
        .placed
        .is_some_and(|(o, r)| o.distance(origin) <= MOVE && r.angle_between(rotation) <= TURN)
    {
        return Need::Keep;
    }
    caster.placed = Some((origin, rotation));
    let at = Placement::new(origin, rotation);
    Need::Build(Frame::blob(box_, &at, glam::Vec3::NEG_Z, control.distance))
}

fn show(
    caster: &Caster,
    draws: &mut Query<(&mut Visibility, &mut Aabb), With<ShadowDraw>>,
    shown: bool,
) {
    let Some((entity, _)) = caster.draw else {
        return;
    };
    if let Ok((mut visibility, _)) = draws.get_mut(entity) {
        let wanted = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Writes the caster's decal into its mesh, made with its draw on the first write.
fn write(
    caster: &mut Caster,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Option<Handle<ShadowMaterial>>,
    draws: &mut Query<(&mut Visibility, &mut Aabb), With<ShadowDraw>>,
) {
    let Some(material) = material else {
        return;
    };
    match &caster.draw {
        Some((entity, handle)) => {
            let Some(mut mesh) = meshes.get_mut(handle) else {
                return;
            };
            fill(&mut mesh, &caster.decal);
            if let (Some(aabb), Ok((_, mut current))) =
                (mesh.compute_aabb(), draws.get_mut(*entity))
            {
                *current = aabb;
            }
        }
        None => {
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            fill(&mut mesh, &caster.decal);
            let aabb = mesh.compute_aabb().unwrap_or_default();
            let handle = meshes.add(mesh);
            let entity = commands
                .spawn((
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    aabb,
                    ShadowDraw,
                    MapOwned,
                ))
                .id();
            caster.draw = Some((entity, handle));
        }
    }
}

/// The decal as mesh attributes in Bevy's axes, reusing the mesh's buffers.
fn fill(mesh: &mut Mesh, decal: &Decal) {
    let point = |p: &glam::Vec3| crate::source_to_bevy(Vec3::from_array(p.to_array())).to_array();
    set3(
        mesh,
        Mesh::ATTRIBUTE_POSITION,
        decal.positions.iter().map(point),
    );
    set3(
        mesh,
        Mesh::ATTRIBUTE_NORMAL,
        decal.normals.iter().map(point),
    );
    match mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(uvs)) => {
            uvs.clear();
            uvs.extend(decal.uvs.iter().map(|uv| uv.to_array()));
        }
        _ => mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            decal.uvs.iter().map(|uv| uv.to_array()).collect::<Vec<_>>(),
        ),
    }
    let fade = |f: &u8| [1., 1., 1., f32::from(*f) / 255.];
    match mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(colors)) => {
            colors.clear();
            colors.extend(decal.fades.iter().map(fade));
        }
        _ => mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            decal.fades.iter().map(fade).collect::<Vec<_>>(),
        ),
    }
    match mesh.indices_mut() {
        Some(Indices::U32(indices)) => {
            indices.clear();
            indices.extend_from_slice(&decal.indices);
        }
        _ => mesh.insert_indices(Indices::U32(decal.indices.clone())),
    }
}

fn set3(
    mesh: &mut Mesh,
    attribute: bevy::mesh::MeshVertexAttribute,
    values: impl Iterator<Item = [f32; 3]>,
) {
    match mesh.attribute_mut(attribute) {
        Some(VertexAttributeValues::Float32x3(buffer)) => {
            buffer.clear();
            buffer.extend(values);
        }
        _ => mesh.insert_attribute(attribute, values.collect::<Vec<_>>()),
    }
}
