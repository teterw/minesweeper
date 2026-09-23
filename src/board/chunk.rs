//! Player state — what's revealed, what's flagged — stored in fixed chunks.
//!
//! The mine field itself is never stored; see [`crate::board::oracle`]. What has to be
//! remembered is only what the player has done, so chunks are allocated the first time
//! they're touched and a run that stays near the origin costs a few hundred bytes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chunks are 32×32. Each of the two bitmaps is then exactly 16 u64 words.
pub const CHUNK: i64 = 32;
const CELLS: usize = (CHUNK * CHUNK) as usize;
const WORDS: usize = CELLS / 64;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Chunk {
    pub revealed: [u64; WORDS],
    pub flagged: [u64; WORDS],
}

impl Chunk {
    fn get(bits: &[u64; WORDS], i: usize) -> bool {
        bits[i / 64] & (1 << (i % 64)) != 0
    }

    fn set(bits: &mut [u64; WORDS], i: usize, value: bool) {
        if value {
            bits[i / 64] |= 1 << (i % 64);
        } else {
            bits[i / 64] &= !(1 << (i % 64));
        }
    }

    pub fn revealed_count(&self) -> u32 {
        self.revealed.iter().map(|w| w.count_ones()).sum()
    }
}

/// Which chunk a world coordinate belongs to.
///
/// This uses floored division, not Rust's default truncating division. `-1 / 32` is `0`,
/// which would put the cell at `-1` in the same chunk as the cell at `+1` and corrupt a
/// strip along both negative axes. `div_euclid` floors, which is what a grid needs.
pub fn chunk_of(x: i64, y: i64) -> (i64, i64) {
    (x.div_euclid(CHUNK), y.div_euclid(CHUNK))
}

/// Index of a world coordinate within its chunk.
pub fn index_in_chunk(x: i64, y: i64) -> usize {
    let lx = x.rem_euclid(CHUNK) as usize;
    let ly = y.rem_euclid(CHUNK) as usize;
    ly * CHUNK as usize + lx
}

#[derive(Clone, Debug, Default)]
pub struct ChunkStore {
    chunks: HashMap<(i64, i64), Chunk>,
}

impl ChunkStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_revealed(&self, x: i64, y: i64) -> bool {
        self.chunks
            .get(&chunk_of(x, y))
            .is_some_and(|c| Chunk::get(&c.revealed, index_in_chunk(x, y)))
    }

    pub fn is_flagged(&self, x: i64, y: i64) -> bool {
        self.chunks
            .get(&chunk_of(x, y))
            .is_some_and(|c| Chunk::get(&c.flagged, index_in_chunk(x, y)))
    }

    pub fn set_revealed(&mut self, x: i64, y: i64, value: bool) {
        let i = index_in_chunk(x, y);
        let c = self.chunks.entry(chunk_of(x, y)).or_default();
        Chunk::set(&mut c.revealed, i, value);
    }

    pub fn set_flagged(&mut self, x: i64, y: i64, value: bool) {
        let i = index_in_chunk(x, y);
        let c = self.chunks.entry(chunk_of(x, y)).or_default();
        Chunk::set(&mut c.flagged, i, value);
    }

    pub fn toggle_flag(&mut self, x: i64, y: i64) {
        let value = !self.is_flagged(x, y);
        self.set_flagged(x, y, value);
    }

    pub fn revealed_total(&self) -> usize {
        self.chunks
            .values()
            .map(|c| c.revealed_count() as usize)
            .sum()
    }

    pub fn flagged_total(&self) -> usize {
        self.chunks
            .values()
            .map(|c| c.flagged.iter().map(|w| w.count_ones() as usize).sum::<usize>())
            .sum()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(i64, i64), &Chunk)> {
        self.chunks.iter()
    }

    pub fn insert(&mut self, coords: (i64, i64), chunk: Chunk) {
        self.chunks.insert(coords, chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_reveals_and_flags() {
        let mut s = ChunkStore::new();
        assert!(!s.is_revealed(3, 4));
        s.set_revealed(3, 4, true);
        assert!(s.is_revealed(3, 4));
        assert!(!s.is_revealed(4, 4), "a neighbour was affected");
        s.set_revealed(3, 4, false);
        assert!(!s.is_revealed(3, 4));

        s.toggle_flag(-7, 9);
        assert!(s.is_flagged(-7, 9));
        s.toggle_flag(-7, 9);
        assert!(!s.is_flagged(-7, 9));
    }

    #[test]
    fn reveals_and_flags_are_independent() {
        let mut s = ChunkStore::new();
        s.set_revealed(1, 1, true);
        assert!(!s.is_flagged(1, 1));
        s.set_flagged(2, 2, true);
        assert!(!s.is_revealed(2, 2));
    }

    /// Review Focus 1. Truncating division would fold -1 into chunk 0 and alias it onto
    /// the positive side, corrupting a strip along both negative axes.
    #[test]
    fn chunks_floor_across_the_origin_seam() {
        assert_eq!(chunk_of(0, 0), (0, 0));
        assert_eq!(chunk_of(31, 31), (0, 0));
        assert_eq!(chunk_of(32, 32), (1, 1));
        assert_eq!(chunk_of(-1, -1), (-1, -1));
        assert_eq!(chunk_of(-32, -32), (-1, -1));
        assert_eq!(chunk_of(-33, -33), (-2, -2));
    }

    #[test]
    fn indices_are_unique_within_a_chunk() {
        let mut seen = std::collections::HashSet::new();
        for y in -32..0i64 {
            for x in -32..0i64 {
                assert!(
                    seen.insert(index_in_chunk(x, y)),
                    "({x},{y}) collided with another cell in its chunk"
                );
            }
        }
        assert_eq!(seen.len(), CELLS);
    }

    #[test]
    fn negative_and_positive_cells_do_not_alias() {
        let mut s = ChunkStore::new();
        s.set_revealed(-1, -1, true);
        assert!(!s.is_revealed(1, 1), "(-1,-1) leaked into (1,1)");
        assert!(!s.is_revealed(31, 31), "(-1,-1) leaked into (31,31)");
        assert!(s.is_revealed(-1, -1));
    }

    #[test]
    fn untouched_chunks_allocate_nothing() {
        let mut s = ChunkStore::new();
        assert_eq!(s.chunk_count(), 0);
        assert!(!s.is_revealed(10_000, -10_000));
        assert_eq!(s.chunk_count(), 0, "a read allocated a chunk");
        s.set_revealed(10_000, -10_000, true);
        assert_eq!(s.chunk_count(), 1);
    }

    #[test]
    fn counts_revealed_cells_across_chunks() {
        let mut s = ChunkStore::new();
        for i in 0..100i64 {
            s.set_revealed(i, i, true);
        }
        assert_eq!(s.revealed_total(), 100);
    }
}
