//! Saving runs and scores.
//!
//! Only the player's moves are stored. The board regenerates from the seed, so a
//! ten-thousand-cell run is a few kilobytes rather than a few megabytes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::board::chunk::Chunk;
use crate::board::classic::Difficulty;
use crate::board::infinite::InfiniteField;
use crate::game::{Game, Mode};

const WORDS: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedChunk {
    pub cx: i64,
    pub cy: i64,
    pub revealed: String,
    pub flagged: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRun {
    pub seed: u64,
    pub lives: u8,
    pub elapsed_secs: u64,
    pub cursor: (i64, i64),
    pub chunks: Vec<SavedChunk>,
    pub detonated: Vec<(i64, i64)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scores {
    /// Most cells ever cleared in one infinite run.
    pub infinite_best: usize,
    pub beginner_best_secs: Option<u64>,
    pub intermediate_best_secs: Option<u64>,
    pub expert_best_secs: Option<u64>,
}

impl Scores {
    pub fn best_for(&self, mode: Mode) -> Option<u64> {
        match mode {
            Mode::Infinite => None,
            Mode::Classic(Difficulty::Beginner) => self.beginner_best_secs,
            Mode::Classic(Difficulty::Intermediate) => self.intermediate_best_secs,
            Mode::Classic(Difficulty::Expert) => self.expert_best_secs,
        }
    }

    /// Returns true when this result beat the stored best.
    pub fn record(&mut self, mode: Mode, cleared: usize, elapsed: Duration) -> bool {
        match mode {
            Mode::Infinite => {
                if cleared > self.infinite_best {
                    self.infinite_best = cleared;
                    return true;
                }
                false
            }
            Mode::Classic(d) => {
                let secs = elapsed.as_secs();
                let slot = match d {
                    Difficulty::Beginner => &mut self.beginner_best_secs,
                    Difficulty::Intermediate => &mut self.intermediate_best_secs,
                    Difficulty::Expert => &mut self.expert_best_secs,
                };
                if slot.is_none_or(|best| secs < best) {
                    *slot = Some(secs);
                    return true;
                }
                false
            }
        }
    }
}

/// `~/.local/share/minesweeper` on Linux, `%APPDATA%\minesweeper` on Windows.
pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "minesweeper")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn words_to_b64(words: &[u64; WORDS]) -> String {
    let mut bytes = Vec::with_capacity(WORDS * 8);
    for w in words {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    STANDARD.encode(bytes)
}

fn b64_to_words(s: &str) -> Option<[u64; WORDS]> {
    let bytes = STANDARD.decode(s).ok()?;
    if bytes.len() != WORDS * 8 {
        return None;
    }
    let mut words = [0u64; WORDS];
    for (i, w) in words.iter_mut().enumerate() {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[i * 8..i * 8 + 8]);
        *w = u64::from_le_bytes(buf);
    }
    Some(words)
}

/// Write via a temporary file and rename, so an interrupted save can't leave a
/// half-written file where a valid one used to be.
fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn run_path(dir: &Path) -> PathBuf {
    dir.join("run.json")
}

pub fn scores_path(dir: &Path) -> PathBuf {
    dir.join("scores.json")
}

/// Snapshots an infinite run. Classic boards are short enough to finish in one sitting
/// and aren't saved, so this returns `None` for them.
pub fn snapshot(game: &Game) -> Option<SavedRun> {
    if game.mode != Mode::Infinite {
        return None;
    }
    let field = game.field.as_any().downcast_ref::<InfiniteField>()?;
    let chunks = field
        .store()
        .iter()
        .map(|(&(cx, cy), c)| SavedChunk {
            cx,
            cy,
            revealed: words_to_b64(&c.revealed),
            flagged: words_to_b64(&c.flagged),
        })
        .collect();
    Some(SavedRun {
        seed: field.seed(),
        lives: game.lives,
        elapsed_secs: game.elapsed().as_secs(),
        cursor: game.cursor,
        chunks,
        detonated: field.detonated().iter().copied().collect(),
    })
}

pub fn save_run_to(dir: &Path, game: &Game) -> io::Result<()> {
    let Some(run) = snapshot(game) else {
        return Ok(());
    };
    let json = serde_json::to_string(&run).map_err(io::Error::other)?;
    write_atomic(&run_path(dir), &json)
}

/// Reads a saved run. A file that won't parse is moved aside rather than deleted, and
/// reported as absent — losing one run is acceptable, refusing to launch is not.
pub fn load_run_from(dir: &Path) -> Option<SavedRun> {
    let path = run_path(dir);
    let text = fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<SavedRun>(&text) {
        Ok(run) => Some(run),
        Err(_) => {
            let _ = fs::rename(&path, path.with_extension("corrupt"));
            None
        }
    }
}

pub fn clear_run(dir: &Path) {
    let _ = fs::remove_file(run_path(dir));
}

pub fn rebuild(run: SavedRun) -> Game {
    let mut field = InfiniteField::new(run.seed);
    for c in &run.chunks {
        let (Some(revealed), Some(flagged)) =
            (b64_to_words(&c.revealed), b64_to_words(&c.flagged))
        else {
            continue;
        };
        field.store_mut().insert((c.cx, c.cy), Chunk { revealed, flagged });
    }
    for &(x, y) in &run.detonated {
        field.detonate(x, y);
    }
    Game::restored(
        Box::new(field),
        Mode::Infinite,
        run.seed,
        run.lives,
        run.cursor,
        Duration::from_secs(run.elapsed_secs),
    )
}

pub fn load_scores_from(dir: &Path) -> Scores {
    fs::read_to_string(scores_path(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_scores_to(dir: &Path, scores: &Scores) -> io::Result<()> {
    let json = serde_json::to_string(scores).map_err(io::Error::other)?;
    write_atomic(&scores_path(dir), &json)
}

pub fn save_run(game: &Game) -> io::Result<()> {
    save_run_to(&data_dir(), game)
}

pub fn load_run() -> Option<SavedRun> {
    load_run_from(&data_dir())
}

pub fn load_scores() -> Scores {
    load_scores_from(&data_dir())
}

pub fn save_scores(scores: &Scores) -> io::Result<()> {
    save_scores_to(&data_dir(), scores)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::field::CellState;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("minesweeper-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_run_survives_a_save_and_load() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(1234);
        g.reveal_at_cursor();
        g.move_cursor(3, -2);
        g.flag_at_cursor();
        let cleared = g.cleared();
        let cursor = g.cursor;

        save_run_to(&dir, &g).unwrap();
        let restored = rebuild(load_run_from(&dir).expect("run should load"));

        assert_eq!(restored.seed, 1234);
        assert_eq!(restored.lives, g.lives);
        assert_eq!(restored.cursor, cursor);
        assert_eq!(restored.cleared(), cleared, "revealed cells did not survive");
        assert_eq!(
            restored.field.cell(cursor.0, cursor.1),
            CellState::Flagged,
            "the flag did not survive"
        );
    }

    #[test]
    fn detonated_mines_survive_a_save() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(9);
        let mine = (0..2000)
            .map(|x| (x, 0))
            .find(|&(x, y)| g.field.is_mine(x, y))
            .unwrap();
        g.reveal(mine.0, mine.1);
        assert_eq!(g.lives, 2);

        save_run_to(&dir, &g).unwrap();
        let restored = rebuild(load_run_from(&dir).unwrap());
        assert_eq!(restored.lives, 2);
        assert_eq!(restored.field.cell(mine.0, mine.1), CellState::Detonated);
    }

    #[test]
    fn the_board_regenerates_rather_than_being_stored() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(555);
        g.reveal_at_cursor();
        save_run_to(&dir, &g).unwrap();

        let restored = rebuild(load_run_from(&dir).unwrap());
        // Cells never touched must still agree with the original board.
        for x in 0..200i64 {
            assert_eq!(
                g.field.is_mine(x, 40),
                restored.field.is_mine(x, 40),
                "the regenerated board differs at ({x}, 40)"
            );
        }
    }

    #[test]
    fn no_saved_run_reads_as_none() {
        let dir = temp_dir();
        assert!(load_run_from(&dir).is_none());
    }

    /// Review Focus 3. A corrupt file must cost one run, never the ability to launch.
    #[test]
    fn a_corrupt_save_is_moved_aside_and_reported_absent() {
        let dir = temp_dir();
        fs::write(run_path(&dir), "{ this is not json at all").unwrap();
        assert!(load_run_from(&dir).is_none(), "garbage parsed as a run");
        assert!(
            !run_path(&dir).exists(),
            "the corrupt file was left in place to fail again next launch"
        );
        assert!(
            dir.join("run.corrupt").exists(),
            "the corrupt file was destroyed rather than kept aside"
        );
    }

    #[test]
    fn a_truncated_save_does_not_panic() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(3);
        g.reveal_at_cursor();
        save_run_to(&dir, &g).unwrap();

        let full = fs::read_to_string(run_path(&dir)).unwrap();
        fs::write(run_path(&dir), &full[..full.len() / 2]).unwrap();
        assert!(load_run_from(&dir).is_none());
    }

    #[test]
    fn a_chunk_with_mangled_base64_is_skipped_not_fatal() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(3);
        g.reveal_at_cursor();
        save_run_to(&dir, &g).unwrap();

        let mut run: SavedRun =
            serde_json::from_str(&fs::read_to_string(run_path(&dir)).unwrap()).unwrap();
        run.chunks.push(SavedChunk {
            cx: 99,
            cy: 99,
            revealed: "not base64!!".into(),
            flagged: "nor this".into(),
        });
        fs::write(run_path(&dir), serde_json::to_string(&run).unwrap()).unwrap();

        let restored = rebuild(load_run_from(&dir).unwrap());
        assert!(restored.cleared() > 0, "a bad chunk discarded the good ones");
    }

    #[test]
    fn saving_creates_a_missing_data_directory() {
        let dir = temp_dir().join("nested").join("deeper");
        let mut g = Game::new_infinite(1);
        g.reveal_at_cursor();
        save_run_to(&dir, &g).unwrap();
        assert!(run_path(&dir).exists());
    }

    #[test]
    fn saving_leaves_no_temporary_file_behind() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(1);
        g.reveal_at_cursor();
        save_run_to(&dir, &g).unwrap();
        assert!(!dir.join("run.tmp").exists(), "a .tmp file was left behind");
    }

    #[test]
    fn classic_runs_are_not_saved() {
        let dir = temp_dir();
        let mut g = Game::new_classic(Difficulty::Beginner, 1);
        g.reveal(4, 4);
        save_run_to(&dir, &g).unwrap();
        assert!(!run_path(&dir).exists(), "a classic board was saved");
    }

    #[test]
    fn scores_round_trip() {
        let dir = temp_dir();
        let mut s = Scores::default();
        s.record(Mode::Infinite, 500, Duration::ZERO);
        s.record(
            Mode::Classic(Difficulty::Expert),
            0,
            Duration::from_secs(120),
        );
        save_scores_to(&dir, &s).unwrap();

        let loaded = load_scores_from(&dir);
        assert_eq!(loaded.infinite_best, 500);
        assert_eq!(loaded.expert_best_secs, Some(120));
    }

    #[test]
    fn a_better_score_replaces_the_old_one_and_a_worse_one_does_not() {
        let mut s = Scores::default();
        assert!(s.record(Mode::Infinite, 100, Duration::ZERO));
        assert!(!s.record(Mode::Infinite, 50, Duration::ZERO));
        assert_eq!(s.infinite_best, 100);
        assert!(s.record(Mode::Infinite, 150, Duration::ZERO));
        assert_eq!(s.infinite_best, 150);

        let m = Mode::Classic(Difficulty::Beginner);
        assert!(s.record(m, 0, Duration::from_secs(60)));
        assert!(!s.record(m, 0, Duration::from_secs(90)), "a slower time won");
        assert!(s.record(m, 0, Duration::from_secs(30)), "a faster time lost");
        assert_eq!(s.beginner_best_secs, Some(30));
    }

    #[test]
    fn missing_scores_read_as_defaults() {
        let dir = temp_dir();
        let s = load_scores_from(&dir);
        assert_eq!(s.infinite_best, 0);
        assert!(s.beginner_best_secs.is_none());
    }

    #[test]
    fn corrupt_scores_read_as_defaults() {
        let dir = temp_dir();
        fs::write(scores_path(&dir), "!!!").unwrap();
        assert_eq!(load_scores_from(&dir).infinite_best, 0);
    }

    #[test]
    fn clearing_a_run_removes_it() {
        let dir = temp_dir();
        let mut g = Game::new_infinite(1);
        g.reveal_at_cursor();
        save_run_to(&dir, &g).unwrap();
        assert!(run_path(&dir).exists());
        clear_run(&dir);
        assert!(!run_path(&dir).exists());
    }
}
