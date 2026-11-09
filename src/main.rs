#![forbid(unsafe_code)]

mod arrangement;
mod bezier;
mod comparison;
mod evaluate;
mod explorer;
mod gallery;
mod generate;
mod program;
mod render;
mod session;
mod source;
mod studio;
mod variation;

use std::{env, fs, path::Path};

use program::{ParameterBindings, StarfishProgram};
use render::RenderRequest;

fn main() {
    if let Err(error) = run() {
        eprintln!("starfish-studio: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => launch(
            StarfishProgram::from_source(include_str!("../programs/curve_textures.sf"))?,
            "programs/curve_textures.sf".to_owned(),
            true,
        ),
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Usage:\n  starfish-studio [studio] [program.sf]\n  starfish-studio render program.sf output-id image.ppm [width height]\n  starfish-studio generate directory [seed [pairs]]\n  starfish-studio gallery directory\n  starfish-studio vary program.sf session-directory [seed [pairs]]\n  starfish-studio arrange program.sf session-directory [seed [pairs]]\n  starfish-studio explore session-directory\n\nGeneration defaults: seed 42, 12 pairs. The output directory must be new.\nVariation defaults: seed 42, 6 pairs, texture changes only.\nArrangement defaults: seed 42, 6 pairs, composition changes only.\nStudio exploration defaults to explorations/default; explore resumes a named session."
            );
            Ok(())
        }
        [path] => launch(load(path)?, path.clone(), false),
        [command, path] if command == "studio" => launch(load(path)?, path.clone(), false),
        [command, directory] if command == "gallery" => launch_gallery(directory),
        [command, directory] if command == "explore" => launch_exploration(directory),
        [command, path, directory, options @ ..] if command == "vary" || command == "arrange" => {
            let (seed, count) = match options {
                [] => (42, 6),
                [seed] => (parse_seed(seed)?, 6),
                [seed, count] => (
                    parse_seed(seed)?,
                    count
                        .parse::<usize>()
                        .map_err(|_| "Pair count must be an integer")?,
                ),
                _ => {
                    return Err(format!(
                        "Usage: starfish-studio {command} program.sf session-directory [seed [pairs]]"
                    ));
                }
            };
            if !(1..=12).contains(&count) {
                return Err("Choose between 1 and 12 variation pairs".into());
            }
            let program = load(path)?;
            let snapshot = variation::Snapshot::capture(
                &program,
                &ParameterBindings::defaults(&program),
                &program.output,
            )?;
            if command == "arrange" && !arrangement::supports(&program, &snapshot.output) {
                return Err("Arrangement studies require Bézier fields and a scalar or direct palette output".into());
            }
            let store = session::Session::open(Path::new(directory))?;
            let parent = store.save(&snapshot, None)?;
            store.visit(&parent.id)?;
            let settings = if command == "arrange" {
                variation::Settings {
                    texture: false,
                    composition: true,
                    ..Default::default()
                }
            } else {
                variation::Settings::default()
            };
            let round = store.vary(&parent.id, seed, count, settings)?;
            store.view_round(&round.id)?;
            println!(
                "Generated {count} variation pairs in {directory} (seed {seed}).\nExplore with: cargo run -- explore {directory}"
            );
            Ok(())
        }
        [command, directory, options @ ..] if command == "generate" => {
            let (seed, count) = match options {
                [] => (comparison::DEFAULT_SEED, comparison::DEFAULT_PAIRS),
                [seed] => (parse_seed(seed)?, comparison::DEFAULT_PAIRS),
                [seed, count] => (
                    parse_seed(seed)?,
                    count
                        .parse::<usize>()
                        .map_err(|_| "Pair count must be a positive integer".to_owned())?,
                ),
                _ => return Err("Usage: starfish-studio generate directory [seed [pairs]]".into()),
            };
            comparison::write_batch(Path::new(directory), seed, count)?;
            println!(
                "Generated {count} comparison pairs in {directory} (seed {seed}).\nBrowse with: cargo run -- gallery {directory}"
            );
            Ok(())
        }
        [command, path, output, destination] if command == "render" => {
            render_file(path, output, destination, 512, 512)
        }
        [command, path, output, destination, width, height] if command == "render" => {
            let width = width
                .parse()
                .map_err(|_| "Width must be an integer".to_owned())?;
            let height = height
                .parse()
                .map_err(|_| "Height must be an integer".to_owned())?;
            render_file(path, output, destination, width, height)
        }
        _ => Err("Invalid arguments. Run with --help for usage.".into()),
    }
}

fn parse_seed(seed: &str) -> Result<u64, String> {
    seed.parse()
        .map_err(|_| "Seed must be an unsigned 64-bit integer".to_owned())
}

fn load(path: &str) -> Result<StarfishProgram, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
    StarfishProgram::from_source(&source).map_err(|e| format!("{path}: {e}"))
}

fn render_file(
    path: &str,
    output: &str,
    destination: &str,
    width: usize,
    height: usize,
) -> Result<(), String> {
    let program = load(path)?;
    let bindings = ParameterBindings::defaults(&program);
    let output = if output == "final" {
        &program.output
    } else {
        output
    };
    let image = render::render(
        &program,
        &bindings,
        RenderRequest {
            output,
            width,
            height,
        },
    )?;
    let file =
        fs::File::create(destination).map_err(|e| format!("Cannot create {destination}: {e}"))?;
    render::write_ppm(&image, file).map_err(|e| format!("Cannot write {destination}: {e}"))?;
    println!(
        "Rendered {} × {} {} to {} in {:.1} ms",
        width,
        height,
        output,
        destination,
        image.duration.as_secs_f64() * 1000.0
    );
    Ok(())
}

fn launch(program: StarfishProgram, path: String, resume: bool) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 660.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Starfish Studio",
        options,
        Box::new(move |cc| {
            let root = std::path::PathBuf::from(explorer::DEFAULT_SESSION);
            let explorer = if resume {
                explorer::Explorer::resume(&cc.egui_ctx, program, path, root)
            } else {
                explorer::Explorer::new(&cc.egui_ctx, program, path, root)
            };
            Ok(Box::new(explorer))
        }),
    )
    .map_err(|e| e.to_string())
}

fn launch_exploration(directory: &str) -> Result<(), String> {
    let root = std::path::PathBuf::from(directory);
    session::Session::open(&root)?;
    let program = StarfishProgram::from_source(include_str!("../programs/curve_textures.sf"))?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1200.0, 850.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Starfish Exploration",
        options,
        Box::new(move |cc| {
            Ok(Box::new(explorer::Explorer::resume(
                &cc.egui_ctx,
                program,
                "programs/curve_textures.sf".into(),
                root,
            )))
        }),
    )
    .map_err(|error| error.to_string())
}

fn launch_gallery(directory: &str) -> Result<(), String> {
    let pairs = gallery::load(Path::new(directory))?;
    let directory = directory.to_owned();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1100.0, 800.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Starfish Comparisons",
        options,
        Box::new(move |cc| {
            Ok(Box::new(gallery::Gallery::new(
                &cc.egui_ctx,
                directory,
                pairs,
            )))
        }),
    )
    .map_err(|error| error.to_string())
}
