#![forbid(unsafe_code)]

mod evaluate;
mod program;
mod render;
mod source;
mod studio;

use std::{env, fs, io::Write};

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
            StarfishProgram::sample(),
            "programs/shared_tides.sf".to_owned(),
        ),
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Usage:\n  starfish-studio [studio] [program.sf]\n  starfish-studio render program.sf output-id image.ppm [width height]"
            );
            Ok(())
        }
        [path] => launch(load(path)?, path.clone()),
        [command, path] if command == "studio" => launch(load(path)?, path.clone()),
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
    let mut file =
        fs::File::create(destination).map_err(|e| format!("Cannot create {destination}: {e}"))?;
    write!(file, "P6\n{} {}\n255\n", image.width, image.height)
        .map_err(|e| format!("Cannot write {destination}: {e}"))?;
    for pixel in image.rgba.chunks_exact(4) {
        file.write_all(&pixel[..3])
            .map_err(|e| format!("Cannot write {destination}: {e}"))?;
    }
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

fn launch(program: StarfishProgram, path: String) -> Result<(), String> {
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
        Box::new(move |cc| Ok(Box::new(studio::Studio::new(cc, program, path)))),
    )
    .map_err(|e| e.to_string())
}
