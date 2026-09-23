//! The three original boards: real edges, an exact mine count, one life.
//!
//! Classic mode does not use the oracle. The oracle gives a *density*, and Beginner
//! needs exactly ten mines, so this places them by shuffle instead — and, like the
//! original, not until the player's first click, so that click is always safe.

use super::field::{CellState, Field, Rect, RevealOutcome, NEIGHBOURS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Expert,
}

impl Difficulty {
    /// Width, height, mines.
    pub fn dimensions(self) -> (i64, i64, usize) {
        match self {
            Difficulty::Beginner => (9, 9, 10),
            Difficulty::Intermediate => (16, 16, 40),
            Difficulty::Expert => (30, 16, 99),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Beginner => "Beginner",
            Difficulty::Intermediate => "Intermediate",
            Difficulty::Expert => "Expert",
        }
    }
}

/// A seeded generator, so a board is reproducible from its seed like the infinite one.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

pub struct ClassicField {
    w: i64,
    h: i64,
    mine_count: usize,
    seed: u64,
    /// `None` until the first reveal decides where the safe opening is.
    mines: Option<Vec<bool>>,
    revealed: Vec<bool>,
    flagged: Vec<bool>,
    detonated: Vec<bool>,
    lost: bool,
}

impl ClassicField {
    pub fn new(difficulty: Difficulty, seed: u64) -> Self {
        let (w, h, mine_count) = difficulty.dimensions();
        let cells = (w * h) as usize;
        Self {
            w,
            h,
            mine_count,
            seed,
            mines: None,
            revealed: vec![false; cells],
            flagged: vec![false; cells],
            detonated: vec![false; cells],
            lost: false,
        }
    }

    pub fn width(&self) -> i64 {
        self.w
    }

    pub fn height(&self) -> i64 {
        self.h
    }

    fn in_bounds(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    fn idx(&self, x: i64, y: i64) -> usize {
        (y * self.w + x) as usize
    }

    /// Places mines everywhere except the 3×3 around the opening click.
    fn generate(&mut self, safe_x: i64, safe_y: i64) {
        let mut eligible: Vec<usize> = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                if (x - safe_x).abs() <= 1 && (y - safe_y).abs() <= 1 {
                    continue;
                }
                eligible.push(self.idx(x, y));
            }
        }

        let take = self.mine_count.min(eligible.len());
        let mut rng = Rng(self.seed);
        for i in 0..take {
            let j = i + (rng.next() % (eligible.len() - i) as u64) as usize;
            eligible.swap(i, j);
        }

        let mut mines = vec![false; (self.w * self.h) as usize];
        for &p in &eligible[..take] {
            mines[p] = true;
        }
        self.mines = Some(mines);
    }

    fn flood(&mut self, x: i64, y: i64) -> usize {
        let mut stack = vec![(x, y)];
        let mut cleared = 0usize;
        while let Some((cx, cy)) = stack.pop() {
            if !self.in_bounds(cx, cy) {
                continue;
            }
            let i = self.idx(cx, cy);
            if self.revealed[i] || self.flagged[i] || self.is_mine(cx, cy) {
                continue;
            }
            self.revealed[i] = true;
            cleared += 1;
            if self.adjacent_mines(cx, cy) == 0 {
                for (dx, dy) in NEIGHBOURS {
                    stack.push((cx + dx, cy + dy));
                }
            }
        }
        cleared
    }
}

impl Field for ClassicField {
    fn cell(&self, x: i64, y: i64) -> CellState {
        if !self.in_bounds(x, y) {
            return CellState::OutOfBounds;
        }
        let i = self.idx(x, y);
        if self.detonated[i] {
            CellState::Detonated
        } else if self.flagged[i] {
            CellState::Flagged
        } else if self.revealed[i] {
            CellState::Revealed(self.adjacent_mines(x, y))
        } else {
            CellState::Hidden
        }
    }

    fn is_mine(&self, x: i64, y: i64) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        self.mines
            .as_ref()
            .is_some_and(|m| m[self.idx(x, y)])
    }

    fn adjacent_mines(&self, x: i64, y: i64) -> u8 {
        NEIGHBOURS
            .iter()
            .filter(|(dx, dy)| self.is_mine(x + dx, y + dy))
            .count() as u8
    }

    fn reveal(&mut self, x: i64, y: i64) -> RevealOutcome {
        if !self.in_bounds(x, y) {
            return RevealOutcome::Blocked;
        }
        if self.mines.is_none() {
            self.generate(x, y);
        }
        let i = self.idx(x, y);
        if self.revealed[i] || self.flagged[i] || self.detonated[i] {
            return RevealOutcome::Blocked;
        }
        if self.is_mine(x, y) {
            self.detonated[i] = true;
            self.lost = true;
            return RevealOutcome::Mine;
        }
        RevealOutcome::Cleared(self.flood(x, y))
    }

    fn toggle_flag(&mut self, x: i64, y: i64) {
        if !self.in_bounds(x, y) {
            return;
        }
        let i = self.idx(x, y);
        if self.revealed[i] || self.detonated[i] {
            return;
        }
        self.flagged[i] = !self.flagged[i];
    }

    fn chord(&mut self, x: i64, y: i64) -> RevealOutcome {
        let CellState::Revealed(n) = self.cell(x, y) else {
            return RevealOutcome::Blocked;
        };
        if n == 0 {
            return RevealOutcome::Blocked;
        }
        let flags = NEIGHBOURS
            .iter()
            .filter(|(dx, dy)| matches!(self.cell(x + dx, y + dy), CellState::Flagged))
            .count();
        if flags != n as usize {
            return RevealOutcome::Blocked;
        }

        let mut cleared = 0usize;
        let mut hit_mine = false;
        for (dx, dy) in NEIGHBOURS {
            match self.reveal(x + dx, y + dy) {
                RevealOutcome::Cleared(c) => cleared += c,
                RevealOutcome::Mine => hit_mine = true,
                RevealOutcome::Blocked => {}
            }
        }
        if hit_mine {
            RevealOutcome::Mine
        } else {
            RevealOutcome::Cleared(cleared)
        }
    }

    fn bounds(&self) -> Option<Rect> {
        Some(Rect {
            x: 0,
            y: 0,
            w: self.w,
            h: self.h,
        })
    }

    fn is_won(&self) -> bool {
        if self.lost || self.mines.is_none() {
            return false;
        }
        let total = (self.w * self.h) as usize;
        let revealed = self.revealed.iter().filter(|r| **r).count();
        revealed == total - self.mine_count
    }

    fn cleared_count(&self) -> usize {
        self.revealed.iter().filter(|r| **r).count()
    }

    fn flags_placed(&self) -> usize {
        self.flagged.iter().filter(|f| **f).count()
    }

    fn mine_total(&self) -> Option<usize> {
        Some(self.mine_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulties_have_the_canonical_dimensions() {
        assert_eq!(Difficulty::Beginner.dimensions(), (9, 9, 10));
        assert_eq!(Difficulty::Intermediate.dimensions(), (16, 16, 40));
        assert_eq!(Difficulty::Expert.dimensions(), (30, 16, 99));
    }

    #[test]
    fn places_exactly_the_right_number_of_mines() {
        for d in [
            Difficulty::Beginner,
            Difficulty::Intermediate,
            Difficulty::Expert,
        ] {
            let (w, h, expected) = d.dimensions();
            let mut f = ClassicField::new(d, 99);
            f.reveal(w / 2, h / 2);
            let actual = (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .filter(|&(x, y)| f.is_mine(x, y))
                .count();
            assert_eq!(actual, expected, "{} had {actual} mines", d.name());
        }
    }

    #[test]
    fn the_first_click_is_always_safe() {
        for seed in 0..100u64 {
            let mut f = ClassicField::new(Difficulty::Expert, seed);
            let outcome = f.reveal(5, 5);
            assert_ne!(outcome, RevealOutcome::Mine, "seed {seed} lost on click one");
            assert_eq!(f.adjacent_mines(5, 5), 0, "seed {seed} opened beside a mine");
        }
    }

    #[test]
    fn reveals_outside_the_board_are_blocked() {
        let mut f = ClassicField::new(Difficulty::Beginner, 1);
        assert_eq!(f.reveal(-1, 0), RevealOutcome::Blocked);
        assert_eq!(f.reveal(0, -1), RevealOutcome::Blocked);
        assert_eq!(f.reveal(9, 0), RevealOutcome::Blocked);
        assert_eq!(f.reveal(0, 9), RevealOutcome::Blocked);
        assert_eq!(f.cell(-1, -1), CellState::OutOfBounds);
    }

    #[test]
    fn bounds_match_the_difficulty() {
        let f = ClassicField::new(Difficulty::Expert, 1);
        assert_eq!(
            f.bounds(),
            Some(Rect {
                x: 0,
                y: 0,
                w: 30,
                h: 16
            })
        );
    }

    #[test]
    fn the_board_is_won_exactly_when_every_safe_cell_is_revealed() {
        let mut f = ClassicField::new(Difficulty::Beginner, 7);
        f.reveal(4, 4);
        assert!(!f.is_won(), "won before clearing the board");

        for y in 0..9 {
            for x in 0..9 {
                if !f.is_mine(x, y) {
                    f.reveal(x, y);
                }
            }
        }
        assert!(f.is_won(), "cleared every safe cell but was not won");
    }

    #[test]
    fn hitting_a_mine_is_not_a_win() {
        let mut f = ClassicField::new(Difficulty::Beginner, 7);
        f.reveal(4, 4);
        let mine = (0..9)
            .flat_map(|y| (0..9).map(move |x| (x, y)))
            .find(|&(x, y)| f.is_mine(x, y))
            .expect("beginner has mines");
        assert_eq!(f.reveal(mine.0, mine.1), RevealOutcome::Mine);
        for y in 0..9 {
            for x in 0..9 {
                if !f.is_mine(x, y) {
                    f.reveal(x, y);
                }
            }
        }
        assert!(!f.is_won(), "a lost board reported a win");
    }

    #[test]
    fn a_fresh_board_is_not_won() {
        let f = ClassicField::new(Difficulty::Beginner, 1);
        assert!(!f.is_won(), "an untouched board reported a win");
    }

    #[test]
    fn the_same_seed_and_first_click_reproduce_the_board() {
        let mut a = ClassicField::new(Difficulty::Intermediate, 4242);
        let mut b = ClassicField::new(Difficulty::Intermediate, 4242);
        a.reveal(8, 8);
        b.reveal(8, 8);
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(a.is_mine(x, y), b.is_mine(x, y), "differed at ({x},{y})");
            }
        }
    }

    #[test]
    fn flags_are_counted() {
        let mut f = ClassicField::new(Difficulty::Beginner, 1);
        assert_eq!(f.flags_placed(), 0);
        f.toggle_flag(0, 0);
        f.toggle_flag(1, 1);
        assert_eq!(f.flags_placed(), 2);
        f.toggle_flag(0, 0);
        assert_eq!(f.flags_placed(), 1);
    }
}
