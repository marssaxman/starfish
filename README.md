# Starfish Studio

A small native Linux workspace for exploring an image as a functional program.

## Run

From this directory, with Rust 1.88 or newer and a Linux desktop session:

```sh
cargo run
```

The first build downloads and compiles eframe/egui. The ordinary executable is
`target/debug/starfish-studio`. X11 and Wayland are enabled; eframe uses OpenGL
to present the UI, while the procedural image is evaluated entirely on the CPU.
On a minimal Debian/Ubuntu installation, the usual build/runtime prerequisites
are `build-essential`, `pkg-config`, `libx11-dev`, `libxi-dev`, `libxrandr-dev`,
`libxcursor-dev`, `libxinerama-dev`, `libwayland-dev`, `libxkbcommon-dev`, and
`libegl1-mesa-dev`.

## Explore

- Start with the composed **Final Image** in color.
- Select **Pattern A**, **Pattern B**, **Interference**, **Radial**, or **Field**
  to render that computation as grayscale (−1 is black, +1 is white).
- Read its operation, expression, inputs, type, and stable ID in the inspector.
- Move **Rotation** to rotate the shared frame used by both waves and the
  offset radial center. Change the frequencies to alter their spacing.
- Adjust the two strengths to rebalance the fine weave and broad rings.
- Select **Final Image** to return to the palette-mapped composition.
- **Reset parameters** restores the deterministic defaults. The inspector
  scrolls if the window is too short to show all controls.

All seven sliders affect the program bindings, regardless of which output is
selected. A parameter only changes an intermediate if that intermediate depends
on it. Numeric values can also be edited directly. The preview keeps its square
aspect ratio and reports its 448 × 448 render dimensions and CPU render time.

## Implementation and checks

`src/program.rs` declares the eight-node sample and parameter metadata.
`src/evaluate.rs` evaluates that graph at continuous coordinates.
`src/render.rs` accepts a program, bindings, and output ID and returns RGBA bytes.
`src/studio.rs` owns selection, sliders, and texture presentation. These first
three modules have no egui dependency. Selecting an intermediate never changes
the program's final-output declaration.

```sh
cargo fmt --check
cargo check
cargo test
```

The tests cover known sine values, changed bindings, deterministic evaluation,
shared rotation, neutral composition, grayscale/color rendering, returning to
the final output, and invalid render requests. No additional test framework is
used.

## Deliberate shortcuts

- One fixed, authored program; node counts, parameter slots, and topological
  input indices are trusted. Loading or editing arbitrary graphs would require
  validation and a more flexible representation.
- Rendering runs synchronously when selection or bindings change, at a fixed
  resolution. Larger or more expensive programs would need background work and
  cancellation. Earlier unrelated nodes may be evaluated for an intermediate.
- Coordinates are present in the model and inspector input names but are not
  selectable images. Scalar visualization assumes values in [−1, 1].
- The palette is fixed. There is no persistence, graph editor, language,
  generation, search, or GPU image evaluator.
