# Starfish Codex starter packet

This packet is intended to be copied into the root of a Starfish repository before
giving Codex the task in `CODEX_TASK.md`.

Recommended reading order for Codex:

1. `CODEX_TASK.md`
2. `docs/Starfish System Architecture.txt`
3. `docs/Artistic Novelty Review.txt`

The task is deliberately a disposable vertical slice. Do not try to implement the
full architecture described in the background documents.

The selected prototype stack is Rust + eframe/egui. It produces a native Linux
desktop executable and requires no JavaScript, Node, browser, webview, or server.
