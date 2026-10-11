# DeckCraft

The repository's `/docs` folder is stale and must not be treated as authoritative.

## Locations

- `apps/deckcraft/` — native desktop entry point and control server.
- `apps/deckcraft-cli/` — command-line argument parsing and dispatch.
- `apps/deckcraft-web/` — browser entry point.
- `crates/mcp/` — MCP and automation integration.
- `crates/model/` — presentation data; `crates/engine/src/cmd/` — editing commands.
- `crates/format/` — native files; `crates/pptx/` — PowerPoint file I/O.
- `crates/render/` and `crates/text/` — slide rendering and text layout.
- `crates/ui-egui/src/lib.rs` — app state and `UI_COMMANDS`.
- `crates/ui-egui/` — interface, panels and controls; `src/control.rs` handles live automation.
- `Cargo.toml` — workspace packages and dependencies; `xtask/src/` — development tasks.

## Commands

Independent examples, run from the repository root.

```sh
cargo run --release -p deckcraft -- --sample --control 7990
cargo run -p deckcraft-cli -- commands
cargo run -p deckcraft-cli -- mcp --sample  # Separate headless session
cargo test -p deckcraft-engine
```

Desktop control: newline-delimited JSON over unauthenticated loopback TCP at
`127.0.0.1:7990`. The screenshot path is an example output filename.

```json
{"id":1,"method":"ui.inspect","params":{}}
{"id":2,"method":"ui.screenshot","params":{"path":"/tmp/deckcraft-window.png"}}
```

MCP connected to that running desktop session:

```sh
cargo run -p deckcraft-cli -- mcp --connect 7990
```

Slide renders without the interface:

```sh
deckcraft_output=$(mktemp -d)
cargo run -p deckcraft-cli -- render --sample --all --scale 1 "$deckcraft_output"
```
