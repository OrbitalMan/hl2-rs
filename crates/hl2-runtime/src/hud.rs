//! Macroquad drawing adapter for the shared Source-resource HUD.
use crate::{
    gameplay::{Inventory, Weapon},
    selection::Selection,
};
use anyhow::Result;
use hl2_ui::canvas::{Canvas, FilterMode as CpuFilter, Texture2D as CpuTexture};
use macroquad::prelude::*;
use source_assets::vpk::Vfs;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ops::Deref,
};
const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;
varying lowp vec4 color;
varying highp vec2 uv;
void main(){gl_Position=Projection*Model*vec4(position,1.0);color=color0/255.0;uv=texcoord;}
"#;
const FRAGMENT: &str = r#"#version 100
precision mediump float;
uniform sampler2D Texture;
varying lowp vec4 color;
varying highp vec2 uv;
void main(){gl_FragColor=color*texture2D(Texture,uv);}
"#;

pub struct WeaponHud {
    hud: hl2_ui::hud::WeaponHud,
    textures: RefCell<HashMap<usize, (CpuTexture, Texture2D)>>,
    additive: Material,
}
impl Deref for WeaponHud {
    type Target = hl2_ui::hud::WeaponHud;
    fn deref(&self) -> &Self::Target {
        &self.hud
    }
}
impl WeaponHud {
    pub fn load(vfs: &Vfs) -> Result<Self> {
        let canvas = Canvas::default();
        let hud = hl2_ui::hud::WeaponHud::load(vfs, canvas)?;
        let additive = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(miniquad::BlendState::new(
                        miniquad::Equation::Add,
                        miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                        miniquad::BlendFactor::One,
                    )),
                    depth_write: false,
                    depth_test: miniquad::Comparison::Always,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        Ok(Self {
            hud,
            textures: RefCell::new(HashMap::new()),
            additive,
        })
    }
    fn viewport(&self) {
        self.hud.canvas.resize(screen_width(), screen_height());
    }
    fn paint(&self) {
        let mut textures = self.textures.borrow_mut();
        for quad in self.hud.canvas.drain() {
            if quad.additive {
                gl_use_material(&self.additive);
            } else {
                gl_use_default_material();
            }
            let color = Color::new(quad.color.r, quad.color.g, quad.color.b, quad.color.a);
            let r = quad.destination;
            if let Some(cpu) = quad.texture {
                let texture = &textures
                    .entry(cpu.id())
                    .or_insert_with(|| {
                        let texture = Texture2D::from_rgba8(cpu.0.width, cpu.0.height, &cpu.0.rgba);
                        texture.set_filter(match cpu.filter() {
                            CpuFilter::Nearest => FilterMode::Nearest,
                            CpuFilter::Linear => FilterMode::Linear,
                        });
                        (cpu.clone(), texture)
                    })
                    .1;
                let source = quad.source;
                draw_texture_ex(
                    texture,
                    r.x,
                    r.y,
                    color,
                    DrawTextureParams {
                        dest_size: Some(vec2(r.w, r.h)),
                        source: Some(Rect::new(source.x, source.y, source.w, source.h)),
                        ..Default::default()
                    },
                );
            } else {
                draw_rectangle(r.x, r.y, r.w, r.h, color);
            }
        }
        gl_use_default_material();
    }
    pub fn draw_status(
        &self,
        inv: &Inventory,
        weapons: &BTreeMap<String, Weapon>,
        selection: &Selection,
        time: f64,
    ) {
        self.viewport();
        self.hud.draw_status(inv, weapons, selection, time);
        self.paint();
    }
    pub fn draw_selection(
        &self,
        selection: &Selection,
        inv: &Inventory,
        weapons: &BTreeMap<String, Weapon>,
        time: f64,
    ) {
        self.viewport();
        self.hud.draw_selection(selection, inv, weapons, time);
        self.paint();
    }
    pub fn draw_crosshair(&self, inv: &Inventory) {
        self.viewport();
        self.hud.draw_crosshair(inv);
        self.paint();
    }
}
