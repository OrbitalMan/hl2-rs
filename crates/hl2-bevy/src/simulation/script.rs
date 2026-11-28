//! Movement script commands and parser for automated replay validation.

use anyhow::{Result, bail};
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub(crate) tick: u64,
    #[serde(default)]
    pub(crate) actions: Vec<crate::gameplay::Action>,
    #[serde(default)]
    pub(crate) ui: Vec<crate::ui_host::Action>,
    #[serde(default)]
    pub(crate) primary: bool,
    #[serde(default)]
    pub(crate) secondary: bool,
    pub(crate) label: Option<String>,
    pub(crate) profile_window: Option<ProfileWindow>,
    pub(crate) eye: Option<[f32; 3]>,
    pub(crate) yaw: Option<f32>,
    pub(crate) pitch: Option<f32>,
    /// Horizontal 4:3 field of view in degrees, like the Source `fov` command.
    pub(crate) fov: Option<f32>,
    pub(crate) paused: Option<bool>,
    pub(crate) extension: Option<serde_json::Value>,
    pub(crate) fly: Option<bool>,
    #[serde(default)]
    pub(crate) forward: f32,
    #[serde(default)]
    pub(crate) side: f32,
    #[serde(default)]
    pub(crate) jump: bool,
    #[serde(default)]
    pub(crate) crouch: bool,
    #[serde(default)]
    pub(crate) sprint: bool,
    #[serde(default)]
    pub(crate) slow: bool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProfileWindow {
    Start,
    End,
}

pub fn read_script(path: &Path) -> Result<Vec<Command>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > 1024 * 1024 {
        bail!("movement script exceeds 1 MiB");
    }
    let commands: Vec<Command> = serde_json::from_slice(&bytes)?;
    if commands.is_empty() || commands.len() > 1024 {
        bail!("movement script must have 1..1024 commands");
    }
    let mut previous = 0;
    let mut profiling = false;
    for c in &commands {
        if c.ui.len() > 64
            || c.ui.iter().any(|a| !a.valid())
            || c.actions.len() > 64
            || c.actions.iter().any(|a| !a.valid())
            || c.tick <= previous
            || c.tick > 40000
            || !c.forward.is_finite()
            || !c.side.is_finite()
            || c.forward.abs() > 1.
            || c.side.abs() > 1.
            || c.eye.is_some_and(|p| p.iter().any(|n| !n.is_finite()))
            || c.yaw.is_some_and(|n| !n.is_finite())
            || c.pitch.is_some_and(|n| !(-89. ..=89.).contains(&n))
            || c.fov.is_some_and(|n| !(1. ..=170.).contains(&n))
            || c.label.as_ref().is_some_and(|s| s.len() > 128)
        {
            bail!("invalid movement command at tick {}", c.tick);
        }
        match c.profile_window {
            Some(ProfileWindow::Start) if !profiling && c.label.is_some() => profiling = true,
            Some(ProfileWindow::End) if profiling && c.label.is_some() => profiling = false,
            Some(_) => bail!("unpaired profiling marker at tick {}", c.tick),
            None => (),
        }
        previous = c.tick;
    }
    if profiling {
        bail!("movement script leaves a profile window open");
    }
    Ok(commands)
}

/// Scans movement-script `ui` console commands for `npc_create`, `npc_create_aimed`, and
/// `ent_create` NPC spawns, returning their pool labels for selective map-load preloading.
pub(crate) fn requested_npc_classes(commands: &[Command]) -> std::collections::BTreeSet<String> {
    let mut console = hl2_simulation::ai::spawn::Console::default();
    let mut classes = std::collections::BTreeSet::new();
    for cmd in commands {
        for action in &cmd.ui {
            let crate::ui_host::Action::Command { command } = action else {
                continue;
            };
            for segment in command.split(';') {
                if let Ok(Some(hl2_simulation::ai::spawn::Command::Spawn(req))) =
                    console.parse(segment)
                {
                    classes.insert(req.pool_label().to_owned());
                }
            }
        }
    }
    classes
}

/// Scans movement-script `Action::Loadout`, `impulse 101`, pickup spawns, and NPC equipment
/// commands so script replays keep deterministic weapon availability at map load.
pub(crate) fn requested_weapons(
    commands: &[Command],
) -> (
    std::collections::BTreeSet<String>,
    std::collections::BTreeSet<String>,
) {
    use hl2_simulation::{ai::spawn, gameplay::IMPLEMENTED};
    let mut console = spawn::Console::default();
    let (mut player, mut npc) = (
        std::collections::BTreeSet::new(),
        std::collections::BTreeSet::new(),
    );
    let mut grant_all = false;
    let insert_player_if_implemented = |player: &mut std::collections::BTreeSet<String>,
                                        w: &str| {
        if IMPLEMENTED.contains(&w) {
            player.insert(w.to_owned());
        }
    };
    for cmd in commands {
        if cmd
            .actions
            .iter()
            .any(|action| matches!(action, crate::gameplay::Action::Loadout))
        {
            grant_all = true;
        }
        for action in &cmd.ui {
            let crate::ui_host::Action::Command { command } = action else {
                continue;
            };
            for segment in command.split(';') {
                let mut words = segment.split_whitespace();
                if words
                    .next()
                    .is_some_and(|w| w.eq_ignore_ascii_case("impulse"))
                    && words.next() == Some("101")
                    && words.next().is_none()
                {
                    grant_all = true;
                    continue;
                }
                match console.parse(segment) {
                    Ok(Some(spawn::Command::SpawnPickup(class, _))) => {
                        insert_player_if_implemented(&mut player, &class.to_ascii_lowercase());
                    }
                    Ok(Some(spawn::Command::SetEquipment(eq))) => {
                        let lower = eq.trim().to_ascii_lowercase();
                        if !lower.is_empty() {
                            insert_player_if_implemented(&mut player, &lower);
                            npc.insert(lower);
                        }
                    }
                    Ok(Some(spawn::Command::Spawn(req))) => {
                        let lower = req.equipment.trim().to_ascii_lowercase();
                        if !lower.is_empty() {
                            insert_player_if_implemented(&mut player, &lower);
                            npc.insert(lower);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    if grant_all {
        player.extend(IMPLEMENTED.into_iter().map(str::to_owned));
    }
    (player, npc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_at(tick: u64, lines: &[&str]) -> Command {
        let ui: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::json!({ "action": "command", "command": line }))
            .collect();
        serde_json::from_value(serde_json::json!({ "tick": tick, "ui": ui })).unwrap()
    }

    #[test]
    fn requested_weapons_collects_loadout_impulse_pickups_and_npc_equipment() {
        let mut loadout_cmd = command_at(1, &["sv_cheats 1"]);
        loadout_cmd.actions.push(crate::gameplay::Action::Loadout);
        let (player, npc) = requested_weapons(&[loadout_cmd]);
        assert_eq!(player.len(), hl2_simulation::gameplay::IMPLEMENTED.len());
        assert!(npc.is_empty());

        let (player, npc) = requested_weapons(&[
            command_at(1, &["sv_cheats 1; impulse   101"]),
            command_at(2, &["npc_create_equipment weapon_stunstick"]),
        ]);
        assert_eq!(player.len(), hl2_simulation::gameplay::IMPLEMENTED.len());
        assert_eq!(npc, ["weapon_stunstick".to_owned()].into());

        let (player, npc) = requested_weapons(&[
            command_at(1, &["ent_create weapon_pistol"]),
            command_at(
                2,
                &["ent_create npc_metropolice additionalequipment weapon_smg1"],
            ),
        ]);
        assert_eq!(
            player,
            ["weapon_pistol", "weapon_smg1"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(npc, ["weapon_smg1".to_owned()].into());
    }

    #[test]
    fn requested_npc_classes_collects_spawn_labels_and_ignores_other_commands() {
        let commands = vec![
            command_at(1, &["sv_cheats 1"]),
            command_at(
                2,
                &["ent_create npc_metropolice additionalequipment weapon_smg1"],
            ),
            command_at(3, &["npc_create npc_citizen_rebel"]),
            command_at(
                4,
                &[
                    "npc_create_aimed npc_zombie; prop_physics_create props_c17/oildrum001.mdl; ent_create npc_combine_s targetname c1",
                ],
            ),
        ];
        assert_eq!(
            requested_npc_classes(&commands),
            [
                "npc_citizen_rebel",
                "npc_combine_s",
                "npc_metropolice",
                "npc_zombie",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
    }
}
