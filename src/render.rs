//! Rendering returns plain RGBA bytes. It has no dependency on egui.

use crate::{
    evaluate::{Evaluator, Value},
    program::{ParameterBindings, StarfishProgram, ValueType},
};
use std::time::{Duration, Instant};

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

pub fn render(
    program: &StarfishProgram,
    bindings: &ParameterBindings,
    request: RenderRequest<'_>,
) -> Result<RenderedImage, String> {
    let start = Instant::now();
    if request.width == 0 || request.height == 0 || request.width > 4096 || request.height > 4096 {
        return Err("Render dimensions must be between 1 and 4096.".into());
    }
    let node = program
        .node(request.output)
        .ok_or_else(|| format!("Unknown output: {}", request.output))?;
    if node.value_type == ValueType::Coordinates {
        return Err("Coordinate outputs cannot be displayed as images in this prototype.".into());
    }
    let evaluator = Evaluator::new(program, bindings, request.output)?;
    let mut rgba = Vec::with_capacity(request.width * request.height * 4);
    for row in 0..request.height {
        let y = 1.0 - 2.0 * (row as f32 + 0.5) / request.height as f32;
        for column in 0..request.width {
            let x = 2.0 * (column as f32 + 0.5) / request.width as f32 - 1.0;
            let rgb = match evaluator.sample(x, y) {
                Value::Scalar(value) => [(value + 1.0) * 0.5; 3],
                Value::Color(rgb) => rgb,
                Value::Coordinates(_) => unreachable!("coordinate output rejected above"),
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
        let before = image(program.output);
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
        assert_eq!(before.rgba, image(program.output).rgba);
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
