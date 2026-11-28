//! Weapon pickups: a dropped weapon's clip, and a weapon whose assets are still loading.
use super::super::*;
use super::definitions;

#[test]
fn a_dropped_weapon_keeps_its_clip_when_equipped() {
    // An NPC's SMG1 drop carries 25 rounds (Weapon_Drop); Weapon_Equip keeps them.
    let defs = definitions();
    let mut world = World::default();
    world.entities.push(modkit_core::Entity {
        properties: vec![
            ("classname".into(), "weapon_smg1".into()),
            ("origin".into(), "0 0 24".into()),
            ("ammo".into(), "25".into()),
        ],
    });
    let mut scene = Scene::new(&world);
    let mut inv = Inventory::default();
    inv.tick(&world, &mut scene, &defs, Vec3::ZERO, false, 0.015);
    assert!(scene.states[0].killed);
    assert_eq!(inv.owned.get("weapon_smg1"), Some(&25));
}
#[test]
fn a_weapon_whose_assets_are_still_loading_stays_where_it_lies() {
    let defs = definitions();
    let mut world = World::default();
    world.entities.push(modkit_core::Entity {
        properties: vec![
            ("classname".into(), "weapon_smg1".into()),
            ("origin".into(), "0 0 24".into()),
        ],
    });
    let mut scene = Scene::new(&world);
    let mut inv = Inventory::default();
    inv.pickup_blocked.insert("weapon_smg1".into());
    inv.tick(&world, &mut scene, &defs, Vec3::ZERO, false, 0.015);
    assert!(!scene.states[0].killed, "left lying");
    assert!(!inv.owned.contains_key("weapon_smg1"));
    // Once the host has loaded it, the next touch picks it up.
    inv.pickup_blocked.clear();
    inv.tick(&world, &mut scene, &defs, Vec3::ZERO, false, 0.015);
    assert!(scene.states[0].killed);
    assert!(inv.owned.contains_key("weapon_smg1"));
}
