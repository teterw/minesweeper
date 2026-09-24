# minesweeper

An infinite Minesweeper for your terminal. No edges, no corners to run out of — just keep clearing.

```
  lives ++-              cleared 1,284                03:12

  # # # # # # # # # # # # # # # # # # # # # # # #
  # # # # # # # # # # 1 1 2 3 # # # # # # # # # #
  # # # # # # # # # # 1     1 # # # # # # # # # #
  # # # # # # # # # # 2     1 # # # # # # # # # #
  # # # # # # # # # # 1     1 2 # # # # # # # # #
  # # # # # # # # # # 1       1 # # F # # # # # #
  # # # # # # # # # # 1 1     1 # # # # # * # # #
  # # # # # # # # # # # 1 1 1 1 # # # # # # # # #
  # # # # # # # # # # # # # # # # # # # # # # # #

  left reveal   right flag   middle chord   arrows move   q quit
```

`#` is unexplored ground, blank is cleared, `F` is a flag and `*` is a mine you set off.
Numbers keep the traditional colours and the cell under the cursor inverts, so the board
is carried by colour rather than by decorative glyphs.

Every character on the board is ASCII on purpose. Symbols like `■` and `⚑` have an
*ambiguous* East Asian width, so whether a terminal gives them one column or two depends
on the font — and one double-width glyph shifts the rest of its row and throws the whole
grid out of alignment.

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

You get **three lives**. Hitting a mine costs one and leaves the mine visible as `*`,
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
| Pan the view    | Scroll wheel  | Cursor reaches the edge |
| Jump to origin  | —             | `O`                  |
| Cycle board style | —           | `V`                  |
| New run         | —             | `R`                  |
| Quit (saves)    | —             | `Q` / `Esc`          |

**Chording** reveals every neighbour of an already-revealed number once you've flagged
exactly that many mines around it. It's how you play fast, and it's the single biggest
difference between a beginner and a good player.

## Board styles

Seven looks, switchable with **`v`** while you play so you can compare them on the same
board, or chosen up front with `--style <name>`. The active one is named in the status
line. Default is `blocks`.

| Style    | Cell size | What it does |
| -------- | --------: | ------------ |
| `blocks` | 3 × 2 | Filled tiles with a narrow space on both axes |
| `tiles`  | 3 × 1 | The same tiles, no space between rows — denser, more board on screen |
| `dots`   | 2 × 1 | A `#` then a gap, no fill |
| `grid`   | 4 × 2 | Boxed cells with a rule under every row |
| `boxed`  | 4 × 1 | Vertical bars only, content centred between them |
| `lines`  | 3 × 1 | A single rule after each cell |
| `tight`  | 1 × 1 | One column per cell, no gaps |

```
blocks  (## is a filled tile)       grid
## ## ## 1  1  2  3  ## ##          | # | # | 1 | 1 | 2 | 3 |
                                    +---+---+---+---+---+---+
## ## ## 1  .  .  1  ## ##          | # | # | 1 | . | . | 1 |
                                    +---+---+---+---+---+---+
## ## ## 2  .  .  1  ## ##          | # | # | 2 | . | . | 1 |
```

A tile needs separating below as much as beside, or tiles in consecutive rows run
together vertically and the grid closes up again. A terminal row is the smallest
vertical step there is, so that gap cannot be made narrower — but it can be made to
*look* narrower by widening the tile. `blocks` runs a two-column tile against a
one-column gap, which reads as a seam; a one-column tile with the same gap is half
space and looks sparse. `tiles` drops the blank row entirely for twice the board.

Three rules hold in every style, and tests enforce all of them:

- **A filled tile never covers its whole cell.** At least one column is always left as a
  gap, or neighbouring tiles touch and the board becomes one mass with no grid to see.
- **Cleared ground is never blank.** An empty cell leaves nothing marking its column, and
  a large cleared area then reads as though the grid has come apart.
- **Every cell is exactly the same number of columns, in ASCII.** Nothing can shift a row
  out of line, in any terminal or font.

Content always starts in the same column its tile does, so numbers line up with the grid
instead of sitting off to one side. In the boxed styles the glyph sits exactly halfway
between one bar and the next.

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

Requires Rust 1.82 or newer. The only dependency that matters is
[crossterm](https://github.com/crossterm-rs/crossterm), which handles the mouse, raw
mode and resize events identically on Windows and Linux.

## License

MIT
