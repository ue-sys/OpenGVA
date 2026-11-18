//! The on-screen keyboard typing into something that is not a form of the
//! HMI's: another program's page.
//!
//! Nothing is known here of what is typed into - not what it holds, nor
//! whether it has anywhere for a key to go. So every key is available, and
//! what the keyboard types is not kept: it is handed on, stroke by stroke
//! ([`Typing::take_strokes`]), for whoever has the keyboard up to pass to the
//! program. At the most a program says what the field with its focus takes,
//! as a form says it of each of its own ([`Kind`]): then the keyboard is the
//! one a form would have for such a field - the pad for a number, with its
//! point if the number can have one.
//!
//! The buttons are those a form's page has while data is being entered into
//! it, each doing here what it does there: F17, F18, F6 and
//! F12 move the highlight, F20 selects the key it is on, F10 is Space and
//! F11 Backspace. Where a form has 'Next Field' and 'Previous Field', this
//! types what goes from one field to the next in a program and back - Tab,
//! and Tab with Shift - and where a form has 'Save', Return, which is how a
//! program is told that what has been entered is to be taken. They are what
//! lets a page be filled in from the bezel alone: without a
//! touch there is no other way to say which field is meant.

use crate::entry::button::{BACKSPACE, CANCEL, DOWN, ENTER, LEFT, NEXT_FIELD, PREVIOUS_FIELD, RIGHT, SAVE, SPACE, UP};
use crate::form::Kind;
use crate::keyboard::{Direction, Keyboard, KeyboardView, Layout, Stroke, Takes};

/// The button that types Return: the one that saves, on a form.
const RETURN: u8 = SAVE;

/// An on-screen keyboard that is up over something to be typed into, and
/// what it has typed that has not yet been handed on.
#[derive(Debug, Clone)]
pub struct Typing {
    keyboard: Keyboard,
    /// What is typed into takes, so far as that is known: free text if
    /// nothing is.
    kind: Kind,
    typed: Vec<Stroke>,
}

impl Default for Typing {
    fn default() -> Self {
        Self::new()
    }
}

impl Typing {
    /// The keyboard as it comes up: the QWERTY one, what is wanted being
    /// unknown, in small letters.
    pub fn new() -> Self {
        Self::of(Kind::Text)
    }

    /// The keyboard for something that takes what a form's field of this
    /// kind does: QWERTY in small letters for text, the pad for a number.
    pub fn of(kind: Kind) -> Self {
        let keyboard = match kind {
            Kind::Text => Keyboard::new(Layout::Qwerty).in_small_letters(),
            Kind::Number | Kind::Decimal => Keyboard::new(Layout::Phone),
        };
        Typing { keyboard, kind, typed: Vec::new() }
    }

    /// What the keyboard is for.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// What can be typed: anything, but for the pad's point where the
    /// number is a whole one. How much has been typed is the program's to
    /// know, and how much more it will take.
    fn takes(&self) -> Takes {
        Takes { point: self.kind == Kind::Decimal, ..Takes::ANYTHING }
    }

    /// F`n` has been pressed. Returns whether it is one of the keyboard's
    /// buttons; one that is not is for whoever has the keyboard up - F13
    /// among them, which is theirs to put it away with.
    pub fn press(&mut self, n: u8) -> bool {
        let takes = self.takes();
        let keyboard = &mut self.keyboard;
        // A key still shown as pressed, the letting go of its button having
        // gone astray, is let go now.
        keyboard.release();
        let typed = match n {
            UP | DOWN | LEFT | RIGHT => {
                keyboard.step(match n {
                    UP => Direction::Up,
                    DOWN => Direction::Down,
                    LEFT => Direction::Left,
                    _ => Direction::Right,
                });
                None
            }
            ENTER => keyboard.select(takes),
            // The pad has no space, and a number takes none.
            SPACE => keyboard.strike(Stroke::Character(' '), takes),
            BACKSPACE => keyboard.strike(Stroke::Backspace, takes),
            NEXT_FIELD => Some(Stroke::Tab),
            PREVIOUS_FIELD => Some(Stroke::BackTab),
            RETURN => Some(Stroke::Enter),
            _ => return false,
        };
        self.typed.extend(typed);
        true
    }

    /// F`n` has been let go: a key it was selecting is no longer pressed.
    pub fn release(&mut self, n: u8) {
        if matches!(n, ENTER | SPACE | BACKSPACE) {
            self.keyboard.release();
        }
    }

    /// A finger has come down on a key: it is selected. `key`
    /// is its place among the keys as [`Typing::view`] lists them.
    pub fn touch(&mut self, key: usize) {
        let typed = self.keyboard.touch(key, self.takes());
        self.typed.extend(typed);
    }

    /// The finger has gone.
    pub fn lift(&mut self) {
        self.keyboard.release();
    }

    /// What has been typed since this was last asked, in the order it was.
    pub fn take_strokes(&mut self) -> Vec<Stroke> {
        std::mem::take(&mut self.typed)
    }

    /// The labels of the keyboard's buttons that are not common functions,
    /// each with the number of its button and whether it can do anything -
    /// Space cannot, on the pad - and of F13, which is 'Cancel' while the
    /// keyboard is up as it is while a form is entered into.
    /// The words are a form's where the button does for a program what it
    /// does for a form.
    pub fn labels(&self) -> impl Iterator<Item = (u8, &'static str, bool)> {
        [
            (PREVIOUS_FIELD, "Previous\nField", true),
            (LEFT, "Left", true),
            (NEXT_FIELD, "Next\nField", true),
            (RETURN, "Return", true),
            (SPACE, "Space", self.kind == Kind::Text),
            (BACKSPACE, "Backspace", true),
            (RIGHT, "Right", true),
            (CANCEL, "Cancel", true),
        ]
        .into_iter()
    }

    /// The keyboard as it is to be drawn.
    pub fn view(&self) -> KeyboardView {
        self.keyboard.view(self.takes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Stroke::{BackTab, Backspace, Character, Enter, Tab};

    /// The place among the keys of the one that says `text`.
    fn key(typing: &Typing, text: &str) -> usize {
        let keys = typing.view().keys;
        keys.iter().position(|key| key.text == text).unwrap_or_else(|| panic!("no key says {text}"))
    }

    /// What is written on the key that is lit, if one is.
    fn lit(typing: &Typing) -> Option<String> {
        typing.view().keys.into_iter().find(|key| key.pressed).map(|key| key.text)
    }

    #[test]
    fn the_keyboard_comes_up_in_small_letters_with_every_key_available() {
        let typing = Typing::new();
        let view = typing.view();
        assert_eq!(view.layout, Layout::Qwerty);
        assert!(view.keys.iter().all(|key| key.available));
        assert!(view.keys.iter().any(|key| key.text == "g" && key.highlighted));
        assert!(view.keys.iter().all(|key| !key.on && !key.pressed));
    }

    #[test]
    fn the_buttons_of_data_entry_type_and_what_is_typed_is_handed_on_once() {
        let mut typing = Typing::new();
        // Enter selects the key the highlight is on; F12 and F18 move it.
        assert!(typing.press(ENTER));
        assert_eq!(lit(&typing).as_deref(), Some("g"), "Lit while the button is down");
        typing.release(ENTER);
        assert_eq!(lit(&typing), None);
        for n in [RIGHT, ENTER, DOWN, ENTER, LEFT, UP, UP, ENTER] {
            assert!(typing.press(n));
            typing.release(n);
        }
        // Space and Backspace have buttons, which light their keys.
        assert!(typing.press(SPACE));
        assert_eq!(lit(&typing).as_deref(), Some("Space"));
        typing.release(SPACE);
        assert!(typing.press(BACKSPACE));
        assert_eq!(lit(&typing).as_deref(), Some("Backspace"));
        typing.release(BACKSPACE);
        // And the buttons that go from field to field on a form, and save.
        assert!(typing.press(NEXT_FIELD) && typing.press(PREVIOUS_FIELD) && typing.press(RETURN));
        assert_eq!(
            typing.take_strokes(),
            [Character('g'), Character('h'), Character('b'), Character('t'), Character(' '), Backspace, Tab, BackTab, Enter]
        );
        assert_eq!(typing.take_strokes(), [], "handed on once");
    }

    #[test]
    fn a_button_that_is_not_the_keyboards_is_its_owners() {
        let mut typing = Typing::new();
        let before = typing.view();
        for n in [2, 3, 4, 5, 9, CANCEL, 14, 15, 16, 19] {
            assert!(!typing.press(n), "F{n}");
        }
        assert_eq!(typing.view(), before);
        assert_eq!(typing.take_strokes(), []);
        // Every button it does answer says so, and F13 is 'Cancel'.
        let labelled: Vec<u8> = typing.labels().map(|(n, _, _)| n).collect();
        assert_eq!(labelled, [PREVIOUS_FIELD, LEFT, NEXT_FIELD, RETURN, SPACE, BACKSPACE, RIGHT, CANCEL]);
        assert!(typing.labels().any(|label| label == (CANCEL, "Cancel", true)));
        assert!(typing.labels().all(|(_, _, able)| able));
    }

    #[test]
    fn a_number_is_typed_on_the_pad_with_its_point_if_it_can_have_one() {
        // A whole number: the digits, and no point.
        let mut whole = Typing::of(Kind::Number);
        let view = whole.view();
        assert_eq!((view.layout, whole.kind()), (Layout::Phone, Kind::Number));
        assert!(view.keys.iter().all(|key| key.available == (key.text != ".")));
        assert!(view.keys.iter().any(|key| key.text == "5" && key.highlighted));
        for text in ["4", ".", "0", "Backspace"] {
            whole.touch(key(&whole, text));
            whole.lift();
        }
        // Space has no key on the pad: its button does nothing, and says so.
        assert!(whole.press(SPACE) && whole.press(ENTER) && whole.press(NEXT_FIELD) && whole.press(RETURN));
        assert_eq!(
            whole.take_strokes(),
            [Character('4'), Character('0'), Backspace, Backspace, Tab, Enter],
            "the point types nothing; Enter selects the key last touched, which the highlight went to"
        );
        assert!(whole.labels().all(|(n, _, able)| able == (n != SPACE)));

        // One that can have a point has the pad's.
        let mut decimal = Typing::of(Kind::Decimal);
        assert!(decimal.view().keys.iter().all(|key| key.available));
        for text in ["1", ".", "5"] {
            decimal.touch(key(&decimal, text));
        }
        assert_eq!(decimal.take_strokes(), [Character('1'), Character('.'), Character('5')]);
        // And free text is what is typed where nothing is said.
        assert_eq!((Typing::new().kind(), Typing::of(Kind::Text).view().layout), (Kind::Text, Layout::Qwerty));
    }

    #[test]
    fn a_touched_key_types_and_is_lit_until_the_finger_goes() {
        let mut typing = Typing::new();
        typing.touch(key(&typing, "k"));
        assert_eq!(lit(&typing).as_deref(), Some("k"));
        assert!(typing.view().keys.iter().any(|key| key.text == "k" && key.highlighted), "the highlight follows");
        typing.lift();
        assert_eq!(lit(&typing), None);

        // Caps and the symbols change the keyboard and type nothing.
        for text in ["Caps", "k", "#+=", "@", "ABC"] {
            typing.touch(key(&typing, if text == "k" { "K" } else { text }));
            typing.lift();
        }
        typing.touch(999);
        assert_eq!(typing.take_strokes(), [Character('k'), Character('K'), Character('@')]);
    }
}
