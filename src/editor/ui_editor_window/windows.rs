//! UI Editor Windows Manager
//!
//! Multi-window management for UI editor

use egui::Context;
use std::path::{Path, PathBuf};

use skope_game_ui::{UiAsset, UiRenderer};

use super::window::UiEditorWindow;

/// Multi-window manager
pub struct UiEditorWindows {
    /// Open editors
    pub editors: Vec<UiEditorWindow>,
    /// Next window ID
    next_id: u32,
    /// Shared UiRenderer (optional - set on creation)
    ui_renderer: Option<UiRenderer>,
}

impl Default for UiEditorWindows {
    fn default() -> Self {
        Self {
            editors: Vec::new(),
            next_id: 1,
            ui_renderer: None,
        }
    }
}

impl UiEditorWindows {
    /// Initialize UiRenderer (called from State)
    pub fn init_renderer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if self.ui_renderer.is_none() {
            self.ui_renderer = Some(UiRenderer::new(device, queue, format, 1920, 1080));
            log::info!("[UiEditorWindows] UiRenderer initialized");
        }
    }

    /// Open editor by file path
    pub fn open(&mut self, path: PathBuf) {
        // Check if file is already open
        for editor in &mut self.editors {
            if editor.file_path.as_ref() == Some(&path) {
                editor.focus_requested = true;
                return;
            }
        }

        // Create new editor
        let id = self.next_id;
        self.next_id += 1;

        let editor = UiEditorWindow::from_file(id, path);
        self.editors.push(editor);
    }

    /// Create new UI editor (empty file)
    pub fn create_new(&mut self) {
        let id = self.next_id;
        self.next_id += 1;

        let editor = UiEditorWindow::new(id);
        self.editors.push(editor);
    }

    /// Create new UI file in directory and open
    pub fn create_new_in_dir(&mut self, dir: &Path) -> Option<PathBuf> {
        // Generate unique filename
        let mut counter = 1;
        let mut path;
        loop {
            let name = if counter == 1 {
                "new_ui.ui.ron".to_string()
            } else {
                format!("new_ui_{}.ui.ron", counter)
            };
            path = dir.join(&name);
            if !path.exists() {
                break;
            }
            counter += 1;
        }

        // Create and save default UiAsset
        let asset = UiAsset::new(path.file_stem().unwrap_or_default().to_string_lossy());
        if asset.save(&path).is_ok() {
            self.open(path.clone());
            Some(path)
        } else {
            log::error!("Failed to create UI file: {:?}", path);
            None
        }
    }

    /// Update viewport textures for all editors
    pub fn update_viewports(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
    ) {
        for editor in &mut self.editors {
            editor.ensure_viewport(device, egui_renderer, format);
        }
    }

    /// Render all editors' UI to viewports
    pub fn render_all(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // Skip if no UiRenderer
        let Some(ref mut ui_renderer) = self.ui_renderer else {
            return;
        };

        for editor in &mut self.editors {
            if !editor.open {
                continue;
            }

            // Skip if no viewport
            let Some(ref viewport) = editor.viewport else {
                continue;
            };

            // Calculate layout
            let resolution = editor.canvas_resolution.size();
            editor.canvas_ui_system.set_screen_size(resolution.0 as f32, resolution.1 as f32);
            editor.canvas_ui_system.calculate_layout();

            // Clear render target (background color)
            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("UI Editor Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: viewport.render_target(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.12,
                                g: 0.13,
                                b: 0.15,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            // Render UI
            if let Some(ref root) = editor.canvas_ui_system.root {
                // Set UiRenderer screen size
                ui_renderer.resize(queue, resolution.0, resolution.1);

                // Render widgets
                ui_renderer.render(
                    device,
                    encoder,
                    viewport.render_target(),
                    queue,
                    root,
                );
            }
        }
    }

    /// Render all windows UI (egui)
    pub fn show(&mut self, ctx: &Context) {
        // Remove closed windows
        self.editors.retain(|e| e.open);

        // Render each editor window
        for editor in &mut self.editors {
            editor.show(ctx);
        }
    }

    /// Number of open editors
    pub fn count(&self) -> usize {
        self.editors.len()
    }

    /// Close all editors
    pub fn close_all(&mut self) {
        self.editors.clear();
    }

    /// Check for unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.editors.iter().any(|e| e.dirty)
    }

    /// Check if any editor needs rendering
    pub fn needs_render(&self) -> bool {
        self.editors.iter().any(|e| e.open && e.viewport.is_some())
    }
}
