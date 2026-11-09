//! Native, initially blind A/B browsing. Inspection delegates to normal Studio.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use eframe::egui::{self, RichText};

use crate::{
    comparison::{self, PREVIEW_SIZE},
    explorer::Explorer,
    program::{ParameterBindings, StarfishProgram},
    render::{self, RenderRequest, RenderedImage},
    session::{self, Preference, Round, Session},
    variation::Snapshot,
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
    studio: Option<Explorer>,
    inspected_label: String,
    reviews: HashMap<usize, Round>,
    votes: Vec<Option<Preference>>,
    feedback_error: Option<String>,
}

impl Gallery {
    pub fn new(ctx: &egui::Context, directory: String, pairs: Vec<LoadedPair>) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        let pairs: Vec<Pair> = pairs
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
        let count = pairs.len();
        let mut gallery = Self {
            directory,
            pairs,
            studio: None,
            inspected_label: String::new(),
            reviews: HashMap::new(),
            votes: vec![None; count],
            feedback_error: None,
        };
        if gallery.feedback_root().exists() {
            if let Err(error) = gallery.restore_votes() {
                gallery.feedback_error = Some(error);
            }
        }
        gallery
    }

    fn feedback_root(&self) -> PathBuf {
        PathBuf::from(&self.directory).join("feedback")
    }

    fn snapshots(&self, index: usize) -> Result<[Snapshot; 2], String> {
        let candidates = &self.pairs[index].candidates;
        let capture = |slot: usize| {
            let program = &candidates[slot].program;
            Snapshot::capture(
                program,
                &ParameterBindings::defaults(program),
                &program.output,
            )
        };
        Ok([capture(0)?, capture(1)?])
    }

    fn restore_votes(&mut self) -> Result<(), String> {
        let store = Session::open(&self.feedback_root())?;
        let rounds = store.rounds()?;
        let feedback = store.feedback()?;
        for index in 0..self.pairs.len() {
            let context =
                session::review_context(self.pairs[index].number, &self.snapshots(index)?);
            if let Some(round) = rounds.iter().find(|round| round.context == context) {
                self.votes[index] = feedback.choices.get(&(round.id.clone(), 0)).copied();
                self.reviews.insert(index, round.clone());
            }
        }
        Ok(())
    }

    fn record_vote(&mut self, index: usize, preference: Preference) -> Result<(), String> {
        let store = Session::open(&self.feedback_root())?;
        let snapshots = self.snapshots(index)?;
        let context = session::review_context(self.pairs[index].number, &snapshots);
        let round = store.review(&context, &snapshots)?;
        store.choose(&round, 0, preference)?;
        self.reviews.insert(index, round);
        self.votes[index] = Some(preference);
        Ok(())
    }

    fn show_pairs(&mut self, ctx: &egui::Context) {
        let mut inspect = None;
        let mut vote = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("COMPARISON STUDY");
            ui.weak(format!("{} pairs · {}", self.pairs.len(), self.directory));
            ui.label("A/B order is shuffled. Compare the images before inspecting their programs.");
            ui.weak("Look for recurring motifs, relationships across the image, and structure at several scales.");
            ui.label("Choose A, B, both, or neither. Your judgments are saved with the programs.");
            if let Some(error) = &self.feedback_error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
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
                    ui.horizontal(|ui| {
                        ui.label("Prefer:");
                        for choice in Preference::ALL {
                            if ui.selectable_label(self.votes[pair_index] == Some(choice), choice.label()).clicked() {
                                vote = Some((pair_index, choice));
                            }
                        }
                    });
                    ui.add_space(16.0);
                }
            });
        });
        if let Some((index, choice)) = vote {
            self.feedback_error = self.record_vote(index, choice).err();
        }
        if let Some((pair_index, slot)) = inspect {
            let pair = &self.pairs[pair_index];
            let candidate = &pair.candidates[slot];
            self.inspected_label = format!("Pair {:02} · {}", pair.number, ["A", "B"][slot]);
            let mut explorer = Explorer::new(
                ctx,
                candidate.program.clone(),
                candidate.path.display().to_string(),
                self.feedback_root(),
            );
            if let Some(round) = self.reviews.get(&pair_index) {
                explorer.attach(round.pairs[0][slot].clone());
            }
            self.studio = Some(explorer);
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
    fn gallery_choices_persist_across_reopening_and_capture_the_judged_programs() {
        let root = std::env::temp_dir().join(format!(
            "starfish-gallery-feedback-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let loaded = || {
            let pair = crate::generate::comparison_pair(42, 0);
            vec![LoadedPair {
                number: 1,
                candidates: [pair.a, pair.b].map(|plan| {
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
                        path: root.join(format!("{}.sf", plan.condition.name())),
                        program,
                        image,
                    }
                }),
            }]
        };
        let ctx = egui::Context::default();
        let mut gallery = Gallery::new(&ctx, root.display().to_string(), loaded());
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1180.0, 1000.0),
            )),
            ..Default::default()
        };
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| gallery.show_pairs(ctx));
        }
        let output = ctx.run(input.clone(), |ctx| gallery.show_pairs(ctx));
        let rect = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Both" => {
                    Some(shape.shape.visual_bounding_rect())
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            let mut frame = input.clone();
            frame.events = vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let _ = ctx.run(frame, |ctx| gallery.show_pairs(ctx));
        }
        assert!(
            gallery.feedback_error.is_none(),
            "{:?}",
            gallery.feedback_error
        );
        assert_eq!(gallery.votes, vec![Some(Preference::Both)]);
        let reopened = Gallery::new(&ctx, root.display().to_string(), loaded());
        assert_eq!(reopened.votes, vec![Some(Preference::Both)]);
        let store = Session::open(&root.join("feedback")).unwrap();
        let review = &reopened.reviews[&0];
        assert_eq!(
            store.snapshot(&review.pairs[0][0]).unwrap(),
            reopened.snapshots(0).unwrap()[0]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

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
