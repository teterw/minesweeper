//! The interface both modes implement.
//!
//! Game, render and input code talk only to `Field`, so the infinite board and the
//! classic 9×9 share everything above the board itself. The only thing they disagree
//! about is [`Field::bounds`].

/// What the player sees in a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellState {
    Hidden,
    Flagged,
    /// Revealed, carrying its adjacent-mine count (0 renders blank).
    Revealed(u8),
    /// A mine the player set off. Stays visible for the rest of the run.
    Detonated,
    /// Outside a finite board. The infinite field never returns this.
    OutOfBounds,
}

/// What happened when the player acted on a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealOutcome {
    /// Newly revealed this many cells.
    Cleared(usize),
    /// Hit a mine. The caller decides what that costs.
    Mine,
    /// Nothing happened — already revealed, flagged, or out of bounds.
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl Rect {
    pub fn contains(&self, x: i64, y: i64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// The eight neighbours of a cell, in reading order.
pub const NEIGHBOURS: [(i64, i64); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

pub trait Field {
    fn cell(&self, x: i64, y: i64) -> CellState;
    fn is_mine(&self, x: i64, y: i64) -> bool;
    fn adjacent_mines(&self, x: i64, y: i64) -> u8;
    fn reveal(&mut self, x: i64, y: i64) -> RevealOutcome;
    fn toggle_flag(&mut self, x: i64, y: i64);
    fn chord(&mut self, x: i64, y: i64) -> RevealOutcome;

    /// `None` for the infinite field, `Some` for classic boards.
    fn bounds(&self) -> Option<Rect>;

    /// Infinite runs are never won; they only end.
    fn is_won(&self) -> bool;

    /// Cells revealed so far. Read from the board rather than accumulated by the caller,
    /// so a chord that reveals several cells at once can't drift out of sync.
    fn cleared_count(&self) -> usize;

    /// Mines remaining by the flag count, for the status line. Infinite mode has no
    /// total to count down from, so it reports flags placed instead.
    fn flags_placed(&self) -> usize;

    /// Total mines on the board, if that is a knowable number.
    fn mine_total(&self) -> Option<usize>;
}
