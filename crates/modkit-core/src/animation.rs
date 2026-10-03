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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rig {
    pub bones: Vec<Bone>,
    pub clips: BTreeMap<String, Clip>,
    pub warnings: Vec<String>,
}
impl Rig {
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
