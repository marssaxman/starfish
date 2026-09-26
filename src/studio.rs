use crate::{
    program::{ParameterBindings, StarfishProgram, ValueType},
    render::{self, RenderRequest},
};
use eframe::egui::{self, Color32, RichText};
use std::time::Duration;

const PREVIEW_SIZE: usize = 448;

pub struct Studio {
    program: StarfishProgram,
    bindings: ParameterBindings,
    selected: &'static str,
    texture: Option<egui::TextureHandle>,
    render_duration: Duration,
    dirty: bool,
    error: Option<String>,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.visuals.selection.bg_fill = Color32::from_rgb(40, 87, 93);
            style.visuals.selection.stroke.color = Color32::from_rgb(211, 240, 230);
        });
        let program = StarfishProgram::sample();
        let bindings = ParameterBindings::defaults(&program);
        let selected = program.output;
        Self {
            program,
            bindings,
            selected,
            texture: None,
            render_duration: Duration::ZERO,
            dirty: true,
            error: None,
        }
    }

    fn output_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("program")
            .default_width(200.0)
            .width_range(170.0..=260.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("PROGRAM");
                ui.weak("Select a computation to preview.");
                ui.add_space(12.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 46.0],
                        egui::Button::new("Final Image\nPalette · Color")
                            .selected(self.selected == self.program.output),
                    )
                    .clicked()
                {
                    self.selected = self.program.output;
                    self.dirty = true;
                }
                ui.add_space(10.0);
                ui.label(RichText::new("INTERMEDIATE OUTPUTS").small().weak());
                for node in &self.program.nodes {
                    if node.value_type != ValueType::Scalar {
                        continue;
                    }
                    if ui
                        .add_sized(
                            [ui.available_width(), 44.0],
                            egui::Button::new(format!("{}\n{}", node.name, node.operation.name()))
                                .selected(self.selected == node.id),
                        )
                        .clicked()
                    {
                        self.selected = node.id;
                        self.dirty = true;
                    }
                }
                ui.add_space(16.0);
                ui.separator();
                ui.label(RichText::new("SHARED COORDINATES").small().weak());
                ui.monospace("(x, y) → rotate → (x′, y′)");
                ui.weak("Pattern A, Pattern B, and Radial all use this frame.");
                ui.add_space(8.0);
                ui.weak("Scalar outputs: −1 is black, +1 is white.");
            });
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .default_width(304.0)
            .width_range(280.0..=380.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("INSPECTOR");
                    let node = self
                        .program
                        .node(self.selected)
                        .expect("selection comes from the program");
                    ui.label(RichText::new(node.name).size(18.0));
                    egui::Grid::new("node_metadata")
                        .spacing([12.0, 5.0])
                        .show(ui, |ui| {
                            ui.weak("ID");
                            ui.monospace(node.id);
                            ui.end_row();
                            ui.weak("Operation");
                            ui.label(node.operation.name());
                            ui.end_row();
                            ui.weak("Type");
                            ui.label(node.value_type.name());
                            ui.end_row();
                            ui.weak("Inputs");
                            ui.label(
                                node.inputs
                                    .iter()
                                    .map(|&index| self.program.nodes[index].name)
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            );
                            ui.end_row();
                        });
                    ui.add_space(6.0);
                    ui.label(node.description);
                    ui.add_space(4.0);
                    ui.label(RichText::new(node.expression).monospace().size(11.0));
                    ui.add_space(8.0);
                    ui.separator();
                    ui.heading("PARAMETERS");
                    ui.weak("Shared across the whole program.");
                    ui.spacing_mut().slider_width = (ui.available_width() - 84.0).max(100.0);
                    for parameter in &self.program.parameters {
                        ui.push_id(parameter.id as usize, |ui| {
                            ui.label(parameter.name)
                                .on_hover_text(parameter.description);
                            self.dirty |= ui
                                .add(
                                    egui::Slider::new(
                                        self.bindings.get_mut(parameter.id),
                                        parameter.min..=parameter.max,
                                    )
                                    .step_by(parameter.step)
                                    .suffix(parameter.suffix)
                                    .max_decimals(2),
                                )
                                .changed();
                        });
                    }
                    if ui.button("Reset parameters").clicked() {
                        self.bindings = ParameterBindings::defaults(&self.program);
                        self.dirty = true;
                    }
                });
            });
    }

    fn rerender(&mut self, ctx: &egui::Context) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        match render::render(
            &self.program,
            &self.bindings,
            RenderRequest {
                output: self.selected,
                width: PREVIEW_SIZE,
                height: PREVIEW_SIZE,
            },
        ) {
            Ok(image) => {
                self.render_duration = image.duration;
                self.error = None;
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width, image.height],
                    &image.rgba,
                );
                if let Some(texture) = &mut self.texture {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.texture =
                        Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR));
                }
            }
            Err(error) => {
                self.texture = None;
                self.error = Some(error);
            }
        }
    }

    fn preview(&self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading(self.program.name);
            let node = self
                .program
                .node(self.selected)
                .expect("selection comes from the program");
            let name = if self.selected == self.program.output {
                "Final Image · Palette"
            } else {
                node.name
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).strong());
                ui.weak(format!(
                    "{} · {PREVIEW_SIZE} × {PREVIEW_SIZE}",
                    node.value_type.name()
                ));
            });
            ui.separator();

            let available = ui.available_size();
            let area_height = (available.y - 52.0).max(1.0);
            let size = available.x.min(area_height).max(1.0);
            let (area, _) =
                ui.allocate_exact_size(egui::vec2(available.x, area_height), egui::Sense::hover());
            let image_rect = egui::Rect::from_center_size(area.center(), egui::vec2(size, size));
            if let Some(texture) = &self.texture {
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            if let Some(error) = &self.error {
                ui.colored_label(Color32::LIGHT_RED, error);
            }
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "render: {:.1} ms  ·  nodes: {}\nselected: {}",
                    self.render_duration.as_secs_f64() * 1000.0,
                    self.program.nodes.len(),
                    self.selected,
                ))
                .monospace()
                .small()
                .weak(),
            );
        });
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.output_list(ctx);
        self.inspector(ctx);
        // Apply this frame's controls before drawing the preview and diagnostics.
        self.rerender(ctx);
        self.preview(ctx);
    }
}
