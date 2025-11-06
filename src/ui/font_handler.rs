use crate::ui::overlay;
use crate::ui::overlay::SHADERS;
use fontdue::Font;
use std::collections::HashMap;
use std::slice::from_raw_parts;
use std::sync::{LazyLock, OnceLock};
use windows::Win32::Foundation::{FALSE, TRUE};
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
use crate::utilities::is_crimson_loaded;

static BLEND_STATE: OnceLock<ID3D11BlendState> = OnceLock::new();
static SAMPLER: OnceLock<ID3D11SamplerState> = OnceLock::new();
static FONT_COLOR: OnceLock<ID3D11Buffer> = OnceLock::new();
static D3D_SHADERS: OnceLock<(ID3D11VertexShader, ID3D11PixelShader)> = OnceLock::new();

static FONT: LazyLock<Font> = LazyLock::new(|| {
    let font_data = include_bytes!("../data/Roboto-Regular.ttf") as &[u8];
    Font::from_bytes(font_data, fontdue::FontSettings::default()).unwrap()
});

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub x: u32,         // atlas x
    pub y: u32,         // atlas y
    pub width: u32,     // glyph bitmap width
    pub height: u32,    // glyph bitmap height
    pub advance: i32,   // cursor advance after drawing
    pub bearing_x: i32, // left offset from cursor to glyph
    pub bearing_y: i32, // top offset from baseline
}

pub struct FontAtlas {
    pub texture: Option<ID3D11ShaderResourceView>,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub atlas_width: u32,
    pub atlas_height: u32,
}

impl FontAtlas {
    pub(crate) fn glyph_advance(&self, c: char) -> f32 {
        self.glyphs.get(&c).unwrap().advance as f32
    }
}

/// Glyph quad in pixel space (top-left/bottom-right) and UVs.
pub struct GlyphQuad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Vertex {
    pos: [f32; 3], // x, y, z (z = 0)
    uv: [f32; 2],  // u, v in [0,1]
}

#[repr(C)]
pub struct FontColorCB {
    color: [f32; 4], // RGBA
}

impl FontColorCB {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            color: [r, g, b, a],
        }
    }
}

pub const BLACK: FontColorCB = FontColorCB::new(0.0, 0.0, 0.0, 1.0);
pub const WHITE: FontColorCB = FontColorCB::new(1.0, 1.0, 1.0, 1.0);

pub const RED: FontColorCB = FontColorCB::new(1.0, 0.0, 0.0, 1.0);

pub const GREEN: FontColorCB = FontColorCB::new(0.0, 1.0, 0.0, 1.0);
pub const YELLOW: FontColorCB = FontColorCB::new(0.98, 0.98, 0.824, 1.0); // Used for other slots

pub fn get_default_color() -> &'static FontColorCB {
    if is_crimson_loaded() {
        &WHITE
    } else {
        &BLACK
    }
}

pub fn create_rgba_font_atlas(
    device: &ID3D11Device,
    chars: &[char],
    font_size: f32,
    max_row_width: u32,
) -> Option<FontAtlas> {
    let mut glyph_bitmaps: Vec<(char, Vec<u8>, fontdue::Metrics)> = Vec::new();
    for &c in chars {
        let (metrics, bitmap) = (&*FONT).rasterize(c, font_size);
        glyph_bitmaps.push((c, bitmap, metrics));
    }

    let mut rows: Vec<Vec<(char, Vec<u8>, fontdue::Metrics)>> = Vec::new();
    let mut current_row = Vec::new();
    let mut current_width = 0;

    for (c, bitmap, metrics) in glyph_bitmaps.into_iter() {
        let w = metrics.width as u32;
        if current_width + w > max_row_width {
            rows.push(current_row);
            current_row = Vec::new();
            current_width = 0;
        }
        current_row.push((c, bitmap, metrics));
        current_width += w;
    }
    if !current_row.is_empty() {
        rows.push(current_row);
    }

    let atlas_width = max_row_width;
    let atlas_height: u32 = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|(_, _, m)| m.height as u32)
                .max()
                .unwrap_or(0)
        })
        .sum();

    let mut atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize]; // RGBA

    let mut glyph_infos = HashMap::new();
    let mut y_offset = 0;

    for row in rows {
        let row_height = row
            .iter()
            .map(|(_, bitmap, metrics)| {
                let w = metrics.width.max(1);
                if bitmap.is_empty() {
                    0
                } else {
                    bitmap.len() as u32 / w as u32
                }
            })
            .max()
            .unwrap_or(0);

        let mut x_offset = 0;

        for (c, bitmap, metrics) in row {
            let w = metrics.width as u32;
            let h = if w > 0 { bitmap.len() as u32 / w } else { 0 };

            if w > 0 && h > 0 {
                for y in 0..h {
                    for x in 0..w {
                        let src_idx = (y * w + x) as usize;
                        let dst_idx =
                            (((y_offset + y) * atlas_width + (x_offset + x)) * 4) as usize;
                        let value = bitmap[src_idx];
                        atlas_data[dst_idx + 0] = 255;
                        atlas_data[dst_idx + 1] = 255;
                        atlas_data[dst_idx + 2] = 255;
                        atlas_data[dst_idx + 3] = value;
                    }
                }
            }

            glyph_infos.insert(
                c,
                GlyphInfo {
                    x: x_offset,
                    y: y_offset,
                    width: w,
                    height: h,
                    advance: metrics.advance_width.round() as i32,
                    bearing_x: metrics.xmin,
                    bearing_y: metrics.ymin,
                },
            );

            x_offset += w;
        }

        y_offset += row_height;
    }

    let desc = D3D11_TEXTURE2D_DESC {
        Width: atlas_width,
        Height: atlas_height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        ..Default::default()
    };

    let init = D3D11_SUBRESOURCE_DATA {
        pSysMem: atlas_data.as_ptr() as *const _,
        SysMemPitch: atlas_width * 4,
        SysMemSlicePitch: 0,
    };

    unsafe {
        let mut texture: Option<ID3D11Texture2D> = None;
        if device
            .CreateTexture2D(&desc, Some(&init), Some(&mut texture))
            .is_err()
            || texture.is_none()
        {
            return None;
        }

        let mut shader_view: Option<ID3D11ShaderResourceView> = None;
        if device
            .CreateShaderResourceView(texture.as_ref().unwrap(), None, Some(&mut shader_view))
            .is_err()
            || shader_view.is_none()
        {
            return None;
        }

        Some(FontAtlas {
            texture: shader_view,
            glyphs: glyph_infos,
            atlas_width,
            atlas_height,
        })
    }
}

/// Compute quad for a glyph given a baseline and atlas info
pub fn glyph_quad(
    x: f32,
    baseline: f32,
    glyph: &GlyphInfo,
    atlas_width: u32,
    atlas_height: u32,
) -> GlyphQuad {
    let px0 = x + glyph.bearing_x as f32;
    let py0 = baseline - glyph.height as f32 - glyph.bearing_y as f32; // top
    let px1 = px0 + glyph.width as f32;
    let py1 = py0 + glyph.height as f32; // bottom

    let u0 = glyph.x as f32 / atlas_width as f32;
    let v0 = glyph.y as f32 / atlas_height as f32;
    let u1 = (glyph.x + glyph.width) as f32 / atlas_width as f32;
    let v1 = (glyph.y + glyph.height) as f32 / atlas_height as f32;

    GlyphQuad {
        x0: px0,
        y0: py0,
        x1: px1,
        y1: py1,
        u0,
        v0,
        u1,
        v1,
    }
}

pub fn glyph_vertices(quad: &GlyphQuad, screen_width: f32, screen_height: f32) -> [Vertex; 6] {
    let x0 = (quad.x0 / screen_width) * 2.0 - 1.0;
    let y0 = 1.0 - (quad.y0 / screen_height) * 2.0;
    let x1 = (quad.x1 / screen_width) * 2.0 - 1.0;
    let y1 = 1.0 - (quad.y1 / screen_height) * 2.0;

    [
        Vertex {
            pos: [x0, y0, 0.0],
            uv: [quad.u0, quad.v0],
        }, // TL
        Vertex {
            pos: [x1, y0, 0.0],
            uv: [quad.u1, quad.v0],
        }, // TR
        Vertex {
            pos: [x0, y1, 0.0],
            uv: [quad.u0, quad.v1],
        }, // BL
        Vertex {
            pos: [x1, y0, 0.0],
            uv: [quad.u1, quad.v0],
        }, // TR
        Vertex {
            pos: [x1, y1, 0.0],
            uv: [quad.u1, quad.v1],
        }, // BR
        Vertex {
            pos: [x0, y1, 0.0],
            uv: [quad.u0, quad.v1],
        }, // BL
    ]
}

pub fn draw_string(
    state: &overlay::D3D11State,
    text: &String,
    x: f32,
    y: f32,
    screen_width: f32,
    screen_height: f32,
    color: &FontColorCB,
) {
    let context = &state.context;
    let vertex_buffer = &state.vertex_buffer;
    let input_layout = &state.input_layout;
    let font_atlas = &state.atlas.as_ref().unwrap();
    unsafe {
        let stride = size_of::<Vertex>() as u32;
        let offset = 0u32;
        context.IASetInputLayout(Some(input_layout));
        context.IASetVertexBuffers(
            0,
            1,
            Some(&Some(vertex_buffer.clone())),
            Some(&stride),
            Some(&offset),
        );
        context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

        let (vs, ps) = D3D_SHADERS.get_or_init(|| set_shaders(&&context.GetDevice().unwrap()));
        context.VSSetShader(vs, None);
        context.PSSetShader(ps, None);

        if let Some(srv) = &font_atlas.texture {
            context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
        }
        let sampler = SAMPLER.get_or_init(|| {
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            let mut sampler = None;
            context
                .GetDevice()
                .unwrap()
                .CreateSamplerState(&desc, Some(&mut sampler))
                .unwrap();
            sampler.unwrap()
        });
        context.PSSetSamplers(0, Some(&[Some(sampler.clone())]));

        let blend_state = BLEND_STATE.get_or_init(|| {
            let desc = D3D11_BLEND_DESC {
                AlphaToCoverageEnable: FALSE,
                IndependentBlendEnable: FALSE,
                RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: TRUE,
                    SrcBlend: D3D11_BLEND_SRC_ALPHA,
                    DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                    BlendOp: D3D11_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D11_BLEND_ONE,
                    DestBlendAlpha: D3D11_BLEND_ZERO,
                    BlendOpAlpha: D3D11_BLEND_OP_ADD,
                    RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                }; 8],
            };
            let mut bs = None;
            context
                .GetDevice()
                .unwrap()
                .CreateBlendState(&desc, Some(&mut bs))
                .unwrap();
            bs.unwrap()
        });
        context.OMSetBlendState(Some(blend_state), None, 0xffffffff);

        let mut vertices: Vec<Vertex> = Vec::with_capacity(text.len() * 6);

        let baseline = y + 30.0; // baseline + constant y level
        let mut pen_x = x;

        let font_color_cb = FONT_COLOR.get_or_init(|| {
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: size_of::<FontColorCB>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                MiscFlags: 0,
                StructureByteStride: 0,
                ..Default::default()
            };
            let mut font_color_cb: Option<ID3D11Buffer> = None;
            if let Err(e) =
                &&context
                    .GetDevice()
                    .unwrap()
                    .CreateBuffer(&desc, None, Some(&mut font_color_cb))
            {
                log::error!("Error creating font color buffer: {:?}", e);
            }
            font_color_cb.unwrap()
        });
        let mut mapped: D3D11_MAPPED_SUBRESOURCE = std::mem::zeroed();
        context
            .Map(
                font_color_cb,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped),
            )
            .unwrap();
        std::ptr::copy_nonoverlapping(color, mapped.pData as *mut FontColorCB, 1);
        context.Unmap(font_color_cb, 0);
        context.PSSetConstantBuffers(0, Some(&[Some(font_color_cb.clone())]));

        for c in text.chars() {
            if let Some(glyph) = font_atlas.glyphs.get(&c) {
                let quad = glyph_quad(
                    pen_x,
                    baseline,
                    glyph,
                    font_atlas.atlas_width,
                    font_atlas.atlas_height,
                );
                let verts = glyph_vertices(&quad, screen_width, screen_height);
                vertices.extend_from_slice(&verts);
                pen_x += glyph.advance as f32;
            } else {
                pen_x += 8.0;
            }
        }

        if !vertices.is_empty() {
            let mut mapped: D3D11_MAPPED_SUBRESOURCE = std::mem::zeroed();
            let hr = context.Map(
                vertex_buffer,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped),
            );
            if hr.is_ok() {
                let dst_ptr = mapped.pData as *mut Vertex;
                std::ptr::copy_nonoverlapping(vertices.as_ptr(), dst_ptr, vertices.len());
                context.Unmap(vertex_buffer, 0);
                context.Draw(vertices.len() as u32, 0);
            } else {
                log::error!("Map vertex_buffer failed: {:?}", hr);
            }
        }
    }
}

pub(crate) fn set_shaders(device: &&ID3D11Device) -> (ID3D11VertexShader, ID3D11PixelShader) {
    let (psb, vsb) = &*SHADERS;
    let mut vs: Option<ID3D11VertexShader> = None;
    let mut ps: Option<ID3D11PixelShader> = None;
    unsafe {
        device
            .CreateVertexShader(
                from_raw_parts(vsb.GetBufferPointer() as *const u8, vsb.GetBufferSize()),
                None,
                Some(&mut vs),
            )
            .unwrap();
        device
            .CreatePixelShader(
                from_raw_parts(psb.GetBufferPointer() as *const u8, psb.GetBufferSize()),
                None,
                Some(&mut ps),
            )
            .unwrap();
    }
    (vs.unwrap(), ps.unwrap())
}

