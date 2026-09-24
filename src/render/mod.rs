//! Drawing the board, the status line and the key hints.

pub mod style;
pub mod viewport;

pub use style::Style;
pub use viewport::Viewport;

use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};

use crate::board::field::Rect;
use crate::game::{Game, Mode, Status};

const CURSOR_BG: Color = Color::White;
const CURSOR_FG: Color = Color::Black;

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

/// Lives as ASCII pips — `♥` is ambiguous-width and would shift the status line.
fn hearts(lives: u8, max: u8) -> String {
    let mut s = String::from("lives ");
    for i in 0..max {
        s.push(if i < lives { '+' } else { '-' });
    }
    s
}

/// The characters of a border cell, if this coordinate sits on the frame around a finite
/// board. Without it a classic board dissolves as you clear it, because a cleared cell
/// and the void outside both draw blank.
fn border_at(b: Rect, x: i64, y: i64, cell_w: i64) -> Option<String> {
    let (left, right) = (b.x - 1, b.x + b.w);
    let (top, bottom) = (b.y - 1, b.y + b.h);
    let side = x == left || x == right;
    let cap = y == top || y == bottom;
    let pad = " ".repeat((cell_w - 1).max(0) as usize);

    if side && cap {
        Some(if x == left {
            format!("{pad}+")
        } else {
            format!("+{pad}")
        })
    } else if cap && b.contains(x, b.y) {
        Some("-".repeat(cell_w as usize))
    } else if side && y >= b.y && y < b.y + b.h {
        Some(if x == left {
            format!("{pad}|")
        } else {
            format!("|{pad}")
        })
    } else {
        None
    }
}

/// The visible board as plain text, one line per row, for previews and debugging.
pub fn plain_text(game: &Game, vp: &Viewport, style: Style) -> String {
    let bounds = game.field.bounds();
    let mut out = String::new();
    for dy in 0..vp.rows {
        for dx in 0..vp.cols {
            let (x, y) = (vp.origin_x + dx, vp.origin_y + dy);
            match bounds.and_then(|b| border_at(b, x, y, vp.cell_w)) {
                Some(s) => out.push_str(&s),
                None => out.push_str(&style.render(game.field.cell(x, y), x, y).text),
            }
        }
        out.push('\n');
        if let Some(rule) = style.row_rule() {
            for _ in 0..vp.cols {
                out.push_str(rule);
            }
            out.push('\n');
        }
    }
    out
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

fn draw_status<W: Write>(out: &mut W, game: &Game, style: Style, term_w: u16) -> io::Result<()> {
    let left = match game.mode {
        Mode::Infinite => hearts(game.lives, crate::game::INFINITE_LIVES),
        Mode::Classic(_) => {
            let total = game.field.mine_total().unwrap_or(0);
            let flags = game.field.flags_placed();
            format!("mines {}", total as i64 - flags as i64)
        }
    };
    let middle = format!("cleared {}", thousands(game.cleared()));
    let style_tag = format!("[{}]", style.name());
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
        ResetColor,
        Print("   "),
        SetForegroundColor(Color::Yellow),
        Print(&style_tag),
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
        Status::Playing => "left reveal  right flag  middle chord  v style  r restart  q quit",
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
    style: Style,
    term_w: u16,
    term_h: u16,
) -> io::Result<()> {
    draw_status(out, game, style, term_w)?;
    let bounds = game.field.bounds();

    for (x, y) in vp.visible_cells() {
        let Some((col, row)) = vp.to_screen(x, y) else {
            continue;
        };

        // A finite board gets a frame, so its edges stay visible once cleared.
        if let Some(edge) = bounds.and_then(|b| border_at(b, x, y, vp.cell_w)) {
            queue!(
                out,
                MoveTo(col, row),
                SetForegroundColor(style.rule_colour()),
                Print(edge),
                ResetColor
            )?;
            continue;
        }

        let cell = style.render(game.field.cell(x, y), x, y);
        let is_cursor = (x, y) == game.cursor;
        let chars: Vec<char> = cell.text.chars().collect();
        let gi = cell.glyph_at;

        queue!(out, MoveTo(col, row))?;

        // Character by character, because a cell's background may cover only part of it.
        // That partial fill is what keeps tiles apart: fill the whole cell and
        // neighbouring tiles merge into one mass with no grid left to see.
        for (i, ch) in chars.iter().enumerate() {
            let content = i == gi;
            let filled = cell.bg_span.contains(&i);

            if is_cursor && content {
                queue!(
                    out,
                    SetBackgroundColor(CURSOR_BG),
                    SetForegroundColor(CURSOR_FG)
                )?;
            } else {
                if let Some(bg) = cell.bg.filter(|_| filled) {
                    queue!(out, SetBackgroundColor(bg))?;
                }
                queue!(
                    out,
                    SetForegroundColor(if content {
                        cell.fg
                    } else {
                        style.rule_colour()
                    })
                )?;
            }
            queue!(out, Print(ch), ResetColor)?;
        }

        // Styles that separate rows draw their rule directly beneath the cell.
        if let Some(rule) = style.row_rule() {
            queue!(
                out,
                MoveTo(col, row + 1),
                SetForegroundColor(style.rule_colour()),
                Print(rule),
                ResetColor
            )?;
        }
    }

    draw_footer(out, game, term_h)?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::classic::Difficulty;
    use crate::board::field::CellState;

    fn vp(w: u16, h: u16, style: Style) -> Viewport {
        Viewport::with_cell_size(w, h, style.cell_width(), style.cell_height()).expect("should fit")
    }

    /// Replays what `draw` actually sends to the terminal into a character grid, by
    /// interpreting the cursor-positioning and clear escapes. Tests on `plain_text` check
    /// the model; this checks the bytes the terminal receives, where an alignment bug
    /// would actually live.
    fn replay(stream: &str, w: u16, h: u16) -> Vec<String> {
        let mut grid = vec![vec![' '; w as usize]; h as usize];
        let (mut cx, mut cy) = (0usize, 0usize);
        let mut it = stream.chars().peekable();

        while let Some(c) = it.next() {
            if c == '\x1b' {
                if it.peek() == Some(&'[') {
                    it.next();
                }
                let mut params = String::new();
                let mut final_byte = ' ';
                for c2 in it.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        final_byte = c2;
                        break;
                    }
                    params.push(c2);
                }
                match final_byte {
                    'H' => {
                        let mut p = params.split(';');
                        let row: usize = p.next().unwrap_or("1").parse().unwrap_or(1);
                        let col: usize = p.next().unwrap_or("1").parse().unwrap_or(1);
                        cy = row.saturating_sub(1);
                        cx = col.saturating_sub(1);
                    }
                    'K' => {
                        if let Some(line) = grid.get_mut(cy) {
                            line.iter_mut().for_each(|ch| *ch = ' ');
                        }
                    }
                    'J' => grid
                        .iter_mut()
                        .for_each(|l| l.iter_mut().for_each(|ch| *ch = ' ')),
                    _ => {}
                }
                continue;
            }
            if c == '\n' {
                cy += 1;
                cx = 0;
                continue;
            }
            if let Some(line) = grid.get_mut(cy) {
                if let Some(slot) = line.get_mut(cx) {
                    *slot = c;
                }
            }
            cx += 1;
        }
        grid.into_iter().map(|r| r.into_iter().collect()).collect()
    }

    fn screen(game: &Game, v: &Viewport, style: Style, w: u16, h: u16) -> Vec<String> {
        let mut buf = Vec::new();
        draw(&mut buf, game, v, style, w, h).unwrap();
        replay(&String::from_utf8_lossy(&buf), w, h)
    }

    /// A board with a cleared region, a detonated mine and a flag all in view — the
    /// conditions a style actually has to cope with.
    fn demo_game() -> Game {
        let mut g = Game::new_infinite(20260924);
        g.reveal_at_cursor();
        let start = g.cursor;
        let mine = (start.0 - 8..start.0 + 8)
            .flat_map(|x| (start.1 - 4..start.1 + 4).map(move |y| (x, y)))
            .find(|&(x, y)| g.field.is_mine(x, y))
            .expect("a mine must exist nearby");
        g.reveal(mine.0, mine.1);
        g.toggle_flag(mine.0, mine.1 + 1);
        // Keep the view on the cleared region, which is what the styles differ over.
        g.cursor = start;
        g
    }

    /// Renders a board showing filled background as `#`, since a plain-text replay
    /// cannot show colour and a filled tile would otherwise look like empty space.
    fn debug_text(game: &Game, vp: &Viewport, style: Style) -> String {
        let mut out = String::new();
        for dy in 0..vp.rows {
            for dx in 0..vp.cols {
                let (x, y) = (vp.origin_x + dx, vp.origin_y + dy);
                let cell = style.render(game.field.cell(x, y), x, y);
                for (i, ch) in cell.text.chars().enumerate() {
                    let filled = cell.bg.is_some() && cell.bg_span.contains(&i);
                    out.push(if filled && ch == ' ' { '#' } else { ch });
                }
            }
            out.push('\n');
            if let Some(rule) = style.row_rule() {
                for _ in 0..vp.cols {
                    out.push_str(rule);
                }
                out.push('\n');
            }
        }
        out
    }

    /// `cargo test preview_styles -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn preview_styles() {
        let (w, h) = (44u16, 14u16);
        let g = demo_game();
        for style in style::ALL {
            let mut v = vp(w, h, style);
            v.centre_on(g.cursor.0, g.cursor.1);
            println!(
                "\n=== {} ({} cols per cell) ===",
                style.name(),
                style.cell_width()
            );
            for line in debug_text(&g, &v, style).lines().take(9) {
                println!("|{line}|");
            }
        }
    }

    #[test]
    fn thousands_separates_correctly() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1284), "1,284");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn the_clock_formats_as_minutes_and_seconds() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(192), "03:12");
    }

    #[test]
    fn hearts_show_lives_remaining() {
        assert_eq!(hearts(3, 3), "lives +++");
        assert_eq!(hearts(2, 3), "lives ++-");
        assert_eq!(hearts(0, 3), "lives ---");
    }

    /// The real alignment check: replay the bytes and confirm each row of the board is
    /// the same length and every cell starts on a multiple of the cell width.
    #[test]
    fn every_style_produces_an_aligned_grid() {
        let (w, h) = (44u16, 16u16);
        let g = demo_game();
        for style in style::ALL {
            let mut v = vp(w, h, style);
            v.centre_on(g.cursor.0, g.cursor.1);
            let rows = screen(&g, &v, style, w, h);

            let board: Vec<&String> = rows
                .iter()
                .skip(viewport::HEADER_ROWS as usize)
                .take(v.rows as usize)
                .collect();

            let widths: std::collections::HashSet<usize> =
                board.iter().map(|r| r.chars().count()).collect();
            assert_eq!(
                widths.len(),
                1,
                "style {} produced rows of differing width: {widths:?}",
                style.name()
            );

            // Every cell must land exactly where the viewport says it does.
            for (x, y) in v.visible_cells() {
                let (col, _) = v.to_screen(x, y).unwrap();
                assert_eq!(
                    col as i64 % v.cell_w,
                    0,
                    "style {} put a cell off the column grid",
                    style.name()
                );
            }
        }
    }

    #[test]
    fn the_status_line_names_the_active_style() {
        let g = demo_game();
        for style in style::ALL {
            let v = vp(60, 16, style);
            let rows = screen(&g, &v, style, 60, 16);
            assert!(
                rows[0].contains(&format!("[{}]", style.name())),
                "style {} was not named in the status line: {:?}",
                style.name(),
                rows[0]
            );
        }
    }

    #[test]
    fn a_finite_board_keeps_a_visible_edge_once_cleared() {
        let mut g = Game::new_classic(Difficulty::Beginner, 4242);
        for y in 0..9 {
            for x in 0..9 {
                if !g.field.is_mine(x, y) {
                    g.reveal(x, y);
                }
            }
        }
        for style in style::ALL {
            // Tall and wide enough that the 9x9 board and its frame fit even in the
            // styles that spend two rows or four columns on every cell.
            let mut v = vp(60, 32, style);
            v.centre_on(4, 4);
            let text = plain_text(&g, &v, style);
            assert!(
                text.contains('+') && text.contains('|') && text.contains('-'),
                "style {} drew no frame around a cleared board",
                style.name()
            );
        }
    }

    #[test]
    fn the_infinite_board_never_draws_a_frame() {
        let mut g = Game::new_infinite(1);
        g.reveal_at_cursor();
        let v = vp(40, 18, Style::Dots);
        let text = plain_text(&g, &v, Style::Dots);
        assert!(
            !text.contains('+') && !text.contains('|'),
            "an endless board drew a border it has no edges for"
        );
    }

    #[test]
    fn the_frame_surrounds_the_board_without_covering_it() {
        let b = Rect {
            x: 0,
            y: 0,
            w: 9,
            h: 9,
        };
        assert_eq!(border_at(b, -1, -1, 2).as_deref(), Some(" +"));
        assert_eq!(border_at(b, 9, -1, 2).as_deref(), Some("+ "));
        assert_eq!(border_at(b, 4, -1, 2).as_deref(), Some("--"));
        assert_eq!(border_at(b, -1, 4, 2).as_deref(), Some(" |"));
        assert_eq!(border_at(b, 9, 4, 2).as_deref(), Some("| "));
        // Widths other than two keep the frame the same size as a cell.
        assert_eq!(border_at(b, 4, -1, 3).as_deref(), Some("---"));
        assert_eq!(border_at(b, -1, 4, 3).as_deref(), Some("  |"));
        assert_eq!(border_at(b, -1, 4, 1).as_deref(), Some("|"));
        for y in 0..9 {
            for x in 0..9 {
                assert_eq!(border_at(b, x, y, 2), None, "the frame covered ({x},{y})");
            }
        }
    }

    #[test]
    fn plain_text_rows_match_the_viewport_width() {
        let mut g = Game::new_infinite(5);
        g.reveal_at_cursor();
        for style in style::ALL {
            let v = vp(40, 20, style);
            let text = plain_text(&g, &v, style);
            let lines: Vec<_> = text.lines().collect();
            // Styles that rule between rows emit a line per cell row plus a rule line.
            assert_eq!(
                lines.len() as i64,
                v.rows * v.cell_h,
                "style {} produced the wrong number of lines",
                style.name()
            );
            for line in lines {
                assert_eq!(
                    line.chars().count() as i64,
                    v.cols * v.cell_w,
                    "style {} produced a row of the wrong width",
                    style.name()
                );
            }
        }
    }

    #[test]
    fn hidden_and_cleared_cells_look_different_in_every_style() {
        for style in style::ALL {
            let hidden = style.render(CellState::Hidden, 0, 0);
            let cleared = style.render(CellState::Revealed(0), 0, 0);
            assert!(
                hidden.text != cleared.text || hidden.bg != cleared.bg,
                "style {} draws hidden and cleared ground identically",
                style.name()
            );
        }
    }

    #[test]
    fn drawing_survives_every_reasonable_terminal_size() {
        let mut g = Game::new_infinite(2);
        g.reveal_at_cursor();
        for style in style::ALL {
            for w in [20u16, 40, 81, 200] {
                for h in [10u16, 24, 60] {
                    let v = vp(w, h, style);
                    let mut buf = Vec::new();
                    draw(&mut buf, &g, &v, style, w, h).unwrap();
                }
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
        let v = vp(80, 24, Style::Dots);
        let mut buf = Vec::new();
        draw(&mut buf, &g, &v, Style::Dots, 80, 24).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("out of lives"));
    }
}
