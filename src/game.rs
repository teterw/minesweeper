//! Run state: which field is in play, how many lives are left, how long it's been.
//!
//! Everything here is expressed against the `Field` trait, so infinite and classic runs
//! differ only in how they're constructed and how many lives they start with.

use std::time::{Duration, Instant};

use crate::board::classic::{ClassicField, Difficulty};
use crate::board::field::{CellState, Field, RevealOutcome};
use crate::board::infinite::InfiniteField;

/// An endless board would be miserable with one life, and trivial with unlimited ones.
pub const INFINITE_LIVES: u8 = 3;
pub const CLASSIC_LIVES: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Infinite,
    Classic(Difficulty),
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Infinite => "Infinite",
            Mode::Classic(d) => d.name(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    Lost,
}

pub struct Game {
    pub field: Box<dyn Field>,
    pub mode: Mode,
    pub lives: u8,
    pub cursor: (i64, i64),
    pub status: Status,
    pub seed: u64,
    /// `None` until the first reveal — the clock shouldn't run while you read the menu.
    started: Option<Instant>,
    /// Time carried over from earlier sittings of a resumed run.
    accumulated: Duration,
}

impl Game {
    pub fn new_infinite(seed: u64) -> Self {
        Self {
            cursor: InfiniteField::safe_start(seed),
            field: Box::new(InfiniteField::new(seed)),
            mode: Mode::Infinite,
            lives: INFINITE_LIVES,
            status: Status::Playing,
            seed,
            started: None,
            accumulated: Duration::ZERO,
        }
    }

    pub fn new_classic(difficulty: Difficulty, seed: u64) -> Self {
        let (w, h, _) = difficulty.dimensions();
        Self {
            field: Box::new(ClassicField::new(difficulty, seed)),
            mode: Mode::Classic(difficulty),
            lives: CLASSIC_LIVES,
            cursor: (w / 2, h / 2),
            status: Status::Playing,
            seed,
            started: None,
            accumulated: Duration::ZERO,
        }
    }

    /// Rebuilds a run loaded from disk. The clock stays paused until the next action.
    pub fn restored(
        field: Box<dyn Field>,
        mode: Mode,
        seed: u64,
        lives: u8,
        cursor: (i64, i64),
        elapsed: Duration,
    ) -> Self {
        Self {
            field,
            mode,
            lives,
            cursor,
            status: Status::Playing,
            seed,
            started: None,
            accumulated: elapsed,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.accumulated + self.started.map_or(Duration::ZERO, |s| s.elapsed())
    }

    pub fn timer_running(&self) -> bool {
        self.started.is_some()
    }

    pub fn cleared(&self) -> usize {
        self.field.cleared_count()
    }

    pub fn is_over(&self) -> bool {
        self.status != Status::Playing
    }

    fn start_clock(&mut self) {
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
    }

    fn stop_clock(&mut self) {
        if let Some(s) = self.started.take() {
            self.accumulated += s.elapsed();
        }
    }

    fn apply(&mut self, outcome: RevealOutcome) {
        if outcome == RevealOutcome::Mine {
            self.lives = self.lives.saturating_sub(1);
            if self.lives == 0 {
                self.status = Status::Lost;
                self.stop_clock();
                return;
            }
        }
        if self.field.is_won() {
            self.status = Status::Won;
            self.stop_clock();
        }
    }

    pub fn reveal(&mut self, x: i64, y: i64) {
        if self.is_over() {
            return;
        }
        self.start_clock();
        let outcome = self.field.reveal(x, y);
        self.apply(outcome);
    }

    pub fn toggle_flag(&mut self, x: i64, y: i64) {
        if self.is_over() {
            return;
        }
        self.start_clock();
        self.field.toggle_flag(x, y);
    }

    pub fn chord(&mut self, x: i64, y: i64) {
        if self.is_over() {
            return;
        }
        self.start_clock();
        let outcome = self.field.chord(x, y);
        self.apply(outcome);
    }

    pub fn reveal_at_cursor(&mut self) {
        let (x, y) = self.cursor;
        self.reveal(x, y);
    }

    pub fn flag_at_cursor(&mut self) {
        let (x, y) = self.cursor;
        self.toggle_flag(x, y);
    }

    pub fn chord_at_cursor(&mut self) {
        let (x, y) = self.cursor;
        self.chord(x, y);
    }

    /// Moves the cursor, clamped to the board in classic mode and free in infinite mode.
    pub fn move_cursor(&mut self, dx: i64, dy: i64) {
        let (mut x, mut y) = self.cursor;
        x = x.saturating_add(dx);
        y = y.saturating_add(dy);
        if let Some(b) = self.field.bounds() {
            x = x.clamp(b.x, b.x + b.w - 1);
            y = y.clamp(b.y, b.y + b.h - 1);
        }
        self.cursor = (x, y);
    }

    pub fn cell_at_cursor(&self) -> CellState {
        self.field.cell(self.cursor.0, self.cursor.1)
    }

    /// Called before saving, so the stored elapsed time includes this sitting.
    pub fn pause(&mut self) {
        self.stop_clock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::field::CellState;

    fn safe_cell(g: &Game, from: i64) -> (i64, i64) {
        (from..from + 500)
            .map(|x| (x, 0))
            .find(|&(x, y)| !g.field.is_mine(x, y))
            .expect("no safe cell in 500")
    }

    fn a_mine(g: &Game) -> (i64, i64) {
        (0..2000)
            .map(|x| (x, 0))
            .find(|&(x, y)| g.field.is_mine(x, y))
            .expect("no mine in 2000 cells")
    }

    #[test]
    fn an_infinite_run_starts_with_three_lives_and_a_safe_cursor() {
        let g = Game::new_infinite(5);
        assert_eq!(g.lives, INFINITE_LIVES);
        assert_eq!(g.status, Status::Playing);
        assert!(!g.field.is_mine(g.cursor.0, g.cursor.1));
        assert_eq!(g.field.adjacent_mines(g.cursor.0, g.cursor.1), 0);
    }

    #[test]
    fn the_clock_starts_on_the_first_reveal_not_at_construction() {
        let mut g = Game::new_infinite(5);
        assert!(!g.timer_running(), "the clock ran before the first move");
        assert_eq!(g.elapsed(), Duration::ZERO);
        g.reveal_at_cursor();
        assert!(g.timer_running(), "the clock did not start on the first reveal");
    }

    #[test]
    fn mines_cost_a_life_and_the_third_ends_the_run() {
        let mut g = Game::new_infinite(9);
        let (x, y) = a_mine(&g);
        g.reveal(x, y);
        assert_eq!(g.lives, 2);
        assert_eq!(g.status, Status::Playing, "one mine should not end an infinite run");

        let mut hit = 1;
        let mut x = x;
        while hit < 3 {
            x += 1;
            if g.field.is_mine(x, 0) {
                g.reveal(x, 0);
                hit += 1;
            }
        }
        assert_eq!(g.lives, 0);
        assert_eq!(g.status, Status::Lost);
    }

    #[test]
    fn a_finished_run_ignores_further_moves() {
        let mut g = Game::new_infinite(9);
        g.status = Status::Lost;
        let before = g.cleared();
        let (x, y) = safe_cell(&g, 0);
        g.reveal(x, y);
        assert_eq!(g.cleared(), before, "a lost run still accepted a reveal");
    }

    #[test]
    fn classic_loses_on_the_first_mine() {
        let mut g = Game::new_classic(Difficulty::Beginner, 3);
        assert_eq!(g.lives, CLASSIC_LIVES);
        g.reveal(4, 4);
        let mine = (0..9)
            .flat_map(|y| (0..9).map(move |x| (x, y)))
            .find(|&(x, y)| g.field.is_mine(x, y))
            .expect("beginner has mines");
        g.reveal(mine.0, mine.1);
        assert_eq!(g.status, Status::Lost);
    }

    #[test]
    fn classic_is_won_when_the_board_is_cleared() {
        let mut g = Game::new_classic(Difficulty::Beginner, 21);
        g.reveal(4, 4);
        for y in 0..9 {
            for x in 0..9 {
                if !g.field.is_mine(x, y) {
                    g.reveal(x, y);
                }
            }
        }
        assert_eq!(g.status, Status::Won);
        assert!(!g.timer_running(), "the clock kept running after a win");
    }

    #[test]
    fn the_score_reads_from_the_board_not_an_accumulator() {
        let mut g = Game::new_infinite(11);
        g.reveal_at_cursor();
        assert_eq!(g.cleared(), g.field.cleared_count());
        // Revealing the same cell again must not inflate the score.
        let before = g.cleared();
        g.reveal_at_cursor();
        assert_eq!(g.cleared(), before);
    }

    #[test]
    fn the_cursor_is_clamped_in_classic_and_free_in_infinite() {
        let mut c = Game::new_classic(Difficulty::Beginner, 1);
        for _ in 0..50 {
            c.move_cursor(-1, -1);
        }
        assert_eq!(c.cursor, (0, 0), "the cursor escaped a finite board");
        for _ in 0..50 {
            c.move_cursor(1, 1);
        }
        assert_eq!(c.cursor, (8, 8));

        let mut i = Game::new_infinite(1);
        let start = i.cursor;
        for _ in 0..1000 {
            i.move_cursor(-1, 0);
        }
        assert_eq!(i.cursor, (start.0 - 1000, start.1));
    }

    #[test]
    fn flagging_marks_the_cell() {
        let mut g = Game::new_infinite(2);
        g.flag_at_cursor();
        assert_eq!(g.cell_at_cursor(), CellState::Flagged);
        g.flag_at_cursor();
        assert_eq!(g.cell_at_cursor(), CellState::Hidden);
    }

    #[test]
    fn a_restored_run_keeps_its_elapsed_time_and_starts_paused() {
        let field = Box::new(InfiniteField::new(4));
        let g = Game::restored(
            field,
            Mode::Infinite,
            4,
            2,
            (10, 10),
            Duration::from_secs(90),
        );
        assert_eq!(g.lives, 2);
        assert_eq!(g.cursor, (10, 10));
        assert_eq!(g.elapsed(), Duration::from_secs(90));
        assert!(!g.timer_running());
    }
}
