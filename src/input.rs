//! Turning terminal events into game actions.
//!
//! Mouse and keyboard converge on one `Action` enum here, so the game loop has a single
//! code path and both input methods are tested the same way.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

use crate::render::Viewport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Reveal(i64, i64),
    Flag(i64, i64),
    Chord(i64, i64),
    MoveCursor(i64, i64),
    RevealAtCursor,
    FlagAtCursor,
    ChordAtCursor,
    /// Scroll the view without moving the cursor.
    Pan(i64, i64),
    JumpToStart,
    Restart,
    Quit,
    Resize(u16, u16),
}

pub fn map_event(event: &Event, vp: &Viewport) -> Option<Action> {
    match event {
        Event::Resize(w, h) => Some(Action::Resize(*w, *h)),

        Event::Mouse(m) => {
            let at = |kind: fn(i64, i64) -> Action| {
                vp.to_world(m.column, m.row).map(|(x, y)| kind(x, y))
            };
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => at(Action::Reveal),
                MouseEventKind::Down(MouseButton::Right) => at(Action::Flag),
                MouseEventKind::Down(MouseButton::Middle) => at(Action::Chord),
                MouseEventKind::ScrollUp => Some(Action::Pan(0, -3)),
                MouseEventKind::ScrollDown => Some(Action::Pan(0, 3)),
                MouseEventKind::ScrollLeft => Some(Action::Pan(-3, 0)),
                MouseEventKind::ScrollRight => Some(Action::Pan(3, 0)),
                _ => None,
            }
        }

        Event::Key(k) => {
            // Windows reports key releases as well as presses. Without this filter every
            // keystroke would act twice there and once on Linux.
            if k.kind == KeyEventKind::Release {
                return None;
            }
            if k.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'))
            {
                return Some(Action::Quit);
            }
            match k.code {
                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => {
                    Some(Action::MoveCursor(0, -1))
                }
                KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => {
                    Some(Action::MoveCursor(0, 1))
                }
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                    Some(Action::MoveCursor(-1, 0))
                }
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                    Some(Action::MoveCursor(1, 0))
                }
                KeyCode::Char(' ') | KeyCode::Enter => Some(Action::RevealAtCursor),
                KeyCode::Char('f') | KeyCode::Char('F') => Some(Action::FlagAtCursor),
                KeyCode::Char('c') | KeyCode::Char('C') => Some(Action::ChordAtCursor),
                KeyCode::Char('o') | KeyCode::Char('O') => Some(Action::JumpToStart),
                KeyCode::Char('r') | KeyCode::Char('R') => Some(Action::Restart),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Some(Action::Quit),
                _ => None,
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, MouseEvent};

    fn vp() -> Viewport {
        Viewport::new(80, 24).unwrap()
    }

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    fn code(c: KeyCode) -> Event {
        Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn clicks_map_to_the_cell_under_the_pointer() {
        let mut v = vp();
        v.centre_on(500, -500);
        let (x, y) = (502, -498);
        let (col, row) = v.to_screen(x, y).unwrap();

        assert_eq!(
            map_event(
                &mouse(MouseEventKind::Down(MouseButton::Left), col, row),
                &v
            ),
            Some(Action::Reveal(x, y))
        );
        assert_eq!(
            map_event(
                &mouse(MouseEventKind::Down(MouseButton::Right), col, row),
                &v
            ),
            Some(Action::Flag(x, y))
        );
        assert_eq!(
            map_event(
                &mouse(MouseEventKind::Down(MouseButton::Middle), col, row),
                &v
            ),
            Some(Action::Chord(x, y))
        );
    }

    #[test]
    fn clicks_outside_the_board_area_do_nothing() {
        let v = vp();
        // Row 0 is the status line.
        assert_eq!(
            map_event(&mouse(MouseEventKind::Down(MouseButton::Left), 5, 0), &v),
            None
        );
        // Below the board.
        let below = (viewport::HEADER_ROWS + v.rows) as u16;
        assert_eq!(
            map_event(
                &mouse(MouseEventKind::Down(MouseButton::Left), 5, below),
                &v
            ),
            None
        );
    }

    use crate::render::viewport;

    #[test]
    fn the_scroll_wheel_pans_without_moving_the_cursor() {
        let v = vp();
        assert_eq!(
            map_event(&mouse(MouseEventKind::ScrollUp, 0, 5), &v),
            Some(Action::Pan(0, -3))
        );
        assert_eq!(
            map_event(&mouse(MouseEventKind::ScrollDown, 0, 5), &v),
            Some(Action::Pan(0, 3))
        );
    }

    #[test]
    fn arrows_and_wasd_move_the_same_way() {
        let v = vp();
        for (arrow, letter, expected) in [
            (KeyCode::Up, 'w', Action::MoveCursor(0, -1)),
            (KeyCode::Down, 's', Action::MoveCursor(0, 1)),
            (KeyCode::Left, 'a', Action::MoveCursor(-1, 0)),
            (KeyCode::Right, 'd', Action::MoveCursor(1, 0)),
        ] {
            assert_eq!(map_event(&code(arrow), &v), Some(expected));
            assert_eq!(map_event(&key(letter), &v), Some(expected));
            assert_eq!(
                map_event(&key(letter.to_ascii_uppercase()), &v),
                Some(expected),
                "uppercase {letter} should work too"
            );
        }
    }

    #[test]
    fn the_keyboard_covers_everything_the_mouse_does() {
        let v = vp();
        assert_eq!(map_event(&key(' '), &v), Some(Action::RevealAtCursor));
        assert_eq!(
            map_event(&code(KeyCode::Enter), &v),
            Some(Action::RevealAtCursor)
        );
        assert_eq!(map_event(&key('f'), &v), Some(Action::FlagAtCursor));
        assert_eq!(map_event(&key('c'), &v), Some(Action::ChordAtCursor));
    }

    #[test]
    fn quit_keys_quit() {
        let v = vp();
        assert_eq!(map_event(&key('q'), &v), Some(Action::Quit));
        assert_eq!(map_event(&code(KeyCode::Esc), &v), Some(Action::Quit));
        assert_eq!(
            map_event(
                &Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
                &v
            ),
            Some(Action::Quit),
            "ctrl-c should quit rather than chord"
        );
    }

    #[test]
    fn plain_c_chords_but_ctrl_c_does_not() {
        let v = vp();
        assert_eq!(map_event(&key('c'), &v), Some(Action::ChordAtCursor));
    }

    #[test]
    fn other_keys_are_ignored() {
        let v = vp();
        for c in ['z', 'x', '1', '\\'] {
            assert_eq!(map_event(&key(c), &v), None, "{c} should do nothing");
        }
    }

    /// On Windows crossterm emits a Release for every key. Without filtering, one press
    /// of `f` would place a flag and immediately remove it.
    #[test]
    fn key_releases_are_ignored() {
        let v = vp();
        let mut ev = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        ev.kind = KeyEventKind::Release;
        assert_eq!(map_event(&Event::Key(ev), &v), None);

        let mut press = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        press.kind = KeyEventKind::Press;
        assert_eq!(
            map_event(&Event::Key(press), &v),
            Some(Action::FlagAtCursor),
            "presses must still register"
        );
    }

    #[test]
    fn resize_is_passed_through() {
        let v = vp();
        assert_eq!(
            map_event(&Event::Resize(100, 40), &v),
            Some(Action::Resize(100, 40))
        );
    }

    #[test]
    fn restart_and_jump_are_mapped() {
        let v = vp();
        assert_eq!(map_event(&key('r'), &v), Some(Action::Restart));
        assert_eq!(map_event(&key('o'), &v), Some(Action::JumpToStart));
    }
}
