# Infinite Minesweeper — Design

**Date:** 2026-09-23
**Status:** Approved for planning

## Intent

A terminal Minesweeper whose headline mode is an endless board. You install it once
with `cargo install infinisweeper`, type `minesweeper`, and play — the same
one-command feel as `ttyper`. It runs identically on Windows and Linux, and takes
mouse or keyboard input interchangeably.

Success looks like: a run you can start in under two seconds, leave mid-game, and come
back to tomorrow exactly where you left it.

### Explicitly in scope

- Infinite mode: endless board, three lives, constant 16% mine density, auto-saving runs
- Classic mode: Beginner, Intermediate and Expert boards with real edges and one life
- Mouse and keyboard controls, both always live
- A launch menu, with CLI flags to bypass it
- Persistent best scores and one resumable infinite run

### Explicitly out of scope

These were considered and cut. They can be added later without restructuring anything.

- No-guess boards (needs a constraint solver in the generation loop)
- Daily challenge seeds
- Custom board dimensions
- Difficulty that scales with distance from the origin
- Hint or solver keys

## Architecture

The design turns on one decision: **the board is a pure function, not an array.**

```
main.rs ── cli.rs ── menu.rs
                 └── game.rs ──┬── field.rs   (trait Field)
                               │     ├── infinite.rs  (oracle + chunks)
                               │     └── classic.rs   (finite grid)
                               ├── render.rs  (viewport, drawing)
                               ├── input.rs   (events → Action)
                               └── persist.rs (save/load)
```

`game.rs` drives everything through the `Field` trait, so it never learns whether it's
running an infinite board or a 9×9 one. That's what keeps two modes from becoming two
codebases.

```rust
trait Field {
    fn cell(&self, x: i64, y: i64) -> CellState;
    fn is_mine(&self, x: i64, y: i64) -> bool;
    fn adjacent_mines(&self, x: i64, y: i64) -> u8;
    fn reveal(&mut self, x: i64, y: i64) -> RevealOutcome;
    fn toggle_flag(&mut self, x: i64, y: i64);
    fn chord(&mut self, x: i64, y: i64) -> RevealOutcome;
    /// None for the infinite field; Some(rect) for classic boards.
    fn bounds(&self) -> Option<Rect>;
}
```

### The mine oracle

Whether a cell holds a mine is derived from the run's seed and the cell's coordinates,
with no state involved:

```rust
fn is_mine(seed: u64, x: i64, y: i64) -> bool {
    splitmix64(seed ^ mix(x, y)) < DENSITY_THRESHOLD
}
```

`DENSITY_THRESHOLD` is `(u64::MAX as f64 * 0.16) as u64`. The hash must mix well in both
axes — a weak one produces visible diagonal streaks of mines, which is the failure mode
to watch for in tests.

`adjacent_mines` is eight oracle calls. That is cheap enough to do on demand, and the
result is memoised per chunk once computed.

This one choice buys four properties:

- Storage is proportional to what you've touched, not where you've been
- Scrolling back a thousand cells is consistent without remembering anything
- Saves hold your moves, not the board
- Seeds are shareable and reproduce a board exactly

### Chunk storage

Player state — revealed, flagged — lives in 32 × 32 chunks in a
`HashMap<(i64, i64), Chunk>`, allocated when first touched. Each chunk holds two 1024-bit
bitmaps plus the memoised adjacency counts. A chunk costs 256 bytes of player state.

Chunk coordinates use floored division, not truncating division — `-1 / 32` must land in
chunk `-1`, not chunk `0`. Getting this wrong produces a seam of corrupted cells along
both negative axes, so it gets a dedicated test.

### Flood fill

On a finite board, clearing a zero-region is bounded by the board. On an infinite one it
isn't, so the BFS carries a hard cap of 50,000 cells and generates chunks as it spreads.
At 16% density, regions that large are vanishingly rare, but the cap is what stops a
pathological seed from hanging the game.

### First click is safe

Mines can't be relocated after the fact — the hash is the truth — so instead of editing
the board the game searches for a starting coordinate whose 3 × 3 neighbourhood is
mine-free, spiralling outward from the origin. At 16% density roughly one position in
five qualifies, so this terminates in a handful of iterations.

The same trick serves classic mode: it picks the seed, rather than moving mines, by
rejecting seeds whose chosen first cell isn't clear.

## Game rules

**Infinite.** Three lives. Revealing a mine costs one, leaves it drawn as `✹`, and play
continues; the third ends the run. Score is cells cleared. The timer runs from the first
reveal.

**Classic.** One life, real edges, standard win condition — every non-mine cell revealed.
Scored by completion time per difficulty.

**Chording**, in both modes: revealing every neighbour of a revealed number once exactly
that many neighbours are flagged. If the flags are wrong, it detonates, which is correct
and intended.

## Rendering

A viewport over the field, scrolling to follow the cursor and keeping it off the edges by
a margin of three cells. Cells render two columns wide so the grid looks square in a
terminal's rectangular cells.

Numbers take the traditional palette (1 blue, 2 green, 3 red, 4 navy, …) via 256-colour
ANSI. A full redraw of an 80 × 24 viewport is under 2,000 cells, so the first
implementation redraws the whole viewport per frame and only optimises if it measurably
needs to.

Resize events relayout rather than crash. Terminals smaller than 20 × 10 display a
"terminal too small" notice instead of a broken grid.

## Input

Events map to a single `Action` enum before reaching game logic, so mouse and keyboard
paths converge immediately and the game layer has one code path to test.

| Action         | Mouse        | Keyboard               |
| -------------- | ------------ | ---------------------- |
| Reveal         | Left click   | `Space`                |
| Flag           | Right click  | `F`                    |
| Chord          | Middle click | `C`                    |
| Move cursor    | —            | Arrows / `WASD`        |
| Pan            | Drag         | Cursor at edge         |
| Jump to origin | —            | `O`                    |
| Restart        | —            | `R`                    |
| Quit           | —            | `Q` / `Esc`            |

Mouse capture is requested at startup. If the terminal doesn't report mouse events the
game says so once in the status line and stays fully playable on the keyboard.

## Persistence

| Platform | Location                                     |
| -------- | -------------------------------------------- |
| Linux    | `~/.local/share/minesweeper/`                |
| Windows  | `%APPDATA%\minesweeper\`                     |
| macOS    | `~/Library/Application Support/minesweeper/` |

Resolved with the `directories` crate rather than hand-rolled environment lookups.

`run.json` holds the in-progress infinite run: seed, lives, elapsed time, cursor, and the
touched chunks as base64 bitmaps. A 10,000-cell run is a few kilobytes. `scores.json`
holds best results per mode.

Saves are written to a temporary file and renamed into place, so a quit or crash
mid-write can't leave a half-written file. A save that fails to parse is renamed aside and
treated as absent — a corrupt file loses one run, never the ability to launch.

Autosave fires on quit and every 30 seconds.

## Error handling

The terminal must always be restored. Raw mode and mouse capture are owned by a guard
struct whose `Drop` releases them, and a panic hook restores the terminal before printing
the panic, so a crash never leaves an unusable shell.

Failures to read or write the data directory degrade to in-memory play with a status-line
warning, rather than refusing to start.

## Testing

The architecture exists partly to make the interesting parts testable without a terminal.
`field`, `infinite`, `classic` and `persist` are pure logic and carry the real tests:

- **Oracle determinism** — the same `(seed, x, y)` always answers the same
- **Density** — over a large sample, the mine rate sits within tolerance of 16%
- **Distribution** — no axis-aligned or diagonal periodicity in the mine field
- **Negative coordinates** — chunk indexing is correct across both axes at the origin seam
- **Flood fill** — terminates, respects the cap, never reveals a flagged cell
- **First-click safety** — the chosen start has a clear 3 × 3 in both modes
- **Chording** — correct flags reveal neighbours; wrong flags detonate
- **Classic win/loss** — recognised exactly, at the right moment
- **Save round-trip** — state survives save/load; a truncated file is handled without panic

`render`, `input` and `menu` get smoke tests only. Terminal output is verified by playing
it, not by asserting on escape sequences.

## Build and release

`cargo build` for both targets; CI runs `cargo test`, `cargo clippy -- -D warnings` and
`cargo fmt --check` on Windows and Linux. Release builds attach a `.exe` and a Linux
binary so people can skip `cargo install` entirely.
