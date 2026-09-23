//! The mine oracle.
//!
//! There is no board. Whether a cell holds a mine is a pure function of the run's seed
//! and the cell's coordinates, so the field costs nothing to store, stays consistent
//! when you scroll back to it, and reproduces exactly from a shared seed.

/// Fraction of cells that hold a mine. Expert is 99/480 ≈ 20.6%; 16% is a little kinder,
/// which matters when the board never ends.
pub const DENSITY: f64 = 0.16;

const THRESHOLD: u64 = (u64::MAX as f64 * DENSITY) as u64;

const PRIME_X: u64 = 0x9E37_79B9_7F4A_7C15;
const PRIME_Y: u64 = 0xC2B2_AE3D_27D4_EB4F;

/// SplitMix64. Cheap, and its avalanche is good enough that neighbouring coordinates
/// produce uncorrelated output — which is the whole requirement here.
fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Two rounds, one per axis. A single round mixing `x ^ y` leaves visible diagonal
/// streaks of mines, because coordinates on a diagonal collide before being mixed.
fn hash(seed: u64, x: i64, y: i64) -> u64 {
    let h = splitmix64(seed ^ (x as u64).wrapping_mul(PRIME_X));
    splitmix64(h ^ (y as u64).wrapping_mul(PRIME_Y))
}

/// Whether the cell at `(x, y)` holds a mine, for this run's seed.
pub fn is_mine(seed: u64, x: i64, y: i64) -> bool {
    hash(seed, x, y) < THRESHOLD
}

/// How many of the eight neighbours of `(x, y)` hold mines.
pub fn adjacent_mines(seed: u64, x: i64, y: i64) -> u8 {
    let mut n = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if (dx, dy) != (0, 0) && is_mine(seed, x.wrapping_add(dx), y.wrapping_add(dy)) {
                n += 1;
            }
        }
    }
    n
}

/// Whether the 3×3 neighbourhood centred on `(x, y)` is entirely free of mines.
pub fn is_clear_start(seed: u64, x: i64, y: i64) -> bool {
    !is_mine(seed, x, y) && adjacent_mines(seed, x, y) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_cell_always_answers_the_same() {
        for i in 0..10_000i64 {
            let (x, y) = (i % 137 - 68, i / 137 - 36);
            assert_eq!(is_mine(42, x, y), is_mine(42, x, y));
        }
    }

    #[test]
    fn different_seeds_give_different_boards() {
        let differences = (0..1000i64)
            .filter(|&i| is_mine(1, i, 0) != is_mine(2, i, 0))
            .count();
        assert!(
            differences > 100,
            "seeds 1 and 2 differed in only {differences}/1000 cells; seed is barely mixed in"
        );
    }

    #[test]
    fn density_is_close_to_the_constant() {
        let mut mines = 0u32;
        let total = 1_000_000u32;
        for y in -500..500i64 {
            for x in -500..500i64 {
                if is_mine(7, x, y) {
                    mines += 1;
                }
            }
        }
        let rate = f64::from(mines) / f64::from(total);
        assert!(
            (rate - DENSITY).abs() < 0.005,
            "density was {rate}, expected about {DENSITY}"
        );
    }

    /// The failure this guards against is a weak hash producing regular stripes of mines.
    /// If the field is well mixed, two cells a fixed offset apart are independent, so the
    /// chance both hold mines is DENSITY².
    #[test]
    fn no_periodicity_along_any_axis() {
        let expected = DENSITY * DENSITY;
        for &(dx, dy) in &[(1, 0), (0, 1), (1, 1), (1, -1), (32, 0), (0, 32), (16, 16)] {
            let mut both = 0u32;
            let total = 250_000u32;
            for y in -250..250i64 {
                for x in -250..250i64 {
                    if is_mine(7, x, y) && is_mine(7, x + dx, y + dy) {
                        both += 1;
                    }
                }
            }
            let rate = f64::from(both) / f64::from(total);
            assert!(
                (rate - expected).abs() < 0.01,
                "offset ({dx},{dy}) gave joint rate {rate}, expected about {expected} — \
                 the field is correlated at that offset"
            );
        }
    }

    #[test]
    fn adjacent_mines_matches_a_manual_count() {
        for y in -20..20i64 {
            for x in -20..20i64 {
                let manual = [
                    (-1, -1), (0, -1), (1, -1),
                    (-1, 0), (1, 0),
                    (-1, 1), (0, 1), (1, 1),
                ]
                .iter()
                .filter(|(dx, dy)| is_mine(9, x + dx, y + dy))
                .count() as u8;
                assert_eq!(adjacent_mines(9, x, y), manual, "at ({x}, {y})");
            }
        }
    }

    #[test]
    fn adjacent_mines_never_exceeds_eight() {
        for y in -100..100i64 {
            for x in -100..100i64 {
                assert!(adjacent_mines(3, x, y) <= 8);
            }
        }
    }

    #[test]
    fn works_at_and_across_zero() {
        // Purely that it does not panic and stays consistent around the origin seam,
        // where i64 -> u64 casts flip sign bits.
        for &(x, y) in &[(0, 0), (-1, -1), (-1, 0), (0, -1), (i64::MIN + 1, 0), (i64::MAX, 0)] {
            let a = is_mine(5, x, y);
            assert_eq!(a, is_mine(5, x, y));
        }
    }

    #[test]
    fn clear_starts_exist_and_are_actually_clear() {
        let found = (0..500i64).find(|&x| is_clear_start(11, x, 0));
        let x = found.expect("no clear start in 500 cells; density or hash is wrong");
        assert!(!is_mine(11, x, 0));
        assert_eq!(adjacent_mines(11, x, 0), 0);
    }
}
