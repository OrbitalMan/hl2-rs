//! Which materials a loaded map draws: the world's surfaces and overlays, its visible brush entities and
//! entity models.
use modkit_core::World;
use std::collections::BTreeSet;

pub(crate) fn visible_entity(world: &World, id: usize) -> bool {
    world.entities.get(id).is_some_and(|entity| {
        !entity.class().starts_with("trigger_") && !entity.class().starts_with("func_areaportal")
    })
}

pub(super) fn rendered_material_names(world: &World) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut append = |surfaces: &[modkit_core::Surface]| {
        names.extend(
            surfaces
                .iter()
                .filter(|surface| !surface.indices.is_empty())
                .map(|surface| surface.material.clone()),
        );
    };
    // Includes displacements and already-transformed static props. terrain is a
    // duplicate collision representation, and unused model_assets are not draws.
    append(&world.surfaces);
    append(&world.overlays);
    for (id, entity) in world.entities.iter().enumerate() {
        if !visible_entity(world, id) {
            continue;
        }
        if let Some(brush) = entity
            .get("model")
            .and_then(|name| name.strip_prefix('*'))
            .and_then(|id| id.parse::<usize>().ok())
            .and_then(|id| world.brush_models.iter().find(|model| model.id == id))
        {
            append(&brush.surfaces);
        }
    }
    for instance in &world.model_instances {
        if !instance.entity.is_some_and(|id| visible_entity(world, id)) {
            continue;
        }
        if let Some(surfaces) = world.model_assets.get(&instance.asset_key()) {
            append(surfaces);
        }
    }
    names
}
