//! HUD blend variants warmed behind the loading screen (O9). Each HUD batch draws with one of
//! three blend states (`HudMaterialKey`: normal, additive, modulate), each its own pipeline,
//! which Bevy creates the first time a batch uses it (synchronously on macOS): the normal and
//! additive ones when the HUD first draws after a map's first `logic_auto` think, the modulate
//! one at the first modulating screen fade. While the variant warm-up runs, the HUD camera draws
//! one zero-area quad per state, outside the pooled batches (no `HudDraw`), so the report's batch
//! and texture counts stay as they were; they are despawned when the warm-up ends.
use super::{HudMaterial, QuadGeometry};
use crate::rendering::variant_warmup::VariantWarmup;
use bevy::{
    asset::{Assets, RenderAssetUsages},
    camera::visibility::{NoFrustumCulling, RenderLayers},
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    image::Image,
    mesh::{Mesh, Mesh2d},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    sprite_render::MeshMaterial2d,
    transform::components::Transform,
};

/// The blend states a HUD batch can use (`Canvas::modulate` never sets additive too).
const KEYS: [(bool, bool); 3] = [(false, false), (true, false), (false, true)];

/// One warm-up quad of the HUD camera.
#[derive(Component)]
pub(crate) struct WarmingHud;

type HudAssets<'w> = (
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<HudMaterial>>,
    ResMut<'w, Assets<Mesh>>,
);

pub(crate) fn warm_hud_variants(
    mut commands: Commands,
    warmup: Option<Res<VariantWarmup>>,
    warming: Query<Entity, With<WarmingHud>>,
    (mut images, mut materials, mut meshes): HudAssets,
) {
    if !warmup.is_some_and(|w| w.active()) {
        for entity in &warming {
            commands.entity(entity).despawn();
        }
        return;
    }
    if !warming.is_empty() {
        return;
    }
    let texture = images.add(Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255; 4],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    ));
    // The batch layout of every HUD draw, with nothing to rasterize.
    let mesh = meshes.add(QuadGeometry::batch_mesh(&[QuadGeometry {
        positions: [[0.; 3]; 4],
        colors: [[0.; 4]; 4],
        uvs: [[0.; 2]; 4],
    }]));
    for (additive, modulate) in KEYS {
        let material = materials.add(HudMaterial {
            texture: texture.clone(),
            additive,
            modulate,
        });
        commands.spawn((
            crate::campaign::MapOwned,
            WarmingHud,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material),
            Transform::default(),
            RenderLayers::layer(2),
            NoFrustumCulling,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{HudMaterial, KEYS, WarmingHud, warm_hud_variants};
    use crate::rendering::variant_warmup::VariantWarmup;
    use bevy::{
        asset::Assets,
        ecs::{query::With, system::RunSystemOnce, world::World},
        image::Image,
        mesh::Mesh,
        sprite_render::MeshMaterial2d,
    };

    #[test]
    fn one_quad_per_blend_state_draws_through_the_warm_up_then_goes() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<Assets<HudMaterial>>();
        world.init_resource::<Assets<Mesh>>();
        world.insert_resource(VariantWarmup::new());
        for _ in 0..2 {
            world.run_system_once(warm_hud_variants).unwrap();
        }
        let mut quads = world.query_filtered::<&MeshMaterial2d<HudMaterial>, With<WarmingHud>>();
        let materials = world.resource::<Assets<HudMaterial>>();
        let mut keys: Vec<_> = quads
            .iter(&world)
            .map(|m| {
                let m = materials.get(&m.0).expect("material");
                (m.additive, m.modulate)
            })
            .collect();
        keys.sort();
        let mut expected = KEYS.to_vec();
        expected.sort();
        assert_eq!(keys, expected, "one quad per state, spawned once");
        world.remove_resource::<VariantWarmup>();
        world.run_system_once(warm_hud_variants).unwrap();
        let mut left = world.query_filtered::<(), With<WarmingHud>>();
        assert_eq!(left.iter(&world).count(), 0);
    }
}
