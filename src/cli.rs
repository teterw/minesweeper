//! Command-line flags. Bare `minesweeper` opens the menu; the flags skip it.

use clap::Parser;

use crate::board::classic::Difficulty;
use crate::render::style as render_style;
use crate::render::Style;

#[derive(Parser, Debug)]
#[command(
    name = "minesweeper",
    version,
    about = "An infinite Minesweeper for your terminal"
)]
pub struct Cli {
    /// Start an endless board (the main mode)
    #[arg(long)]
    pub infinite: bool,

    /// Classic 9x9 with 10 mines
    #[arg(long, conflicts_with_all = ["infinite", "intermediate", "expert", "continue_run"])]
    pub beginner: bool,

    /// Classic 16x16 with 40 mines
    #[arg(long, conflicts_with_all = ["infinite", "beginner", "expert", "continue_run"])]
    pub intermediate: bool,

    /// Classic 30x16 with 99 mines
    #[arg(long, conflicts_with_all = ["infinite", "beginner", "intermediate", "continue_run"])]
    pub expert: bool,

    /// Resume the infinite run you left
    #[arg(long = "continue", conflicts_with = "infinite")]
    pub continue_run: bool,

    /// Play a specific infinite board; the same seed is the same board for everyone
    #[arg(long, requires = "infinite")]
    pub seed: Option<u64>,

    /// Board look to start in: tiles, blocks, dots, grid, boxed, lines or tight. Press
    /// `v` in game to cycle through them.
    #[arg(long, value_parser = parse_style)]
    pub style: Option<Style>,
}

fn parse_style(s: &str) -> Result<Style, String> {
    Style::parse(s).ok_or_else(|| {
        let names: Vec<_> = render_style::ALL.iter().map(|v| v.name()).collect();
        format!("unknown style '{s}'; try one of: {}", names.join(", "))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    Menu,
    Infinite(Option<u64>),
    Continue,
    Classic(Difficulty),
}

impl Cli {
    pub fn launch(&self) -> Launch {
        if self.continue_run {
            Launch::Continue
        } else if self.infinite {
            Launch::Infinite(self.seed)
        } else if self.beginner {
            Launch::Classic(Difficulty::Beginner)
        } else if self.intermediate {
            Launch::Classic(Difficulty::Intermediate)
        } else if self.expert {
            Launch::Classic(Difficulty::Expert)
        } else {
            Launch::Menu
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch_from(args: &[&str]) -> Launch {
        let mut full = vec!["minesweeper"];
        full.extend_from_slice(args);
        Cli::parse_from(full).launch()
    }

    #[test]
    fn no_flags_opens_the_menu() {
        assert_eq!(launch_from(&[]), Launch::Menu);
    }

    #[test]
    fn each_flag_starts_its_mode() {
        assert_eq!(launch_from(&["--infinite"]), Launch::Infinite(None));
        assert_eq!(launch_from(&["--continue"]), Launch::Continue);
        assert_eq!(
            launch_from(&["--beginner"]),
            Launch::Classic(Difficulty::Beginner)
        );
        assert_eq!(
            launch_from(&["--intermediate"]),
            Launch::Classic(Difficulty::Intermediate)
        );
        assert_eq!(
            launch_from(&["--expert"]),
            Launch::Classic(Difficulty::Expert)
        );
    }

    #[test]
    fn a_seed_is_carried_into_the_run() {
        assert_eq!(
            launch_from(&["--infinite", "--seed", "8675309"]),
            Launch::Infinite(Some(8675309))
        );
    }

    #[test]
    fn a_seed_without_infinite_is_rejected() {
        let r = Cli::try_parse_from(["minesweeper", "--seed", "1"]);
        assert!(r.is_err(), "--seed should require --infinite");
    }

    #[test]
    fn two_difficulties_at_once_are_rejected() {
        assert!(Cli::try_parse_from(["minesweeper", "--beginner", "--expert"]).is_err());
        assert!(Cli::try_parse_from(["minesweeper", "--infinite", "--expert"]).is_err());
        assert!(Cli::try_parse_from(["minesweeper", "--continue", "--infinite"]).is_err());
    }

    #[test]
    fn the_flags_are_documented() {
        // A parse failure here means clap's own definition is malformed.
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
