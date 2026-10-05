//! Shared landmark arrival semantics. Full saved entity/player transfer is unfinished.
use glam::Vec3;
use modkit_core::World;
pub fn arrival(old: &World, new: &World, eye: Vec3, landmark: &str) -> Vec3 {
    let find = |world: &World| {
        world
            .entities
            .iter()
            .find(|e| e.class() == "info_landmark" && e.get("targetname") == Some(landmark))
            .map(|e| e.origin())
    };
    find(old)
        .zip(find(new))
        .map_or_else(|| new.spawn().0, |(a, b)| eye - a + b)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn world(origin: &str) -> World {
        World {
            entities: vec![modkit_core::Entity {
                properties: vec![
                    ("classname".into(), "info_landmark".into()),
                    ("targetname".into(), "station".into()),
                    ("origin".into(), origin.into()),
                ],
            }],
            ..Default::default()
        }
    }
    #[test]
    fn arrival_preserves_landmark_relative_eye_and_falls_back_to_spawn() {
        let old = world("10 20 30");
        let new = world("100 200 300");
        let eye = Vec3::new(17., 28., 39.);
        assert_eq!(
            arrival(&old, &new, eye, "station"),
            Vec3::new(107., 208., 309.)
        );
        assert_eq!(arrival(&old, &new, eye, "missing"), new.spawn().0);
    }
}
