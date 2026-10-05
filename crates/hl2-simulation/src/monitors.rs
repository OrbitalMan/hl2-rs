//! Map-authored camera/link state. Rendering and player PVS remain host responsibilities.
use crate::entities::State;
use modkit_core::World;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize)]
pub struct Camera {
    pub on: bool,
    pub fov: f32,
    pub resolution: f32,
    pub use_screen_aspect: bool,
    pub fog_enabled: bool,
    pub fog_end: f32,
    change: Option<FovChange>,
}
#[derive(Clone, Serialize)]
struct FovChange {
    target: f32,
    rate: f32,
    next: f64,
}
#[derive(Serialize)]
struct Link {
    owner: usize,
    target: Option<usize>,
    camera: Option<usize>,
}
#[derive(Default, Serialize)]
pub struct Cameras {
    pub cameras: BTreeMap<usize, Camera>,
    links: BTreeMap<usize, Link>,
}
fn named(world: &World, name: &str) -> Option<usize> {
    (!name.is_empty())
        .then(|| {
            world
                .entities
                .iter()
                .position(|e| e.get("targetname") == Some(name))
        })
        .flatten()
}
fn camera_named(world: &World, name: &str) -> Option<usize> {
    named(world, name).filter(|&id| world.entities[id].class() == "point_camera")
}
fn number(e: &modkit_core::Entity, key: &str) -> f32 {
    e.get(key)
        .and_then(|v| v.parse().ok())
        .filter(|v: &f32| v.is_finite())
        .unwrap_or(0.)
}
impl Cameras {
    pub fn new(world: &World) -> Self {
        let mut result = Self::default();
        for (id, e) in world.entities.iter().enumerate() {
            if e.class() == "point_camera" {
                result.cameras.insert(
                    id,
                    Camera {
                        on: number(e, "spawnflags") as u32 & 1 == 0,
                        fov: number(e, "FOV"),
                        resolution: number(e, "resolution"),
                        use_screen_aspect: number(e, "UseScreenAspectRatio") != 0.,
                        fog_enabled: number(e, "fogEnable") != 0.,
                        fog_end: number(e, "fogEnd"),
                        change: None,
                    },
                );
            }
            if matches!(e.class(), "func_monitor" | "info_camera_link") {
                let monitor = e.class() == "func_monitor";
                result.links.insert(
                    id,
                    Link {
                        owner: id,
                        target: if monitor {
                            Some(id)
                        } else {
                            named(world, e.get("target").unwrap_or(""))
                        },
                        camera: camera_named(
                            world,
                            e.get(if monitor { "target" } else { "PointCamera" })
                                .unwrap_or(""),
                        ),
                    },
                );
            }
        }
        result
    }
    /// Called in the existing entity I/O order. Invalid numeric inputs remain unsupported.
    pub fn input(
        &mut self,
        world: &World,
        states: &[State],
        id: usize,
        input: &str,
        parameter: &str,
        time: f64,
    ) -> bool {
        if self.cameras.contains_key(&id) {
            match input {
                "setonandturnothersoff" => {
                    for (&other, camera) in &mut self.cameras {
                        if !states[other].killed {
                            camera.on = other == id;
                        }
                    }
                }
                "seton" => self.cameras.get_mut(&id).unwrap().on = true,
                "setoff" => self.cameras.get_mut(&id).unwrap().on = false,
                "changefov" => {
                    let camera = self.cameras.get_mut(&id).unwrap();
                    let mut values = parameter.split_whitespace();
                    let target = values.next().map_or(Ok(camera.fov), str::parse::<f32>);
                    let duration = values.next().map_or(Ok(1.), str::parse::<f32>);
                    let (Ok(target), Ok(duration)) = (target, duration) else {
                        return false;
                    };
                    if !target.is_finite() || !duration.is_finite() || duration < 0. {
                        return false;
                    }
                    if duration == 0. {
                        camera.fov = target;
                        camera.change = None;
                    } else {
                        let rate = (target - camera.fov) / duration;
                        if !rate.is_finite() {
                            return false;
                        }
                        camera.change = Some(FovChange {
                            target,
                            rate,
                            next: time,
                        });
                        // Native input schedules the first think at current time.
                        Self::think(camera, time);
                    }
                }
                _ => return false,
            }
            return true;
        }
        if let Some(link) = self.links.get_mut(&id).filter(|_| input == "setcamera") {
            if world.entities[id].class() == "func_monitor" {
                // FuncMonitor releases the old generated link before resolving its replacement.
                link.camera = camera_named(world, parameter).filter(|&id| !states[id].killed);
            } else if let Some(target) = named(world, parameter).filter(|&id| !states[id].killed) {
                // InfoCameraLink preserves the old camera when name lookup itself fails.
                link.camera = (world.entities[target].class() == "point_camera").then_some(target);
            }
            return true;
        }
        false
    }
    fn think(camera: &mut Camera, time: f64) {
        let Some(change) = &mut camera.change else {
            return;
        };
        if time + 1e-9 < change.next {
            return;
        }
        let value = camera.fov + change.rate * 0.05;
        let done = if change.rate < 0. {
            value <= change.target
        } else {
            value >= change.target
        };
        camera.fov = if done { change.target } else { value };
        change.next = time + 0.05;
        if done {
            camera.change = None;
        }
    }
    pub fn tick(&mut self, states: &[State], time: f64) {
        for (&id, camera) in &mut self.cameras {
            if !states[id].killed {
                Self::think(camera, time);
            }
        }
    }
    /// Enabled cameras linked to visible targets in player PVS. Area connectivity is not supplied yet.
    pub fn eligible<E>(
        &self,
        states: &[State],
        mut in_pvs: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<Vec<usize>, E> {
        let mut active = BTreeSet::new();
        for link in self.links.values() {
            let (Some(target), Some(camera)) = (link.target, link.camera) else {
                continue;
            };
            if states[link.owner].killed
                || states[target].killed
                || !states[target].visible
                || states[camera].killed
            {
                continue;
            }
            if self.cameras.get(&camera).is_some_and(|c| c.on) && in_pvs(target)? {
                active.insert(camera);
            }
        }
        // Class lists insert at the head; the final render overwrites the shared target.
        Ok(active.into_iter().rev().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Scene;
    fn world() -> World {
        let entities = [
            ("point_camera", "a", vec![("FOV", "90")]),
            (
                "point_camera",
                "b",
                vec![("FOV", "40"), ("spawnflags", "1")],
            ),
            (
                "func_monitor",
                "screen",
                vec![("target", "a"), ("StartDisabled", "1")],
            ),
            (
                "info_camera_link",
                "link",
                vec![("PointCamera", "b"), ("target", "screen")],
            ),
        ]
        .into_iter()
        .map(|(class, name, extra)| modkit_core::Entity {
            properties: [("classname", class), ("targetname", name)]
                .into_iter()
                .chain(extra)
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        })
        .collect();
        World {
            entities,
            ..Default::default()
        }
    }
    #[test]
    fn camera_spawn_link_visibility_and_exclusive_inputs_are_preserved() {
        let world = world();
        let mut scene = Scene::new(&world);
        assert!(scene.monitors.cameras[&0].on);
        assert!(!scene.monitors.cameras[&1].on);
        assert!(scene
            .monitors
            .eligible(&scene.states, |_| Ok::<_, ()>(true))
            .unwrap()
            .is_empty());
        scene.send(2, "Enable", "");
        scene.send(1, "SetOnAndTurnOthersOff", "");
        scene.tick(&world, glam::Vec3::ZERO, 0.015);
        assert!(scene.states[2].visible);
        assert!(!scene.monitors.cameras[&0].on);
        assert_eq!(
            scene
                .monitors
                .eligible(&scene.states, |_| Ok::<_, ()>(true))
                .unwrap(),
            vec![1]
        );
        assert!(scene
            .monitors
            .eligible(&scene.states, |_| Ok::<_, ()>(false))
            .unwrap()
            .is_empty());
        scene.send(3, "Kill", "");
        scene.tick(&world, glam::Vec3::ZERO, 0.015);
        assert!(scene
            .monitors
            .eligible(&scene.states, |_| Ok::<_, ()>(true))
            .unwrap()
            .is_empty());
    }
    #[test]
    fn fov_uses_fifty_millisecond_thinks_clamps_and_retargets() {
        let world = world();
        let mut scene = Scene::new(&world);
        scene.send(0, "ChangeFOV", "40 1");
        scene.tick(&world, glam::Vec3::ZERO, 0.015);
        assert_eq!(scene.monitors.cameras[&0].fov, 87.5);
        scene.tick(&world, glam::Vec3::ZERO, 0.03);
        assert_eq!(scene.monitors.cameras[&0].fov, 87.5);
        scene.tick(&world, glam::Vec3::ZERO, 0.03);
        assert_eq!(scene.monitors.cameras[&0].fov, 85.);
        for _ in 0..30 {
            scene.tick(&world, glam::Vec3::ZERO, 0.06);
        }
        assert_eq!(scene.monitors.cameras[&0].fov, 40.);
        scene.send(0, "ChangeFOV", "90 0.1");
        scene.tick(&world, glam::Vec3::ZERO, 0.015);
        assert_eq!(scene.monitors.cameras[&0].fov, 65.);
        scene.tick(&world, glam::Vec3::ZERO, 0.06);
        assert_eq!(scene.monitors.cameras[&0].fov, 90.);
    }
    #[test]
    fn monitor_failed_retarget_releases_but_info_link_failed_lookup_preserves() {
        let world = world();
        let mut scene = Scene::new(&world);
        scene.send(2, "SetCamera", "missing");
        scene.send(3, "SetCamera", "missing");
        scene.tick(&world, glam::Vec3::ZERO, 0.015);
        assert_eq!(scene.monitors.links[&2].camera, None);
        assert_eq!(scene.monitors.links[&3].camera, Some(1));
        scene.send(2, "SetCamera", "b");
        scene.send(3, "SetCamera", "screen");
        scene.tick(&world, glam::Vec3::ZERO, 0.015);
        assert_eq!(scene.monitors.links[&2].camera, Some(1));
        assert_eq!(scene.monitors.links[&3].camera, None);
    }
}
