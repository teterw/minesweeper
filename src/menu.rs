//! The front door: what you see when you type `minesweeper` with no flags.
//!
//! The selection logic is kept separate from the event loop so it can be tested without
//! a terminal attached.

use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};

use crate::board::classic::Difficulty;
use crate::persist::Scores;
use crate::render::clock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuChoice {
    Continue,
    Infinite,
    Classic(Difficulty),
    Quit,
}

pub struct MenuItem {
    pub choice: MenuChoice,
    pub label: String,
    pub detail: String,
}

pub struct MenuState {
    pub items: Vec<MenuItem>,
    pub selected: usize,
}

/// The first row of the terminal that holds a menu item, used to map clicks.
const FIRST_ITEM_ROW: u16 = 4;

impl MenuState {
    pub fn new(saved_run: Option<usize>, scores: &Scores) -> Self {
        let mut items = Vec::new();

        if let Some(cleared) = saved_run {
            items.push(MenuItem {
                choice: MenuChoice::Continue,
                label: "Continue run".into(),
                detail: format!("{cleared} cleared"),
            });
        }

        items.push(MenuItem {
            choice: MenuChoice::Infinite,
            label: "Infinite".into(),
            detail: if scores.infinite_best > 0 {
                format!("best {}", scores.infinite_best)
            } else {
                "no edges".into()
            },
        });

        for d in [
            Difficulty::Beginner,
            Difficulty::Intermediate,
            Difficulty::Expert,
        ] {
            let (w, h, m) = d.dimensions();
            let best = scores.best_for(crate::game::Mode::Classic(d));
            items.push(MenuItem {
                choice: MenuChoice::Classic(d),
                label: d.name().into(),
                detail: match best {
                    Some(secs) => format!("{w}x{h}  {m}   best {}", clock(secs)),
                    None => format!("{w}x{h}  {m}"),
                },
            });
        }

        items.push(MenuItem {
            choice: MenuChoice::Quit,
            label: "Quit".into(),
            detail: String::new(),
        });

        Self { items, selected: 0 }
    }

    pub fn up(&mut self) {
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
    }

    pub fn down(&mut self) {
        self.selected = (self.selected + 1) % self.items.len();
    }

    pub fn current(&self) -> MenuChoice {
        self.items[self.selected].choice
    }

    /// Which item a terminal row corresponds to, if any.
    pub fn item_at_row(&self, row: u16) -> Option<usize> {
        let i = row.checked_sub(FIRST_ITEM_ROW)? as usize;
        (i < self.items.len()).then_some(i)
    }
}

fn draw<W: Write>(out: &mut W, state: &MenuState) -> io::Result<()> {
    queue!(out, Clear(ClearType::All), MoveTo(2, 1), ResetColor)?;
    queue!(
        out,
        SetForegroundColor(Color::Red),
        Print("M I N E S W E E P E R"),
        ResetColor
    )?;

    for (i, item) in state.items.iter().enumerate() {
        let row = FIRST_ITEM_ROW + i as u16;
        let selected = i == state.selected;
        queue!(out, MoveTo(2, row))?;
        if selected {
            queue!(
                out,
                SetForegroundColor(Color::Black),
                SetBackgroundColor(Color::Grey),
                Print(format!(" {:<16}", item.label)),
                ResetColor,
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  {}", item.detail)),
                ResetColor
            )?;
        } else {
            queue!(
                out,
                Print(" "),
                SetForegroundColor(Color::Grey),
                Print(format!("{:<16}", item.label)),
                ResetColor,
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  {}", item.detail)),
                ResetColor
            )?;
        }
    }

    let hint_row = FIRST_ITEM_ROW + state.items.len() as u16 + 1;
    queue!(
        out,
        MoveTo(2, hint_row),
        SetForegroundColor(Color::DarkGrey),
        Print("up/down move   enter select   click to pick   q quit"),
        ResetColor
    )?;
    out.flush()
}

/// Runs the menu until the player picks something. `None` means they quit.
pub fn run<W: Write>(
    out: &mut W,
    saved_run: Option<usize>,
    scores: &Scores,
) -> io::Result<Option<MenuChoice>> {
    let mut state = MenuState::new(saved_run, scores);
    loop {
        draw(out, &state)?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                if k.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'))
                {
                    return Ok(None);
                }
                match k.code {
                    KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => state.up(),
                    KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => state.down(),
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        return Ok(match state.current() {
                            MenuChoice::Quit => None,
                            other => Some(other),
                        })
                    }
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(None),
                    _ => {}
                }
            }
            Event::Mouse(m) => match m.kind {
                MouseEventKind::Moved => {
                    if let Some(i) = state.item_at_row(m.row) {
                        state.selected = i;
                    }
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(i) = state.item_at_row(m.row) {
                        state.selected = i;
                        return Ok(match state.current() {
                            MenuChoice::Quit => None,
                            other => Some(other),
                        });
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(saved: Option<usize>) -> MenuState {
        MenuState::new(saved, &Scores::default())
    }

    #[test]
    fn without_a_saved_run_there_is_no_continue_entry() {
        let s = state(None);
        assert!(!s.items.iter().any(|i| i.choice == MenuChoice::Continue));
        assert_eq!(s.current(), MenuChoice::Infinite, "infinite should lead");
    }

    #[test]
    fn a_saved_run_adds_continue_at_the_top() {
        let s = state(Some(1284));
        assert_eq!(s.current(), MenuChoice::Continue);
        assert!(s.items[0].detail.contains("1284"));
    }

    #[test]
    fn every_mode_is_offered() {
        let s = state(None);
        let choices: Vec<_> = s.items.iter().map(|i| i.choice).collect();
        assert!(choices.contains(&MenuChoice::Infinite));
        assert!(choices.contains(&MenuChoice::Classic(Difficulty::Beginner)));
        assert!(choices.contains(&MenuChoice::Classic(Difficulty::Intermediate)));
        assert!(choices.contains(&MenuChoice::Classic(Difficulty::Expert)));
        assert!(choices.contains(&MenuChoice::Quit));
    }

    #[test]
    fn selection_wraps_at_both_ends() {
        let mut s = state(None);
        let last = s.items.len() - 1;
        s.up();
        assert_eq!(
            s.selected, last,
            "up from the top should wrap to the bottom"
        );
        s.down();
        assert_eq!(s.selected, 0, "down from the bottom should wrap to the top");
    }

    #[test]
    fn moving_down_through_every_item_returns_to_the_start() {
        let mut s = state(Some(1));
        let n = s.items.len();
        for _ in 0..n {
            s.down();
        }
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn clicks_map_to_items_and_miss_gracefully() {
        let s = state(None);
        assert_eq!(s.item_at_row(FIRST_ITEM_ROW), Some(0));
        assert_eq!(s.item_at_row(FIRST_ITEM_ROW + 1), Some(1));
        assert_eq!(
            s.item_at_row(0),
            None,
            "a click on the title picked an item"
        );
        assert_eq!(
            s.item_at_row(FIRST_ITEM_ROW + s.items.len() as u16),
            None,
            "a click below the list picked an item"
        );
    }

    #[test]
    fn best_scores_appear_when_they_exist() {
        let scores = Scores {
            infinite_best: 900,
            expert_best_secs: Some(125),
            ..Scores::default()
        };
        let s = MenuState::new(None, &scores);
        let infinite = s
            .items
            .iter()
            .find(|i| i.choice == MenuChoice::Infinite)
            .unwrap();
        assert!(infinite.detail.contains("900"));
        let expert = s
            .items
            .iter()
            .find(|i| i.choice == MenuChoice::Classic(Difficulty::Expert))
            .unwrap();
        assert!(expert.detail.contains("02:05"), "got {}", expert.detail);
    }

    #[test]
    fn the_menu_draws_without_a_terminal() {
        let mut buf = Vec::new();
        draw(&mut buf, &state(Some(10))).unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("M I N E S W E E P E R"));
        assert!(text.contains("Infinite"));
        assert!(text.contains("Expert"));
    }
}
