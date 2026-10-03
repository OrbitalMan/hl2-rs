//! Example mod that knows only the shared world API. Reload config with F5.
use anyhow::Result;
use modkit_core::{Block, ModContext, ModPlugin, World};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub name: String,
    pub blocks: Vec<Block>,
}
pub struct Sandbox {
    pub config: Config,
    pub blocks: Vec<Block>,
}
impl Sandbox {
    pub fn read(path: &Path) -> Result<Self> {
        let config: Config = serde_json::from_slice(&std::fs::read(path)?)?;
        anyhow::ensure!(
            config.blocks.iter().all(|b| b.position.is_finite()
                && b.size.is_finite()
                && b.size > 0.
                && b.size <= 1024.),
            "invalid mod block"
        );
        Ok(Self {
            config,
            blocks: Vec::new(),
        })
    }
    pub fn empty() -> Self {
        Self {
            config: Config {
                name: "Sandbox".into(),
                blocks: Vec::new(),
            },
            blocks: Vec::new(),
        }
    }
}
impl ModPlugin for Sandbox {
    fn name(&self) -> &str {
        &self.config.name
    }
    fn on_load(&mut self, world: &World) {
        let (spawn, yaw) = world.spawn();
        let forward = glam::Vec3::new(yaw.cos(), yaw.sin(), 0.);
        let right = glam::Vec3::new(yaw.sin(), -yaw.cos(), 0.);
        self.blocks = self
            .config
            .blocks
            .iter()
            .cloned()
            .map(|mut b| {
                b.position = spawn
                    + forward * b.position.x
                    + right * b.position.y
                    + glam::Vec3::Z * b.position.z;
                b
            })
            .collect();
    }
    fn tick(&mut self, _dt: f32, _context: &mut ModContext<'_>) {}
}
