use anyhow::Result;
use macroquad::prelude::*;
use modkit_core::World;
use source_assets::vpk::Vfs;
const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;
uniform mat4 Model;
uniform mat4 Projection;
varying lowp vec4 color;
varying highp vec2 uv;
varying highp vec2 light_uv;
void main(){gl_Position=Projection*Model*vec4(position,1.0);color=color0/255.0;uv=texcoord;light_uv=normal.xy;}
"#;
const FRAGMENT: &str = r#"#version 100
precision mediump float;
uniform sampler2D Texture;
uniform sampler2D Lightmap;
uniform sampler2D Texture2;
uniform float Cutoff;
uniform float TextureAlpha;
uniform float Modulate;
uniform vec2 Scroll;
uniform vec4 Tint;
varying lowp vec4 color;
varying highp vec2 uv;
varying highp vec2 light_uv;
void main(){vec4 base=texture2D(Texture,uv)*texture2D(Texture2,uv+Scroll);base.a=mix(1.0,base.a,TextureAlpha);base*=color*Tint;if(base.a<Cutoff)discard;gl_FragColor=vec4(base.rgb*texture2D(Lightmap,light_uv).rgb*Modulate,base.a);}
"#;
pub fn kind(vfs: &Vfs, name: &str) -> usize {
    if vfs
        .material_value(name, "$additive")
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
    {
        3
    } else if vfs
        .material_value(name, "$translucent")
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
    {
        2
    } else if vfs
        .material_value(name, "$alphatest")
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
    {
        1
    } else {
        0
    }
}
pub struct Materials {
    variants: Vec<Material>,
    white: usize,
    textures: Vec<Texture2D>,
    binding: std::cell::Cell<Option<(usize, usize, miniquad::TextureId)>>,
}
impl Materials {
    pub fn new(world: &World) -> Result<Self> {
        let mut variants = Vec::new();
        let mut textures = world
            .lightmaps
            .iter()
            .map(|p| Texture2D::from_rgba8(p.width, p.height, &p.rgba))
            .collect::<Vec<_>>();
        let white = textures.len();
        textures.push(Texture2D::from_rgba8(1, 1, &[255; 4]));
        for texture in &textures {
            texture.set_filter(FilterMode::Linear);
        }
        for kind in 0..5 {
            let blend = if kind == 4 {
                Some(miniquad::BlendState::new(
                    miniquad::Equation::Add,
                    miniquad::BlendFactor::Value(miniquad::BlendValue::DestinationColor),
                    miniquad::BlendFactor::Zero,
                ))
            } else if kind >= 2 {
                Some(miniquad::BlendState::new(
                    miniquad::Equation::Add,
                    miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                    if kind == 3 {
                        miniquad::BlendFactor::One
                    } else {
                        miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha)
                    },
                ))
            } else {
                None
            };
            let m = load_material(
                miniquad::ShaderSource::Glsl {
                    vertex: VERTEX,
                    fragment: FRAGMENT,
                },
                MaterialParams {
                    pipeline_params: miniquad::PipelineParams {
                        depth_test: miniquad::Comparison::LessOrEqual,
                        depth_write: kind < 2,
                        color_blend: blend,
                        ..Default::default()
                    },
                    uniforms: vec![
                        miniquad::UniformDesc::new("Modulate", miniquad::UniformType::Float1),
                        miniquad::UniformDesc::new("Cutoff", miniquad::UniformType::Float1),
                        miniquad::UniformDesc::new("TextureAlpha", miniquad::UniformType::Float1),
                        miniquad::UniformDesc::new("Scroll", miniquad::UniformType::Float2),
                        miniquad::UniformDesc::new("Tint", miniquad::UniformType::Float4),
                    ],
                    textures: vec!["Lightmap".into(), "Texture2".into()],
                },
            )?;
            m.set_uniform("Cutoff", if kind == 1 { 0.5f32 } else { 0.001f32 });
            // Opaque Source textures may use alpha for self-illumination/envmap masks.
            m.set_uniform(
                "TextureAlpha",
                if kind == 0 || kind == 4 { 0f32 } else { 1f32 },
            );
            m.set_uniform("Modulate", if kind == 4 { 2f32 } else { 1f32 });
            variants.push(m);
        }
        Ok(Self {
            variants,
            white,
            textures,
            binding: std::cell::Cell::new(None),
        })
    }
    pub fn select(
        &self,
        lightmap: Option<usize>,
        kind: usize,
        second: Option<&Texture2D>,
        scroll: Vec2,
        tint: Vec4,
    ) {
        // Custom samplers are pipeline state in macroquad, not per-draw snapshots.
        // Flush before rebinding to keep earlier draws correct and stay below its 32-pipeline limit.
        let material = &self.variants[kind];
        let page = lightmap.unwrap_or(self.white);
        let second = second
            .cloned()
            .unwrap_or_else(|| self.textures[self.white].clone());
        let binding = (page, kind, second.raw_miniquad_id());
        if self.binding.get() != Some(binding) {
            unsafe {
                get_internal_gl().flush();
            }
            material.set_texture("Lightmap", self.textures[page].clone());
            material.set_texture("Texture2", second);
            self.binding.set(Some(binding));
        }
        material.set_uniform("Scroll", scroll);
        material.set_uniform("Tint", tint);
        gl_use_material(material);
    }
}
