# minesweeper

An infinite Minesweeper for your terminal. No edges, no corners to run out of — just keep clearing.

```
  ♥ ♥ ♡                  cleared 1,284                03:12

        ·  ·  1  ▓  ▓  ▓  2  ·  ·  ·  1  ▓  ▓  ▓  ▓  ▓
        ·  ·  1  1  2  ▓  2  1  1  ·  1  ▓  ▓  ▓  ▓  ▓
        ·  ·  ·  ·  1  ⚑  1  ·  ·  ·  1  2  ▓  ▓  ▓  ▓
        1  1  ·  ·  1  1  1  ·  ·  ·  ·  1  ▓  ▓  ▓  ▓
        ▓  1  ·  ·  ·  ·  ·  ·  1  1  1  1  2  ▓  ▓  ▓
        ▓  2  1  ·  ·  1  1  2  2 [▓] ▓  ▓  ▓  ▓  ▓  ▓
        ▓  ▓  1  ·  ·  1  ⚑  2  ▓  ▓  ▓  ▓  ▓  ▓  ▓  ▓
        ▓  ▓  2  1  1  1  1  2  ▓  ▓  ✹  ▓  ▓  ▓  ▓  ▓

  left reveal   right flag   middle chord   ↑↓←→ move   q quit
```

## Install

```sh
cargo install infinisweeper
```

The crate is `infinisweeper` because `minesweeper` was already taken on crates.io. The
command it installs is just:

```sh
minesweeper
```

That opens the menu. If you already know what you want, skip it:

```sh
minesweeper --infinite      # the main event
minesweeper --beginner      #  9 × 9,  10 mines
minesweeper --intermediate  # 16 × 16, 40 mines
minesweeper --expert        # 30 × 16, 99 mines
minesweeper --continue      # resume your last infinite run
```

## Modes

### Infinite

The main mode. The board has no boundary — scroll in any direction forever and the
terrain is generated as you reach it. Mine density is a constant 16% everywhere, so
the board is exactly as fair a thousand cells out as it is where you started.

You get **three lives**. Hitting a mine costs one and leaves the mine visible as `✹`,
but the run continues; the third ends it. Your score is the number of cells you
cleared.

Runs auto-save. Quit whenever and `minesweeper --continue` puts you back exactly
where you were.

### Classic

The three original boards, with real edges and one life, scored by completion time:

| Mode         | Board   | Mines | Density |
| ------------ | ------- | ----: | ------: |
| Beginner     | 9 × 9   |    10 |   12.3% |
| Intermediate | 16 × 16 |    40 |   15.6% |
| Expert       | 30 × 16 |    99 |   20.6% |

## Controls

Mouse and keyboard both work, always — use whichever you feel like mid-game.

| Action          | Mouse         | Keyboard             |
| --------------- | ------------- | -------------------- |
| Move cursor     | —             | `↑` `↓` `←` `→`/`WASD` |
| Reveal          | Left click    | `Space`              |
| Flag            | Right click   | `F`                  |
| Chord           | Middle click  | `C`                  |
| Pan the view    | Drag          | Move to the edge     |
| Jump to origin  | —             | `O`                  |
| New run         | —             | `R`                  |
| Quit (saves)    | —             | `Q` / `Esc`          |

**Chording** reveals every neighbour of an already-revealed number once you've flagged
exactly that many mines around it. It's how you play fast, and it's the single biggest
difference between a beginner and a good player.

## How the infinite board works

There's no array. Whether a cell holds a mine is a pure function of the run's seed and
the cell's coordinates:

```rust
fn is_mine(seed: u64, x: i64, y: i64) -> bool {
    hash(seed, x, y) < DENSITY_THRESHOLD
}
```

That single decision gives you a few things at once:

- **The board costs nothing to store.** Only cells you've actually revealed or flagged
  are kept, in 32 × 32 chunks allocated on demand.
- **Scrolling back is consistent.** A cell a thousand steps away answers the same way
  now as it will in an hour, without anything being remembered.
- **Saves are tiny.** The file holds your seed and your moves; the board regenerates
  from the seed. A 10,000-cell run is a few kilobytes.
- **Seeds are shareable.** `minesweeper --infinite --seed 8675309` gives you and anyone
  else the identical infinite board.

First click is always safe. Since mines can't be relocated after the fact — the hash is
the truth — the game instead looks for a starting coordinate whose 3 × 3 neighbourhood
is mine-free and drops you there.

## Where files are kept

| Platform | Location                                        |
| -------- | ----------------------------------------------- |
| Linux    | `~/.local/share/minesweeper/`                   |
| Windows  | `%APPDATA%\minesweeper\`                        |
| macOS    | `~/Library/Application Support/minesweeper/`    |

`run.json` holds the infinite run in progress; `scores.json` holds your best results per
mode. Deleting either is harmless — the game just starts fresh.

## Terminal support

Needs a terminal with 256-colour and mouse reporting, which covers essentially
everything modern:

- **Windows** — Windows Terminal (the Windows 11 default), or any terminal running
  PowerShell 7. Works in the legacy `cmd.exe` console too, without mouse support.
- **Linux** — any of the usual suspects: GNOME Terminal, Konsole, Alacritty, kitty,
  WezTerm, xterm.
- **Over SSH or in tmux** — fine. Mouse may be captured by tmux depending on your
  config; the keyboard controls cover everything the mouse does.

The board redraws to fit whenever you resize the window.

## Building from source

```sh
git clone https://github.com/teterw/minesweeper
cd minesweeper
cargo run --release
```

Requires Rust 1.75 or newer. The only dependency that matters is
[crossterm](https://github.com/crossterm-rs/crossterm), which handles the mouse, raw
mode and resize events identically on Windows and Linux.

## License

MIT
