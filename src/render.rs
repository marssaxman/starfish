//! Rendering returns plain RGBA bytes. It has no dependency on egui.

use crate::{
    evaluate::{Evaluator, Value},
    program::{ParameterBindings, StarfishProgram},
};
use std::{
    io::{self, BufWriter, Write},
    time::{Duration, Instant},
};

pub struct RenderRequest<'a> {
    pub output: &'a str,
    pub width: usize,
    pub height: usize,
}

pub struct RenderedImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub duration: Duration,
}

pub fn write_ppm(image: &RenderedImage, destination: impl Write) -> io::Result<()> {
    let mut writer = BufWriter::new(destination);
    write!(writer, "P6\n{} {}\n255\n", image.width, image.height)?;
    for pixel in image.rgba.chunks_exact(4) {
        writer.write_all(&pixel[..3])?;
    }
    writer.flush()
}

pub fn render(
    program: &StarfishProgram,
    bindings: &ParameterBindings,
    request: RenderRequest<'_>,
) -> Result<RenderedImage, String> {
    let start = Instant::now();
    if request.width == 0 || request.height == 0 || request.width > 4096 || request.height > 4096 {
        return Err("Render dimensions must be between 1 and 4096.".into());
    }
    let mut evaluator = Evaluator::new(program, bindings, request.output)?;
    let mut rgba = Vec::with_capacity(request.width * request.height * 4);
    for row in 0..request.height {
        let y = 1.0 - 2.0 * (row as f32 + 0.5) / request.height as f32;
        for column in 0..request.width {
            let x = 2.0 * (column as f32 + 0.5) / request.width as f32 - 1.0;
            let rgb = match evaluator.sample(x, y) {
                Value::Scalar(value) => [(value + 1.0) * 0.5; 3],
                Value::Color(rgb) => rgb,
                Value::Point(_) => unreachable!("point output rejected above"),
            };
            rgba.extend(rgb.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8));
            rgba.push(255);
        }
    }
    Ok(RenderedImage {
        width: request.width,
        height: request.height,
        rgba,
        duration: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_lab_fields_render_and_each_live_control_changes_the_image() {
        let program =
            StarfishProgram::from_source(include_str!("../programs/wave_lab.sf")).unwrap();
        let bindings = ParameterBindings::defaults(&program);
        let image = |bindings: &ParameterBindings, output: &str| {
            render(
                &program,
                bindings,
                RenderRequest {
                    output,
                    width: 24,
                    height: 24,
                },
            )
            .unwrap()
            .rgba
        };
        let original = image(&bindings, &program.output);
        for item in program.items.iter().filter(|item| {
            matches!(
                item.ty,
                crate::program::Type::FieldScalar | crate::program::Type::FieldColor
            )
        }) {
            let pixels = image(&bindings, &item.id);
            assert!(
                pixels.chunks_exact(4).any(|pixel| pixel != &pixels[..4]),
                "{}",
                item.id
            );
        }
        for (index, parameter) in program.parameters.iter().enumerate() {
            let mut changed = bindings.clone();
            changed.0[index] = parameter.min;
            assert_ne!(
                original,
                image(&changed, &program.output),
                "{} has no effect",
                parameter.id
            );
        }
    }

    #[test]
    fn bezier_study_renders_fields_and_all_controls_change_the_final_image() {
        let program =
            StarfishProgram::from_source(include_str!("../programs/bezier_study.sf")).unwrap();
        let bindings = ParameterBindings::defaults(&program);
        let image = |bindings: &ParameterBindings, output: &str| {
            render(
                &program,
                bindings,
                RenderRequest {
                    output,
                    width: 32,
                    height: 32,
                },
            )
            .unwrap()
            .rgba
        };
        let original = image(&bindings, &program.output);
        // The curve study must retain visual activity across the whole image.
        for tile_y in 0..4 {
            for tile_x in 0..4 {
                let mut minimum = [255u8; 3];
                let mut maximum = [0u8; 3];
                for y in tile_y * 8..(tile_y + 1) * 8 {
                    for x in tile_x * 8..(tile_x + 1) * 8 {
                        let pixel = &original[(y * 32 + x) * 4..][..3];
                        for channel in 0..3 {
                            minimum[channel] = minimum[channel].min(pixel[channel]);
                            maximum[channel] = maximum[channel].max(pixel[channel]);
                        }
                    }
                }
                assert!(
                    (0..3).any(|channel| maximum[channel] - minimum[channel] > 32),
                    "inactive image tile {tile_x}, {tile_y}"
                );
            }
        }
        for item in program
            .items
            .iter()
            .filter(|item| item.ty == crate::program::Type::FieldScalar)
        {
            let pixels = image(&bindings, &item.id);
            assert!(
                pixels
                    .chunks_exact(4)
                    .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2])
            );
            assert!(
                pixels.chunks_exact(4).any(|pixel| pixel != &pixels[..4]),
                "{}",
                item.id
            );
        }
        for (index, parameter) in program.parameters.iter().enumerate() {
            let mut changed = bindings.clone();
            changed.0[index] = parameter.min;
            assert_ne!(
                original,
                image(&changed, &program.output),
                "{} has no effect",
                parameter.id
            );
        }
        assert_eq!(original, image(&bindings, &program.output));
    }

    #[test]
    fn intermediate_rendering_preserves_final_output_and_is_grayscale() {
        let program = StarfishProgram::sample();
        let bindings = ParameterBindings::defaults(&program);
        let image = |output| {
            render(
                &program,
                &bindings,
                RenderRequest {
                    output,
                    width: 32,
                    height: 32,
                },
            )
            .unwrap()
        };
        let before = image(&program.output);
        for id in ["pattern_a", "pattern_b", "interference", "radial", "field"] {
            let scalar = image(id);
            assert_eq!(scalar.rgba.len(), 32 * 32 * 4);
            assert!(
                scalar
                    .rgba
                    .chunks_exact(4)
                    .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
            );
            assert!(scalar.rgba.chunks_exact(4).any(|p| p[0] != scalar.rgba[0]));
        }
        assert_eq!(program.output, "color");
        assert_eq!(before.rgba, image(&program.output).rgba);
        assert!(before.rgba.chunks_exact(4).any(|p| p[0] != p[1]));
    }

    #[test]
    fn invalid_requests_are_reported() {
        let program = StarfishProgram::sample();
        let bindings = ParameterBindings::defaults(&program);
        for (output, width) in [
            ("missing", 16),
            ("rotated", 16),
            ("color", 0),
            ("color", 4097),
        ] {
            assert!(
                render(
                    &program,
                    &bindings,
                    RenderRequest {
                        output,
                        width,
                        height: 16
                    }
                )
                .is_err()
            );
        }
    }
}
