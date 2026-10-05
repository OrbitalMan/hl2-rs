//! Engine-neutral skeletons, sampled clips and linear-blend skinning.
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Weights {
    pub bones: [u8; 3],
    pub weights: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub bind: Pose,
    pub inverse_bind: Mat4,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    #[serde(default)]
    pub events: Vec<ClipEvent>,
    pub fps: f32,
    pub looping: bool,
    pub frames: Vec<Vec<Pose>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipEvent {
    pub cycle: f32,
    pub id: i32,
    pub flags: u32,
    pub name: String,
    pub options: String,
}
/// Authored movement block. Speeds describe distance across the block's cycle fraction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MovementRecord {
    pub end_frame: i32,
    pub motion_flags: u32,
    pub v0: f32,
    pub v1: f32,
    pub end_yaw_degrees: f32,
    pub direction: Vec3,
    pub cumulative_position: Vec3,
}
/// Movement of one animation blend; independent of skeletal pose sampling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootMotion {
    pub fps: f32,
    pub frame_count: u32,
    pub records: Vec<MovementRecord>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RootMotionSample {
    pub position: Vec3,
    pub yaw_degrees: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootMotionError {
    InvalidData(&'static str),
    NoMovement,
    InvalidCycle,
    UncoveredCycle,
    NonFiniteSample,
}
impl std::fmt::Display for RootMotionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidData(reason) => write!(f, "invalid root motion: {reason}"),
            Self::NoMovement => f.write_str("animation has no authored movement records"),
            Self::InvalidCycle => {
                f.write_str("root-motion cycle is nonfinite or outside loop range")
            }
            Self::UncoveredCycle => {
                f.write_str("root-motion records do not cover the sampled frame")
            }
            Self::NonFiniteSample => f.write_str("root-motion sample overflows finite coordinates"),
        }
    }
}
impl std::error::Error for RootMotionError {}
impl RootMotion {
    pub fn validate(&self) -> Result<(), RootMotionError> {
        if !self.fps.is_finite() || self.fps <= 0. {
            return Err(RootMotionError::InvalidData("frame rate"));
        }
        if self.frame_count == 0 || self.frame_count > 4096 || self.records.len() > 4096 {
            return Err(RootMotionError::InvalidData("frame/record budget"));
        }
        let mut previous = 0;
        for record in &self.records {
            if record.end_frame <= previous || record.end_frame >= self.frame_count as i32 {
                return Err(RootMotionError::InvalidData("movement frame interval"));
            }
            if !record.v0.is_finite()
                || !record.v1.is_finite()
                || !record.end_yaw_degrees.is_finite()
                || !record.direction.is_finite()
                || !record.cumulative_position.is_finite()
            {
                return Err(RootMotionError::InvalidData("nonfinite movement record"));
            }
            previous = record.end_frame;
        }
        Ok(())
    }
    pub fn duration(&self) -> Result<f32, RootMotionError> {
        self.validate()?;
        let duration = (self.frame_count - 1) as f32 / self.fps;
        if !duration.is_finite() {
            return Err(RootMotionError::NonFiniteSample);
        }
        Ok(duration)
    }
    /// Cumulative local position/yaw, including complete cycles in either direction.
    /// The caller applies sequence loop/clamping policy; this method samples animation cycles.
    pub fn position(&self, cycle: f32) -> Result<RootMotionSample, RootMotionError> {
        self.validate()?;
        let Some(last) = self.records.last() else {
            return Err(RootMotionError::NoMovement);
        };
        if !cycle.is_finite() || cycle as f64 <= i32::MIN as f64 || cycle as f64 >= i32::MAX as f64
        {
            return Err(RootMotionError::InvalidCycle);
        }
        // Keep the endpoint convention: cycle1 samples its final frame; negative exact
        // integers sample the prior loop's final frame, rather than changing block selection.
        let loops = if cycle > 1. {
            cycle as i32
        } else if cycle < 0. {
            cycle as i32 - 1
        } else {
            0
        };
        let frame = (cycle - loops as f32) * (self.frame_count - 1) as f32;
        let mut previous_frame = 0.;
        let mut previous_position = Vec3::ZERO;
        let mut previous_yaw = 0.;
        for record in &self.records {
            if frame <= record.end_frame as f32 {
                let fraction =
                    (frame - previous_frame) / (record.end_frame as f32 - previous_frame);
                let distance = (record.v0 + 0.5 * (record.v1 - record.v0) * fraction) * fraction;
                let sample = RootMotionSample {
                    position: previous_position
                        + distance * record.direction
                        + loops as f32 * last.cumulative_position,
                    yaw_degrees: previous_yaw * (1. - fraction)
                        + record.end_yaw_degrees * fraction
                        + loops as f32 * last.end_yaw_degrees,
                };
                return finite_motion(sample);
            }
            previous_frame = record.end_frame as f32;
            previous_position = record.cumulative_position;
            previous_yaw = record.end_yaw_degrees;
        }
        Err(RootMotionError::UncoveredCycle)
    }
    /// Interval displacement expressed relative to the starting heading, with unwrapped yaw.
    pub fn movement(&self, from: f32, to: f32) -> Result<RootMotionSample, RootMotionError> {
        let start = self.position(from)?;
        let end = self.position(to)?;
        let delta = end.position - start.position;
        let (sin, cos) = (-start.yaw_degrees).to_radians().sin_cos();
        finite_motion(RootMotionSample {
            position: Vec3::new(
                delta.x * cos - delta.y * sin,
                delta.x * sin + delta.y * cos,
                delta.z,
            ),
            yaw_degrees: end.yaw_degrees - start.yaw_degrees,
        })
    }
}
fn finite_motion(sample: RootMotionSample) -> Result<RootMotionSample, RootMotionError> {
    if sample.position.is_finite() && sample.yaw_degrees.is_finite() {
        Ok(sample)
    } else {
        Err(RootMotionError::NonFiniteSample)
    }
}
impl Clip {
    pub fn duration(&self) -> f32 {
        self.frames.len().saturating_sub(1) as f32 / self.fps.max(1.)
    }
    /// Event intervals are open on the left, closed on the right, so frame updates never replay them.
    pub fn events_between(&self, previous: f32, current: f32) -> Vec<&ClipEvent> {
        if current < previous || !previous.is_finite() || !current.is_finite() {
            return Vec::new();
        }
        let duration = self.duration();
        let first = if self.looping && duration > 0. {
            (previous.max(0.) / duration).floor() as i32
        } else {
            0
        };
        let last = if self.looping && duration > 0. {
            (current.max(0.) / duration).floor() as i32
        } else {
            0
        };
        let mut result = Vec::new();
        for cycle in first..=last.min(first.saturating_add(64)) {
            for event in &self.events {
                let at = (cycle as f32 + event.cycle) * duration;
                if at > previous && at <= current {
                    result.push(event);
                }
            }
        }
        result
    }
}
/// Model-authored activity assignment. Ordering preserves the include traversal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence {
    pub name: String,
    pub activity: String,
    pub weight: i32,
    #[serde(default)]
    pub order: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rig {
    pub bones: Vec<Bone>,
    pub clips: BTreeMap<String, Clip>,
    #[serde(default)]
    pub sequences: Vec<Sequence>,
    pub warnings: Vec<String>,
}
impl Rig {
    /// Merge preloaded cohorts without changing model traversal order or label ownership.
    pub fn merge_sequence_metadata(&mut self, sequences: impl IntoIterator<Item = Sequence>) {
        for sequence in sequences {
            if let Some(existing) = self.sequences.iter_mut().find(|s| s.name == sequence.name) {
                if sequence.order < existing.order {
                    *existing = sequence;
                }
            } else {
                self.sequences.push(sequence);
            }
        }
        self.sequences.sort_by_key(|s| s.order);
    }
    /// Label first, then model-authored activity. The caller supplies a bounded
    /// integer in 0..total_weight; native RNG/prediction stream parity is separate.
    /// Pass no current sequence for native LookupSequence semantics; activity selection
    /// can retain an appropriate negative-weight current sequence without drawing RNG.
    pub fn lookup_sequence(
        &self,
        name: &str,
        current: Option<&str>,
        mut draw: impl FnMut(u32) -> u32,
    ) -> Option<&str> {
        if let Some(sequence) = self
            .sequences
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
        {
            return Some(&sequence.name);
        }
        if let Some((label, _)) = self.clips.get_key_value(&name.to_lowercase()) {
            return Some(label);
        }
        if let Some(sequence) = self.sequences.iter().find(|s| {
            s.weight < 0
                && s.activity.eq_ignore_ascii_case(name)
                && current.is_some_and(|c| c.eq_ignore_ascii_case(&s.name))
        }) {
            return Some(&sequence.name);
        }
        let choices: Vec<_> = self
            .sequences
            .iter()
            .filter(|s| !s.activity.is_empty() && s.activity.eq_ignore_ascii_case(name))
            .collect();
        let total = choices
            .iter()
            .try_fold(0u32, |total, s| total.checked_add(s.weight.unsigned_abs()))?;
        // The native all-zero-weight table has no valid random interval. Reject it.
        if total == 0 || total > i32::MAX as u32 {
            return None;
        }
        let mut ticket = draw(total);
        if ticket >= total {
            return None;
        }
        for sequence in choices {
            // Native tuple construction gives a zero-weight entry one slot, while
            // its total interval is still the sum of absolute authored weights.
            let slots = sequence.weight.unsigned_abs().max(1);
            if ticket < slots {
                return Some(&sequence.name);
            }
            ticket -= slots;
        }
        None
    }
    pub fn matrices(&self, name: &str, time: f32) -> Vec<Mat4> {
        let clip = self.clips.get(&name.to_lowercase());
        let sample = clip.map(|c| {
            let frames = c.frames.len().saturating_sub(1).max(1) as f32;
            let frame = if c.looping {
                (time.max(0.) * c.fps) % frames
            } else {
                (time.max(0.) * c.fps).min(frames)
            };
            let a = (frame.floor() as usize).min(c.frames.len() - 1);
            let b = (a + 1).min(c.frames.len() - 1);
            (c, a, b, frame.fract())
        });
        let mut world = Vec::<Mat4>::with_capacity(self.bones.len());
        for (id, bone) in self.bones.iter().enumerate() {
            let pose = sample
                .map(|(c, a, b, t)| {
                    let p = &c.frames[a][id];
                    let q = &c.frames[b][id];
                    Pose {
                        position: p.position.lerp(q.position, t),
                        rotation: p.rotation.slerp(q.rotation, t),
                    }
                })
                .unwrap_or_else(|| bone.bind.clone());
            let local = Mat4::from_rotation_translation(pose.rotation, pose.position);
            world.push(
                bone.parent
                    .and_then(|p| world.get(p))
                    .map_or(local, |parent| *parent * local),
            );
        }
        world
            .iter()
            .zip(&self.bones)
            .map(|(m, b)| *m * b.inverse_bind)
            .collect()
    }
}
pub fn skin(position: Vec3, weights: &Weights, matrices: &[Mat4]) -> Vec3 {
    let mut output = Vec3::ZERO;
    let mut total = 0.;
    for (&id, &weight) in weights.bones.iter().zip(&weights.weights) {
        if weight > 0. {
            if let Some(m) = matrices.get(id as usize) {
                output += m.transform_point3(position) * weight;
                total += weight;
            }
        }
    }
    if total > 0. {
        output / total
    } else {
        position
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequence_lookup_preserves_labels_weight_intervals_and_negative_current() {
        let mut rig = Rig {
            sequences: vec![
                Sequence {
                    name: "idle_a".into(),
                    activity: "ACT_IDLE".into(),
                    weight: 2,
                    order: 0,
                },
                Sequence {
                    name: "idle_b".into(),
                    activity: "ACT_IDLE".into(),
                    weight: -1,
                    order: 1,
                },
                Sequence {
                    name: "act_idle".into(),
                    activity: "ACT_OTHER".into(),
                    weight: 1,
                    order: 2,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            rig.lookup_sequence("ACT_IDLE", None, |_| panic!("labels use no random draw")),
            Some("act_idle")
        );
        rig.sequences.pop();
        for (ticket, expected) in [(0, "idle_a"), (1, "idle_a"), (2, "idle_b")] {
            assert_eq!(
                rig.lookup_sequence("aCt_IdLe", None, |total| {
                    assert_eq!(total, 3);
                    ticket
                }),
                Some(expected)
            );
        }
        assert_eq!(
            rig.lookup_sequence("ACT_IDLE", Some("IDLE_B"), |_| panic!(
                "negative current is retained"
            )),
            Some("idle_b")
        );
        assert_eq!(
            rig.lookup_sequence("ACT_IDLE", Some("idle_a"), |_| 2),
            Some("idle_b")
        );
        assert!(rig
            .lookup_sequence("ACT_MISSING", None, |_| panic!())
            .is_none());
        assert!(rig.lookup_sequence("ACT_IDLE", None, |_| 3).is_none());
        rig.sequences[0].weight = 0;
        rig.sequences[1].weight = 0;
        assert!(rig
            .lookup_sequence("ACT_IDLE", None, |_| panic!("zero total is rejected"))
            .is_none());
        rig.sequences[1].weight = 2;
        assert_eq!(rig.lookup_sequence("ACT_IDLE", None, |_| 0), Some("idle_a"));
        assert_eq!(rig.lookup_sequence("ACT_IDLE", None, |_| 1), Some("idle_b"));
        rig.sequences[0].weight = i32::MIN;
        assert!(rig
            .lookup_sequence("ACT_IDLE", None, |_| panic!("overflow rejected"))
            .is_none());
        let first = Sequence {
            name: "first".into(),
            activity: "ACT_NEW".into(),
            weight: 1,
            order: 3,
        };
        let last = Sequence {
            name: "last".into(),
            activity: "ACT_NEW".into(),
            weight: 1,
            order: 8,
        };
        rig.merge_sequence_metadata([last.clone()]);
        rig.merge_sequence_metadata([last, first]);
        assert_eq!(
            rig.sequences
                .iter()
                .filter(|s| s.activity == "ACT_NEW")
                .count(),
            2
        );
        assert_eq!(rig.lookup_sequence("ACT_NEW", None, |_| 0), Some("first"));
        assert_eq!(rig.lookup_sequence("ACT_NEW", None, |_| 1), Some("last"));
    }
    fn turning_motion() -> RootMotion {
        RootMotion {
            fps: 20.,
            frame_count: 21,
            records: vec![
                MovementRecord {
                    end_frame: 10,
                    motion_flags: 0xc0,
                    v0: 4.,
                    v1: 8.,
                    end_yaw_degrees: 90.,
                    direction: Vec3::X,
                    cumulative_position: Vec3::X * 6.,
                },
                MovementRecord {
                    end_frame: 20,
                    motion_flags: 0xc0,
                    v0: 8.,
                    v1: 12.,
                    end_yaw_degrees: 180.,
                    direction: Vec3::Y,
                    cumulative_position: Vec3::new(6., 10., 0.),
                },
            ],
        }
    }
    #[test]
    fn root_motion_piecewise_acceleration_and_cumulative_blocks() {
        let motion = turning_motion();
        assert_eq!(motion.duration(), Ok(1.));
        assert_eq!(
            motion.position(0.25).unwrap(),
            RootMotionSample {
                position: Vec3::X * 2.5,
                yaw_degrees: 45.
            }
        );
        assert_eq!(
            motion.position(0.5).unwrap(),
            RootMotionSample {
                position: Vec3::X * 6.,
                yaw_degrees: 90.
            }
        );
        assert_eq!(
            motion.position(0.75).unwrap(),
            RootMotionSample {
                position: Vec3::new(6., 4.5, 0.),
                yaw_degrees: 135.
            }
        );
        assert_eq!(
            motion.position(1.).unwrap(),
            RootMotionSample {
                position: Vec3::new(6., 10., 0.),
                yaw_degrees: 180.
            }
        );
    }
    #[test]
    fn root_motion_turning_intervals_rotate_by_negative_initial_yaw() {
        let motion = turning_motion();
        let interval = motion.movement(0.5, 0.75).unwrap();
        assert!((interval.position - Vec3::X * 4.5).length() < 1e-5);
        assert_eq!(interval.yaw_degrees, 45.);
        let crossing = motion.movement(0.75, 1.25).unwrap();
        let expected = Vec3::new(3. / 2f32.sqrt(), -8. / 2f32.sqrt(), 0.);
        assert!((crossing.position - expected).length() < 1e-5);
        assert_eq!(crossing.yaw_degrees, 90.);
    }
    #[test]
    fn root_motion_accumulates_complete_positive_and_negative_loops() {
        let motion = turning_motion();
        assert_eq!(
            motion.position(2.).unwrap(),
            RootMotionSample {
                position: Vec3::new(12., 20., 0.),
                yaw_degrees: 360.
            }
        );
        assert_eq!(
            motion.position(-1.).unwrap(),
            RootMotionSample {
                position: Vec3::new(-6., -10., 0.),
                yaw_degrees: -180.
            }
        );
        assert_eq!(
            motion.position(-0.25).unwrap(),
            RootMotionSample {
                position: Vec3::new(0., -5.5, 0.),
                yaw_degrees: -45.
            }
        );
        assert_eq!(
            motion.position(1.25).unwrap(),
            RootMotionSample {
                position: Vec3::new(8.5, 10., 0.),
                yaw_degrees: 225.
            }
        );
    }
    #[test]
    fn root_motion_rejects_invalid_records_and_reports_missing_coverage() {
        let mut motion = turning_motion();
        for cycle in [f32::NAN, f32::INFINITY, i32::MAX as f32, i32::MIN as f32] {
            assert_eq!(motion.position(cycle), Err(RootMotionError::InvalidCycle));
        }
        assert_eq!(
            motion.movement(0., f32::NAN),
            Err(RootMotionError::InvalidCycle)
        );
        motion.records[1].end_frame = 10;
        assert!(motion.validate().is_err());
        motion.records[1].end_frame = 21;
        assert!(motion.validate().is_err());
        motion.records[1].end_frame = 20;
        motion.records[0].v0 = f32::NAN;
        assert!(motion.validate().is_err());
        motion.records[0].v0 = 4.;
        motion.records.truncate(1);
        assert_eq!(motion.position(0.75), Err(RootMotionError::UncoveredCycle));
        motion.records.clear();
        assert_eq!(motion.movement(0., 1.), Err(RootMotionError::NoMovement));
        motion.fps = 0.;
        assert!(motion.duration().is_err());
    }
    fn event_clip(looping: bool) -> Clip {
        Clip {
            fps: 1.,
            looping,
            frames: vec![vec![], vec![]],
            events: vec![
                ClipEvent {
                    cycle: 0.,
                    id: 5004,
                    flags: 0,
                    name: String::new(),
                    options: "start".into(),
                },
                ClipEvent {
                    cycle: 0.5,
                    id: 5004,
                    flags: 0,
                    name: String::new(),
                    options: "middle".into(),
                },
            ],
        }
    }
    #[test]
    fn events_play_once_at_start_and_frame_boundaries() {
        let clip = event_clip(false);
        assert_eq!(clip.events_between(-f32::EPSILON, 0.)[0].options, "start");
        assert!(clip.events_between(0., 0.49).is_empty());
        assert_eq!(clip.events_between(0.49, 0.5)[0].options, "middle");
        assert!(clip.events_between(0.5, 2.).is_empty());
        assert!(clip.events_between(1., 0.5).is_empty());
        assert!(clip.events_between(0., f32::NAN).is_empty());
    }
    #[test]
    fn looping_events_cross_wrap_without_repeating_previous_boundary() {
        let clip = event_clip(true);
        let names = clip
            .events_between(0.75, 1.5)
            .into_iter()
            .map(|e| e.options.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["start", "middle"]);
        assert!(clip.events_between(1.5, 1.75).is_empty());
    }
    #[test]
    fn hierarchy_bind_and_interpolated_skin() {
        let bone = Bone {
            name: "root".into(),
            parent: None,
            bind: Pose {
                position: Vec3::X,
                rotation: Quat::IDENTITY,
            },
            inverse_bind: Mat4::from_translation(-Vec3::X),
        };
        let mut rig = Rig {
            bones: vec![bone],
            ..Default::default()
        };
        assert_eq!(rig.matrices("", 0.)[0], Mat4::IDENTITY);
        rig.clips.insert(
            "move".into(),
            Clip {
                events: Vec::new(),
                fps: 1.,
                looping: false,
                frames: vec![
                    vec![Pose {
                        position: Vec3::X,
                        rotation: Quat::IDENTITY,
                    }],
                    vec![Pose {
                        position: Vec3::X * 3.,
                        rotation: Quat::IDENTITY,
                    }],
                ],
            },
        );
        let p = skin(
            Vec3::Y,
            &Weights {
                bones: [0; 3],
                weights: [1., 0., 0.],
            },
            &rig.matrices("move", 0.5),
        );
        assert!((p - Vec3::new(1., 1., 0.)).length() < 0.0001);
    }
}
