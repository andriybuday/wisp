use std::sync::Arc;
use winit::{dpi::PhysicalSize, window::Window};

use crate::config::{ColorPalette, Config};
use crate::font::FontManager;
use crate::terminal::{Color, Terminal};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    color_palette: ColorPalette,
    /// Padding in physical pixels (config padding scaled by the display factor)
    padding: f32,
    font_manager: FontManager,
    texture: wgpu::Texture,
    texture_bind_group: wgpu::BindGroup,
    /// Glyph atlas bookkeeping: size of one square slot, slots per row, and the
    /// top-left atlas position assigned to each character on first use.
    atlas_slot: u32,
    atlas_cols: u32,
    glyph_positions: std::collections::HashMap<char, (u32, u32)>,
    /// Persistent frame buffers, recreated only when capacity must grow
    /// and updated in place via `queue.write_buffer`. Capacities are in
    /// vertices / indices respectively.
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    vertex_capacity: usize,
    index_capacity: usize,
}

/// Side length of the (square) glyph atlas texture, in texels.
const ATLAS_SIZE: u32 = 1024;

impl Renderer {
    pub fn new(window: Arc<Window>, terminal_config: &Config) -> Self {
        let size = window.inner_size();
        // Rasterize and lay out in physical pixels so text is crisp and
        // correctly sized on HiDPI / Retina displays (scale_factor > 1.0).
        let scale = window.scale_factor() as f32;

        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .expect("Failed to create surface");

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        // Request device
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Wisp Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let mut font_manager = FontManager::new(terminal_config.font_size * scale);

        // Size each atlas slot to comfortably hold a glyph at the current
        // (physical) font size, then derive how many slots fit per row. This
        // keeps every glyph inside the atlas regardless of its character code.
        let atlas_slot = (font_manager.cell_width().ceil() as u32)
            .max(font_manager.cell_height().ceil() as u32)
            + 4;
        let atlas_cols = (ATLAS_SIZE / atlas_slot).max(1);

        // Create texture for glyph atlas (simple 1024x1024 for now)
        let texture_size = wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Initialize texture with a white pixel at (0,0) for background rendering
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8], // White pixel
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("texture_bind_group"),
        });

        // Create render pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            color_palette: ColorPalette::from_config(terminal_config),
            padding: terminal_config.padding * scale,
            font_manager,
            texture,
            texture_bind_group,
            atlas_slot,
            atlas_cols,
            glyph_positions: std::collections::HashMap::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_capacity: 0,
            index_capacity: 0,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Grow the persistent frame buffers if `vertex_count`/`index_count`
    /// exceed capacity; otherwise leave them untouched.
    fn ensure_buffer_capacity(&mut self, vertex_count: usize, index_count: usize) {
        if self.vertex_buffer.is_none() || vertex_count > self.vertex_capacity {
            self.vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Vertex Buffer"),
                size: (vertex_count * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_capacity = vertex_count;
        }
        if self.index_buffer.is_none() || index_count > self.index_capacity {
            self.index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Index Buffer"),
                size: (index_count * std::mem::size_of::<u32>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.index_capacity = index_count;
        }
    }

    pub fn render(&mut self, terminal: &Terminal, blink_on: bool) {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Surface out of memory");
                return;
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Build vertices for all visible characters
        let vertices = self.build_vertices(terminal, blink_on);
        let indices = Self::build_indices(vertices.len() / 4);

        log::debug!(
            "Render: vertices={}, indices={}",
            vertices.len(),
            indices.len()
        );

        // If there are no vertices to render, just clear the screen and present
        if vertices.is_empty() || indices.is_empty() {
            {
                let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: self.color_palette.background[0] as f64,
                                g: self.color_palette.background[1] as f64,
                                b: self.color_palette.background[2] as f64,
                                a: self.color_palette.background[3] as f64,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
                // Render pass is dropped here, which clears the screen
            }

            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();
            return;
        }

        self.ensure_buffer_capacity(vertices.len(), indices.len());
        // Just ensured, so both buffers exist.
        let (Some(vertex_buffer), Some(index_buffer)) =
            (self.vertex_buffer.as_ref(), self.index_buffer.as_ref())
        else {
            return;
        };
        self.queue
            .write_buffer(vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        self.queue
            .write_buffer(index_buffer, 0, bytemuck::cast_slice(&indices));
        let vertex_bytes = (vertices.len() * std::mem::size_of::<Vertex>()) as u64;
        let index_bytes = (indices.len() * std::mem::size_of::<u32>()) as u64;

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.color_palette.background[0] as f64,
                            g: self.color_palette.background[1] as f64,
                            b: self.color_palette.background[2] as f64,
                            a: self.color_palette.background[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..vertex_bytes));
            render_pass
                .set_index_buffer(index_buffer.slice(..index_bytes), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn build_vertices(&mut self, terminal: &Terminal, blink_on: bool) -> Vec<Vertex> {
        let mut vertices = Vec::new();

        let cell_width = self.font_manager.cell_width();
        let cell_height = self.font_manager.cell_height();
        let ascent = self.font_manager.ascent();

        let width_ndc = 2.0 / self.size.width as f32;
        let height_ndc = 2.0 / self.size.height as f32;

        // The cursor is shown only in the blink-on phase, when the app has
        // not hidden it, and when the viewport is not scrolled back.
        let show_cursor = blink_on && terminal.cursor_visible() && terminal.scroll_offset() == 0;
        let (cursor_col, cursor_row) = terminal.cursor();

        // Texture coords for solid color (white pixel at 0,0)
        let bg_u0 = 0.0;
        let bg_v0 = 0.0;
        let bg_u1 = 1.0 / 1024.0;
        let bg_v1 = 1.0 / 1024.0;

        for row in 0..terminal.rows() {
            for col in 0..terminal.cols() {
                if let Some(cell) = terminal.get_viewport_cell(col, row) {
                    let x = col as f32 * cell_width + self.padding;
                    let y = row as f32 * cell_height + self.padding;

                    // Convert to NDC
                    let x_ndc = x * width_ndc - 1.0;
                    let y_ndc = 1.0 - y * height_ndc;
                    let cell_w_ndc = cell_width * width_ndc;
                    let cell_h_ndc = cell_height * height_ndc;

                    // Bold brightens only the base 8 indexed colors.
                    let mut fg = cell.fg;
                    if cell.flags.contains(crate::terminal::CellFlags::BOLD) {
                        if let Color::Indexed(n) = fg {
                            if n <= 7 {
                                fg = Color::Indexed(n + 8);
                            }
                        }
                    }

                    // Resolve to RGBA; inverse swaps the resolved colors.
                    let resolve = |c: Color, default: [f32; 4]| match c {
                        Color::Default => default,
                        Color::Indexed(n) => self.color_palette.ansi_256(n),
                        Color::Rgb(r, g, b) => {
                            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
                        }
                    };
                    let mut fg_color = resolve(fg, self.color_palette.foreground);
                    let mut bg_color = resolve(cell.bg, self.color_palette.background);
                    if cell.flags.contains(crate::terminal::CellFlags::INVERSE) {
                        std::mem::swap(&mut fg_color, &mut bg_color);
                    }

                    // Check if this cell is selected
                    let is_selected = terminal.is_cell_selected(col, row);
                    if is_selected {
                        // Use a distinct selection color (light blue/gray)
                        bg_color = [0.3, 0.5, 0.7, 1.0];
                    }

                    // Shown cursor: it becomes this cell's background, and
                    // the glyph (drawn below, after the background quad)
                    // takes the palette background color so the character
                    // under the cursor stays readable.
                    let is_cursor = show_cursor && col == cursor_col && row == cursor_row;
                    if is_cursor {
                        bg_color = self.color_palette.cursor;
                        fg_color = self.color_palette.background;
                    }

                    // Render background if not the default (or if inverse or selected)
                    if cell.bg != Color::Default
                        || cell.flags.contains(crate::terminal::CellFlags::INVERSE)
                        || is_selected
                        || is_cursor
                    {
                        // Background quad (using a solid color, no texture)
                        vertices.extend_from_slice(&[
                            Vertex {
                                position: [x_ndc, y_ndc, 0.0],
                                tex_coords: [bg_u0, bg_v0],
                                color: bg_color,
                            },
                            Vertex {
                                position: [x_ndc + cell_w_ndc, y_ndc, 0.0],
                                tex_coords: [bg_u1, bg_v0],
                                color: bg_color,
                            },
                            Vertex {
                                position: [x_ndc + cell_w_ndc, y_ndc - cell_h_ndc, 0.0],
                                tex_coords: [bg_u1, bg_v1],
                                color: bg_color,
                            },
                            Vertex {
                                position: [x_ndc, y_ndc - cell_h_ndc, 0.0],
                                tex_coords: [bg_u0, bg_v1],
                                color: bg_color,
                            },
                        ]);
                    }

                    if cell.ch == ' ' {
                        continue;
                    }

                    let glyph = self.font_manager.get_glyph(cell.ch);

                    // Assign this character a fixed atlas slot the first time we
                    // see it, uploading its bitmap once. Slots are bounded to the
                    // atlas, so any character code is safe (no overrun panic).
                    if !self.glyph_positions.contains_key(&cell.ch) {
                        let rows = ATLAS_SIZE / self.atlas_slot;
                        let capacity = self.atlas_cols * rows;
                        if self.glyph_positions.len() as u32 >= capacity {
                            // Atlas full: recycle slots (rare for typical use).
                            self.glyph_positions.clear();
                        }
                        let idx = self.glyph_positions.len() as u32;
                        let atlas_x = (idx % self.atlas_cols) * self.atlas_slot;
                        let atlas_y = (idx / self.atlas_cols) * self.atlas_slot;
                        self.glyph_positions.insert(cell.ch, (atlas_x, atlas_y));

                        if !glyph.bitmap.is_empty() {
                            // Clamp to the slot so an oversized glyph can never
                            // write outside its slot / the atlas.
                            let gw = (glyph.width as u32).min(self.atlas_slot);
                            let gh = (glyph.height as u32).min(self.atlas_slot);
                            self.queue.write_texture(
                                wgpu::ImageCopyTexture {
                                    texture: &self.texture,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d {
                                        x: atlas_x,
                                        y: atlas_y,
                                        z: 0,
                                    },
                                    aspect: wgpu::TextureAspect::All,
                                },
                                &glyph.bitmap,
                                wgpu::ImageDataLayout {
                                    offset: 0,
                                    bytes_per_row: Some(glyph.width as u32),
                                    rows_per_image: Some(glyph.height as u32),
                                },
                                wgpu::Extent3d {
                                    width: gw,
                                    height: gh,
                                    depth_or_array_layers: 1,
                                },
                            );
                        }
                    }

                    let (atlas_x, atlas_y) = self.glyph_positions[&cell.ch];
                    let gw = (glyph.width as u32).min(self.atlas_slot);
                    let gh = (glyph.height as u32).min(self.atlas_slot);

                    let w_ndc = gw as f32 * width_ndc;
                    let h_ndc = gh as f32 * height_ndc;

                    let u0 = atlas_x as f32 / ATLAS_SIZE as f32;
                    let v0 = atlas_y as f32 / ATLAS_SIZE as f32;
                    let u1 = (atlas_x + gw) as f32 / ATLAS_SIZE as f32;
                    let v1 = (atlas_y + gh) as f32 / ATLAS_SIZE as f32;

                    // Position the glyph on the text baseline using fontdue's
                    // per-glyph bearings (xmin / ymin), so glyphs of different
                    // sizes line up instead of all sticking to the cell top.
                    // ymin is the offset from baseline to the bitmap bottom, so
                    // the bitmap top sits (ymin + height) above the baseline.
                    let baseline = y + ascent;
                    let glyph_left = x + glyph.offset_x as f32;
                    let glyph_top = baseline - (glyph.offset_y as f32 + glyph.height as f32);
                    let gx_ndc = glyph_left * width_ndc - 1.0;
                    let gy_ndc = 1.0 - glyph_top * height_ndc;

                    // Create quad (two triangles) for the glyph
                    vertices.extend_from_slice(&[
                        Vertex {
                            position: [gx_ndc, gy_ndc, 0.0],
                            tex_coords: [u0, v0],
                            color: fg_color,
                        },
                        Vertex {
                            position: [gx_ndc + w_ndc, gy_ndc, 0.0],
                            tex_coords: [u1, v0],
                            color: fg_color,
                        },
                        Vertex {
                            position: [gx_ndc + w_ndc, gy_ndc - h_ndc, 0.0],
                            tex_coords: [u1, v1],
                            color: fg_color,
                        },
                        Vertex {
                            position: [gx_ndc, gy_ndc - h_ndc, 0.0],
                            tex_coords: [u0, v1],
                            color: fg_color,
                        },
                    ]);

                    // Render underline if flag is set, or for a hyperlink
                    // cell (drawn once when both apply).
                    if cell.flags.contains(crate::terminal::CellFlags::UNDERLINE) || cell.link != 0
                    {
                        let underline_y_ndc = y_ndc - cell_h_ndc + (2.0 * height_ndc);
                        let underline_h_ndc = 2.0 * height_ndc;

                        vertices.extend_from_slice(&[
                            Vertex {
                                position: [x_ndc, underline_y_ndc, 0.0],
                                tex_coords: [bg_u0, bg_v0],
                                color: fg_color,
                            },
                            Vertex {
                                position: [x_ndc + cell_w_ndc, underline_y_ndc, 0.0],
                                tex_coords: [bg_u1, bg_v0],
                                color: fg_color,
                            },
                            Vertex {
                                position: [
                                    x_ndc + cell_w_ndc,
                                    underline_y_ndc - underline_h_ndc,
                                    0.0,
                                ],
                                tex_coords: [bg_u1, bg_v1],
                                color: fg_color,
                            },
                            Vertex {
                                position: [x_ndc, underline_y_ndc - underline_h_ndc, 0.0],
                                tex_coords: [bg_u0, bg_v1],
                                color: fg_color,
                            },
                        ]);
                    }
                }
            }
        }

        // No separate cursor block: the cursor cell is handled inline
        // above (cursor-colored background, glyph in background color),
        // so the character under the cursor is never hidden.
        vertices
    }

    fn build_indices(quad_count: usize) -> Vec<u32> {
        let mut indices = Vec::new();
        for i in 0..quad_count {
            let base = (i * 4) as u32;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_indices_empty_and_single_quad() {
        assert!(Renderer::build_indices(0).is_empty());
        assert_eq!(Renderer::build_indices(1), vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn build_indices_20000_quads_do_not_wrap_u32() {
        let indices = Renderer::build_indices(20_000);
        assert_eq!(indices.len(), 20_000 * 6);
        // First quad.
        assert_eq!(&indices[0..6], &[0, 1, 2, 0, 2, 3]);
        // Quad 16384 is where u16 indices would have wrapped (base 65536).
        assert_eq!(
            &indices[16_384 * 6..16_384 * 6 + 6],
            &[65536, 65537, 65538, 65536, 65538, 65539]
        );
        // Last quad (19999): base = 79996.
        assert_eq!(
            &indices[indices.len() - 6..],
            &[79996, 79997, 79998, 79996, 79998, 79999]
        );
    }
}
