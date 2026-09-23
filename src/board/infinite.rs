//! The endless board.

use std::collections::{HashSet, VecDeque};

use super::chunk::ChunkStore;
use super::field::{CellState, Field, Rect, RevealOutcome, NEIGHBOURS};
use super::oracle;

/// A zero-region on an infinite board has no boundary to stop it. At 16% density a
/// region this large is vanishingly unlikely, but the cap is what stops a pathological
/// seed from hanging the game mid-click.
pub const FLOOD_CAP: usize = 50_000;

pub struct InfiniteField {
    seed: u64,
    store: ChunkStore,
    detonated: HashSet<(i64, i64)>,
}

impl InfiniteField {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            store: ChunkStore::new(),
            detonated: HashSet::new(),
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn store(&self) -> &ChunkStore {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut ChunkStore {
        &mut self.store
    }

    pub fn detonated(&self) -> &HashSet<(i64, i64)> {
        &self.detonated
    }

    pub fn detonate(&mut self, x: i64, y: i64) {
        self.detonated.insert((x, y));
    }

    /// Where a run begins: the nearest cell to the origin with a mine-free 3×3 around it.
    ///
    /// Mines can't be moved out of the way after the fact — the hash is the truth — so
    /// first-click safety is a matter of choosing where to stand. At 16% density about
    /// one cell in five qualifies, so this finds one almost immediately.
    pub fn safe_start(seed: u64) -> (i64, i64) {
        for radius in 0..1_000i64 {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    if oracle::is_clear_start(seed, dx, dy) {
                        return (dx, dy);
                    }
                }
            }
        }
        (0, 0)
    }

    /// Breadth-first reveal of a zero-region, generating chunks as it spreads.
    fn flood(&mut self, x: i64, y: i64) -> usize {
        let mut queue = VecDeque::new();
        queue.push_back((x, y));
        let mut cleared = 0usize;

        while let Some((cx, cy)) = queue.pop_front() {
            if cleared >= FLOOD_CAP {
                break;
            }
            // Duplicates are allowed into the queue and filtered here, which is cheaper
            // than keeping a separate visited set alongside the bitmaps we already have.
            if self.store.is_revealed(cx, cy) || self.store.is_flagged(cx, cy) {
                continue;
            }
            if oracle::is_mine(self.seed, cx, cy) {
                continue;
            }

            self.store.set_revealed(cx, cy, true);
            cleared += 1;

            if oracle::adjacent_mines(self.seed, cx, cy) == 0 {
                for (dx, dy) in NEIGHBOURS {
                    let nx = cx.saturating_add(dx);
                    let ny = cy.saturating_add(dy);
                    if !self.store.is_revealed(nx, ny) && !self.store.is_flagged(nx, ny) {
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
        cleared
    }
}

impl Field for InfiniteField {
    fn cell(&self, x: i64, y: i64) -> CellState {
        if self.detonated.contains(&(x, y)) {
            CellState::Detonated
        } else if self.store.is_flagged(x, y) {
            CellState::Flagged
        } else if self.store.is_revealed(x, y) {
            CellState::Revealed(oracle::adjacent_mines(self.seed, x, y))
        } else {
            CellState::Hidden
        }
    }

    fn is_mine(&self, x: i64, y: i64) -> bool {
        oracle::is_mine(self.seed, x, y)
    }

    fn adjacent_mines(&self, x: i64, y: i64) -> u8 {
        oracle::adjacent_mines(self.seed, x, y)
    }

    fn reveal(&mut self, x: i64, y: i64) -> RevealOutcome {
        if self.store.is_flagged(x, y)
            || self.store.is_revealed(x, y)
            || self.detonated.contains(&(x, y))
        {
            return RevealOutcome::Blocked;
        }
        if oracle::is_mine(self.seed, x, y) {
            self.detonated.insert((x, y));
            return RevealOutcome::Mine;
        }
        RevealOutcome::Cleared(self.flood(x, y))
    }

    fn toggle_flag(&mut self, x: i64, y: i64) {
        if self.store.is_revealed(x, y) || self.detonated.contains(&(x, y)) {
            return;
        }
        self.store.toggle_flag(x, y);
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
            .filter(|(dx, dy)| self.store.is_flagged(x + dx, y + dy))
            .count();
        if flags != n as usize {
            return RevealOutcome::Blocked;
        }

        // The flag count matches, but the flags may be on the wrong cells. Revealing the
        // rest is then a detonation, which is exactly what chording risks.
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
        None
    }

    fn is_won(&self) -> bool {
        false
    }

    fn cleared_count(&self) -> usize {
        self.store.revealed_total()
    }

    fn flags_placed(&self) -> usize {
        self.store.flagged_total()
    }

    fn mine_total(&self) -> Option<usize> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Finds a cell that is safe to reveal, for tests that don't care which.
    fn safe_cell(f: &InfiniteField, from: i64) -> (i64, i64) {
        (from..from + 500)
            .map(|x| (x, 0))
            .find(|&(x, y)| !f.is_mine(x, y))
            .expect("no safe cell in 500")
    }

    #[test]
    fn revealing_a_safe_cell_clears_at_least_it() {
        let mut f = InfiniteField::new(1);
        let (x, y) = safe_cell(&f, 0);
        match f.reveal(x, y) {
            RevealOutcome::Cleared(n) => assert!(n >= 1),
            other => panic!("expected Cleared, got {other:?}"),
        }
        assert!(matches!(f.cell(x, y), CellState::Revealed(_)));
    }

    #[test]
    fn revealing_a_mine_reports_it_and_marks_it() {
        let mut f = InfiniteField::new(1);
        let (x, y) = (0..500)
            .map(|x| (x, 0))
            .find(|&(x, y)| f.is_mine(x, y))
            .expect("no mine in 500 cells");
        assert_eq!(f.reveal(x, y), RevealOutcome::Mine);
        assert_eq!(f.cell(x, y), CellState::Detonated);
    }

    #[test]
    fn revealing_the_same_cell_twice_is_blocked() {
        let mut f = InfiniteField::new(2);
        let (x, y) = safe_cell(&f, 0);
        f.reveal(x, y);
        assert_eq!(f.reveal(x, y), RevealOutcome::Blocked);
    }

    #[test]
    fn flagged_cells_are_never_revealed() {
        let mut f = InfiniteField::new(3);
        let (x, y) = safe_cell(&f, 0);
        f.toggle_flag(x, y);
        assert_eq!(f.reveal(x, y), RevealOutcome::Blocked);
        assert_eq!(f.cell(x, y), CellState::Flagged);
    }

    #[test]
    fn flood_fill_spreads_from_a_zero_cell() {
        let mut f = InfiniteField::new(11);
        let (x, y) = InfiniteField::safe_start(11);
        let RevealOutcome::Cleared(n) = f.reveal(x, y) else {
            panic!("start should be safe");
        };
        assert!(n > 1, "a zero cell cleared only {n} cells; fill did not spread");
    }

    /// Review Focus 5. The fill must stop and leave a consistent board rather than
    /// running away or half-revealing a region.
    #[test]
    fn flood_fill_respects_the_cap() {
        let mut f = InfiniteField::new(12);
        let (x, y) = InfiniteField::safe_start(12);
        let RevealOutcome::Cleared(n) = f.reveal(x, y) else {
            panic!("start should be safe");
        };
        assert!(n <= FLOOD_CAP, "cleared {n}, over the cap of {FLOOD_CAP}");
        assert_eq!(
            n,
            f.cleared_count(),
            "the reported count disagrees with the board"
        );
    }

    #[test]
    fn flood_fill_never_reveals_a_mine() {
        let mut f = InfiniteField::new(13);
        let (sx, sy) = InfiniteField::safe_start(13);
        f.reveal(sx, sy);
        for y in sy - 40..sy + 40 {
            for x in sx - 40..sx + 40 {
                if matches!(f.cell(x, y), CellState::Revealed(_)) {
                    assert!(!f.is_mine(x, y), "revealed a mine at ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn toggling_a_flag_on_a_revealed_cell_does_nothing() {
        let mut f = InfiniteField::new(4);
        let (x, y) = safe_cell(&f, 0);
        f.reveal(x, y);
        f.toggle_flag(x, y);
        assert!(matches!(f.cell(x, y), CellState::Revealed(_)));
    }

    #[test]
    fn chording_needs_a_revealed_number() {
        let mut f = InfiniteField::new(5);
        assert_eq!(f.chord(0, 0), RevealOutcome::Blocked);
    }

    #[test]
    fn chording_with_too_few_flags_is_blocked() {
        let mut f = InfiniteField::new(21);
        // Find a revealed cell with a non-zero count and no flags around it.
        let (sx, sy) = InfiniteField::safe_start(21);
        f.reveal(sx, sy);
        let spot = (sx - 30..sx + 30)
            .flat_map(|x| (sy - 30..sy + 30).map(move |y| (x, y)))
            .find(|&(x, y)| matches!(f.cell(x, y), CellState::Revealed(n) if n > 0));
        if let Some((x, y)) = spot {
            assert_eq!(f.chord(x, y), RevealOutcome::Blocked);
        }
    }

    /// Review Focus 4. Correct flag *count* on the wrong cells must detonate — that is
    /// the risk chording carries, and silently doing nothing would be wrong.
    #[test]
    fn chording_with_misplaced_flags_detonates() {
        let mut f = InfiniteField::new(31);
        let (sx, sy) = InfiniteField::safe_start(31);
        f.reveal(sx, sy);

        let numbered = (sx - 30..sx + 30)
            .flat_map(|x| (sy - 30..sy + 30).map(move |y| (x, y)))
            .find(|&(x, y)| matches!(f.cell(x, y), CellState::Revealed(n) if n > 0));

        let Some((x, y)) = numbered else {
            return; // no numbered cell in range for this seed; nothing to assert
        };
        let CellState::Revealed(n) = f.cell(x, y) else {
            unreachable!()
        };

        // Flag exactly n neighbours, preferring ones that are NOT mines.
        let mut placed = 0;
        for (dx, dy) in NEIGHBOURS {
            if placed == n {
                break;
            }
            let (nx, ny) = (x + dx, y + dy);
            if !f.is_mine(nx, ny) && matches!(f.cell(nx, ny), CellState::Hidden) {
                f.toggle_flag(nx, ny);
                placed += 1;
            }
        }
        if placed == n {
            // Every flag is on a safe cell, so the real mines are still hidden and
            // chording must walk into one.
            assert_eq!(f.chord(x, y), RevealOutcome::Mine);
        }
    }

    #[test]
    fn safe_start_is_actually_safe() {
        for seed in 0..50u64 {
            let (x, y) = InfiniteField::safe_start(seed);
            let f = InfiniteField::new(seed);
            assert!(!f.is_mine(x, y), "seed {seed} started on a mine");
            assert_eq!(f.adjacent_mines(x, y), 0, "seed {seed} started beside a mine");
        }
    }

    #[test]
    fn the_board_has_no_bounds_and_is_never_won() {
        let f = InfiniteField::new(1);
        assert!(f.bounds().is_none());
        assert!(!f.is_won());
        assert!(f.mine_total().is_none());
    }

    #[test]
    fn works_far_from_the_origin_and_in_negative_space() {
        let mut f = InfiniteField::new(77);
        for &(x, y) in &[(-5_000i64, -5_000i64), (5_000, -5_000), (-1, -1)] {
            let _ = f.reveal(x, y);
            let _ = f.cell(x, y);
        }
    }
}
