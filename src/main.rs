//! Entry point: terminal setup, the event loop, and putting the terminal back.

use std::io::{self, stdout, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
    },
};

use infinisweeper::cli::{Cli, Launch};
use infinisweeper::game::{Game, Mode};
use infinisweeper::input::{map_event, Action};
use infinisweeper::menu::{self, MenuChoice};
use infinisweeper::persist::{self, Scores};
use infinisweeper::render::{self, Style, Viewport};

const AUTOSAVE_EVERY: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(200);

/// Owns the terminal's raw mode and mouse capture. Dropping it puts the terminal back,
/// on every exit path including `?` returning an error early.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = execute!(stdout(), Show, DisableMouseCapture, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// A panic inside raw mode would otherwise leave the shell unusable — no echo, no
/// newlines, mouse reporting still on.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

fn random_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED)
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    install_panic_hook();

    let dir = persist::data_dir();
    let mut scores = persist::load_scores_from(&dir);
    let saved = persist::load_run_from(&dir).map(persist::rebuild);

    let _guard = TerminalGuard::new()?;
    let mut out = stdout();

    let game = match cli.launch() {
        Launch::Menu => {
            let cleared = saved.as_ref().map(|g| g.cleared());
            match menu::run(&mut out, cleared, &scores)? {
                None | Some(MenuChoice::Quit) => return Ok(()),
                Some(MenuChoice::Continue) => {
                    saved.unwrap_or_else(|| Game::new_infinite(random_seed()))
                }
                Some(MenuChoice::Infinite) => Game::new_infinite(random_seed()),
                Some(MenuChoice::Classic(d)) => Game::new_classic(d, random_seed()),
            }
        }
        Launch::Continue => saved.unwrap_or_else(|| Game::new_infinite(random_seed())),
        Launch::Infinite(seed) => Game::new_infinite(seed.unwrap_or_else(random_seed)),
        Launch::Classic(d) => Game::new_classic(d, random_seed()),
    };

    // `dots` by default: the most compact style that still keeps every column marked.
    // Press `v` to cycle to the others.
    let style = cli.style.unwrap_or(Style::Dots);
    play(&mut out, game, style, &dir, &mut scores)
}

fn play<W: Write>(
    out: &mut W,
    mut game: Game,
    mut style: Style,
    dir: &std::path::Path,
    scores: &mut Scores,
) -> io::Result<()> {
    let (mut term_w, mut term_h) = size()?;
    let mut vp = Viewport::with_cell_size(term_w, term_h, style.cell_width(), style.cell_height());
    if let Some(v) = vp.as_mut() {
        v.centre_on(game.cursor.0, game.cursor.1);
    }

    let mut last_save = Instant::now();
    let mut scored = false;

    loop {
        match vp.as_ref() {
            Some(v) => render::draw(out, &game, v, style, term_w, term_h)?,
            None => render::draw_too_small(out, term_w, term_h)?,
        }

        if game.is_over() && !scored {
            scored = true;
            game.pause();
            if scores.record(game.mode, game.cleared(), game.elapsed()) {
                let _ = persist::save_scores_to(dir, scores);
            }
            if game.mode == Mode::Infinite {
                persist::clear_run(dir);
            }
        }

        if !event::poll(POLL)? {
            autosave(&mut game, dir, &mut last_save);
            continue;
        }

        // Mouse mapping needs a viewport; when the window is too small there is no board
        // to click, so a placeholder keeps the keyboard working.
        let map_vp = vp.unwrap_or(Viewport {
            origin_x: 0,
            origin_y: 0,
            cols: 1,
            rows: 1,
            cell_w: style.cell_width(),
            cell_h: style.cell_height(),
        });
        let Some(action) = map_event(&event::read()?, &map_vp) else {
            continue;
        };

        let mut cursor_moved = false;
        match action {
            Action::Quit => {
                if game.mode == Mode::Infinite && !game.is_over() {
                    game.pause();
                    let _ = persist::save_run_to(dir, &game);
                }
                return Ok(());
            }

            Action::Resize(w, h) => {
                term_w = w;
                term_h = h;
                vp = Viewport::with_cell_size(w, h, style.cell_width(), style.cell_height());
                if let Some(v) = vp.as_mut() {
                    v.centre_on(game.cursor.0, game.cursor.1);
                }
            }

            Action::CycleStyle => {
                style = style.next();
                // Cell width can change with the style, so the viewport is rebuilt.
                vp = Viewport::with_cell_size(
                    term_w,
                    term_h,
                    style.cell_width(),
                    style.cell_height(),
                );
                if let Some(v) = vp.as_mut() {
                    v.centre_on(game.cursor.0, game.cursor.1);
                }
            }

            Action::Restart => {
                game = match game.mode {
                    Mode::Infinite => Game::new_infinite(random_seed()),
                    Mode::Classic(d) => Game::new_classic(d, random_seed()),
                };
                scored = false;
                if let Some(v) = vp.as_mut() {
                    v.centre_on(game.cursor.0, game.cursor.1);
                }
                if game.mode == Mode::Infinite {
                    persist::clear_run(dir);
                }
            }

            Action::MoveCursor(dx, dy) => {
                game.move_cursor(dx, dy);
                cursor_moved = true;
            }

            Action::JumpToStart => {
                game.cursor = match game.mode {
                    Mode::Infinite => {
                        infinisweeper::board::infinite::InfiniteField::safe_start(game.seed)
                    }
                    Mode::Classic(d) => {
                        let (w, h, _) = d.dimensions();
                        (w / 2, h / 2)
                    }
                };
                cursor_moved = true;
            }

            Action::Pan(dx, dy) => {
                if let Some(v) = vp.as_mut() {
                    v.origin_x += dx;
                    v.origin_y += dy;
                }
            }

            Action::Reveal(x, y) => {
                game.cursor = (x, y);
                game.reveal(x, y);
                cursor_moved = true;
            }
            Action::Flag(x, y) => {
                game.cursor = (x, y);
                game.toggle_flag(x, y);
                cursor_moved = true;
            }
            Action::Chord(x, y) => {
                game.cursor = (x, y);
                game.chord(x, y);
                cursor_moved = true;
            }

            Action::RevealAtCursor => game.reveal_at_cursor(),
            Action::FlagAtCursor => game.flag_at_cursor(),
            Action::ChordAtCursor => game.chord_at_cursor(),
        }

        // Only chase the cursor when it actually moved, so a deliberate pan stays put.
        if cursor_moved {
            if let Some(v) = vp.as_mut() {
                v.follow(game.cursor.0, game.cursor.1);
            }
        }

        autosave(&mut game, dir, &mut last_save);
    }
}

fn autosave(game: &mut Game, dir: &std::path::Path, last: &mut Instant) {
    if game.mode != Mode::Infinite || game.is_over() || last.elapsed() < AUTOSAVE_EVERY {
        return;
    }
    let _ = persist::save_run_to(dir, game);
    *last = Instant::now();
}
