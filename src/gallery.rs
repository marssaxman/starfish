//! Native, initially blind A/B browsing. Inspection delegates to normal Studio.

use std::{
    fs,
    path::{Path, PathBuf},
};

use eframe::egui::{self, RichText};

use crate::{
    comparison::{self, PREVIEW_SIZE},
    program::{ParameterBindings, StarfishProgram},
    render::{self, RenderRequest, RenderedImage},
    studio::Studio,
};

pub struct LoadedCandidate {
    path: PathBuf,
    program: StarfishProgram,
    image: RenderedImage,
}

pub struct LoadedPair {
    number: usize,
    candidates: [LoadedCandidate; 2],
}

/// This runs without a window, so gallery loading is also verifiable headlessly.
pub fn load(directory: &Path) -> Result<Vec<LoadedPair>, String> {
    comparison::load_index(directory)?
        .into_iter()
        .map(|pair| {
            let load_candidate = |path: PathBuf| {
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
                let program = StarfishProgram::from_source(&source)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let image = render::render(
                    &program,
                    &ParameterBindings::defaults(&program),
                    RenderRequest {
                        output: &program.output,
                        width: PREVIEW_SIZE,
                        height: PREVIEW_SIZE,
                    },
                )?;
                Ok::<_, String>(LoadedCandidate {
                    path,
                    program,
                    image,
                })
            };
            Ok(LoadedPair {
                number: pair.number,
                candidates: [load_candidate(pair.a)?, load_candidate(pair.b)?],
            })
        })
        .collect()
}

struct Candidate {
    path: PathBuf,
    program: StarfishProgram,
    texture: egui::TextureHandle,
}

struct Pair {
    number: usize,
    candidates: [Candidate; 2],
}

pub struct Gallery {
    directory: String,
    pairs: Vec<Pair>,
    studio: Option<Studio>,
    inspected_label: String,
}

impl Gallery {
    pub fn new(ctx: &egui::Context, directory: String, pairs: Vec<LoadedPair>) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        let pairs = pairs
            .into_iter()
            .map(|pair| Pair {
                number: pair.number,
                candidates: pair.candidates.map(|candidate| Candidate {
                    texture: ctx.load_texture(
                        candidate.path.display().to_string(),
                        egui::ColorImage::from_rgba_unmultiplied(
                            [candidate.image.width, candidate.image.height],
                            &candidate.image.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    ),
                    path: candidate.path,
                    program: candidate.program,
                }),
            })
            .collect();
        Self {
            directory,
            pairs,
            studio: None,
            inspected_label: String::new(),
        }
    }

    fn show_pairs(&mut self, ctx: &egui::Context) {
        let mut inspect = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("COMPARISON STUDY");
            ui.weak(format!("{} pairs · {}", self.pairs.len(), self.directory));
            ui.label("A/B order is shuffled. Compare the images before inspecting their programs.");
            ui.weak("Look for recurring motifs, relationships across the image, and structure at several scales.");
            ui.separator();
            egui::ScrollArea::vertical().id_salt("comparison_pairs").show(ui, |ui| {
                for (pair_index, pair) in self.pairs.iter().enumerate() {
                    ui.label(RichText::new(format!("PAIR {:02}", pair.number)).strong());
                    ui.columns(2, |columns| {
                        for (slot, candidate) in pair.candidates.iter().enumerate() {
                            egui::Frame::group(columns[slot].style()).show(&mut columns[slot], |ui| {
                                ui.vertical_centered(|ui| {
                                    let label = ["A", "B"][slot];
                                    ui.label(RichText::new(label).size(18.0).strong());
                                    let side = ui.available_width().min(360.0);
                                    let clicked_image = ui.add(egui::Image::new((candidate.texture.id(), egui::vec2(side, side)))
                                        .sense(egui::Sense::click())).clicked();
                                    if ui.button(format!("Inspect {label}")).clicked() || clicked_image {
                                        inspect = Some((pair_index, slot));
                                    }
                                });
                            });
                        }
                    });
                    ui.add_space(16.0);
                }
            });
        });
        if let Some((pair_index, slot)) = inspect {
            let pair = &self.pairs[pair_index];
            let candidate = &pair.candidates[slot];
            self.inspected_label = format!("Pair {:02} · {}", pair.number, ["A", "B"][slot]);
            self.studio = Some(Studio::new(
                ctx,
                candidate.program.clone(),
                candidate.path.display().to_string(),
            ));
        }
    }
}

impl eframe::App for Gallery {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.studio.is_some() {
            let mut back = false;
            egui::TopBottomPanel::top("gallery_navigation").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    back = ui.button("← Back to pairs").clicked();
                    ui.label(&self.inspected_label);
                });
            });
            if back {
                self.studio = None;
            }
        }
        if let Some(studio) = &mut self.studio {
            studio.show(ctx);
        } else {
            self.show_pairs(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_a_gallery_image_opens_a_renderable_studio() {
        let pair = crate::generate::comparison_pair(42, 0);
        let candidates = [pair.a, pair.b].map(|plan| {
            let program = StarfishProgram::from_source(&plan.source()).unwrap();
            let image = render::render(
                &program,
                &ParameterBindings::defaults(&program),
                RenderRequest {
                    output: &program.output,
                    width: 32,
                    height: 32,
                },
            )
            .unwrap();
            LoadedCandidate {
                path: PathBuf::from(format!("{}.sf", plan.condition.name())),
                program,
                image,
            }
        });
        let ctx = egui::Context::default();
        let mut gallery = Gallery::new(
            &ctx,
            "test study".into(),
            vec![LoadedPair {
                number: 1,
                candidates,
            }],
        );
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1180.0, 800.0),
            )),
            ..Default::default()
        };
        let output = ctx.run(input.clone(), |ctx| gallery.show_pairs(ctx));
        let texture = gallery.pairs[0].candidates[1].texture.id();
        let rectangle = output
            .shapes
            .iter()
            .find(|shape| shape.shape.texture_id() == texture)
            .map(|shape| shape.shape.visual_bounding_rect())
            .expect("B thumbnail is displayed");
        for pressed in [true, false] {
            let mut click = input.clone();
            click.events = vec![
                egui::Event::PointerMoved(rectangle.center()),
                egui::Event::PointerButton {
                    pos: rectangle.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let _ = ctx.run(click, |ctx| gallery.show_pairs(ctx));
        }
        assert_eq!(gallery.inspected_label, "Pair 01 · B");
        let studio = gallery
            .studio
            .as_mut()
            .expect("clicked thumbnail opens Studio");
        let output = ctx.run(input, |ctx| studio.show(ctx));
        assert!(
            output
                .shapes
                .iter()
                .any(|shape| shape.shape.texture_id() != egui::TextureId::default())
        );
    }
}
