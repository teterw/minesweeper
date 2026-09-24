//! Interchangeable board looks, so a rendering choice can be judged by playing rather
//! than by arguing about it.
//!
//! Cycle them in game with `v`, or start in one with `--style <name>`.

use crossterm::style::Color;

use crate::board::field::CellState;

pub const TILE: char = '#';
pub const FLAG: char = 'F';
pub const BOOM: char = '*';

const TILE_COLOUR: Color = Color::AnsiValue(245);
const DIM: Color = Color::AnsiValue(238);
const SHADE: Color = Color::AnsiValue(236);
const BLOCK_BG: Color = Color::AnsiValue(243);
const RULE: Color = Color::AnsiValue(240);

/// What to put on screen for one board cell.
pub struct Cell {
    /// Exactly `Style::cell_width()` characters, all ASCII.
    pub text: String,
    pub fg: Color,
    pub bg: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Cleared ground is blank. The original.
    Blank,
    /// Cleared ground keeps a dim dot, so every column stays occupied.
    Dots,
    /// A vertical rule after each cell, like a spreadsheet.
    Lines,
    /// Alternating column tint, like ruled paper.
    Shaded,
    /// Unexplored ground is a filled block of colour rather than a glyph.
    Blocks,
    /// One column per cell, no gaps — twice as much board on screen.
    Tight,
}

pub const ALL: [Style; 6] = [
    Style::Blank,
    Style::Dots,
    Style::Lines,
    Style::Shaded,
    Style::Blocks,
    Style::Tight,
];

impl Style {
    pub fn name(self) -> &'static str {
        match self {
            Style::Blank => "blank",
            Style::Dots => "dots",
            Style::Lines => "lines",
            Style::Shaded => "shaded",
            Style::Blocks => "blocks",
            Style::Tight => "tight",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        ALL.into_iter().find(|v| v.name() == s.to_ascii_lowercase())
    }

    /// Terminal columns one cell occupies.
    pub fn cell_width(self) -> i64 {
        match self {
            Style::Lines => 3,
            Style::Tight => 1,
            _ => 2,
        }
    }

    pub fn next(self) -> Self {
        let i = ALL.iter().position(|&s| s == self).unwrap_or(0);
        ALL[(i + 1) % ALL.len()]
    }

    /// The bare glyph and its colour, before any style decoration.
    fn glyph(self, state: CellState) -> (char, Color) {
        match state {
            CellState::OutOfBounds => (' ', Color::Reset),
            CellState::Hidden => (TILE, TILE_COLOUR),
            CellState::Flagged => (FLAG, Color::Red),
            CellState::Detonated => (BOOM, Color::Red),
            CellState::Revealed(0) => match self {
                Style::Dots => ('.', DIM),
                _ => (' ', Color::Reset),
            },
            CellState::Revealed(n) => (
                char::from_digit(n as u32, 10).unwrap_or('?'),
                number_colour(n),
            ),
        }
    }

    pub fn render(self, state: CellState, x: i64, _y: i64) -> Cell {
        let (glyph, fg) = self.glyph(state);

        match self {
            Style::Tight => Cell {
                text: glyph.to_string(),
                fg,
                bg: None,
            },

            Style::Lines => Cell {
                text: format!("{glyph} |"),
                fg,
                bg: None,
            },

            Style::Shaded => Cell {
                text: format!("{glyph} "),
                fg,
                // Tint every other column so the grid reads like ruled paper.
                bg: (x.rem_euclid(2) == 0).then_some(SHADE),
            },

            Style::Blocks => {
                // Unexplored ground becomes a filled background rather than a glyph.
                if state == CellState::Hidden {
                    Cell {
                        text: "  ".into(),
                        fg: BLOCK_BG,
                        bg: Some(BLOCK_BG),
                    }
                } else {
                    Cell {
                        text: format!("{glyph} "),
                        fg,
                        bg: matches!(state, CellState::Flagged).then_some(BLOCK_BG),
                    }
                }
            }

            Style::Blank | Style::Dots => Cell {
                text: format!("{glyph} "),
                fg,
                bg: None,
            },
        }
    }

    /// Colour of the frame drawn around a finite board.
    pub fn rule_colour(self) -> Color {
        RULE
    }
}

/// The palette Minesweeper has used since 1990. Eight is grey because there is no ninth
/// colour anyone remembers.
pub fn number_colour(n: u8) -> Color {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_has_a_unique_name_that_parses_back() {
        let mut seen = std::collections::HashSet::new();
        for s in ALL {
            assert!(seen.insert(s.name()), "duplicate style name {}", s.name());
            assert_eq!(Style::parse(s.name()), Some(s));
            assert_eq!(Style::parse(&s.name().to_uppercase()), Some(s));
        }
        assert_eq!(Style::parse("nonsense"), None);
    }

    #[test]
    fn cycling_visits_every_style_and_returns_to_the_start() {
        let mut s = ALL[0];
        let mut seen = vec![s];
        for _ in 1..ALL.len() {
            s = s.next();
            seen.push(s);
        }
        assert_eq!(s.next(), ALL[0], "cycling did not wrap");
        for style in ALL {
            assert!(seen.contains(&style), "{} was never reached", style.name());
        }
    }

    /// The alignment guarantee: whatever the style, every cell is exactly the same
    /// number of columns, and every character is ASCII so it is one column wide.
    #[test]
    fn every_cell_is_exactly_the_style_width_in_ascii() {
        let mut states = vec![
            CellState::Hidden,
            CellState::Flagged,
            CellState::Detonated,
            CellState::OutOfBounds,
        ];
        for n in 0..=8u8 {
            states.push(CellState::Revealed(n));
        }

        for style in ALL {
            for &state in &states {
                for x in -3..3i64 {
                    let cell = style.render(state, x, 0);
                    assert_eq!(
                        cell.text.chars().count() as i64,
                        style.cell_width(),
                        "style {} rendered {state:?} as {:?}, wrong width",
                        style.name(),
                        cell.text
                    );
                    assert!(
                        cell.text.is_ascii(),
                        "style {} rendered non-ASCII {:?}",
                        style.name(),
                        cell.text
                    );
                }
            }
        }
    }

    #[test]
    fn dots_fills_cleared_ground_and_blank_does_not() {
        let dots = Style::Dots.render(CellState::Revealed(0), 0, 0);
        assert_eq!(dots.text, ". ");
        let blank = Style::Blank.render(CellState::Revealed(0), 0, 0);
        assert_eq!(blank.text, "  ");
    }

    #[test]
    fn shaded_alternates_by_column() {
        let a = Style::Shaded.render(CellState::Hidden, 0, 0);
        let b = Style::Shaded.render(CellState::Hidden, 1, 0);
        assert!(a.bg.is_some());
        assert!(b.bg.is_none());
        // Negative coordinates must alternate too, not mirror around zero.
        let c = Style::Shaded.render(CellState::Hidden, -1, 0);
        assert!(c.bg.is_none(), "the tint did not alternate below zero");
    }

    #[test]
    fn blocks_fills_hidden_ground() {
        let hidden = Style::Blocks.render(CellState::Hidden, 0, 0);
        assert_eq!(hidden.text, "  ");
        assert!(hidden.bg.is_some(), "hidden ground was not filled");
        let cleared = Style::Blocks.render(CellState::Revealed(0), 0, 0);
        assert!(cleared.bg.is_none(), "cleared ground was filled");
    }

    #[test]
    fn lines_puts_a_rule_after_every_cell() {
        for state in [CellState::Hidden, CellState::Revealed(3)] {
            let c = Style::Lines.render(state, 0, 0);
            assert!(c.text.ends_with('|'), "{:?} had no rule", c.text);
        }
    }
}
