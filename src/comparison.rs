//! Batch orchestration for the generator experiment; no Studio dependency.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    generate::{self, GENERATOR_VERSION},
    program::{ParameterBindings, StarfishProgram},
    render::{self, RenderRequest, RenderedImage},
};

pub const PREVIEW_SIZE: usize = 256;
pub const DEFAULT_SEED: u64 = 42;
pub const DEFAULT_PAIRS: usize = 12;
pub const MAX_PAIRS: usize = 64;

pub struct PairFiles {
    pub number: usize,
    pub a: PathBuf,
    pub b: PathBuf,
}

pub fn load_index(directory: &Path) -> Result<Vec<PairFiles>, String> {
    let path = directory.join("pairs.tsv");
    let index = fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let mut lines = index.lines();
    if lines.next() != Some("pair\tseed\ta\tb") {
        return Err("Expected a Starfish comparison index (pairs.tsv)".into());
    }
    let mut pairs = Vec::new();
    for (line, row) in lines.enumerate() {
        let columns: Vec<_> = row.split('\t').collect();
        if columns.len() != 4 {
            return Err(format!("Invalid comparison index at line {}", line + 2));
        }
        let number = columns[0]
            .parse::<usize>()
            .map_err(|_| format!("Invalid pair number at line {}", line + 2))?;
        columns[1]
            .parse::<u64>()
            .map_err(|_| format!("Invalid seed at line {}", line + 2))?;
        if number != pairs.len() + 1 || pairs.len() >= MAX_PAIRS {
            return Err("Comparison index must contain consecutive pairs, starting at 1, with at most 64 pairs".into());
        }
        for file in &columns[2..] {
            if Path::new(file).file_name().and_then(|name| name.to_str()) != Some(*file)
                || !file.ends_with(".sf")
            {
                return Err(format!(
                    "Invalid source filename in comparison index: {file}"
                ));
            }
        }
        pairs.push(PairFiles {
            number,
            a: directory.join(columns[2]),
            b: directory.join(columns[3]),
        });
    }
    if pairs.is_empty() {
        return Err("The comparison index is empty".into());
    }
    Ok(pairs)
}

pub fn write_batch(directory: &Path, seed: u64, count: usize) -> Result<(), String> {
    if !(1..=MAX_PAIRS).contains(&count) {
        return Err(format!("Pair count must be between 1 and {MAX_PAIRS}"));
    }
    // Check the destination before doing expensive work. Never overwrite a study.
    if directory.exists() {
        return Err(format!(
            "{} already exists; choose a new output directory",
            directory.display()
        ));
    }
    // Validate and render every candidate before creating the output directory.
    let mut candidates: Vec<(String, String, RenderedImage)> = Vec::new();
    let mut index = String::from("pair\tseed\ta\tb\n");
    let mut answers = String::from("pair\tseed\ta\tb\n");
    for pair_index in 0..count {
        let pair = generate::comparison_pair(seed, pair_index);
        let number = pair_index + 1;
        index.push_str(&format!(
            "{number}\t{}\tpair-{number:02}-a.sf\tpair-{number:02}-b.sf\n",
            pair.seed
        ));
        answers.push_str(&format!(
            "{number}\t{}\t{}\t{}\n",
            pair.seed,
            pair.a.condition.name(),
            pair.b.condition.name()
        ));
        for (label, plan) in [("a", pair.a), ("b", pair.b)] {
            let source = format!(
                "// batch seed: {seed}; pair: {number}; slot: {label}\n{}",
                plan.source()
            );
            let program = StarfishProgram::from_source(&source)?;
            let image = render::render(
                &program,
                &ParameterBindings::defaults(&program),
                RenderRequest {
                    output: &program.output,
                    width: PREVIEW_SIZE,
                    height: PREVIEW_SIZE,
                },
            )?;
            candidates.push((format!("pair-{number:02}-{label}"), source, image));
        }
    }
    if let Some(parent) = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
    }
    fs::create_dir(directory)
        .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
    let write_result = (|| {
        for (name, source, image) in candidates {
            fs::write(directory.join(format!("{name}.sf")), source)?;
            render::write_ppm(
                &image,
                fs::File::create(directory.join(format!("{name}.ppm")))?,
            )?;
        }
        fs::write(directory.join("pairs.tsv"), index)?;
        fs::write(directory.join("answers.tsv"), answers)?;
        fs::write(
            directory.join("README.txt"),
            format!(
                "Starfish comparison study\nGenerator: {GENERATOR_VERSION}\nBatch seed: {seed}\nPairs: {count}\n\n\
             Two to four cubic curves bend interacting wave fields across the whole image.\n\
             Textures explore etched lines, braided crossings, and nested ripples,\n\
             with sine, triangle, saw, pulse, harmonic, and modulated carriers.\n\
             Each pair has the same motifs, waveform profiles, detail controls, composition, weights, palette, and field-operation budget.\n\
             One condition varies a shared curve gesture, with related orientations, harmonic scales, phase offsets, and mirrored placements.\n\
             The other samples gestures and relative choices independently, retaining each instance's broad scale band.\n\
             Texture recipe, bend, texture amount, and texture scale are matched within each pair.\n\
             Scene rotation, base frequency, and phase controls remain shared in both conditions.\n\n\
             A/B order is shuffled deterministically; pairs.tsv contains neutral source filenames.\n\
             Browse with: cargo run -- gallery <this directory>\n\
             Compare A and B before inspecting source or opening answers.tsv.\n\
             A/B, Both, and Neither judgments are saved with program snapshots in feedback/.\n\
             Inspect a candidate to keep edits, make variations, and browse exploration history.\n\
             Each .sf is self-contained and includes generator provenance and its explicit plan.\n\
             Each .ppm is a {PREVIEW_SIZE} x {PREVIEW_SIZE} preview from the existing CPU renderer.\n\n\
             In Studio, enable Curve guides to see the paths; select a curve and drag its points.\n\
             Try Bend, Texture amount, Texture scale, and the intermediate grain/texture fields.\n\n\
             Look for recurring gestures, long-range relationships, structure at several scales,\n\
             and whether inspecting the program explains the image.\n\
             This is a small visual experiment, not a claim of aesthetic superiority.\n\n\
             Reproduce with: cargo run -- generate <new directory> {seed} {count}\n"
            ),
        )?;
        Ok::<_, std::io::Error>(())
    })();
    write_result.map_err(|error| format!("Cannot finish study in {}: {error}", directory.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_round_trip_is_reproducible_and_refuses_overwrites() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "starfish-study-test-{}-{nonce}",
            std::process::id()
        ));
        let first = root.join("first");
        let second = root.join("second");
        write_batch(&first, 42, 2).unwrap();
        write_batch(&second, 42, 2).unwrap();
        let pairs = load_index(&first).unwrap();
        assert_eq!(pairs.len(), 2);
        for pair in &pairs {
            for source in [&pair.a, &pair.b] {
                let text = fs::read_to_string(source).unwrap();
                StarfishProgram::from_source(&text).unwrap();
                assert_eq!(
                    text,
                    fs::read_to_string(second.join(source.file_name().unwrap())).unwrap()
                );
                let preview = source.with_extension("ppm");
                let pixels = fs::read(&preview).unwrap();
                assert!(pixels.starts_with(b"P6\n256 256\n255\n"));
                assert_eq!(
                    pixels,
                    fs::read(second.join(preview.file_name().unwrap())).unwrap()
                );
            }
        }
        let before = fs::read(first.join("pairs.tsv")).unwrap();
        assert!(
            write_batch(&first, 99, 2)
                .unwrap_err()
                .contains("already exists")
        );
        assert_eq!(before, fs::read(first.join("pairs.tsv")).unwrap());
        assert!(write_batch(&root.join("invalid"), 42, 0).is_err());
        assert!(write_batch(&root.join("invalid"), 42, MAX_PAIRS + 1).is_err());
        assert!(!root.join("invalid").exists());
        // Exercise the same source loading and thumbnail rendering as the GUI.
        assert_eq!(crate::gallery::load(&first).unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
