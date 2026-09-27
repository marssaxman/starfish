# Starfish Studio

A native Linux workspace for exploring continuous image programs. The first
prototype was a single image authored in Rust. Studio now loads versioned
Starfish source files, validates them, and renders named intermediate fields
through the same CPU evaluator used by the command-line renderer.

## Run the Studio

From this directory, with Rust 1.88 or newer and a Linux desktop session:

```sh
cargo run
cargo run -- programs/crosscurrent.sf
```

The first command opens [Shared tides](programs/shared_tides.sf). The second
opens [Crosscurrent](programs/crosscurrent.sf). In the left pane, type a source
path and press **Open**, or use the two sample buttons. An invalid file leaves
the current program open and shows the validation error. Select a named field
to inspect its grayscale output, then select **Final Image** to return to the
declared color output. The right pane shows the field expression, its direct
dependencies, program source, and live parameter bindings.
Frames, axes, and component definitions in the left pane can also be selected
for inspection while the image preview stays on the last selected field.

The first build downloads and compiles eframe/egui. X11 and Wayland are enabled.
On a minimal Debian/Ubuntu installation the typical prerequisites are
`build-essential`, `pkg-config`, `libx11-dev`, `libxi-dev`,
`libxrandr-dev`, `libxcursor-dev`, `libxinerama-dev`,
`libwayland-dev`, `libxkbcommon-dev`, and `libegl1-mesa-dev`.

## Render a saved program

```sh
cargo run -- render programs/shared_tides.sf final /tmp/shared-tides.ppm 512 512
cargo run -- render programs/crosscurrent.sf fine /tmp/fine.ppm 512 512
```

`final` selects the program's declared output. A stable field ID such as
`fine` selects an intermediate. Width and height default to 512 if omitted.
The files use binary PPM (P6) to keep the CLI independent of image libraries.

See [the language guide](docs/LANGUAGE.md) for syntax and semantics. The source
file is the authored artifact. The compiler retains named declarations and
component definitions for inspection while lowering them to a validated graph
for execution. Parameter values from sliders are separate runtime bindings;
they do not rewrite source or change the declared output.

## Checks

```sh
cargo fmt --check
cargo check
cargo test
```

Tests cover parsing and validation, both saved programs, deterministic wave
behavior, changed bindings, shared rotation, grayscale intermediate rendering,
the final color output, and invalid render requests.

## Current scope

The language deliberately has a small field vocabulary: coordinate transforms,
waves, arithmetic, composition, and palettes. It supports typed reusable
components and named frames, but no imports, loops, recursive components,
source editor, graphical editing, generator, persistence of slider presets,
or linear/Bézier elements yet. The latter can become new structured
components and field operations without changing the program/render boundary.

Rendering runs synchronously at 448 × 448 in Studio. Color stops are normalized
RGB values and palette interpolation is specified in the language guide.
The Studio shows loaded source verbatim; it does not yet reformat or edit it.
The legacy application is independent background material.
