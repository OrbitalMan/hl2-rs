//! Opt-in deterministic input playback for regression scenes. This drives the runtime's normal handlers.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Source ClearInputButton consumes each confirming button until its own release.
#[derive(Default)]
pub struct AttackSuppression {
    primary: bool,
    secondary: bool,
}
impl AttackSuppression {
    pub fn consume(&mut self) {
        self.primary = true;
        self.secondary = true;
    }
    pub fn update(&mut self, primary_down: bool, secondary_down: bool) {
        if !primary_down {
            self.primary = false;
        }
        if !secondary_down {
            self.secondary = false;
        }
    }
    pub fn primary_allowed(&self) -> bool {
        !self.primary
    }
    pub fn secondary_allowed(&self) -> bool {
        !self.secondary
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Move {
        #[serde(default)]
        forward: f32,
        #[serde(default)]
        side: f32,
        #[serde(default)]
        jump: bool,
        #[serde(default)]
        crouch: bool,
        #[serde(default)]
        sprint: bool,
        #[serde(default)]
        slow: bool,
    },
    Escape,
    ToggleConsole,
    Resume,
    Console {
        command: String,
    },
    Loadout,
    Slot {
        slot: usize,
    },
    Wheel {
        delta: i32,
    },
    Confirm,
    Cancel,
    Previous,
    Fire,
    FireDown,
    FireUp,
    Secondary,
    SecondaryDown,
    SecondaryUp,
    Look {
        yaw: f32,
        pitch: f32,
    },
    /// Controlled initial actor pose for regression fixtures, not campaign navigation.
    ActorPose {
        target: String,
        origin: [f32; 3],
        yaw: f32,
    },
    Reload,
    Use,
    Capture {
        name: String,
    },
    Quit,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    pub time: f64,
    #[serde(flatten)]
    pub action: Action,
}
#[derive(Default)]
pub struct Playback {
    events: Vec<Event>,
    next: usize,
    pub held: bool,
    pub secondary_held: bool,
}
impl Playback {
    pub fn read(path: Option<&Path>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        Self::parse(&std::fs::read(path)?)
    }
    fn parse(data: &[u8]) -> Result<Self> {
        let events: Vec<Event> = serde_json::from_slice(data)?;
        if events.len() > 4096
            || events.iter().any(|e| !e.time.is_finite() || e.time < 0.)
            || events.windows(2).any(|p| p[0].time > p[1].time)
        {
            bail!("input playback must contain at most 4096 events with ascending finite times");
        }
        for e in &events {
            if let Action::Move { forward, side, .. } = &e.action {
                if !forward.is_finite()
                    || !side.is_finite()
                    || forward.abs() > 1.
                    || side.abs() > 1.
                {
                    bail!("playback movement components must be finite and within [-1, 1]");
                }
            }
            if let Action::Console { command } = &e.action {
                if command.len() > 1024 {
                    bail!("playback console command exceeds 1024 bytes");
                }
            }
            if let Action::Look { yaw, pitch } = &e.action {
                if !yaw.is_finite() || !pitch.is_finite() {
                    bail!("playback look angles must be finite");
                }
            }
            if let Action::ActorPose {
                target,
                origin,
                yaw,
            } = &e.action
            {
                if target.is_empty()
                    || target.len() > 128
                    || !origin.iter().all(|v| v.is_finite())
                    || !yaw.is_finite()
                {
                    bail!("playback actor pose requires a bounded name and finite pose");
                }
            }
            if let Action::Capture { name } = &e.action {
                if name.is_empty()
                    || name.len() > 80
                    || !name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    bail!("capture name must be a simple filename stem");
                }
            }
        }
        Ok(Self {
            events,
            next: 0,
            held: false,
            secondary_held: false,
        })
    }
    pub fn poll(&mut self, time: f64) -> Vec<Action> {
        let start = self.next;
        while self.next < self.events.len() && self.events[self.next].time <= time {
            self.next += 1;
        }
        self.events[start..self.next]
            .iter()
            .map(|e| e.action.clone())
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirming_buttons_are_consumed_until_their_own_releases() {
        let mut suppression = AttackSuppression::default();
        suppression.consume();
        suppression.update(true, true);
        assert!(!suppression.primary_allowed());
        assert!(!suppression.secondary_allowed());
        suppression.update(false, true);
        assert!(suppression.primary_allowed());
        assert!(!suppression.secondary_allowed());
        // A new primary press works while the old secondary remains held.
        suppression.update(true, true);
        assert!(suppression.primary_allowed());
        assert!(!suppression.secondary_allowed());
        suppression.update(true, false);
        assert!(suppression.primary_allowed());
        assert!(suppression.secondary_allowed());
    }
    #[test]
    fn secondary_release_and_repress_in_one_poll_rearms_the_new_press() {
        let mut playback = Playback::parse(
            br#"[{"time":1,"action":"secondary_up"},{"time":1,"action":"secondary_down"}]"#,
        )
        .unwrap();
        playback.secondary_held = true;
        let mut suppression = AttackSuppression::default();
        suppression.consume();
        suppression.update(false, playback.secondary_held);
        let mut new_presses = 0;
        for action in playback.poll(1.) {
            match action {
                Action::SecondaryUp => playback.secondary_held = false,
                Action::SecondaryDown => {
                    playback.secondary_held = true;
                    if suppression.secondary_allowed() {
                        new_presses += 1;
                    }
                }
                _ => unreachable!(),
            }
            suppression.update(playback.held, playback.secondary_held);
        }
        assert_eq!(new_presses, 1);
        assert!(playback.secondary_held);
        assert!(suppression.secondary_allowed());
        assert!(playback.poll(1.).is_empty());
    }
    #[test]
    fn timed_inputs_preserve_equal_time_order_and_are_never_repeated() {
        let mut playback = Playback::parse(br#"[{"time":1,"action":"slot","slot":1},{"time":1,"action":"confirm"},{"time":2,"action":"quit"}]"#).unwrap();
        assert!(playback.poll(0.9).is_empty());
        let first = playback.poll(1.);
        assert!(matches!(first[0], Action::Slot { slot: 1 }));
        assert!(matches!(first[1], Action::Confirm));
        assert!(playback.poll(1.5).is_empty());
        assert!(matches!(playback.poll(2.)[0], Action::Quit));
    }
    #[test]
    fn invalid_order_and_capture_paths_are_rejected() {
        assert!(
            Playback::parse(br#"[{"time":2,"action":"quit"},{"time":1,"action":"fire"}]"#).is_err()
        );
        assert!(
            Playback::parse(br#"[{"time":0,"action":"capture","name":"../outside"}]"#).is_err()
        );
        assert!(Playback::parse(br#"[{"time":-1,"action":"fire"}]"#).is_err());
        assert!(Playback::parse(br#"[{"time":0,"action":"move","forward":1.1}]"#).is_err());
        assert!(Playback::parse(br#"[{"time":0,"action":"move","side":-2}]"#).is_err());
        let mut movement =
            Playback::parse(br#"[{"time":0,"action":"move","jump":true,"crouch":true}]"#).unwrap();
        assert!(matches!(
            movement.poll(0.)[0],
            Action::Move {
                forward: 0.,
                side: 0.,
                jump: true,
                crouch: true,
                sprint: false,
                slow: false
            }
        ));
    }
}
