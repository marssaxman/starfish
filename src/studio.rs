use std::{fs, time::Duration};

use crate::{
    program::{DefinitionKind, ParameterBindings, StarfishProgram, Type, display_name},
    render::{self, RenderRequest},
    source::Decl,
};
use eframe::egui::{self, Color32, RichText};

const PREVIEW_SIZE: usize = 448;

pub struct Studio {
    program: StarfishProgram,
    bindings: ParameterBindings,
    selected: String,
    inspected: String,
    texture: Option<egui::TextureHandle>,
    render_duration: Duration,
    dirty: bool,
    render_error: Option<String>,
    load_error: Option<String>,
    path: String,
    path_input: String,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>, program: StarfishProgram, path: String) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.visuals.selection.bg_fill = Color32::from_rgb(40, 87, 93);
            style.visuals.selection.stroke.color = Color32::from_rgb(211, 240, 230);
        });
        let bindings = ParameterBindings::defaults(&program);
        let selected = program.output.clone();
        let inspected = selected.clone();
        Self {
            program,
            bindings,
            selected,
            inspected,
            texture: None,
            render_duration: Duration::ZERO,
            dirty: true,
            render_error: None,
            load_error: None,
            path: path.clone(),
            path_input: path,
        }
    }

    fn open(&mut self, path: &str) {
        match fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {path}: {e}"))
            .and_then(|source| StarfishProgram::from_source(&source))
        {
            Ok(program) => {
                self.selected = program.output.clone();
                self.inspected = self.selected.clone();
                self.bindings = ParameterBindings::defaults(&program);
                self.program = program;
                self.path = path.to_owned();
                self.path_input = path.to_owned();
                self.texture = None;
                self.dirty = true;
                self.load_error = None;
            }
            Err(error) => self.load_error = Some(error),
        }
    }

    fn output_list(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("program")
            .default_width(210.0)
            .width_range(185.0..=280.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("PROGRAM");
                    ui.weak(&self.program.name);
                    ui.monospace(&self.program.id);
                    ui.add_space(8.0);
                    ui.label(RichText::new("OPEN SOURCE FILE").small().weak());
                    let enter = ui.text_edit_singleline(&mut self.path_input).lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Open").clicked() || enter {
                        let path = self.path_input.clone();
                        self.open(&path);
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("Shared tides").clicked() {
                            self.open("programs/shared_tides.sf");
                        }
                        if ui.small_button("Crosscurrent").clicked() {
                            self.open("programs/crosscurrent.sf");
                        }
                    });
                    if let Some(error) = &self.load_error {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    if let Some(final_item) = self.program.item(&self.program.output) {
                        if ui
                            .add_sized(
                                [ui.available_width(), 45.0],
                                egui::Button::new(format!("Final Image\n{}", final_item.name))
                                    .selected(self.selected == self.program.output),
                            )
                            .clicked()
                        {
                            self.selected = self.program.output.clone();
                            self.inspected = self.selected.clone();
                            self.dirty = true;
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("INTERMEDIATE FIELDS").small().weak());
                    for item in &self.program.items {
                        if item.kind != DefinitionKind::Field {
                            continue;
                        }
                        if ui
                            .add_sized(
                                [ui.available_width(), 35.0],
                                egui::Button::new(&item.name).selected(self.selected == item.id),
                            )
                            .clicked()
                        {
                            self.selected = item.id.clone();
                            self.inspected = self.selected.clone();
                            self.dirty = true;
                        }
                    }
                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(RichText::new("STRUCTURE").small().weak());
                    for item in &self.program.items {
                        if matches!(item.kind, DefinitionKind::Frame | DefinitionKind::Axis) {
                            if ui
                                .selectable_label(
                                    self.inspected == item.id,
                                    format!("{} · {}", item.name, item.kind.name()),
                                )
                                .clicked()
                            {
                                self.inspected = item.id.clone();
                            }
                        }
                    }
                    for decl in &self.program.hir {
                        if let Decl::Component(component) = decl {
                            if ui
                                .selectable_label(
                                    self.inspected == component.id,
                                    format!("{} · Component", display_name(&component.name)),
                                )
                                .clicked()
                            {
                                self.inspected = component.id.clone();
                            }
                        }
                    }
                    ui.add_space(8.0);
                    ui.weak("Scalar fields: −1 is black, +1 is white.");
                });
            });
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .default_width(330.0)
            .width_range(290.0..=440.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("INSPECTOR");
                    if let Some(item) = self.program.item(&self.inspected) {
                        ui.label(RichText::new(&item.name).size(18.0));
                        egui::Grid::new("item_metadata")
                            .spacing([12.0, 5.0])
                            .show(ui, |ui| {
                                ui.weak("ID");
                                ui.monospace(&item.id);
                                ui.end_row();
                                ui.weak("Symbol");
                                ui.monospace(&item.symbol);
                                ui.end_row();
                                ui.weak("Kind");
                                ui.label(item.kind.name());
                                ui.end_row();
                                ui.weak("Type");
                                ui.label(item.ty.name());
                                ui.end_row();
                                ui.weak("Uses");
                                ui.label(if item.dependencies.is_empty() {
                                    "none".to_owned()
                                } else {
                                    item.dependencies.join(", ")
                                });
                                ui.end_row();
                            });
                        ui.add_space(6.0);
                        ui.label(RichText::new(&item.expression).monospace().size(11.0));
                    } else if let Some(Decl::Component(component)) = self
                        .program
                        .hir
                        .iter()
                        .find(|decl| decl.id() == self.inspected)
                    {
                        ui.label(RichText::new(display_name(&component.name)).size(18.0));
                        egui::Grid::new("component_metadata")
                            .spacing([12.0, 5.0])
                            .show(ui, |ui| {
                                ui.weak("ID");
                                ui.monospace(&component.id);
                                ui.end_row();
                                ui.weak("Kind");
                                ui.label("Component");
                                ui.end_row();
                                ui.weak("Returns");
                                ui.label(component.return_type.name());
                                ui.end_row();
                                ui.weak("Inputs");
                                ui.label(
                                    component
                                        .params
                                        .iter()
                                        .map(|p| format!("{}: {}", p.name, p.ty.name()))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                );
                                ui.end_row();
                            });
                        ui.add_space(6.0);
                        ui.label(RichText::new(&component.expression).monospace().size(11.0));
                        ui.weak("Preview remains on the last selected field.");
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    ui.heading("PARAMETERS");
                    ui.weak("Bindings for this preview; source defaults stay intact.");
                    ui.spacing_mut().slider_width = (ui.available_width() - 84.0).max(100.0);
                    for (index, parameter) in self.program.parameters.iter().enumerate() {
                        ui.push_id(&parameter.id, |ui| {
                            ui.label(&parameter.name);
                            if parameter.ty == Type::Angle {
                                let mut degrees = self.bindings.0[index].to_degrees();
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut degrees,
                                            parameter.min.to_degrees()..=parameter.max.to_degrees(),
                                        )
                                        .step_by(parameter.step.to_degrees() as f64)
                                        .suffix("°")
                                        .max_decimals(2),
                                    )
                                    .changed()
                                {
                                    self.bindings.0[index] =
                                        degrees.to_radians().clamp(parameter.min, parameter.max);
                                    self.dirty = true;
                                }
                            } else {
                                self.dirty |= ui
                                    .add(
                                        egui::Slider::new(
                                            &mut self.bindings.0[index],
                                            parameter.min..=parameter.max,
                                        )
                                        .step_by(parameter.step as f64)
                                        .max_decimals(2),
                                    )
                                    .changed();
                            }
                        });
                    }
                    if ui.button("Reset parameters").clicked() {
                        self.bindings = ParameterBindings::defaults(&self.program);
                        self.dirty = true;
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    egui::CollapsingHeader::new("SOURCE")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.weak(&self.path);
                            egui::ScrollArea::vertical()
                                .max_height(190.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&self.program.source)
                                                .monospace()
                                                .size(10.0),
                                        )
                                        .wrap(),
                                    );
                                });
                        });
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
                output: &self.selected,
                width: PREVIEW_SIZE,
                height: PREVIEW_SIZE,
            },
        ) {
            Ok(image) => {
                self.render_duration = image.duration;
                self.render_error = None;
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
                self.render_error = Some(error);
            }
        }
    }

    fn preview(&self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading(&self.program.name);
            let item = self
                .program
                .item(&self.selected)
                .expect("selection comes from the program");
            ui.horizontal(|ui| {
                ui.label(RichText::new(&item.name).strong());
                ui.weak(format!(
                    "{} · {PREVIEW_SIZE} × {PREVIEW_SIZE}",
                    item.ty.name()
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
            if let Some(error) = &self.render_error {
                ui.colored_label(Color32::LIGHT_RED, error);
            }
            ui.separator();
            ui.label(
                RichText::new(format!(
                "render: {:.1} ms  ·  declarations: {}  ·  components: {}  ·  nodes: {}\nselected: {}",
                self.render_duration.as_secs_f64() * 1000.0,
                self.program.hir.len(),
                self.program.components.len(),
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
        self.rerender(ctx);
        self.preview(ctx);
    }
}
