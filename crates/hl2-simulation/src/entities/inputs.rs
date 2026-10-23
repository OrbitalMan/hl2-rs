//! Inputs that only entities of one class take, by class name. [`Scene::deliver`] asks the
//! class's handler first; an input the handler does not take falls back to the inputs every
//! entity takes (Kill, Enable, SetParent, ...), then is reported as unsupported.

use super::{Pending, Scene};
use glam::Vec3;
use modkit_core::{Entity, World};

mod doors;
mod env;
mod logic;
mod scenes;
mod triggers;

/// One input being delivered to an entity.
pub(super) struct Delivery<'a> {
    pub world: &'a World,
    pub id: usize,
    pub pending: &'a Pending,
    /// The player's feet, for `OpenAwayFrom !player`.
    pub player_feet: Vec3,
    pub entity: &'a Entity,
    pub class: &'a str,
    /// The input name, lowercased.
    pub input: &'a str,
    /// The parameter as a number (0 when it is not one).
    pub value: f32,
}

/// Runs an input on an entity of its class; `false` when the class does not take that input.
pub(super) type Handler = fn(&mut Scene, &Delivery) -> bool;

/// Class name -> the handler of the inputs only that class takes.
const HANDLERS: &[(&str, Handler)] = &[
    ("func_door", doors::door),
    ("func_door_rotating", doors::door),
    ("prop_door_rotating", doors::door),
    ("func_movelinear", doors::movelinear),
    ("logic_relay", logic::relay),
    ("logic_branch", logic::branch),
    ("logic_case", logic::case),
    ("math_counter", logic::counter),
    ("trigger_changelevel", triggers::changelevel),
    ("logic_choreographed_scene", scenes::choreographed_scene),
    ("scripted_sequence", scenes::scripted_sequence),
    ("point_template", scenes::template),
    ("point_viewcontrol", env::viewcontrol),
    ("env_zoom", env::zoom),
    ("env_fade", env::fade),
    ("env_tonemap_controller", env::tonemap_controller),
    ("env_physexplosion", env::physexplosion),
    ("env_physimpact", env::physimpact),
    ("ambient_generic", env::ambient_generic),
    ("npc_combine_camera", env::combine_camera),
];

/// The handler of `class`'s own inputs, if it has any.
pub(super) fn handler(class: &str) -> Option<Handler> {
    HANDLERS
        .iter()
        .find(|(name, _)| *name == class)
        .map(|&(_, handler)| handler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_one_handler() {
        for (i, (class, _)) in HANDLERS.iter().enumerate() {
            assert!(
                HANDLERS[i + 1..].iter().all(|(other, _)| other != class),
                "duplicate {class}"
            );
        }
        assert!(handler("func_door").is_some());
        assert!(
            handler("FUNC_DOOR").is_none(),
            "class names compare exactly"
        );
        assert!(handler("func_brush").is_none());
    }
}
