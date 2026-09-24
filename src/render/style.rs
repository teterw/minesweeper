//! Interchangeable board looks, so a rendering choice can be judged by playing rather
//! than by arguing about it.
//!
//! Cycle them in game with `v`, or start in one with `--style <name>`.
//!
//! Two rules hold for every style, and are enforced by tests:
//!
//! - every cell renders to exactly `cell_width()` ASCII characters, so nothing can
//!   shift a row out of alignment;
//! - cleared ground is never blank. A blank cell leaves nothing marking its column, and
//!   a large cleared area then reads as though the grid has come apart.

use crossterm::style::Color;

use crate::board::field::CellState;

pub const TILE: char = '#';
pub const FLAG: char = 'F';
pub const BOOM: char = '*';
/// Cleared ground. Dim, but never absent — see the module note.
pub const CLEARED: char = '.';

const TILE_COLOUR: Color = Color::AnsiValue(245);
const DIM: Color = Color::AnsiValue(238);
const BLOCK_BG: Color = Color::AnsiValue(243);
const RULE: Color = Color::AnsiValue(240);

/// What to put on screen for one board cell.
pub struct Cell {
    /// Exactly `Style::cell_width()` characters, all ASCII.
    pub text: String,
    pub fg: Color,
    pub bg: Option<Color>,
    /// Which character of `text` is the cell's content, for highlighting the cursor.
    pub glyph_at: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Full boxes: vertical bars and a horizontal rule under every row.
    Grid,
    /// Vertical bars only, content centred between them.
    Boxed,
    /// A vertical rule after each cell.
    Lines,
    /// Two columns per cell, content then a gap.
    Dots,
    /// Unexplored ground is a filled block of colour rather than a glyph.
    Blocks,
    /// One column per cell, no gaps — twice as much board on screen.
    Tight,
}

pub const ALL: [Style; 6] = [
    Style::Grid,
    Style::Boxed,
    Style::Lines,
    Style::Dots,
    Style::Blocks,
    Style::Tight,
];

impl Style {
    pub fn name(self) -> &'static str {
        match self {
            Style::Grid => "grid",
            Style::Boxed => "boxed",
            Style::Lines => "lines",
            Style::Dots => "dots",
            Style::Blocks => "blocks",
            Style::Tight => "tight",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_ascii_lowercase();
        ALL.into_iter().find(|v| v.name() == s)
    }

    /// Terminal columns one cell occupies.
    pub fn cell_width(self) -> i64 {
        match self {
            Style::Grid | Style::Boxed => 4,
            Style::Lines => 3,
            Style::Dots | Style::Blocks => 2,
            Style::Tight => 1,
        }
    }

    /// Terminal rows one cell occupies. Two means a rule is drawn beneath each row.
    pub fn cell_height(self) -> i64 {
        match self {
            Style::Grid => 2,
            _ => 1,
        }
    }

    /// The rule drawn under a cell, for styles that separate rows.
    pub fn row_rule(self) -> Option<&'static str> {
        match self {
            Style::Grid => Some("+---"),
            _ => None,
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
            CellState::Revealed(0) => (CLEARED, DIM),
            CellState::Revealed(n) => (
                char::from_digit(n as u32, 10).unwrap_or('?'),
                number_colour(n),
            ),
        }
    }

    pub fn render(self, state: CellState, _x: i64, _y: i64) -> Cell {
        let (glyph, fg) = self.glyph(state);

        match self {
            Style::Tight => Cell {
                text: glyph.to_string(),
                fg,
                bg: None,
                glyph_at: 0,
            },

            Style::Dots => Cell {
                text: format!("{glyph} "),
                fg,
                bg: None,
                glyph_at: 0,
            },

            Style::Lines => Cell {
                text: format!("{glyph} |"),
                fg,
                bg: None,
                glyph_at: 0,
            },

            // Bar, space, glyph, space — which puts the glyph exactly halfway between
            // this cell's bar and the next one, so numbers sit centred in their box
            // instead of hugging the left edge.
            Style::Grid | Style::Boxed => Cell {
                text: format!("| {glyph} "),
                fg,
                bg: None,
                glyph_at: 2,
            },

            Style::Blocks => {
                if state == CellState::Hidden {
                    Cell {
                        text: "  ".into(),
                        fg: BLOCK_BG,
                        bg: Some(BLOCK_BG),
                        glyph_at: 0,
                    }
                } else {
                    Cell {
                        text: format!("{glyph} "),
                        fg,
                        bg: matches!(state, CellState::Flagged).then_some(BLOCK_BG),
                        glyph_at: 0,
                    }
                }
            }
        }
    }

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

    fn all_states() -> Vec<CellState> {
        let mut v = vec![
            CellState::Hidden,
            CellState::Flagged,
            CellState::Detonated,
            CellState::OutOfBounds,
        ];
        for n in 0..=8u8 {
            v.push(CellState::Revealed(n));
        }
        v
    }

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
        for style in ALL {
            for state in all_states() {
                let cell = style.render(state, 0, 0);
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
                assert!(
                    cell.glyph_at < cell.text.len(),
                    "style {} points glyph_at outside its own text",
                    style.name()
                );
            }
        }
    }

    /// A blank cleared cell leaves nothing marking its column, which is what made the
    /// grid look broken once a large area opened up.
    #[test]
    fn no_style_leaves_cleared_ground_blank() {
        for style in ALL {
            let cell = style.render(CellState::Revealed(0), 0, 0);
            assert!(
                cell.text.trim() != "" || cell.bg.is_some(),
                "style {} draws cleared ground as nothing at all",
                style.name()
            );
            if style != Style::Blocks {
                assert_eq!(
                    cell.text.chars().nth(cell.glyph_at),
                    Some(CLEARED),
                    "style {} does not mark cleared ground",
                    style.name()
                );
            }
        }
    }

    /// The complaint was that numbers did not sit right in their cell in any style but
    /// `tight`. In a boxed cell the glyph must be centred between the bars.
    #[test]
    fn boxed_styles_centre_their_content() {
        for style in [Style::Grid, Style::Boxed] {
            let cell = style.render(CellState::Revealed(3), 0, 0);
            assert_eq!(cell.text, "| 3 ");

            // The bar opens the cell and the next cell's bar closes it, so the content
            // region is everything after index 0. The glyph must sit in the middle of
            // that region, which is what makes numbers look seated in their box.
            let chars: Vec<char> = cell.text.chars().collect();
            assert_eq!(chars[0], '|', "style {} has no opening bar", style.name());
            let region = 1..cell.text.chars().count();
            let middle = (region.start + region.end) / 2;
            assert_eq!(
                cell.glyph_at,
                middle,
                "style {} put the glyph at {} rather than the middle {middle} of its box",
                style.name(),
                cell.glyph_at
            );
        }
    }

    #[test]
    fn only_the_grid_style_rules_between_rows() {
        assert_eq!(Style::Grid.cell_height(), 2);
        assert_eq!(Style::Grid.row_rule(), Some("+---"));
        assert_eq!(
            Style::Grid.row_rule().map(|r| r.len() as i64),
            Some(Style::Grid.cell_width()),
            "the rule must be exactly as wide as the cell it sits under"
        );
        for style in ALL.into_iter().filter(|s| *s != Style::Grid) {
            assert_eq!(
                style.cell_height(),
                1,
                "{} claimed extra rows",
                style.name()
            );
            assert_eq!(style.row_rule(), None);
        }
    }

    #[test]
    fn blocks_fills_hidden_ground() {
        let hidden = Style::Blocks.render(CellState::Hidden, 0, 0);
        assert!(hidden.bg.is_some(), "hidden ground was not filled");
        let cleared = Style::Blocks.render(CellState::Revealed(0), 0, 0);
        assert!(cleared.bg.is_none(), "cleared ground was filled");
    }

    #[test]
    fn tight_is_one_character_per_cell() {
        assert_eq!(Style::Tight.cell_width(), 1);
        assert_eq!(Style::Tight.render(CellState::Hidden, 0, 0).text, "#");
        assert_eq!(Style::Tight.render(CellState::Revealed(0), 0, 0).text, ".");
        assert_eq!(Style::Tight.render(CellState::Revealed(4), 0, 0).text, "4");
    }
}
