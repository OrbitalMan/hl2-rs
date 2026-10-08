//! Player footstep, jump and landing sounds (SDK CBasePlayer::UpdateStepSound / PlayStepSound,
//! CGameMovement jump and CheckFalling with the HL2_DLL constants).
use glam::Vec3;

/// PLAYER_FALL_PUNCH_THRESHOLD: slower landings make no sound.
const FALL_PUNCH: f32 = 303.;
/// PLAYER_MAX_SAFE_FALL_SPEED.
const MAX_SAFE_FALL: f32 = 526.5;
/// PLAYER_MIN_BOUNCE_SPEED.
const MIN_BOUNCE: f32 = 173.;

/// The ground surface's step sounds and game material character (surfaceproperties).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub step_left: Option<String>,
    pub step_right: Option<String>,
    pub material: char,
}

/// Player state at the start of the move (UpdateStepSound runs before the movement).
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub velocity: Vec3,
    pub grounded: bool,
    pub crouched: bool,
    /// Noclip/fly: no steps.
    pub noclip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub sound: String,
    pub volume: f32,
}

#[derive(Debug, Default)]
pub struct Footsteps {
    /// m_flStepSoundTime in milliseconds.
    timer: f32,
    /// m_nStepside: 1 plays stepleft, 0 stepright.
    side: bool,
    pub played: u64,
}
impl Footsteps {
    /// One movement tick. `surface` is only queried when a step is due.
    pub fn update(
        &mut self,
        state: State,
        dt: f32,
        surface: impl FnOnce() -> Option<Surface>,
    ) -> Option<Step> {
        if self.timer > 0. {
            self.timer = (self.timer - 1000. * dt).max(0.);
        }
        if self.timer > 0. || state.noclip {
            return None;
        }
        let speed = state.velocity.length();
        let ground_speed = state.velocity.truncate().length();
        let (walk, run) = if state.crouched {
            (60., 80.)
        } else {
            (90., 220.)
        };
        if speed < walk || !(state.grounded && ground_speed > 0.0001) {
            return None;
        }
        let walking = speed < run;
        // Water levels and ladders are not modeled yet; a surface is required.
        let surface = surface()?;
        self.timer = if walking { 400. } else { 300. };
        if state.crouched {
            self.timer += 100.;
        }
        let mut volume = match surface.material {
            'D' => {
                if walking {
                    0.25
                } else {
                    0.55
                }
            }
            'V' => {
                if walking {
                    0.4
                } else {
                    0.7
                }
            }
            _ => {
                if walking {
                    0.2
                } else {
                    0.5
                }
            }
        };
        if state.crouched {
            volume *= 0.65;
        }
        self.play(&surface, volume)
    }
    /// A successful jump plays a full-volume step (CheckJumpButton).
    pub fn jump(&mut self, surface: Option<Surface>) -> Option<Step> {
        self.play(&surface?, 1.)
    }
    /// CheckFalling / PlayerRoughLandingEffects with the HL2 fall thresholds.
    pub fn land(
        &mut self,
        fall_speed: f32,
        surface: impl FnOnce() -> Option<Surface>,
    ) -> Option<Step> {
        if fall_speed < FALL_PUNCH {
            return None;
        }
        let volume = if fall_speed > MAX_SAFE_FALL {
            1.
        } else if fall_speed > MAX_SAFE_FALL / 2. {
            0.85
        } else if fall_speed < MIN_BOUNCE {
            0.
        } else {
            0.5
        };
        if volume <= 0. {
            return None;
        }
        self.timer = 400.;
        self.play(&surface()?, volume)
    }
    fn play(&mut self, surface: &Surface, volume: f32) -> Option<Step> {
        let name = if self.side {
            surface.step_left.clone()
        } else {
            surface.step_right.clone()
        }?;
        self.side = !self.side;
        self.played += 1;
        Some(Step {
            sound: name,
            volume,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn concrete() -> Option<Surface> {
        Some(Surface {
            step_left: Some("Concrete.StepLeft".into()),
            step_right: Some("Concrete.StepRight".into()),
            material: 'C',
        })
    }
    fn walking(speed: f32) -> State {
        State {
            velocity: Vec3::new(speed, 0., 0.),
            grounded: true,
            crouched: false,
            noclip: false,
        }
    }

    #[test]
    fn steps_alternate_with_sdk_timing_and_volume() {
        let mut f = Footsteps::default();
        let mut steps = Vec::new();
        // 190 u/s is walking (< 220): 0.2 volume every 400 ms.
        for tick in 0..200 {
            if let Some(step) = f.update(walking(190.), 0.015, concrete) {
                steps.push((tick, step));
            }
        }
        assert_eq!(steps[0].1.sound, "Concrete.StepRight");
        assert_eq!(steps[1].1.sound, "Concrete.StepLeft");
        assert!(steps.iter().all(|(_, s)| s.volume == 0.2));
        // 400 ms at 15 ms ticks: the timer reaches zero on the 27th tick.
        assert_eq!(steps[1].0 - steps[0].0, 27);
        // Sprinting (320) runs: 0.5 volume, 300 ms.
        let mut f = Footsteps::default();
        let ticks: Vec<_> = (0..100)
            .filter_map(|t| {
                f.update(walking(320.), 0.015, concrete)
                    .map(|s| (t, s.volume))
            })
            .collect();
        assert_eq!(ticks[1].0 - ticks[0].0, 20);
        assert_eq!(ticks[0].1, 0.5);
    }

    #[test]
    fn thresholds_crouch_and_air() {
        let mut f = Footsteps::default();
        assert!(f.update(walking(80.), 0.015, concrete).is_none());
        let mut air = walking(190.);
        air.grounded = false;
        assert!(f.update(air, 0.015, concrete).is_none());
        let mut crouch = walking(63.3);
        crouch.crouched = true;
        let step = f.update(crouch, 0.015, concrete).unwrap();
        assert!((step.volume - 0.2 * 0.65).abs() < 1e-6);
        assert_eq!(f.timer, 500.);
        let mut vent = Footsteps::default();
        let surface = || {
            Some(Surface {
                material: 'V',
                ..concrete().unwrap()
            })
        };
        assert_eq!(
            vent.update(walking(190.), 0.015, surface).unwrap().volume,
            0.4
        );
    }

    #[test]
    fn jump_and_landing_volumes() {
        let mut f = Footsteps::default();
        assert_eq!(f.jump(concrete()).unwrap().volume, 1.);
        assert!(f.land(300., concrete).is_none());
        assert_eq!(f.land(310., concrete).unwrap().volume, 0.85);
        assert_eq!(f.timer, 400.);
        assert_eq!(f.land(600., concrete).unwrap().volume, 1.);
    }
}
