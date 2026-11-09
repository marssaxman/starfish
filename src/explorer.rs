//! Human-guided exploration. The UI requests snapshots, variations, and durable votes;
//! generation and disk operations belong to the session and variation modules.

use std::{path::PathBuf, sync::mpsc, time::Duration};

use eframe::egui::{self, Color32, RichText};

use crate::{
    program::StarfishProgram,
    session::{Candidate, Feedback, Preference, Round, Session},
    studio::Studio,
    variation::Settings,
};

pub const DEFAULT_SESSION: &str = "explorations/default";

struct Card {
    id: String,
    texture: egui::TextureHandle,
}

struct Comparison {
    record: Round,
    cards: Vec<[Card; 2]>,
}

pub struct Explorer {
    studio: Studio,
    root: PathBuf,
    current: Option<String>,
    loaded_source: String,
    candidates: Vec<Candidate>,
    rounds: Vec<Round>,
    feedback: Feedback,
    comparison: Option<Comparison>,
    show_comparison: bool,
    show_history: bool,
    kept_only: bool,
    settings: Settings,
    seed: u64,
    count: usize,
    worker: Option<mpsc::Receiver<Result<Round, String>>>,
    message: Option<String>,
    error: Option<String>,
}

impl Explorer {
    pub fn attach(&mut self, id: String) {
        self.current = Some(id);
    }

    pub fn new(ctx: &egui::Context, program: StarfishProgram, path: String, root: PathBuf) -> Self {
        let loaded_source = program.source.clone();
        let mut explorer = Self {
            studio: Studio::new(ctx, program, path),
            root,
            current: None,
            loaded_source,
            candidates: Vec::new(),
            rounds: Vec::new(),
            feedback: Feedback::default(),
            comparison: None,
            show_comparison: false,
            show_history: false,
            kept_only: false,
            settings: Settings::default(),
            seed: 42,
            count: 6,
            worker: None,
            message: None,
            error: None,
        };
        if explorer.root.exists() {
            if let Err(error) = explorer.refresh() {
                explorer.error = Some(error);
            }
        }
        explorer
    }

    pub fn resume(
        ctx: &egui::Context,
        program: StarfishProgram,
        path: String,
        root: PathBuf,
    ) -> Self {
        let mut explorer = Self::new(ctx, program, path, root);
        let last = explorer
            .feedback
            .last
            .clone()
            .or_else(|| explorer.candidates.first().map(|c| c.id.clone()));
        if let Some(id) = last {
            if let Err(error) = explorer.open_candidate(ctx, &id, false) {
                explorer.error = Some(error);
            }
        }
        if let Some(id) = explorer.feedback.last_round.clone() {
            let result = explorer
                .store()
                .and_then(|store| store.round(&id))
                .and_then(|round| explorer.open_round(ctx, round, false));
            if let Err(error) = result {
                explorer.error = Some(error);
            }
        }
        explorer
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.poll(ctx);
        let mut keep = false;
        let mut vary = false;
        let mut picks = false;
        let mut regenerate = false;
        let mut back = false;
        egui::TopBottomPanel::top("exploration").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.comparison.is_some() && !self.show_comparison {
                    back = ui.button("← Back to comparisons").clicked();
                }
                keep = ui
                    .add_enabled(
                        self.worker.is_none() && !self.show_comparison,
                        egui::Button::new("Keep image"),
                    )
                    .clicked();
                vary = ui
                    .add_enabled(
                        self.worker.is_none() && !self.show_comparison,
                        egui::Button::new("Make variations"),
                    )
                    .clicked();
                if self.show_comparison {
                    regenerate = ui
                        .add_enabled(self.worker.is_none(), egui::Button::new("Regenerate"))
                        .on_hover_text("Generate a fresh batch from the same parents using the current settings.")
                        .clicked();
                    picks = ui
                        .add_enabled(self.worker.is_none(), egui::Button::new("Explore picks"))
                        .clicked();
                }
                if ui.button("History").clicked() {
                    self.show_history = true;
                }
                if self.comparison.is_some() {
                    ui.selectable_value(&mut self.show_comparison, false, "Studio");
                    ui.selectable_value(&mut self.show_comparison, true, "Variations");
                }
                if let Some(id) = &self.current {
                    ui.weak(self.label(id));
                }
                if self.worker.is_some() {
                    ui.spinner();
                    ui.label("Generating variations…");
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            });
            ui.add_enabled_ui(self.worker.is_none(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Vary:");
                    ui.checkbox(&mut self.settings.gesture, "Gesture");
                    ui.checkbox(&mut self.settings.texture, "Texture");
                    ui.checkbox(&mut self.settings.palette, "Palette");
                    ui.checkbox(&mut self.settings.composition, "Composition")
                        .on_hover_text("Rearrange curve families using reflection, repetition, alternation, and different instance counts.");
                    ui.add(
                        egui::Slider::new(&mut self.settings.strength, 0.02..=0.35)
                            .text("Amount")
                            .max_decimals(2),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Unchecked features stay fixed.");
                    ui.label("Pairs");
                    ui.add(egui::DragValue::new(&mut self.count).range(1..=12));
                    ui.label("Seed");
                    ui.add(egui::DragValue::new(&mut self.seed));
                    ui.weak(self.root.display().to_string());
                });
            });
            if let Some(message) = &self.message {
                ui.label(message);
            }
            if let Some(error) = &self.error {
                ui.colored_label(Color32::LIGHT_RED, error);
            }
        });
        if back {
            let result = self.back_to_comparison();
            self.result(result);
        }
        if keep {
            let result = self.keep_current().map(|_| ());
            self.result(result);
        }
        if vary {
            let result = self.start_variations();
            self.result(result);
        }
        if picks {
            let result = self.start_picks();
            self.result(result);
        }
        if regenerate {
            let result = self.start_regenerate();
            self.result(result);
        }
        if self.show_comparison {
            self.pairs(ctx);
        } else {
            self.studio.show(ctx);
        }
        self.history(ctx);
    }

    fn store(&self) -> Result<Session, String> {
        Session::open(&self.root)
    }

    fn back_to_comparison(&mut self) -> Result<(), String> {
        let round = &self
            .comparison
            .as_ref()
            .ok_or("Open a comparison first")?
            .record
            .id;
        self.store()?.view_round(round)?;
        self.show_comparison = true;
        self.show_history = false;
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), String> {
        let store = self.store()?;
        self.candidates = store.candidates()?;
        self.rounds = store.rounds()?;
        self.feedback = store.feedback()?;
        Ok(())
    }

    fn label(&self, id: &str) -> String {
        self.candidates
            .iter()
            .position(|candidate| candidate.id == id)
            .map(|i| format!("Image {}", i + 1))
            .unwrap_or_else(|| "Saved image".into())
    }

    fn capture(&mut self) -> Result<Candidate, String> {
        let snapshot = self.studio.snapshot()?;
        let store = self.store()?;
        let parent = self
            .current
            .as_deref()
            .filter(|_| self.studio.source() == self.loaded_source);
        if let Some(id) = parent {
            if store.snapshot(id)? == snapshot {
                return store.candidate(id);
            }
        }
        let record = store.save(&snapshot, parent)?;
        self.current = Some(record.id.clone());
        self.loaded_source = self.studio.source().to_owned();
        self.refresh()?;
        Ok(record)
    }

    fn keep_current(&mut self) -> Result<String, String> {
        let record = self.capture()?;
        self.store()?.keep(&record.id)?;
        self.refresh()?;
        self.message = Some(format!(
            "Kept {} with its current edits.",
            self.label(&record.id)
        ));
        Ok(record.id)
    }

    fn start_variations(&mut self) -> Result<(), String> {
        Settings::from_mask(self.settings.mask(), self.settings.strength)?;
        let parent = self.capture()?;
        let store = self.store()?;
        store.visit(&parent.id)?;
        let settings = self.settings;
        let seed = self.seed;
        let count = self.count;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(store.vary(&parent.id, seed, count, settings));
        });
        self.worker = Some(receiver);
        self.message = Some(format!(
            "Exploring relatives of {}.",
            self.label(self.current.as_ref().unwrap())
        ));
        Ok(())
    }

    fn start_picks(&mut self) -> Result<(), String> {
        Settings::from_mask(self.settings.mask(), self.settings.strength)?;
        let round = self
            .comparison
            .as_ref()
            .ok_or("Open a comparison first")?
            .record
            .id
            .clone();
        if !self
            .feedback
            .choices
            .iter()
            .any(|((id, _), choice)| *id == round && *choice != Preference::Neither)
        {
            return Err(
                "Choose A, B, or Both in at least one pair before exploring your picks".into(),
            );
        }
        let store = self.store()?;
        let settings = self.settings;
        let seed = self.seed;
        let count = self.count;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(store.vary_picks(&round, seed, count, settings));
        });
        self.worker = Some(receiver);
        self.message = Some("Exploring relatives of your selected images.".into());
        Ok(())
    }

    fn start_regenerate(&mut self) -> Result<(), String> {
        Settings::from_mask(self.settings.mask(), self.settings.strength)?;
        let round = self
            .comparison
            .as_ref()
            .ok_or("Open a comparison first")?
            .record
            .id
            .clone();
        let store = self.store()?;
        let settings = self.settings;
        let seed = self.seed;
        let count = self.count;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(store.regenerate(&round, seed, count, settings));
        });
        self.worker = Some(receiver);
        self.message = Some("Regenerating images with the current settings.".into());
        Ok(())
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let result = match self.worker.as_ref().map(|receiver| receiver.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                Some(Err("Variation generation stopped before finishing".into()))
            }
            _ => None,
        };
        if let Some(result) = result {
            self.worker = None;
            let result = result.and_then(|round| {
                self.refresh()?;
                self.open_round(ctx, round, true)?;
                self.message = Some("Choose A, B, both, or neither. Inspect a candidate to keep it or branch again.".into());
                Ok(())
            });
            self.result(result);
        }
    }

    fn open_candidate(&mut self, ctx: &egui::Context, id: &str, visit: bool) -> Result<(), String> {
        let store = self.store()?;
        let snapshot = store.snapshot(id)?;
        let mut studio = Studio::new(
            ctx,
            snapshot.program()?,
            store.root.join(id).join("program.sf").display().to_string(),
        );
        studio.select_output(&snapshot.output)?;
        if visit {
            // Preserve the working image before explicitly navigating elsewhere.
            self.capture()?;
            store.visit(id)?;
        }
        self.loaded_source = studio.source().to_owned();
        self.studio = studio;
        self.current = Some(id.to_owned());
        self.show_comparison = false;
        self.show_history = false;
        self.message = None;
        self.refresh()
    }

    fn open_round(
        &mut self,
        ctx: &egui::Context,
        record: Round,
        visit: bool,
    ) -> Result<(), String> {
        let store = self.store()?;
        let mut cards = Vec::new();
        for pair in &record.pairs {
            let mut pair_cards = Vec::new();
            for id in pair {
                let image = store.image(id)?;
                pair_cards.push(Card {
                    id: id.clone(),
                    texture: ctx.load_texture(
                        id.clone(),
                        egui::ColorImage::from_rgba_unmultiplied(
                            [image.width, image.height],
                            &image.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    ),
                });
            }
            cards.push(pair_cards.try_into().map_err(|_| "Invalid pair")?);
        }
        if visit {
            store.view_round(&record.id)?;
        }
        self.settings = record.settings.unwrap_or(self.settings);
        self.count = record.pairs.len();
        if let Some(seed) = record.seed {
            self.seed = seed.wrapping_add(1);
        }
        self.comparison = Some(Comparison { record, cards });
        self.show_comparison = true;
        self.show_history = false;
        Ok(())
    }

    fn pairs(&mut self, ctx: &egui::Context) {
        let Some(comparison) = &self.comparison else {
            self.show_comparison = false;
            return;
        };
        let mut inspect = None;
        let mut keep = None;
        let mut vote = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("EXPLORE VARIATIONS");
            let parent = if comparison.record.parents.len() > 1 {
                format!("{} selected families", comparison.record.parents.len())
            } else {
                comparison
                    .record
                    .parent
                    .as_deref()
                    .map(|id| self.label(id))
                    .unwrap_or_else(|| "Comparison".into())
            };
            ui.weak(format!(
                "{parent} · {} pairs · seed {}",
                comparison.cards.len(),
                comparison.record.seed.unwrap_or(0)
            ));
            ui.label("Which would you like to explore further? Choices are saved automatically.");
            egui::ScrollArea::vertical()
                .id_salt("variation_pairs")
                .show(ui, |ui| {
                    for (index, pair) in comparison.cards.iter().enumerate() {
                        ui.separator();
                        ui.label(RichText::new(format!("PAIR {:02}", index + 1)).strong());
                        ui.columns(2, |columns| {
                            for (slot, card) in pair.iter().enumerate() {
                                columns[slot].vertical_centered(|ui| {
                                    let label = ["A", "B"][slot];
                                    ui.label(RichText::new(label).strong());
                                    let size = ui.available_width().min(330.0);
                                    if ui
                                        .add(
                                            egui::Image::new((
                                                card.texture.id(),
                                                egui::vec2(size, size),
                                            ))
                                            .sense(egui::Sense::click()),
                                        )
                                        .clicked()
                                        || ui.button(format!("Inspect {label}")).clicked()
                                    {
                                        inspect = Some(card.id.clone());
                                    }
                                    if ui
                                        .button(if self.feedback.kept.contains(&card.id) {
                                            format!("Kept {label}")
                                        } else {
                                            format!("Keep {label}")
                                        })
                                        .clicked()
                                    {
                                        keep = Some(card.id.clone());
                                    }
                                });
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Prefer:");
                            for choice in Preference::ALL {
                                let selected = self
                                    .feedback
                                    .choices
                                    .get(&(comparison.record.id.clone(), index))
                                    == Some(&choice);
                                if ui.selectable_label(selected, choice.label()).clicked() {
                                    vote = Some((index, choice));
                                }
                            }
                        });
                    }
                });
        });
        if let Some((index, choice)) = vote {
            let result = self
                .store()
                .and_then(|store| {
                    store.choose(&self.comparison.as_ref().unwrap().record, index, choice)
                })
                .and_then(|_| self.refresh());
            self.result(result);
        }
        if let Some(id) = keep {
            let result = self
                .store()
                .and_then(|store| store.keep(&id))
                .and_then(|_| self.refresh());
            self.result(result);
        }
        if let Some(id) = inspect {
            let result = self.open_candidate(ctx, &id, true);
            self.result(result);
        }
    }

    fn history(&mut self, ctx: &egui::Context) {
        if !self.show_history {
            return;
        }
        let mut open = None;
        let mut round = None;
        let mut visible = true;
        egui::Window::new("EXPLORATION HISTORY")
            .open(&mut visible)
            .default_size([540.0, 480.0])
            .show(ctx, |ui| {
                ui.weak(self.root.display().to_string());
                ui.checkbox(&mut self.kept_only, "Kept images only");
                if self.candidates.is_empty() {
                    ui.label("Keep an image or make variations to begin this session.");
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Images");
                    for candidate in self.candidates.iter().rev() {
                        if self.kept_only && !self.feedback.kept.contains(&candidate.id) {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            let kept = if self.feedback.kept.contains(&candidate.id) {
                                "★ "
                            } else {
                                ""
                            };
                            if ui
                                .button(format!("{kept}{}", self.label(&candidate.id)))
                                .clicked()
                            {
                                open = Some(candidate.id.clone());
                            }
                            if let Some(parent) = &candidate.parent {
                                ui.weak(format!("from {}", self.label(parent)));
                            }
                            if let Some(seed) = candidate.seed {
                                ui.weak(format!("seed {seed}"));
                            }
                        });
                    }
                    ui.separator();
                    ui.heading("Comparisons");
                    for (index, record) in self.rounds.iter().enumerate().rev() {
                        let reviewed = (0..record.pairs.len())
                            .filter(|i| {
                                self.feedback.choices.contains_key(&(record.id.clone(), *i))
                            })
                            .count();
                        if ui
                            .button(format!(
                                "Round {} · {reviewed}/{} reviewed",
                                index + 1,
                                record.pairs.len()
                            ))
                            .clicked()
                        {
                            round = Some(record.clone());
                        }
                    }
                });
            });
        self.show_history &= visible;
        if let Some(id) = open {
            let result = self.open_candidate(ctx, &id, true);
            self.result(result);
        }
        if let Some(record) = round {
            let result = self.open_round(ctx, record, true);
            self.result(result);
        }
    }

    fn result(&mut self, result: Result<(), String>) {
        self.error = result.err();
        if self.error.is_some() {
            self.message = None;
        }
    }
}

impl eframe::App for Explorer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            ..Default::default()
        }
    }

    fn click(ctx: &egui::Context, explorer: &mut Explorer, label: &str) {
        let output = ctx.run(input(), |ctx| explorer.show(ctx));
        let rect = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(shape.shape.visual_bounding_rect())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing button: {label}"));
        for pressed in [true, false] {
            let mut frame = input();
            frame.events = vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let _ = ctx.run(frame, |ctx| explorer.show(ctx));
        }
    }

    fn finish(ctx: &egui::Context, explorer: &mut Explorer) {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while explorer.worker.is_some() {
            assert!(
                std::time::Instant::now() < deadline,
                "Variation generation timed out"
            );
            let _ = ctx.run(input(), |ctx| explorer.show(ctx));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(explorer.error.is_none(), "{:?}", explorer.error);
    }

    #[test]
    fn keeping_comparing_voting_inspecting_and_resuming_work_through_the_ui() {
        let root = std::env::temp_dir().join(format!(
            "starfish-explorer-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let ctx = egui::Context::default();
        let program =
            StarfishProgram::from_source(include_str!("../programs/curve_textures.sf")).unwrap();
        let mut explorer = Explorer::new(&ctx, program.clone(), "study.sf".into(), root.clone());
        explorer.count = 1;
        for _ in 0..2 {
            let _ = ctx.run(input(), |ctx| explorer.show(ctx));
        }
        assert!(
            !root.exists(),
            "Merely viewing a program must not create a session"
        );
        click(&ctx, &mut explorer, "Keep image");
        let parent = explorer.current.clone().expect("Keep saves a snapshot");
        assert!(
            Session::open(&root)
                .unwrap()
                .feedback()
                .unwrap()
                .kept
                .contains(&parent)
        );
        click(&ctx, &mut explorer, "Make variations");
        finish(&ctx, &mut explorer);
        assert!(explorer.show_comparison);
        let initial = explorer.comparison.as_ref().unwrap().record.clone();
        explorer.settings = Settings {
            texture: false,
            palette: true,
            ..Settings::default()
        };
        explorer.seed = 1234;
        explorer.count = 2;
        click(&ctx, &mut explorer, "Regenerate");
        finish(&ctx, &mut explorer);
        let round = explorer.comparison.as_ref().unwrap().record.clone();
        assert_ne!(round.id, initial.id);
        assert_eq!(round.parents, initial.parents);
        assert_eq!(round.seed, Some(1234));
        assert_eq!(round.pairs.len(), 2);
        assert_eq!(round.settings, Some(explorer.settings));
        assert_eq!(
            Session::open(&root)
                .unwrap()
                .round(&initial.id)
                .unwrap()
                .pairs,
            initial.pairs
        );
        click(&ctx, &mut explorer, "Both");
        assert_eq!(
            Session::open(&root)
                .unwrap()
                .feedback()
                .unwrap()
                .choices
                .get(&(round.id.clone(), 0)),
            Some(&Preference::Both)
        );
        let reopened = Explorer::resume(&ctx, program.clone(), "study.sf".into(), root.clone());
        assert!(reopened.show_comparison);
        assert_eq!(reopened.comparison.as_ref().unwrap().record.id, round.id);
        assert_eq!(reopened.seed, round.seed.unwrap().wrapping_add(1));
        assert_eq!(reopened.count, round.pairs.len());
        let child = explorer.comparison.as_ref().unwrap().cards[0][0].id.clone();
        click(&ctx, &mut explorer, "Inspect A");
        assert!(!explorer.show_comparison);
        assert_eq!(explorer.current.as_deref(), Some(child.as_str()));
        let working = explorer.studio.snapshot().unwrap();
        let settings = explorer.settings;
        let seed = explorer.seed;
        click(&ctx, &mut explorer, "← Back to comparisons");
        assert!(explorer.show_comparison);
        assert_eq!(explorer.comparison.as_ref().unwrap().record.id, round.id);
        assert_eq!(
            explorer.comparison.as_ref().unwrap().record.pairs,
            round.pairs
        );
        assert_eq!(
            explorer.feedback.choices.get(&(round.id.clone(), 0)),
            Some(&Preference::Both)
        );
        assert_eq!(explorer.studio.snapshot().unwrap(), working);
        assert_eq!(explorer.settings, settings);
        assert_eq!(explorer.seed, seed);
        assert_eq!(
            Session::open(&root)
                .unwrap()
                .feedback()
                .unwrap()
                .last_round
                .as_deref(),
            Some(round.id.as_str())
        );
        click(&ctx, &mut explorer, "Studio");
        assert!(!explorer.show_comparison);
        assert_eq!(explorer.studio.snapshot().unwrap(), working);
        click(&ctx, &mut explorer, "Keep image");
        let resumed = Explorer::resume(&ctx, program, "study.sf".into(), root.clone());
        assert_eq!(resumed.current.as_deref(), Some(child.as_str()));
        assert_eq!(
            resumed.studio.snapshot().unwrap(),
            Session::open(&root).unwrap().snapshot(&child).unwrap()
        );
        click(&ctx, &mut explorer, "Variations");
        click(&ctx, &mut explorer, "Explore picks");
        finish(&ctx, &mut explorer);
        assert_eq!(
            explorer.comparison.as_ref().unwrap().record.parents,
            round.pairs[0]
        );
        explorer.settings = Settings {
            texture: false,
            composition: true,
            ..Default::default()
        };
        click(&ctx, &mut explorer, "Regenerate");
        finish(&ctx, &mut explorer);
        let store = Session::open(&root).unwrap();
        for id in explorer
            .comparison
            .as_ref()
            .unwrap()
            .record
            .pairs
            .iter()
            .flatten()
        {
            assert!(
                crate::arrangement::unpack(&store.snapshot(id).unwrap())
                    .unwrap()
                    .plan
                    .is_some()
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
