//! Which part of an endless board is on screen, and where a click landed.

/// Cells are two terminal columns wide, because terminal cells are about twice as tall
/// as they are wide and a one-column grid reads as badly squashed.
pub const CELL_W: i64 = 2;

/// Rows reserved above the board for the status line, and below it for the key hints.
pub const HEADER_ROWS: i64 = 2;
pub const FOOTER_ROWS: i64 = 2;

/// How close the cursor may get to an edge before the view scrolls.
pub const MARGIN: i64 = 3;

/// Below this there isn't room for a board plus its chrome.
pub const MIN_COLS: u16 = 20;
pub const MIN_ROWS: u16 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// World coordinate of the leftmost visible column.
    pub origin_x: i64,
    /// World coordinate of the topmost visible row.
    pub origin_y: i64,
    pub cols: i64,
    pub rows: i64,
}

impl Viewport {
    /// `None` when the terminal is too small to draw a board in — the caller shows a
    /// notice instead. Checking this before any arithmetic is what keeps a 15×5 window
    /// from underflowing the row count.
    pub fn new(term_w: u16, term_h: u16) -> Option<Self> {
        if term_w < MIN_COLS || term_h < MIN_ROWS {
            return None;
        }
        let cols = term_w as i64 / CELL_W;
        let rows = term_h as i64 - HEADER_ROWS - FOOTER_ROWS;
        if cols < 1 || rows < 1 {
            return None;
        }
        Some(Self {
            origin_x: 0,
            origin_y: 0,
            cols,
            rows,
        })
    }

    pub fn centre_on(&mut self, x: i64, y: i64) {
        self.origin_x = x - self.cols / 2;
        self.origin_y = y - self.rows / 2;
    }

    /// Scrolls only far enough to keep the cursor `MARGIN` cells from every edge. On a
    /// viewport too small for that margin, it shrinks rather than fighting itself.
    pub fn follow(&mut self, x: i64, y: i64) {
        let mx = MARGIN.min((self.cols - 1) / 2);
        let my = MARGIN.min((self.rows - 1) / 2);

        if x < self.origin_x + mx {
            self.origin_x = x - mx;
        } else if x > self.origin_x + self.cols - 1 - mx {
            self.origin_x = x - self.cols + 1 + mx;
        }

        if y < self.origin_y + my {
            self.origin_y = y - my;
        } else if y > self.origin_y + self.rows - 1 - my {
            self.origin_y = y - self.rows + 1 + my;
        }
    }

    pub fn contains(&self, x: i64, y: i64) -> bool {
        x >= self.origin_x
            && y >= self.origin_y
            && x < self.origin_x + self.cols
            && y < self.origin_y + self.rows
    }

    /// Terminal column and row for a world cell, if it's on screen.
    pub fn to_screen(&self, x: i64, y: i64) -> Option<(u16, u16)> {
        if !self.contains(x, y) {
            return None;
        }
        let col = (x - self.origin_x) * CELL_W;
        let row = y - self.origin_y + HEADER_ROWS;
        Some((col as u16, row as u16))
    }

    /// World cell under a terminal column and row, if the click was on the board.
    pub fn to_world(&self, col: u16, row: u16) -> Option<(i64, i64)> {
        let row = row as i64;
        if row < HEADER_ROWS || row >= HEADER_ROWS + self.rows {
            return None;
        }
        let x = self.origin_x + col as i64 / CELL_W;
        let y = self.origin_y + row - HEADER_ROWS;
        Some((x, y))
    }

    pub fn visible_cells(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        let (ox, oy, cols, rows) = (self.origin_x, self.origin_y, self.cols, self.rows);
        (0..rows).flat_map(move |dy| (0..cols).map(move |dx| (ox + dx, oy + dy)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp(w: u16, h: u16) -> Viewport {
        Viewport::new(w, h).expect("should fit")
    }

    /// Review Focus 2. A window smaller than the chrome must be refused, not computed
    /// with — `term_h - HEADER - FOOTER` underflows on a 5-row terminal.
    #[test]
    fn a_terminal_too_small_is_refused_rather_than_computed() {
        assert!(Viewport::new(15, 5).is_none());
        assert!(Viewport::new(80, 4).is_none());
        assert!(Viewport::new(10, 40).is_none());
        assert!(Viewport::new(0, 0).is_none());
        assert!(Viewport::new(1, 1).is_none());
        assert!(Viewport::new(MIN_COLS, MIN_ROWS).is_some(), "the minimum should fit");
    }

    #[test]
    fn no_terminal_size_panics() {
        for w in 0..120u16 {
            for h in 0..60u16 {
                if let Some(v) = Viewport::new(w, h) {
                    assert!(v.cols >= 1 && v.rows >= 1);
                    let mut v = v;
                    v.follow(0, 0);
                    v.follow(-10_000, 10_000);
                }
            }
        }
    }

    #[test]
    fn a_cursor_well_inside_the_view_does_not_scroll_it() {
        let mut v = vp(80, 24);
        let before = v;
        v.follow(v.origin_x + v.cols / 2, v.origin_y + v.rows / 2);
        assert_eq!(v, before, "the view scrolled when it did not need to");
    }

    #[test]
    fn crossing_the_margin_scrolls_just_enough() {
        let mut v = vp(80, 24);
        v.follow(v.origin_x + v.cols, v.origin_y);
        assert!(
            v.contains(v.origin_x + v.cols - 1, v.origin_y),
            "the view did not follow the cursor"
        );
    }

    #[test]
    fn the_cursor_stays_clear_of_every_edge() {
        let mut v = vp(80, 24);
        for &(x, y) in &[(0i64, 0i64), (200, 50), (-200, -50), (0, -30), (17, 3)] {
            v.follow(x, y);
            assert!(v.contains(x, y), "cursor ({x},{y}) left the view");
            assert!(x - v.origin_x >= MARGIN, "cursor too close to the left edge");
            assert!(y - v.origin_y >= MARGIN, "cursor too close to the top edge");
            assert!(
                v.origin_x + v.cols - 1 - x >= MARGIN,
                "cursor too close to the right edge"
            );
            assert!(
                v.origin_y + v.rows - 1 - y >= MARGIN,
                "cursor too close to the bottom edge"
            );
        }
    }

    #[test]
    fn screen_and_world_round_trip() {
        let mut v = vp(80, 24);
        v.centre_on(1000, -1000);
        for (x, y) in v.visible_cells().take(200) {
            let (col, row) = v.to_screen(x, y).expect("visible cells must map to screen");
            assert_eq!(
                v.to_world(col, row),
                Some((x, y)),
                "({x},{y}) did not survive the round trip"
            );
        }
    }

    #[test]
    fn round_trips_correctly_in_negative_space() {
        let mut v = vp(80, 24);
        v.centre_on(-5, -5);
        for (x, y) in v.visible_cells() {
            let (col, row) = v.to_screen(x, y).unwrap();
            assert_eq!(v.to_world(col, row), Some((x, y)), "failed at ({x},{y})");
        }
    }

    #[test]
    fn clicks_on_the_header_and_footer_are_not_board_clicks() {
        let v = vp(80, 24);
        assert_eq!(v.to_world(10, 0), None, "a click on the status line hit the board");
        assert_eq!(v.to_world(10, 1), None);
        assert!(v.to_world(10, HEADER_ROWS as u16).is_some());
        let last = (HEADER_ROWS + v.rows - 1) as u16;
        assert!(v.to_world(10, last).is_some());
        assert_eq!(v.to_world(10, last + 1), None, "a click below the board hit it");
    }

    #[test]
    fn both_columns_of_a_cell_select_that_cell() {
        let v = vp(80, 24);
        let a = v.to_world(4, 5);
        let b = v.to_world(5, 5);
        assert_eq!(a, b, "the two halves of a cell selected different cells");
        assert_ne!(a, v.to_world(6, 5), "adjacent cells collapsed into one");
    }

    #[test]
    fn visible_cells_covers_the_whole_view_exactly_once() {
        let v = vp(40, 20);
        let cells: Vec<_> = v.visible_cells().collect();
        assert_eq!(cells.len() as i64, v.cols * v.rows);
        let unique: std::collections::HashSet<_> = cells.iter().collect();
        assert_eq!(unique.len(), cells.len(), "a cell was listed twice");
    }
}
