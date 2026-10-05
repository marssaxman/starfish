# The first generator experiment

Generate a study and browse it in the native gallery:

```sh
cargo run -- generate generated/study-42
cargo run -- gallery generated/study-42
```

Defaults are batch seed 42 and twelve pairs. Optional arguments are a decimal
unsigned 64-bit seed and a pair count from 1 to 64:

```sh
cargo run -- generate generated/another-study 123 24
```

The destination must be new. Generated studies are ignored by Git. Generation
validates and renders all candidates before creating the output directory.

## Seed → plan → components → composition → source

The generator is a pure module: it constructs a `DesignPlan`, then emits ordinary
version-1 Starfish source. It does not render or write files. The batch command
uses the existing compiler and CPU renderer; the native gallery is another client
of the same APIs.

A coordinated plan chooses a scene frame, a base frequency, a shared phase, an
anchor, a palette, and a composition. It instantiates two stripes and one to three
rings from a reusable component library:

- Stripes use relative orientations of 0° and 60°, 90°, or 120°, at 2× and 3× or
  4× the base frequency, with shared/quarter-turn phase offsets.
- Rings use 1×, ½×, and optionally 1½× frequency scales. Their placements use the
  anchor, its mirror, and the origin, depending on the number of ring instances.
- Composition either multiplies the stripes and adds the rings, adds all layers,
  or modulates the stripe sum with the ring sum. Instance weights are explicit.

These are choices in the plan, rather than relationships inferred from an
anonymous expression graph. Source comments retain the plan choices. Named
`oriented_N` and `placed_N` frames, `motif_N` and `layer_N` fields, and reusable
components remain inspectable after compilation. `composition` is the final
scalar field; `image` is the declared color output.

The language needs no additions: frequency ratios scale the coordinate field
before `wave`, phases use uniform angle addition, and placements use coordinate
translation. Rotation, frequency, and phase sliders affect all instances;
individual scale and weight sliders allow intentional variation. Binding changes
remain separate from the saved source defaults.

## What each pair controls

The independent plan is derived from its coordinated partner. It keeps the same
motif kinds and count, composition, weights, palette, scene rotation, base
frequency, and global phase. It independently samples each motif's orientation,
scale, phase offset, and placement. Scale sampling preserves fine stripes and
coarser rings, while dropping harmonic constraints.

Both sources use the same field operations and have the same per-pixel operation
budget. Uniform constant nodes can differ because negative literals introduce
negation nodes. This experiment compares constraints on relative choices;
global scene controls and component reuse remain present in both conditions.

The independent values are sampled from continuous ranges, so this is an
exploratory comparison, not a statistically matched test of correlations alone.
It also does not implement the expression-tree, mutation, or CPPN baselines from
the longer novelty review. Those are possible later experiments.

Compare before inspecting programs, since source comments reveal the condition.
Useful questions are whether motifs recur recognizably, whether distant areas
feel related, whether several scales coexist, and whether inspecting the program
explains something about its image. A/B assignment is shuffled per pair; neither
position identifies a condition.

## Artifacts and reproduction

Each study contains:

- `pair-NN-a.sf` and `pair-NN-b.sf`: self-contained programs.
- Matching `.ppm` files: 256 × 256 final previews from the CPU renderer.
- `pairs.tsv`: neutral pair numbers, derived seeds, and source filenames.
- `answers.tsv`: the coordinated/independent condition for each slot.
- `README.txt`: batch seed, generator version, and comparison instructions.

Open any source directly with `cargo run -- path/to/pair-NN-a.sf`, or render an
intermediate without the gallery:

```sh
cargo run -- render generated/study-42/pair-01-a.sf motif_0 /tmp/motif.ppm
```

The source includes batch seed, pair number, slot, derived pair seed, condition,
generator version, and plan choices. The source itself is sufficient to load and
render an image even if the generator later changes.

`starfish-relations-v1` uses a fixed SplitMix64 stream with separate domains for
planning, ablation, and slot assignment. The same version, batch seed, and pair
count produce identical filenames, source, and pixels on the same runtime.
Increasing the count preserves earlier pairs. Floating-point rendering follows
the existing CPU semantics; cross-platform bit-for-bit image parity is not
promised. Future changes to synthesis behavior should change the generator
version. Repeating a study requires a new destination directory.
