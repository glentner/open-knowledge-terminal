//! The WebGL2 renderer.
//!
//! Instance data is uploaded once per change of what is drawn: every node's position, world
//! radius, colour and style go into two float textures, every drawn edge's ends, colour and width
//! into two instance buffers. A frame uploads only its uniforms and the back-to-front order of the
//! points (one `u32` per point). Points are instanced quads that read their node from the
//! textures; edges are instanced line quads that read both ends from the same position texture.
//! Both passes blend premultiplied "over" with no depth test: edges first, then points, sorted far
//! to near so nearer cards cover farther ones. Depth reads as falling alpha and saturation and an
//! aerial fade toward the background (`--graph-fog`), never blur.
//!
//! The test hooks (`data-*` on `#graph`, `window.__oktPos`) are written here, from what was
//! drawn and uploaded, never from a camera destination or a parallel computation.

use std::cell::RefCell;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Element, HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlContextAttributes,
    WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::dom;

/// Texels per row of the instance textures; node `i` sits at `(i % TEX_W, i / TEX_W)`.
const TEX_W: usize = 1024;

/// How a point is drawn (the fourth channel of its colour texel).
pub mod style {
    /// a filled disc: a card
    pub const DISC: f32 = 0.0;
    /// an outline: records and external systems, as the legend draws them hollow
    pub const RING: f32 = 1.0;
    /// a disc with a rim: a folder
    pub const FOLDER: f32 = 2.0;
    /// a disc with a halo around it: the selection
    pub const HALO: f32 = 3.0;
}

/// Everything a change of what is drawn uploads.
#[derive(Clone, Default)]
pub struct Instances {
    /// per node: x, y, z, world radius
    pub pos: Vec<f32>,
    /// per node: straight r, g, b, and a [`style`]
    pub col: Vec<f32>,
    /// per drawn edge: its two nodes
    pub ends: Vec<u32>,
    /// per drawn edge: straight r, g, b, a, and its width in CSS px
    pub ecol: Vec<f32>,
}

/// What one frame needs beyond the instances.
pub struct Frame<'a> {
    /// view-projection, column-major
    pub vp: [f32; 16],
    /// the drawn points, farthest first
    pub order: &'a [u32],
    /// drawing buffer size, device px
    pub px: (u32, u32),
    /// device px per world unit at view depth 1
    pub scale: f32,
    pub dpr: f32,
    /// nearest and farthest view depth of the drawn points
    pub depth: (f32, f32),
    pub bg: [f32; 4],
    pub fog: [f32; 4],
}

const POINT_VS: &str = r#"#version 300 es
precision highp float;
precision highp int;
precision highp sampler2D;
uniform mat4 u_vp;
uniform sampler2D u_pos;
uniform sampler2D u_col;
uniform vec2 u_px;
uniform float u_scale;
uniform float u_dpr;
uniform vec2 u_depth;
uniform vec4 u_fog;
layout(location = 0) in vec2 a_corner;
layout(location = 1) in uint a_idx;
out vec2 v_uv;
out vec3 v_rgb;
out float v_alpha;
out float v_rpx;
flat out int v_style;
ivec2 texel(uint i) { return ivec2(int(i % 1024u), int(i / 1024u)); }
void main() {
  vec4 p = texelFetch(u_pos, texel(a_idx), 0);
  vec4 c = texelFetch(u_col, texel(a_idx), 0);
  vec4 clip = u_vp * vec4(p.xyz, 1.0);
  v_style = int(c.a + 0.5);
  if (clip.w <= 0.0) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); return; }
  float rpx = max(p.w * u_scale / clip.w, 1.6 * u_dpr);
  float reach = 1.0;
  if (v_style == 3) { rpx = max(rpx, 3.5 * u_dpr); reach = 2.1; }
  float ext = rpx * reach + 1.0;
  v_rpx = rpx;
  v_uv = a_corner * ext / rpx;
  clip.xy += a_corner * ext * 2.0 / u_px * clip.w;
  gl_Position = clip;
  float t = clamp((clip.w - u_depth.x) / max(u_depth.y - u_depth.x, 1e-3), 0.0, 1.0);
  float l = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
  vec3 rgb = mix(c.rgb, vec3(l), 0.5 * t);
  v_rgb = mix(rgb, u_fog.rgb, 0.45 * t);
  v_alpha = mix(1.0, 0.5, t);
}
"#;

const POINT_FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
in vec3 v_rgb;
in float v_alpha;
in float v_rpx;
flat in int v_style;
out vec4 o;
float band(float d, float lo, float hi, float aa) {
  return clamp((d - lo) / aa + 0.5, 0.0, 1.0) * clamp((hi - d) / aa + 0.5, 0.0, 1.0);
}
void main() {
  float d = length(v_uv);
  float aa = 1.0 / v_rpx;
  float a;
  if (v_style == 1) a = band(d, 1.0 - max(0.4, 1.6 * aa), 1.0, aa);
  else if (v_style == 2) a = max(band(d, -1.0, 0.55, aa), band(d, 0.75, 1.0, aa));
  else if (v_style == 3) a = max(band(d, -1.0, 1.0, aa), 0.9 * band(d, 1.5, 1.5 + 2.0 * aa, aa));
  else a = band(d, -1.0, 1.0, aa);
  a *= v_alpha;
  if (a <= 0.003) discard;
  o = vec4(v_rgb * a, a);
}
"#;

const LINE_VS: &str = r#"#version 300 es
precision highp float;
precision highp int;
precision highp sampler2D;
uniform mat4 u_vp;
uniform sampler2D u_pos;
uniform vec2 u_px;
uniform float u_dpr;
uniform vec2 u_depth;
uniform vec4 u_fog;
layout(location = 0) in vec2 a_corner;
layout(location = 1) in uvec2 a_ends;
layout(location = 2) in vec4 a_col;
layout(location = 3) in float a_w;
out vec4 v_col;
out float v_side;
out float v_hw;
ivec2 texel(uint i) { return ivec2(int(i % 1024u), int(i / 1024u)); }
void main() {
  vec4 pa = u_vp * vec4(texelFetch(u_pos, texel(a_ends.x), 0).xyz, 1.0);
  vec4 pb = u_vp * vec4(texelFetch(u_pos, texel(a_ends.y), 0).xyz, 1.0);
  v_col = vec4(0.0);
  v_side = 0.0;
  v_hw = 1.0;
  if (pa.w <= 1e-3 || pb.w <= 1e-3) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); return; }
  vec2 sa = pa.xy / pa.w * u_px * 0.5;
  vec2 sb = pb.xy / pb.w * u_px * 0.5;
  vec2 dv = sb - sa;
  float len = length(dv);
  vec2 n = len > 1e-4 ? vec2(-dv.y, dv.x) / len : vec2(0.0, 1.0);
  float w = max(a_w * u_dpr, 1.0);
  float hw = w * 0.5 + 1.0;
  vec4 p = mix(pa, pb, a_corner.x);
  p.xy += n * a_corner.y * hw * 2.0 / u_px * p.w;
  gl_Position = p;
  v_side = a_corner.y * hw;
  v_hw = w * 0.5;
  float t = clamp((p.w - u_depth.x) / max(u_depth.y - u_depth.x, 1e-3), 0.0, 1.0);
  float alpha = a_col.a * mix(1.0, 0.5, t) * min(a_w * u_dpr, 1.0);
  v_col = vec4(mix(a_col.rgb, u_fog.rgb, 0.45 * t), alpha);
}
"#;

const LINE_FS: &str = r#"#version 300 es
precision highp float;
in vec4 v_col;
in float v_side;
in float v_hw;
out vec4 o;
void main() {
  float a = v_col.a * clamp(v_hw + 0.5 - abs(v_side), 0.0, 1.0);
  if (a <= 0.002) discard;
  o = vec4(v_col.rgb * a, a);
}
"#;

/// The uniforms both programs may use, looked up once per link.
const UNIFORMS: [&str; 8] = [
    "u_vp", "u_pos", "u_col", "u_px", "u_scale", "u_dpr", "u_depth", "u_fog",
];

struct Prog {
    p: WebGlProgram,
    u: Vec<Option<WebGlUniformLocation>>,
}

impl Prog {
    fn loc(&self, name: &str) -> Option<&WebGlUniformLocation> {
        let i = UNIFORMS.iter().position(|n| *n == name)?;
        self.u[i].as_ref()
    }
}

/// The GPU objects; rebuilt from scratch after a lost context comes back.
struct Res {
    pt: Prog,
    ln: Prog,
    vao_pt: WebGlVertexArrayObject,
    vao_ln: WebGlVertexArrayObject,
    order: WebGlBuffer,
    ends: WebGlBuffer,
    ecol: WebGlBuffer,
    tex_pos: WebGlTexture,
    tex_col: WebGlTexture,
}

pub struct Renderer {
    gl: Gl,
    res: Option<Res>,
    n_edges: i32,
    pos_hash: u64,
}

fn shader(gl: &Gl, kind: u32, src: &str) -> Option<WebGlShader> {
    let s = gl.create_shader(kind)?;
    gl.shader_source(&s, src);
    gl.compile_shader(&s);
    if gl
        .get_shader_parameter(&s, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Some(s)
    } else {
        if !gl.is_context_lost() {
            dom::log_err(
                "graph shader",
                &gl.get_shader_info_log(&s).unwrap_or_default(),
            );
        }
        None
    }
}

fn program(gl: &Gl, vs: &str, fs: &str) -> Option<Prog> {
    let (v, f) = (
        shader(gl, Gl::VERTEX_SHADER, vs)?,
        shader(gl, Gl::FRAGMENT_SHADER, fs)?,
    );
    let p = gl.create_program()?;
    gl.attach_shader(&p, &v);
    gl.attach_shader(&p, &f);
    gl.link_program(&p);
    if !gl
        .get_program_parameter(&p, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        if !gl.is_context_lost() {
            dom::log_err(
                "graph program",
                &gl.get_program_info_log(&p).unwrap_or_default(),
            );
        }
        return None;
    }
    let u = UNIFORMS
        .iter()
        .map(|n| gl.get_uniform_location(&p, n))
        .collect();
    Some(Prog { p, u })
}

/// A static buffer of four quad corners, for a triangle strip.
fn corners(gl: &Gl, xy: &[f32; 8]) -> Option<WebGlBuffer> {
    let b = gl.create_buffer()?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&b));
    gl.buffer_data_with_array_buffer_view(
        Gl::ARRAY_BUFFER,
        &js_sys::Float32Array::from(&xy[..]),
        Gl::STATIC_DRAW,
    );
    Some(b)
}

fn texture(gl: &Gl) -> Option<WebGlTexture> {
    let t = gl.create_texture()?;
    gl.bind_texture(Gl::TEXTURE_2D, Some(&t));
    for (k, v) in [
        (Gl::TEXTURE_MIN_FILTER, Gl::NEAREST),
        (Gl::TEXTURE_MAG_FILTER, Gl::NEAREST),
        (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE),
        (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE),
    ] {
        gl.tex_parameteri(Gl::TEXTURE_2D, k, v as i32);
    }
    Some(t)
}

fn init(gl: &Gl) -> Option<Res> {
    let pt = program(gl, POINT_VS, POINT_FS)?;
    let ln = program(gl, LINE_VS, LINE_FS)?;
    // points: a corner per vertex, the node index per instance
    let vao_pt = gl.create_vertex_array()?;
    gl.bind_vertex_array(Some(&vao_pt));
    corners(gl, &[-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0])?;
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_with_i32(0, 2, Gl::FLOAT, false, 0, 0);
    let order = gl.create_buffer()?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&order));
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_i_pointer_with_i32(1, 1, Gl::UNSIGNED_INT, 0, 0);
    gl.vertex_attrib_divisor(1, 1);
    // edges: (along, side) per vertex; ends, colour and width per instance
    let vao_ln = gl.create_vertex_array()?;
    gl.bind_vertex_array(Some(&vao_ln));
    corners(gl, &[0.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0])?;
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_with_i32(0, 2, Gl::FLOAT, false, 0, 0);
    let ends = gl.create_buffer()?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&ends));
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_i_pointer_with_i32(1, 2, Gl::UNSIGNED_INT, 0, 0);
    gl.vertex_attrib_divisor(1, 1);
    let ecol = gl.create_buffer()?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&ecol));
    gl.enable_vertex_attrib_array(2);
    gl.vertex_attrib_pointer_with_i32(2, 4, Gl::FLOAT, false, 20, 0);
    gl.vertex_attrib_divisor(2, 1);
    gl.enable_vertex_attrib_array(3);
    gl.vertex_attrib_pointer_with_i32(3, 1, Gl::FLOAT, false, 20, 16);
    gl.vertex_attrib_divisor(3, 1);
    gl.bind_vertex_array(None);
    let tex_pos = texture(gl)?;
    let tex_col = texture(gl)?;
    gl.disable(Gl::DEPTH_TEST);
    gl.enable(Gl::BLEND);
    gl.blend_func(Gl::ONE, Gl::ONE_MINUS_SRC_ALPHA);
    Some(Res {
        pt,
        ln,
        vao_pt,
        vao_ln,
        order,
        ends,
        ecol,
        tex_pos,
        tex_col,
    })
}

/// FNV-1a 64 over the x, y, z bits of every uploaded node position.
fn hash_positions(pos: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in pos.as_chunks::<4>().0 {
        for c in &p[..3] {
            for b in c.to_bits().to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

/// A float texture of four channels per node, padded to whole rows.
fn upload_texture(gl: &Gl, tex: &WebGlTexture, data: &[f32]) {
    let n = data.len() / 4;
    let rows = n.div_ceil(TEX_W).max(1);
    let mut buf = vec![0.0f32; TEX_W * rows * 4];
    buf[..data.len()].copy_from_slice(data);
    gl.bind_texture(Gl::TEXTURE_2D, Some(tex));
    let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
        Gl::TEXTURE_2D,
        0,
        Gl::RGBA32F as i32,
        TEX_W as i32,
        rows as i32,
        0,
        Gl::RGBA,
        Gl::FLOAT,
        Some(&js_sys::Float32Array::from(&buf[..])),
    );
}

impl Renderer {
    /// A WebGL2 context on the canvas, or `None` when the browser has none to give.
    pub fn new(cv: &HtmlCanvasElement) -> Option<Renderer> {
        let attrs = WebGlContextAttributes::new();
        attrs.set_alpha(false);
        attrs.set_antialias(true);
        attrs.set_depth(false);
        attrs.set_stencil(false);
        attrs.set_premultiplied_alpha(true);
        let gl: Gl = cv
            .get_context_with_context_options("webgl2", &attrs)
            .ok()
            .flatten()?
            .dyn_into()
            .ok()?;
        let res = init(&gl);
        Some(Renderer {
            gl,
            res,
            n_edges: 0,
            pos_hash: 0,
        })
    }

    /// The context is gone (sleep, a GPU switch): drop every GPU object; draws do nothing.
    pub fn lost(&mut self) {
        self.res = None;
        self.n_edges = 0;
    }

    /// The context is back: rebuild the programs and buffers. The caller re-uploads.
    pub fn restore(&mut self) {
        self.res = init(&self.gl);
    }

    /// Upload what is drawn. Returns false when there is no context to upload to.
    pub fn upload(&mut self, inst: &Instances) -> bool {
        self.pos_hash = hash_positions(&inst.pos);
        let Some(r) = &self.res else { return false };
        if self.gl.is_context_lost() {
            return false;
        }
        let gl = &self.gl;
        upload_texture(gl, &r.tex_pos, &inst.pos);
        upload_texture(gl, &r.tex_col, &inst.col);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&r.ends));
        gl.buffer_data_with_array_buffer_view(
            Gl::ARRAY_BUFFER,
            &js_sys::Uint32Array::from(&inst.ends[..]),
            Gl::DYNAMIC_DRAW,
        );
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&r.ecol));
        gl.buffer_data_with_array_buffer_view(
            Gl::ARRAY_BUFFER,
            &js_sys::Float32Array::from(&inst.ecol[..]),
            Gl::DYNAMIC_DRAW,
        );
        self.n_edges = (inst.ends.len() / 2) as i32;
        true
    }

    /// The hash of the last uploaded positions (`data-pos-hash`).
    pub fn pos_hash(&self) -> u64 {
        self.pos_hash
    }

    /// Draw one frame. Returns the number of edges drawn, or `None` when nothing could be.
    pub fn draw(&self, f: &Frame) -> Option<usize> {
        let r = self.res.as_ref()?;
        let gl = &self.gl;
        if gl.is_context_lost() {
            return None;
        }
        gl.viewport(0, 0, f.px.0 as i32, f.px.1 as i32);
        gl.clear_color(f.bg[0], f.bg[1], f.bg[2], 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT);
        let px = (f.px.0 as f32, f.px.1 as f32);
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&r.tex_pos));
        gl.active_texture(Gl::TEXTURE1);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&r.tex_col));
        let common = |p: &Prog| {
            gl.use_program(Some(&p.p));
            gl.uniform_matrix4fv_with_f32_array(p.loc("u_vp"), false, &f.vp);
            gl.uniform1i(p.loc("u_pos"), 0);
            gl.uniform1i(p.loc("u_col"), 1);
            gl.uniform2f(p.loc("u_px"), px.0, px.1);
            gl.uniform1f(p.loc("u_dpr"), f.dpr);
            gl.uniform2f(p.loc("u_depth"), f.depth.0, f.depth.1);
            gl.uniform4f(p.loc("u_fog"), f.fog[0], f.fog[1], f.fog[2], f.fog[3]);
        };
        if self.n_edges > 0 {
            common(&r.ln);
            gl.bind_vertex_array(Some(&r.vao_ln));
            gl.draw_arrays_instanced(Gl::TRIANGLE_STRIP, 0, 4, self.n_edges);
        }
        if !f.order.is_empty() {
            common(&r.pt);
            gl.uniform1f(r.pt.loc("u_scale"), f.scale);
            gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&r.order));
            gl.buffer_data_with_array_buffer_view(
                Gl::ARRAY_BUFFER,
                &js_sys::Uint32Array::from(f.order),
                Gl::STREAM_DRAW,
            );
            gl.bind_vertex_array(Some(&r.vao_pt));
            gl.draw_arrays_instanced(Gl::TRIANGLE_STRIP, 0, 4, f.order.len() as i32);
        }
        gl.bind_vertex_array(None);
        Some(self.n_edges as usize)
    }
}

// ---- hooks ---------------------------------------------------------------------------------

/// What a frame drew, for the hooks.
pub struct Drawn {
    pub scale: &'static str,
    pub theta_deg: f64,
    pub psi_deg: f64,
    pub dist: f64,
    pub anim: bool,
    pub vp: [f32; 16],
    pub cards: usize,
    pub edges: usize,
    pub pos_hash: u64,
    pub dist_galaxy: f64,
    pub focus: String,
}

thread_local! {
    /// The last value written to each hook, so a frame writes only what changed.
    static HOOKS: RefCell<Vec<(&'static str, String)>> = const { RefCell::new(Vec::new()) };
}

/// Degrees to one decimal, with no "-0.0".
fn deg1(x: f64) -> String {
    let r = (x * 10.0).round() / 10.0;
    let s = format!("{r:.1}");
    if s == "-0.0" { "0.0".into() } else { s }
}

/// Three significant figures, written as a plain number.
fn sig3(x: f64) -> String {
    if !x.is_finite() || x <= 0.0 {
        return "0".into();
    }
    let e = x.log10().floor() as i32;
    let q = 10f64.powi(e - 2);
    let v = (x / q).round() * q;
    if e >= 2 {
        format!("{v:.0}")
    } else {
        format!("{v:.*}", (2 - e) as usize)
    }
}

/// Write a frame's hooks on `#graph`, each only when its value changed. No CSS reads them, so a
/// write costs no style recalculation.
pub fn publish(cv: &Element, d: &Drawn) {
    let view = format!(
        "[{}]",
        d.vp.iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let vals: [(&'static str, String); 11] = [
        ("data-scale", d.scale.to_string()),
        ("data-theta", deg1(d.theta_deg)),
        ("data-psi", deg1(d.psi_deg)),
        ("data-dist", sig3(d.dist)),
        ("data-view", view),
        ("data-drawn", d.cards.to_string()),
        ("data-edges", d.edges.to_string()),
        ("data-pos-hash", format!("{:016x}", d.pos_hash)),
        ("data-dist-galaxy", sig3(d.dist_galaxy)),
        ("data-focus", d.focus.clone()),
        ("data-anim", if d.anim { "1" } else { "0" }.to_string()),
    ];
    HOOKS.with(|h| {
        let mut h = h.borrow_mut();
        for (k, v) in vals {
            match h.iter_mut().find(|(n, _)| *n == k) {
                Some((_, old)) if *old == v => {}
                Some((_, old)) => {
                    let _ = cv.set_attribute(k, &v);
                    *old = v;
                }
                None => {
                    let _ = cv.set_attribute(k, &v);
                    h.push((k, v));
                }
            }
        }
    });
}

/// When a test has put an object on `window.__oktPos`, replace it with `{id: [x, y, z]}` from the
/// positions just uploaded.
pub fn publish_positions(ids: &[String], inst: &Instances) {
    let key = JsValue::from_str("__oktPos");
    let win = dom::win();
    if !js_sys::Reflect::get(&win, &key)
        .map(|v| v.is_object())
        .unwrap_or(false)
    {
        return;
    }
    let out = js_sys::Object::new();
    for (id, p) in ids.iter().zip(inst.pos.as_chunks::<4>().0) {
        let xyz = js_sys::Array::of3(
            &(p[0] as f64).into(),
            &(p[1] as f64).into(),
            &(p[2] as f64).into(),
        );
        let _ = js_sys::Reflect::set(&out, &JsValue::from_str(id), &xyz);
    }
    let _ = js_sys::Reflect::set(&win, &key, &out);
}
