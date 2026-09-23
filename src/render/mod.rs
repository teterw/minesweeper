//! Drawing the board, the status line and the key hints.

pub mod viewport;

pub use viewport::Viewport;

use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};

use crate::board::field::CellState;
use crate::game::{Game, Mode, Status};

const HIDDEN: char = '▓';
const EMPTY: char = '·';
const FLAG: char = '⚑';
const BOOM: char = '✹';

/// The palette Minesweeper has used since 1990. Eight is grey because there is no ninth
/// colour anyone remembers.
fn number_colour(n: u8) -> Color {
    match n {
        1 => Color::Blue,
        2 => Color::Green,
        3 => Color::Red,
        4 => Color::DarkBlue,
        5 => Color::DarkRed,
        6 => Color::Cyan,
        7 => Color::Magenta,
        _ => Color::Grey,
    }
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn clock(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn hearts(lives: u8, max: u8) -> String {
    let mut s = String::new();
    for i in 0..max {
        s.push(if i < lives { '♥' } else { '♡' });
        s.push(' ');
    }
    s.trim_end().to_string()
}

/// Shown instead of a board when the window is too small to hold one.
pub fn draw_too_small<W: Write>(out: &mut W, term_w: u16, term_h: u16) -> io::Result<()> {
    queue!(out, Clear(ClearType::All), MoveTo(0, 0), ResetColor)?;
    let lines = [
        "terminal too small".to_string(),
        format!("{term_w} x {term_h}"),
        format!(
            "need at least {} x {}",
            viewport::MIN_COLS,
            viewport::MIN_ROWS
        ),
    ];
    for (i, line) in lines.iter().enumerate() {
        queue!(out, MoveTo(0, i as u16), Print(line))?;
    }
    out.flush()
}

fn draw_status<W: Write>(out: &mut W, game: &Game, term_w: u16) -> io::Result<()> {
    let left = match game.mode {
        Mode::Infinite => hearts(game.lives, crate::game::INFINITE_LIVES),
        Mode::Classic(_) => {
            let total = game.field.mine_total().unwrap_or(0);
            let flags = game.field.flags_placed();
            format!("mines {}", total as i64 - flags as i64)
        }
    };
    let middle = format!("cleared {}", thousands(game.cleared()));
    let right = clock(game.elapsed().as_secs());

    queue!(
        out,
        MoveTo(0, 0),
        Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Red),
        Print(&left),
        ResetColor,
        Print("   "),
        SetForegroundColor(Color::Grey),
        Print(&middle),
        ResetColor
    )?;

    let right_col = term_w.saturating_sub(right.len() as u16 + 1);
    queue!(
        out,
        MoveTo(right_col, 0),
        SetForegroundColor(Color::Grey),
        Print(&right),
        ResetColor,
        MoveTo(0, 1),
        Clear(ClearType::CurrentLine)
    )
}

fn draw_footer<W: Write>(out: &mut W, game: &Game, term_h: u16) -> io::Result<()> {
    let row = term_h.saturating_sub(1);
    let hint = match game.status {
        Status::Playing => {
            "left reveal  right flag  middle chord  ↑↓←→ move  r restart  q quit"
        }
        Status::Won => "you cleared it!   r play again   q quit",
        Status::Lost => "out of lives.   r new run   q quit",
    };
    let colour = match game.status {
        Status::Playing => Color::DarkGrey,
        Status::Won => Color::Green,
        Status::Lost => Color::Red,
    };
    queue!(
        out,
        MoveTo(0, row.saturating_sub(1)),
        Clear(ClearType::CurrentLine),
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        SetForegroundColor(colour),
        Print(hint),
        ResetColor
    )
}

pub fn draw<W: Write>(
    out: &mut W,
    game: &Game,
    vp: &Viewport,
    term_w: u16,
    term_h: u16,
) -> io::Result<()> {
    draw_status(out, game, term_w)?;

    for (x, y) in vp.visible_cells() {
        let Some((col, row)) = vp.to_screen(x, y) else {
            continue;
        };
        let state = game.field.cell(x, y);
        let is_cursor = (x, y) == game.cursor;

        let (glyph, fg) = match state {
            CellState::OutOfBounds => (' ', Color::Reset),
            CellState::Hidden => (HIDDEN, Color::DarkGrey),
            CellState::Flagged => (FLAG, Color::Red),
            CellState::Detonated => (BOOM, Color::Red),
            CellState::Revealed(0) => (EMPTY, Color::DarkGrey),
            CellState::Revealed(n) => (
                char::from_digit(n as u32, 10).unwrap_or('?'),
                number_colour(n),
            ),
        };

        queue!(out, MoveTo(col, row))?;
        if is_cursor {
            queue!(out, SetBackgroundColor(Color::DarkGrey))?;
        }
        queue!(out, SetForegroundColor(fg), Print(glyph))?;
        queue!(out, Print(' '), ResetColor)?;
    }

    draw_footer(out, game, term_h)?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::classic::Difficulty;

    #[test]
    fn thousands_separates_correctly() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1284), "1,284");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn the_clock_formats_as_minutes_and_seconds() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(42), "00:42");
        assert_eq!(clock(192), "03:12");
        assert_eq!(clock(3600), "60:00");
    }

    #[test]
    fn hearts_show_lives_remaining() {
        assert_eq!(hearts(3, 3), "♥ ♥ ♥");
        assert_eq!(hearts(2, 3), "♥ ♥ ♡");
        assert_eq!(hearts(0, 3), "♡ ♡ ♡");
    }

    #[test]
    fn every_number_has_a_colour_and_none_panics() {
        for n in 0..=8u8 {
            let _ = number_colour(n);
        }
    }

    #[test]
    fn drawing_an_infinite_board_writes_something_and_does_not_panic() {
        let mut g = Game::new_infinite(1);
        g.reveal_at_cursor();
        let vp = Viewport::new(80, 24).unwrap();
        let mut buf = Vec::new();
        draw(&mut buf, &g, &vp, 80, 24).unwrap();
        assert!(!buf.is_empty());
    }

    #[test]
    fn drawing_a_classic_board_handles_out_of_bounds_cells() {
        // A 9x9 board in an 80x24 viewport means most visible cells are off the board.
        let mut g = Game::new_classic(Difficulty::Beginner, 1);
        g.reveal(4, 4);
        let mut vp = Viewport::new(80, 24).unwrap();
        vp.centre_on(4, 4);
        let mut buf = Vec::new();
        draw(&mut buf, &g, &vp, 80, 24).unwrap();
        assert!(!buf.is_empty());
    }

    #[test]
    fn drawing_survives_every_reasonable_terminal_size() {
        let mut g = Game::new_infinite(2);
        g.reveal_at_cursor();
        for w in [20u16, 40, 80, 200] {
            for h in [10u16, 24, 60] {
                let vp = Viewport::new(w, h).unwrap();
                let mut buf = Vec::new();
                draw(&mut buf, &g, &vp, w, h).unwrap();
            }
        }
    }

    #[test]
    fn the_too_small_notice_renders_without_a_viewport() {
        let mut buf = Vec::new();
        draw_too_small(&mut buf, 15, 5).unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("too small"));
        assert!(text.contains("15 x 5"));
    }

    #[test]
    fn a_finished_game_draws_its_ending() {
        let mut g = Game::new_infinite(3);
        g.reveal_at_cursor();
        g.status = Status::Lost;
        let vp = Viewport::new(80, 24).unwrap();
        let mut buf = Vec::new();
        draw(&mut buf, &g, &vp, 80, 24).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("out of lives"));
    }
}
