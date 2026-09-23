# Infinite Minesweeper v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A playable terminal Minesweeper whose main mode is an endless board, installable with one command and runnable on Windows and Linux.

**Architecture:** The board is a pure function of `(seed, x, y)` rather than an array, so nothing is stored but the cells the player has touched, held in 32×32 chunks. Game, render and input code talk to a single `Field` trait, so the infinite and classic modes share everything above the board itself.

**Tech Stack:** Rust 1.98, crossterm (terminal/mouse/resize), serde + serde_json (saves), directories (platform data dirs), clap (CLI).

**Spec:** `docs/superpowers/specs/2026-09-23-infinite-minesweeper-design.md`

## Global Constraints

- Rust edition 2021, minimum toolchain 1.75
- Crate name `infinisweeper`; installed binary name `minesweeper`
- Infinite mine density exactly 16% (`DENSITY: f64 = 0.16`)
- Infinite mode: 3 lives; classic mode: 1 life
- Chunk size 32×32; flood-fill cap 50,000 cells
- Coordinates are `i64` throughout and must work at and below zero
- Dependencies limited to: crossterm, serde, serde_json, directories, clap, base64
- The terminal must be restored on every exit path, including panic

## Review Focus

Failure modes the spec implies but that no feature task naturally exercises. Each has its test pinned to the task that owns the code.

1. **Negative coordinates across the origin seam** — `-1 / 32` must floor to chunk `-1`, not truncate to `0`; otherwise cells corrupt along both negative axes. → Task 3
2. **A terminal smaller than the board chrome** — an 80×24 layout in a 15×5 window must show a notice, not panic on a subtraction overflow. → Task 9
3. **A corrupt or truncated save file** — must be renamed aside and treated as absent, never crash the launch. → Task 8
4. **Chording with the wrong flags** — must detonate rather than silently do nothing, and must still respect lives. → Task 5
5. **A flood fill that reaches the cap** — must terminate cleanly and leave a consistent board, not a half-revealed region that breaks the win check. → Task 5

---

### Task 1: Cargo scaffold

**Files:** Create `Cargo.toml`, `src/main.rs`, `src/lib.rs`

**Produces:** the crate `infinisweeper` with binary `minesweeper`.

- [ ] Write `Cargo.toml` with the dependency set from Global Constraints and `[[bin]] name = "minesweeper"`
- [ ] `cargo build` — expect success
- [ ] Commit

### Task 2: Mine oracle

**Files:** Create `src/board/oracle.rs`

**Produces:** `is_mine(seed: u64, x: i64, y: i64) -> bool`, `adjacent_mines(seed, x, y) -> u8`

Hash is splitmix64 over a mix of the coordinates; threshold is `(u64::MAX as f64 * 0.16) as u64`.

- [ ] Tests: determinism (same input, same answer, 10k samples); density within 1% of 0.16 over 1M cells; no periodicity along rows, columns or diagonals; `adjacent_mines` agrees with eight manual `is_mine` calls
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 3: Chunk storage

**Files:** Create `src/board/chunk.rs`

**Produces:** `ChunkStore` with `revealed(x,y) -> bool`, `set_revealed(x,y)`, `flagged(x,y) -> bool`, `toggle_flag(x,y)`, `chunk_coords(x,y) -> (i64,i64,usize)`

- [ ] Tests: round-trip set/get; **floored division at the origin seam — `(-1,-1)`, `(-32,-32)`, `(-33,-33)` land in the expected chunks and don't collide with positives** (Review Focus 1); untouched chunks allocate nothing
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 4: Field trait

**Files:** Create `src/board/mod.rs`, `src/board/field.rs`

**Produces:** `CellState` (`Hidden`/`Revealed(u8)`/`Flagged`/`Detonated`), `RevealOutcome` (`Cleared(usize)`/`Mine`/`Blocked`), `Rect`, and the `Field` trait from the spec.

- [ ] Define the trait and types; no behavior yet
- [ ] `cargo build` — expect success
- [ ] Commit

### Task 5: Infinite field

**Files:** Create `src/board/infinite.rs`

**Consumes:** oracle, `ChunkStore`, `Field`
**Produces:** `InfiniteField::new(seed) -> Self`, `safe_start(seed) -> (i64,i64)`

- [ ] Tests: revealing a non-mine returns `Cleared`; a zero cell flood-fills its region; **flood fill stops at the 50,000 cap and leaves every revealed cell consistent** (Review Focus 5); flagged cells are never revealed by fill; revealing a mine returns `Mine`; **chording with the wrong flag count detonates** (Review Focus 4); chording with correct flags clears neighbours; `safe_start` has a mine-free 3×3
- [ ] Run tests — expect failure
- [ ] Implement, with BFS over a `VecDeque` generating chunks as it spreads
- [ ] Run tests — expect pass
- [ ] Commit

### Task 6: Classic field

**Files:** Create `src/board/classic.rs`

**Produces:** `ClassicField::new(w, h, mines, seed)`, `Difficulty` enum with the three presets, `is_won() -> bool`

Mines placed by rejection-sampling a seed whose first cell is clear, keeping the oracle approach rather than a second mechanism.

- [ ] Tests: exact mine count per difficulty; `bounds()` is `Some` and reveals outside it are `Blocked`; win recognised exactly when all non-mine cells are revealed and not before; first click clear
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 7: Game state

**Files:** Create `src/game.rs`

**Produces:** `Game` holding a boxed `Field`, lives, elapsed timer, cells cleared, cursor, `Status` (`Playing`/`Won`/`Lost`)

- [ ] Tests: a mine decrements lives and keeps `Playing` until the third; the third sets `Lost`; classic mode loses on the first mine; the timer starts on the first reveal, not at construction; score counts cleared cells once, not per reveal call
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 8: Persistence

**Files:** Create `src/persist.rs`

**Produces:** `save_run(&Game)`, `load_run() -> Option<Game>`, `best_scores()`, `record_score(...)`, `data_dir() -> PathBuf`

Chunks serialise as base64 bitmaps. Writes go to a temp file then rename.

- [ ] Tests: save/load round-trip preserves seed, lives, revealed and flagged cells; **a truncated or garbage file returns `None`, is renamed aside, and does not panic** (Review Focus 3); a missing data dir is created; scores sort correctly
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 9: Rendering

**Files:** Create `src/render/mod.rs`, `src/render/viewport.rs`

**Produces:** `Viewport::follow(cursor, term_w, term_h)`, `draw(&mut impl Write, &Game, &Viewport)`

Cells are two columns wide. Numbers use the traditional 256-colour palette.

- [ ] Tests: the viewport keeps the cursor at least 3 cells from every edge; it scrolls only when that margin is crossed; **a 15×5 terminal produces the too-small notice with no arithmetic overflow** (Review Focus 2); visible-cell maths is correct at negative coordinates
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 10: Input

**Files:** Create `src/input.rs`

**Produces:** `Action` enum, `map_event(Event, &Viewport) -> Option<Action>`

- [ ] Tests: left/right/middle click map to Reveal/Flag/Chord at the right board coordinate given a scrolled viewport; arrows and WASD both move; `q` and `Esc` quit; unknown events map to `None`
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 11: Menu and CLI

**Files:** Create `src/menu.rs`, `src/cli.rs`

**Produces:** `Cli` (clap) with `--infinite --beginner --intermediate --expert --continue --seed`, and `run_menu() -> Option<Launch>`

- [ ] Tests: each flag parses to the right `Launch`; no flags yields `Launch::Menu`; `--seed` requires `--infinite`; menu selection wraps at both ends
- [ ] Run tests — expect failure
- [ ] Implement
- [ ] Run tests — expect pass
- [ ] Commit

### Task 12: Main loop

**Files:** Modify `src/main.rs`

Wires everything: terminal guard with `Drop`, panic hook restoring the terminal, the event loop, 30-second autosave, save on quit.

- [ ] Implement the `TerminalGuard` and panic hook
- [ ] Implement the loop: poll event → map to action → apply to game → redraw
- [ ] `cargo test` and `cargo clippy -- -D warnings` — expect clean
- [ ] Play it: infinite, classic, quit and resume
- [ ] Commit
