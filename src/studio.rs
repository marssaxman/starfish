use std::{fs, time::Duration};

use crate::{
    evaluate::{CurveGuide, curve_guides},
    program::{DefinitionKind, ParameterBindings, StarfishProgram, Type, display_name},
    render::{self, RenderRequest},
    source::Decl,
    variation::Snapshot,
};
use eframe::egui::{self, Color32, RichText};

const PREVIEW_SIZE: usize = 448;
const DRAG_PREVIEW_SIZE: usize = 224;

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
    guides: Vec<CurveGuide>,
    show_curves: bool,
    active_curve: usize,
    render_size: usize,
}

impl Studio {
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        Snapshot::capture(&self.program, &self.bindings, &self.selected)
    }

    pub fn source(&self) -> &str {
        &self.program.source
    }

    pub fn select_output(&mut self, output: &str) -> Result<(), String> {
        crate::evaluate::Evaluator::new(&self.program, &self.bindings, output)?;
        self.selected = output.to_owned();
        self.inspected = output.to_owned();
        self.dirty = true;
        Ok(())
    }

    pub fn new(ctx: &egui::Context, program: StarfishProgram, path: String) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        ctx.style_mut(|style| {
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
            guides: Vec::new(),
            show_curves: false,
            active_curve: 0,
            render_size: PREVIEW_SIZE,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.output_list(ctx);
        self.inspector(ctx);
        self.rerender(ctx);
        self.preview(ctx);
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
                self.active_curve = 0;
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
                        if ui.small_button("Curve textures").clicked() {
                            self.open("programs/curve_textures.sf");
                        }
                        if ui.small_button("Bézier study").clicked() {
                            self.open("programs/bezier_study.sf");
                        }
                        if ui.small_button("Wave lab").clicked() {
                            self.open("programs/wave_lab.sf");
                        }
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
                                        .clamping(egui::SliderClamping::Edits)
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
                                        .clamping(egui::SliderClamping::Edits)
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
        let dragging = ctx.input(|input| input.pointer.any_down());
        if !self.dirty && (dragging || self.render_size == PREVIEW_SIZE) {
            return;
        }
        self.dirty = false;
        self.render_size = if dragging {
            DRAG_PREVIEW_SIZE
        } else {
            PREVIEW_SIZE
        };
        match render::render(
            &self.program,
            &self.bindings,
            RenderRequest {
                output: &self.selected,
                width: self.render_size,
                height: self.render_size,
            },
        ) {
            Ok(image) => {
                self.guides =
                    curve_guides(&self.program, &self.bindings, &self.selected).unwrap_or_default();
                self.active_curve = self.active_curve.min(self.guides.len().saturating_sub(1));
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
                self.guides.clear();
                self.texture = None;
                self.render_error = Some(error);
            }
        }
    }

    fn preview(&mut self, ctx: &egui::Context) {
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
                    "{} · {} × {}",
                    item.ty.name(), self.render_size, self.render_size
                ));
            });
            if !self.guides.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.show_curves, "Curve guides");
                    if self.show_curves {
                        egui::ComboBox::from_id_salt("active_curve")
                            .selected_text(&self.guides[self.active_curve].name)
                            .show_ui(ui, |ui| {
                                for (index, guide) in self.guides.iter().enumerate() {
                                    ui.selectable_value(&mut self.active_curve, index, &guide.name);
                                }
                            });
                        ui.weak("Drag the selected curve’s points");
                    }
                });
            }
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
            if self.show_curves {
                self.curve_overlay(ui, image_rect);
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

    fn curve_overlay(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter().with_clip_rect(rect);
        let to_screen = |[x, y]: [f32; 2]| {
            egui::pos2(
                rect.left() + (x + 1.0) * rect.width() * 0.5,
                rect.top() + (1.0 - y) * rect.height() * 0.5,
            )
        };
        for (index, guide) in self.guides.iter().enumerate() {
            let points = guide.points.map(to_screen);
            let active = index == self.active_curve;
            let stroke = egui::Stroke::new(
                if active { 2.0 } else { 1.0 },
                if active {
                    Color32::from_rgb(255, 223, 130)
                } else {
                    Color32::from_white_alpha(110)
                },
            );
            // A dark underlay keeps the curve legible over both light and dark textures.
            for stroke in [
                egui::Stroke::new(stroke.width + 2.0, Color32::from_black_alpha(180)),
                stroke,
            ] {
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    points,
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
            }
            if !active {
                continue;
            }
            painter.line_segment(
                [points[0], points[1]],
                egui::Stroke::new(1.0, Color32::WHITE),
            );
            painter.line_segment(
                [points[2], points[3]],
                egui::Stroke::new(1.0, Color32::WHITE),
            );
            for (slot, point) in points.iter().enumerate() {
                let editable = guide.editable(slot);
                let hit =
                    egui::Rect::from_center_size(*point, egui::vec2(18.0, 18.0)).intersect(rect);
                if hit.is_positive() {
                    let response = ui.interact(
                        hit,
                        ui.id().with(("curve_handle", guide.node, slot)),
                        if editable {
                            egui::Sense::drag()
                        } else {
                            egui::Sense::hover()
                        },
                    );
                    let response = response.on_hover_text(if editable {
                        "Drag to change this control point. Reset parameters restores the source defaults."
                    } else { "This point is computed; adjust its parameters in the inspector." });
                    if response.dragged() || response.drag_stopped() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let world = [
                                2.0 * (pos.x - rect.left()) / rect.width() - 1.0,
                                1.0 - 2.0 * (pos.y - rect.top()) / rect.height(),
                            ];
                            if guide.move_point(slot, world, &self.program, &mut self.bindings) {
                                self.dirty = true;
                                ui.ctx().request_repaint();
                            }
                        }
                    }
                }
                painter.circle(
                    *point,
                    if slot == 0 || slot == 3 { 5.0 } else { 4.0 },
                    if editable {
                        Color32::from_rgb(255, 223, 130)
                    } else {
                        Color32::GRAY
                    },
                    egui::Stroke::new(1.5, Color32::BLACK),
                );
                painter.text(
                    *point + egui::vec2(8.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    ["Start", "Handle A", "Handle B", "End"][slot],
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
        }
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_a_curve_handle_updates_bindings_and_the_preview() {
        let ctx = egui::Context::default();
        let source = include_str!("../programs/bezier_study.sf");
        let program = StarfishProgram::from_source(source).unwrap();
        let mut studio = Studio::new(&ctx, program, "bezier_study.sf".into());
        studio.show_curves = true;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 950.0),
            )),
            ..Default::default()
        };
        // Let egui settle panel widths and text layout before locating the image.
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| studio.show(ctx));
        }
        let mut output = ctx.run(input.clone(), |ctx| studio.show(ctx));
        let texture = studio.texture.as_ref().unwrap().id();
        let rect = output
            .shapes
            .iter()
            .find(|shape| shape.shape.texture_id() == texture)
            .unwrap()
            .shape
            .visual_bounding_rect();
        let point = studio.guides[0].points[0];
        let start = egui::pos2(
            rect.left() + (point[0] + 1.0) * rect.width() * 0.5,
            rect.top() + (1.0 - point[1]) * rect.height() * 0.5,
        );
        let target = start + egui::vec2(30.0, -20.0);
        let original = studio.bindings.0.clone();
        let mut saw_drag_preview = false;
        for events in [
            vec![egui::Event::PointerMoved(start)],
            vec![egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![egui::Event::PointerMoved(target)],
            vec![],
            vec![egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![],
            vec![],
        ] {
            let mut frame = input.clone();
            frame.events = events;
            output = ctx.run(frame, |ctx| studio.show(ctx));
            saw_drag_preview |= studio.render_size == DRAG_PREVIEW_SIZE;
        }
        assert!(saw_drag_preview);
        assert_eq!(studio.render_size, PREVIEW_SIZE);
        assert_ne!(studio.bindings.0, original);
        assert_eq!(&studio.bindings.0[2..], &original[2..]);
        assert_eq!(studio.program.source, source);
        assert_eq!(studio.selected, studio.program.output);
        assert!(studio.render_error.is_none());
        assert!(!studio.dirty);
        let moved = studio.guides[0].points[0];
        let rect = output
            .shapes
            .iter()
            .find(|shape| shape.shape.texture_id() == texture)
            .unwrap()
            .shape
            .visual_bounding_rect();
        let actual = egui::pos2(
            rect.left() + (moved[0] + 1.0) * rect.width() * 0.5,
            rect.top() + (1.0 - moved[1]) * rect.height() * 0.5,
        );
        // Allow the image's pixel rounding and small layout changes between frames.
        assert!(actual.distance(target) < 2.0, "{actual:?} != {target:?}");
    }
}
