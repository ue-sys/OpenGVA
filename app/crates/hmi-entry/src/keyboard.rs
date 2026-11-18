//! The on-screen keyboard.
//!
//! There are two: a QWERTY keyboard
//! for free text and a 'phone style' pad for numbers. Either is keys on a
//! grid. A key is selected by touching it or - what touch can do, the bezel
//! can do too - by moving a highlight to it and selecting the
//! key the highlight is on. Which buttons do that is for whatever owns the
//! keyboard to say: this is told "left", "up", "select".
//!
//! Selecting a key types a character or takes one away ([`Stroke`]), or
//! changes the keyboard: Caps turns the letters to capitals and back, and the
//! key under it turns them to symbols and back. Where the strokes go is not
//! this module's business either. It is told what the thing being typed into
//! can take ([`Takes`]), so that a key that would do nothing is shown as
//! unavailable, as a label is.

use serde::Serialize;

/// Which keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Letters in a typewriter's order, with the digits above them and
    /// symbols behind them: for free text.
    Qwerty,
    /// The digits as a telephone has them, and a decimal point: for numbers.
    Phone,
}

impl Layout {
    /// The grid its keys are placed on: how many columns, and how many rows.
    /// A column of the QWERTY keyboard is half a key, so that a row can be
    /// set half a key in from the one above it.
    fn grid(self) -> (u8, u8) {
        match self {
            Layout::Qwerty => (20, 5),
            Layout::Phone => (3, 4),
        }
    }

    /// The key the highlight starts on, by what it types: one in the middle,
    /// from which no key is far.
    fn home(self) -> char {
        match self {
            Layout::Qwerty => 'g',
            Layout::Phone => '5',
        }
    }
}

/// A way for the highlight to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// What selecting a key has typed - or, of the last three, what a button
/// has that stands for a key no keyboard here has: a form has no use for
/// them, and a program that is typed into has ([`Typing`](crate::Typing)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Character(char),
    /// Take the last character away.
    Backspace,
    /// What was entered is to be taken: Return.
    Enter,
    /// On to the next place to type.
    Tab,
    /// Back to the one before: Tab, with Shift.
    BackTab,
}

/// What the thing being typed into can take now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Takes {
    /// Another character: there is room for one.
    pub character: bool,
    /// A decimal point, where there is room: it is a number that can have
    /// one, and has none yet. This is the pad's point; a keyboard for text
    /// has a full stop, which is a character like any other.
    pub point: bool,
    /// A backspace: there is a character to take away.
    pub backspace: bool,
}

impl Takes {
    pub const ANYTHING: Takes = Takes { character: true, point: true, backspace: true };
}

/// What selecting a key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Does {
    /// Types a character: `letter`, as a capital while Caps is on, or
    /// `symbol` while the symbols are showing. A digit is the same either
    /// way, and so is a mark that is there whichever is showing.
    Types { letter: char, symbol: char },
    Space,
    /// Types a decimal point.
    Point,
    Backspace,
    /// Turns the letters to capitals, and back.
    Caps,
    /// Turns the letters to symbols, and back.
    Symbols,
}

/// A key, and its place on the grid: its row, the column it starts in - both
/// counted from 0 - and how many columns it takes.
#[derive(Debug, Clone, Copy)]
struct Placed {
    does: Does,
    row: u8,
    column: u8,
    span: u8,
}

impl Placed {
    /// Where its middle is across the grid, in half columns: a whole number
    /// whether the key is an odd number of columns wide or an even.
    fn middle(&self) -> u8 {
        2 * self.column + self.span
    }
}

/// Keys that type, side by side from `column`, each `span` columns wide: each
/// a letter, and the symbol in its place while the symbols show.
fn typing(row: u8, column: u8, span: u8, letters: &str, symbols: &str) -> impl Iterator<Item = Placed> {
    letters.chars().zip(symbols.chars()).zip(0u8..).map(move |((letter, symbol), i)| Placed {
        does: Does::Types { letter, symbol },
        row,
        column: column + i * span,
        span,
    })
}

/// The QWERTY keyboard's keys, row by row and left to right. The digits are a row of their own, always there: a
/// call sign or a grid reference is letters and digits together. The symbols
/// are every mark of ASCII's but two that nobody types, `^` and `` ` ``; the
/// four wanted most are beside the space bar whichever is showing.
fn qwerty() -> Vec<Placed> {
    let key = |does, row, column, span| Placed { does, row, column, span };
    let mut keys = Vec::new();
    keys.extend(typing(0, 0, 2, "1234567890", "1234567890"));
    keys.extend(typing(1, 0, 2, "qwertyuiop", "!?'\"()&:;@"));
    keys.extend(typing(2, 1, 2, "asdfghjkl", "+=*%#_<>$"));
    keys.push(key(Does::Caps, 3, 0, 3));
    keys.extend(typing(3, 3, 2, "zxcvbnm", "[]{}\\|~"));
    keys.push(key(Does::Backspace, 3, 17, 3));
    keys.push(key(Does::Symbols, 4, 0, 3));
    keys.push(key(Does::Space, 4, 3, 9));
    keys.extend(typing(4, 12, 2, ",.-/", ",.-/"));
    keys
}

/// The phone pad's keys: 1 2 3 along the
/// top, as a telephone has them and a calculator has not, and 0 under the 8,
/// between the decimal point and Backspace. It is the one pad for every
/// number: where the number is a whole one, the point is unavailable.
fn phone() -> Vec<Placed> {
    let key = |does, column| Placed { does, row: 3, column, span: 1 };
    let mut keys = Vec::new();
    for (row, digits) in (0u8..).zip(["123", "456", "789"]) {
        keys.extend(typing(row, 0, 1, digits, digits));
    }
    keys.push(key(Does::Point, 0));
    keys.extend(typing(3, 1, 1, "0", "0"));
    keys.push(key(Does::Backspace, 2));
    keys
}

/// An on-screen keyboard: its keys, where the highlight is, and whether its
/// letters are capitals or symbols just now.
#[derive(Debug, Clone)]
pub struct Keyboard {
    layout: Layout,
    /// Row by row, and left to right in each.
    keys: Vec<Placed>,
    /// The key the highlight is on: its place in `keys`.
    highlighted: usize,
    /// Where across the keyboard the highlight was last put, by going
    /// sideways or by a finger. Going up and down it keeps to this, so that
    /// down and then up comes back to the same key though the rows are not
    /// in line.
    across: u8,
    caps: bool,
    symbols: bool,
    /// The key being selected, for as long as that lasts: until the button
    /// or the finger that selected it is let go.
    pressed: Option<usize>,
}

impl Keyboard {
    /// A keyboard as it comes up: the highlight in the middle, and capitals,
    /// which is how call signs, grid references and the like are written.
    pub fn new(layout: Layout) -> Self {
        let keys = match layout {
            Layout::Qwerty => qwerty(),
            Layout::Phone => phone(),
        };
        let home = layout.home();
        let highlighted = keys
            .iter()
            .position(|key| matches!(key.does, Does::Types { letter, .. } if letter == home))
            .unwrap_or_default();
        let across = keys[highlighted].middle();
        Keyboard { layout, keys, highlighted, across, caps: true, symbols: false, pressed: None }
    }

    /// The same keyboard with its letters small: for where what is typed is
    /// more often words than call signs.
    pub fn in_small_letters(mut self) -> Self {
        self.caps = false;
        self
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Move the highlight to the next key that way. It goes round: off one
    /// edge of the keyboard is on at the other, which halves the way to the
    /// farthest key.
    pub fn step(&mut self, direction: Direction) {
        let keys = &self.keys;
        let from = keys[self.highlighted];
        let in_row = |row: u8| (0..keys.len()).filter(move |&key| keys[key].row == row);
        match direction {
            Direction::Left | Direction::Right => {
                let row: Vec<usize> = in_row(from.row).collect();
                let place = row.iter().position(|&key| key == self.highlighted).unwrap_or_default();
                let to = if direction == Direction::Right { place + 1 } else { place + row.len() - 1 };
                self.highlighted = row[to % row.len()];
                self.across = keys[self.highlighted].middle();
            }
            Direction::Up | Direction::Down => {
                let (_, rows) = self.layout.grid();
                let row = if direction == Direction::Down { from.row + 1 } else { from.row + rows - 1 } % rows;
                // The key of that row nearest to where the highlight was put
                // across; of two as near, the left.
                let nearest = in_row(row).min_by_key(|&key| keys[key].middle().abs_diff(self.across));
                self.highlighted = nearest.unwrap_or(self.highlighted);
            }
        }
    }

    /// Select the key the highlight is on: what Enter does.
    pub fn select(&mut self, takes: Takes) -> Option<Stroke> {
        self.choose(self.highlighted, takes)
    }

    /// A finger has come down on a key: it is selected, and the
    /// highlight goes to it. `key` is its place among the keys as
    /// [`Keyboard::view`] lists them.
    pub fn touch(&mut self, key: usize, takes: Takes) -> Option<Stroke> {
        let placed = self.keys.get(key)?;
        self.highlighted = key;
        self.across = placed.middle();
        self.choose(key, takes)
    }

    /// Select the key that types `stroke`, wherever the highlight is, and
    /// without moving it: for a button that stands for a key. `None`, and
    /// nothing done, if no key types it just now.
    pub fn strike(&mut self, stroke: Stroke, takes: Takes) -> Option<Stroke> {
        let key = self.keys.iter().position(|key| match stroke {
            Stroke::Character(character) => self.types(key.does) == Some(character),
            Stroke::Backspace => key.does == Does::Backspace,
            Stroke::Enter | Stroke::Tab | Stroke::BackTab => false,
        })?;
        self.choose(key, takes)
    }

    /// The button or the finger that selected a key has been let go.
    pub fn release(&mut self) {
        self.pressed = None;
    }

    fn choose(&mut self, key: usize, takes: Takes) -> Option<Stroke> {
        let does = self.keys[key].does;
        if !self.available(does, takes) {
            return None;
        }
        self.pressed = Some(key);
        match does {
            Does::Caps => self.caps = !self.caps,
            Does::Symbols => self.symbols = !self.symbols,
            Does::Backspace => return Some(Stroke::Backspace),
            Does::Types { .. } | Does::Space | Does::Point => return self.types(does).map(Stroke::Character),
        }
        None
    }

    /// The character a key types as the keyboard now is, if it types one.
    fn types(&self, does: Does) -> Option<char> {
        match does {
            Does::Types { symbol, .. } if self.symbols => Some(symbol),
            Does::Types { letter, .. } if self.caps => Some(letter.to_ascii_uppercase()),
            Does::Types { letter, .. } => Some(letter),
            Does::Space => Some(' '),
            Does::Point => Some('.'),
            Does::Backspace | Does::Caps | Does::Symbols => None,
        }
    }

    /// Whether selecting a key would do anything now.
    fn available(&self, does: Does, takes: Takes) -> bool {
        match does {
            Does::Types { .. } | Does::Space => takes.character,
            Does::Point => takes.character && takes.point,
            Does::Backspace => takes.backspace,
            // There are no capitals among the symbols.
            Does::Caps => !self.symbols,
            Does::Symbols => true,
        }
    }

    /// The keyboard as it is to be drawn, typing into something that can
    /// take `takes`.
    pub fn view(&self, takes: Takes) -> KeyboardView {
        let (columns, rows) = self.layout.grid();
        let keys = (0..)
            .zip(&self.keys)
            .map(|(at, key)| Key {
                text: match key.does {
                    Does::Space => "Space".to_string(),
                    Does::Backspace => "Backspace".to_string(),
                    Does::Caps => "Caps".to_string(),
                    // It says what it will show.
                    Does::Symbols => if self.symbols { "ABC" } else { "#+=" }.to_string(),
                    Does::Types { .. } | Does::Point => self.types(key.does).map(String::from).unwrap_or_default(),
                },
                kind: match key.does {
                    Does::Types { .. } | Does::Point => KeyKind::Character,
                    Does::Space => KeyKind::Space,
                    Does::Backspace => KeyKind::Backspace,
                    Does::Caps => KeyKind::Caps,
                    Does::Symbols => KeyKind::Symbols,
                },
                row: key.row,
                column: key.column,
                span: key.span,
                available: self.available(key.does, takes),
                highlighted: at == self.highlighted,
                pressed: self.pressed == Some(at),
                on: key.does == Does::Caps && self.caps && !self.symbols,
            })
            .collect();
        KeyboardView { layout: self.layout, columns, rows, keys }
    }
}

/// An on-screen keyboard as it is to be drawn: its keys, each with its place
/// on a grid of `columns` by `rows`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyboardView {
    pub layout: Layout,
    pub columns: u8,
    pub rows: u8,
    /// Row by row, and left to right in each. A key touched is told of by
    /// its place in this list ([`Keyboard::touch`]).
    pub keys: Vec<Key>,
}

/// What a key is, for the UI to know how to draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
    Character,
    Space,
    Backspace,
    Caps,
    Symbols,
}

/// A key of the keyboard as it is to be drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Key {
    /// What is written on it: the character it types as the keyboard now
    /// is, or what it does.
    pub text: String,
    pub kind: KeyKind,
    /// Its place on the grid: its row and the column it starts in, both
    /// counted from 0...
    pub row: u8,
    pub column: u8,
    /// ...and how many columns it takes.
    pub span: u8,
    /// Whether selecting it would do anything now. One that would not is
    /// drawn as an unavailable label is.
    pub available: bool,
    /// Whether the highlight is on it: it is the key Enter selects.
    pub highlighted: bool,
    /// Whether it is being selected now: the visual response to that,
    /// which lasts until the button or the finger is let go.
    pub pressed: bool,
    /// Whether it is a key that stays on, and is on: Caps.
    pub on: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::{Down, Left, Right, Up};
    use std::collections::BTreeSet;

    const FULL: Takes = Takes { character: false, point: true, backspace: true };
    const EMPTY: Takes = Takes { character: true, point: true, backspace: false };
    /// A whole number, or one that has its point already.
    const POINTLESS: Takes = Takes { character: true, point: false, backspace: true };

    fn view(keyboard: &Keyboard) -> Vec<Key> {
        keyboard.view(Takes::ANYTHING).keys
    }

    /// What is written on the keys, in their order.
    fn texts(keyboard: &Keyboard) -> Vec<String> {
        view(keyboard).into_iter().map(|key| key.text).collect()
    }

    /// The place among the keys of the one that says `text`.
    fn key(keyboard: &Keyboard, text: &str) -> usize {
        texts(keyboard).iter().position(|said| said == text).unwrap_or_else(|| panic!("no key says {text}"))
    }

    /// What is written on the key the highlight is on.
    fn highlighted(keyboard: &Keyboard) -> String {
        let keys: Vec<Key> = view(keyboard).into_iter().filter(|key| key.highlighted).collect();
        assert_eq!(keys.len(), 1, "the highlight is on one key");
        keys[0].text.clone()
    }

    /// The keyboard with a finger having come down on the key that says
    /// `text`, and gone.
    fn touched(mut keyboard: Keyboard, text: &str) -> Keyboard {
        let at = key(&keyboard, text);
        keyboard.touch(at, Takes::ANYTHING);
        keyboard.release();
        keyboard
    }

    /// Step the highlight these ways and say where it ends.
    fn go(keyboard: &mut Keyboard, steps: &[Direction]) -> String {
        steps.iter().for_each(|&step| keyboard.step(step));
        highlighted(keyboard)
    }

    #[test]
    fn the_qwerty_keyboard_is_the_digits_the_letters_and_what_goes_round_them() {
        let keyboard = Keyboard::new(Layout::Qwerty);
        let rows: Vec<String> = (0..5)
            .map(|row| {
                let keys = view(&keyboard).into_iter().filter(|key| key.row == row);
                keys.map(|key| key.text).collect::<Vec<_>>().join(" ")
            })
            .collect();
        assert_eq!(
            rows,
            [
                "1 2 3 4 5 6 7 8 9 0",
                "Q W E R T Y U I O P",
                "A S D F G H J K L",
                "Caps Z X C V B N M Backspace",
                "#+= Space , . - /",
            ]
        );
        assert_eq!(highlighted(&keyboard), "G", "the highlight starts in the middle");
    }

    #[test]
    fn the_phone_pad_is_the_digits_as_a_telephone_has_them() {
        let keyboard = Keyboard::new(Layout::Phone);
        assert_eq!(texts(&keyboard), ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", "Backspace"]);
        let places: Vec<(u8, u8)> = view(&keyboard).iter().map(|key| (key.row, key.column)).collect();
        assert_eq!(places[..3], [(0, 0), (0, 1), (0, 2)], "1 2 3 along the top");
        assert_eq!(places[9..], [(3, 0), (3, 1), (3, 2)], "0 under the 8, between the point and Backspace");
        assert_eq!(highlighted(&keyboard), "5");
    }

    #[test]
    fn the_keys_fill_their_grid_and_none_lies_over_another() {
        // How much of each row is keys: the QWERTY keyboard's third row is
        // set in half a key at each end.
        for (layout, filled) in [(Layout::Qwerty, vec![20, 20, 18, 20, 20]), (Layout::Phone, vec![3, 3, 3, 3])] {
            let drawn = Keyboard::new(layout).view(Takes::ANYTHING);
            let mut taken = BTreeSet::new();
            for key in &drawn.keys {
                assert!(key.span > 0 && key.column + key.span <= drawn.columns && key.row < drawn.rows, "{key:?}");
                for column in key.column..key.column + key.span {
                    assert!(taken.insert((key.row, column)), "{layout:?}: two keys at row {} column {column}", key.row);
                }
            }
            let in_row = |row| taken.iter().filter(|&&(r, _)| r == row).count();
            assert_eq!((0..drawn.rows).map(in_row).collect::<Vec<_>>(), filled, "{layout:?}");
            // Row by row and left to right: the order a touch names them by.
            let order: Vec<_> = drawn.keys.iter().map(|key| (key.row, key.column)).collect();
            assert!(order.is_sorted(), "{layout:?}");
        }
    }

    #[test]
    fn free_text_is_every_character_of_ascii_but_two() {
        let mut typed = BTreeSet::new();
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        // Capitals as it comes up, then small letters, then the symbols.
        for change in ["Caps", "#+=", "ABC", "Caps"] {
            for at in 0..view(&keyboard).len() {
                if let Some(Stroke::Character(character)) = keyboard.clone().touch(at, Takes::ANYTHING) {
                    typed.insert(character);
                }
            }
            keyboard = touched(keyboard, change);
        }
        let all: BTreeSet<char> = (' '..='~').filter(|&c| c != '^' && c != '`').collect();
        assert_eq!(typed, all);
    }

    #[test]
    fn the_phone_pad_types_digits_and_a_decimal_point() {
        let mut keyboard = Keyboard::new(Layout::Phone);
        let typed: Vec<_> = (0..12).map(|at| keyboard.touch(at, Takes::ANYTHING)).collect();
        let characters: Vec<_> = "123456789.0".chars().map(|character| Some(Stroke::Character(character))).collect();
        assert_eq!(typed[..11], characters);
        assert_eq!(typed[11], Some(Stroke::Backspace));

        // The point is not there to be typed into a whole
        // number, or into one that has its point - and is there again when
        // that has been taken away. Nor into anything that is full.
        let point = key(&keyboard, ".");
        let unavailable = |takes| keyboard.view(takes).keys.into_iter().filter(|key| !key.available).count();
        assert_eq!(unavailable(POINTLESS), 1);
        assert!(!keyboard.view(POINTLESS).keys[point].available);
        assert_eq!(keyboard.clone().touch(point, POINTLESS), None);
        assert_eq!(keyboard.clone().strike(Stroke::Character('.'), POINTLESS), None);
        assert_eq!(keyboard.clone().touch(point, FULL), None);
        assert_eq!(keyboard.touch(point, EMPTY), Some(Stroke::Character('.')));
    }

    #[test]
    fn caps_turns_the_letters_to_capitals_and_back() {
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        let caps = |keyboard: &Keyboard| view(keyboard).into_iter().find(|key| key.kind == KeyKind::Caps).unwrap();
        // Capitals as it comes up, and the key says that it is on.
        assert!(caps(&keyboard).on);
        assert_eq!(keyboard.select(Takes::ANYTHING), Some(Stroke::Character('G')));

        keyboard = touched(keyboard, "Caps");
        assert!(!caps(&keyboard).on);
        assert_eq!(texts(&keyboard)[10..13], ["q", "w", "e"], "the keys say what they will type");
        assert_eq!(texts(&keyboard)[..3], ["1", "2", "3"], "a digit has no capital");
        let q = key(&keyboard, "q");
        assert_eq!(keyboard.touch(q, Takes::ANYTHING), Some(Stroke::Character('q')));

        keyboard = touched(keyboard, "Caps");
        assert!(caps(&keyboard).on);
        assert_eq!(texts(&keyboard)[10], "Q");
    }

    #[test]
    fn the_key_under_caps_turns_the_letters_to_symbols_and_back() {
        let mut keyboard = touched(Keyboard::new(Layout::Qwerty), "#+=");
        let rows = |keyboard: &Keyboard| {
            let said = texts(keyboard);
            [&said[..10], &said[10..20], &said[20..29], &said[30..37], &said[40..]].map(|row| row.concat())
        };
        assert_eq!(rows(&keyboard), ["1234567890", "!?'\"()&:;@", "+=*%#_<>$", "[]{}\\|~", ",.-/"]);
        // The key says what it will show, and Caps has nothing to do.
        assert_eq!(view(&keyboard)[38].text, "ABC");
        let caps = view(&keyboard)[29].clone();
        assert_eq!((caps.kind, caps.available, caps.on), (KeyKind::Caps, false, false));
        let at = key(&keyboard, "Caps");
        assert_eq!(keyboard.touch(at, Takes::ANYTHING), None);
        let ampersand = key(&keyboard, "&");
        assert_eq!(keyboard.touch(ampersand, Takes::ANYTHING), Some(Stroke::Character('&')));

        // And back: capitals, as they were.
        keyboard = touched(keyboard, "ABC");
        assert_eq!(rows(&keyboard)[1], "QWERTYUIOP");
        assert_eq!(view(&keyboard)[38].text, "#+=");
        assert!(view(&keyboard)[29].on);
    }

    #[test]
    fn every_key_is_reached_and_selected_without_touch() {
        for layout in [Layout::Qwerty, Layout::Phone] {
            // Every place the highlight can be got to from where it starts.
            let mut reached = BTreeSet::new();
            let mut to_try = vec![Keyboard::new(layout)];
            while let Some(keyboard) = to_try.pop() {
                if reached.insert((keyboard.highlighted, keyboard.across)) {
                    to_try.extend([Up, Down, Left, Right].map(|way| {
                        let mut next = keyboard.clone();
                        next.step(way);
                        next
                    }));
                }
            }
            let keys: BTreeSet<usize> = reached.iter().map(|&(key, _)| key).collect();
            assert_eq!(keys.len(), Keyboard::new(layout).keys.len(), "{layout:?}");
        }

        // And the one it is on is selected as a touch would select it.
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        assert_eq!(go(&mut keyboard, &[Up, Left, Left, Left]), "W");
        assert_eq!(keyboard.select(Takes::ANYTHING), Some(Stroke::Character('W')));
        assert_eq!(go(&mut keyboard, &[Down, Down, Left]), "Caps");
        assert_eq!(keyboard.select(Takes::ANYTHING), None, "Caps types nothing");
        assert_eq!(go(&mut keyboard, &[Up, Up]), "q");
        assert_eq!(keyboard.select(Takes::ANYTHING), Some(Stroke::Character('q')));
    }

    #[test]
    fn the_highlight_goes_to_the_nearest_key_above_or_below() {
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        // G is under T and Y: of two as near, the left.
        assert_eq!(go(&mut keyboard, &[Up]), "T");
        assert_eq!(go(&mut keyboard, &[Up]), "5");
        // Down again is the way it came: G, not the F that is under T.
        assert_eq!(go(&mut keyboard, &[Down, Down]), "G");
        assert_eq!(go(&mut keyboard, &[Down, Down]), "Space");
        assert_eq!(go(&mut keyboard, &[Up, Up]), "G");

        // Going sideways is what says where across it is.
        assert_eq!(go(&mut keyboard, &[Right, Right, Right, Right]), "L");
        assert_eq!(go(&mut keyboard, &[Down]), "Backspace");
        assert_eq!(go(&mut keyboard, &[Down]), "-");
        assert_eq!(go(&mut keyboard, &[Left, Left, Left]), "Space");
        assert_eq!(go(&mut keyboard, &[Up]), "C", "over the middle of the space bar");

        // Whichever key it is on, down and then up is that key again, and up and then down.
        for layout in [Layout::Qwerty, Layout::Phone] {
            for at in 0..Keyboard::new(layout).keys.len() {
                let mut keyboard = Keyboard::new(layout);
                keyboard.touch(at, Takes::ANYTHING);
                let from = highlighted(&keyboard);
                assert_eq!(go(&mut keyboard, &[Down, Up]), from);
                assert_eq!(go(&mut keyboard, &[Up, Down]), from);
            }
        }
    }

    #[test]
    fn the_highlight_goes_round_at_the_edges() {
        let mut keyboard = touched(Keyboard::new(Layout::Qwerty), "Q");
        assert_eq!(go(&mut keyboard, &[Left]), "P");
        assert_eq!(go(&mut keyboard, &[Right]), "Q");
        assert_eq!(go(&mut keyboard, &[Up]), "1");
        assert_eq!(go(&mut keyboard, &[Up]), "#+=", "off the top is on at the bottom");
        assert_eq!(go(&mut keyboard, &[Down]), "1");

        // The pad likewise. The highlight can be on a key that would do
        // nothing: where the keys are does not change with what they can do.
        let mut pad = touched(Keyboard::new(Layout::Phone), "7");
        assert_eq!(go(&mut pad, &[Down]), ".");
        assert_eq!(go(&mut pad, &[Left]), "Backspace");
        assert_eq!(go(&mut pad, &[Right, Right]), "0");
        assert_eq!(go(&mut pad, &[Down]), "2");
        assert_eq!(go(&mut pad, &[Up, Up]), "8");
    }

    #[test]
    fn a_key_is_shown_as_pressed_while_it_is_being_selected() {
        let pressed = |keyboard: &Keyboard| {
            view(keyboard).into_iter().filter(|key| key.pressed).map(|key| key.text).collect::<Vec<_>>()
        };
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        assert!(pressed(&keyboard).is_empty());

        // By Enter: the key the highlight is on, until Enter is let go.
        keyboard.select(Takes::ANYTHING);
        assert_eq!(pressed(&keyboard), ["G"]);
        keyboard.release();
        assert!(pressed(&keyboard).is_empty());

        // By a finger: the key touched, and the highlight goes there too.
        let m = key(&keyboard, "M");
        keyboard.touch(m, Takes::ANYTHING);
        assert_eq!(pressed(&keyboard), ["M"]);
        assert_eq!(highlighted(&keyboard), "M");
        // A second finger down before the first is up: the key it is on.
        let n = key(&keyboard, "N");
        keyboard.touch(n, Takes::ANYTHING);
        assert_eq!(pressed(&keyboard), ["N"]);
        keyboard.release();

        // A key that changes the keyboard says so twice: it is pressed, and the keys change.
        let caps = key(&keyboard, "Caps");
        keyboard.touch(caps, Takes::ANYTHING);
        assert_eq!(pressed(&keyboard), ["Caps"]);
        assert_eq!(texts(&keyboard)[10], "q");

        // A place where there is no key is no key.
        assert_eq!(keyboard.touch(99, Takes::ANYTHING), None);
        assert_eq!(pressed(&keyboard), ["Caps"]);
    }

    #[test]
    fn a_key_that_would_do_nothing_is_unavailable_and_does_nothing() {
        let unavailable = |keyboard: &Keyboard, takes| {
            let keys = keyboard.view(takes).keys.into_iter();
            keys.filter(|key| !key.available).map(|key| key.text).collect::<Vec<_>>()
        };
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        assert!(unavailable(&keyboard, Takes::ANYTHING).is_empty());

        // Nothing typed yet: there is nothing to take away.
        assert_eq!(unavailable(&keyboard, EMPTY), ["Backspace"]);
        let backspace = key(&keyboard, "Backspace");
        assert_eq!(keyboard.touch(backspace, EMPTY), None);
        assert!(view(&keyboard).iter().all(|key| !key.pressed), "nor is it shown as pressed");
        assert!(view(&keyboard)[backspace].highlighted, "though the highlight can be on it");

        // No room for more: every key that types is unavailable, and the rest are not.
        let full = unavailable(&keyboard, FULL);
        assert_eq!(full.len(), 41, "all but Caps, Backspace and the symbols' key");
        assert!(["Caps", "Backspace", "#+="].iter().all(|text| !full.contains(&text.to_string())));
        assert_eq!(keyboard.touch(0, FULL), None);
        assert_eq!(keyboard.strike(Stroke::Character(' '), FULL), None);
        assert_eq!(keyboard.touch(backspace, FULL), Some(Stroke::Backspace));

        let mut pad = Keyboard::new(Layout::Phone);
        assert_eq!(unavailable(&pad, FULL), ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0"]);
        assert_eq!(pad.select(FULL), None);
    }

    #[test]
    fn a_button_can_stand_for_a_key() {
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        assert_eq!(keyboard.strike(Stroke::Character(' '), Takes::ANYTHING), Some(Stroke::Character(' ')));
        // The key it stands for is the one shown as pressed, and the highlight stays.
        let space = key(&keyboard, "Space");
        assert!(view(&keyboard)[space].pressed);
        assert_eq!(highlighted(&keyboard), "G");
        assert_eq!(keyboard.strike(Stroke::Backspace, Takes::ANYTHING), Some(Stroke::Backspace));
        assert_eq!(keyboard.strike(Stroke::Backspace, EMPTY), None);

        // The pad has no space bar.
        let mut pad = Keyboard::new(Layout::Phone);
        assert_eq!(pad.strike(Stroke::Character(' '), Takes::ANYTHING), None);
        assert_eq!(pad.strike(Stroke::Backspace, Takes::ANYTHING), Some(Stroke::Backspace));
    }

    #[test]
    fn a_keyboard_serialises_in_the_shape_the_ui_reads() {
        let mut keyboard = Keyboard::new(Layout::Qwerty);
        keyboard.select(EMPTY);
        let json = serde_json::to_value(keyboard.view(EMPTY)).unwrap();
        assert_eq!((&json["layout"], &json["columns"], &json["rows"]), (&"qwerty".into(), &20.into(), &5.into()));
        assert_eq!(json["keys"].as_array().unwrap().len(), 44);
        assert_eq!(
            json["keys"][24],
            serde_json::json!({
                "text": "G", "kind": "character", "row": 2, "column": 9, "span": 2,
                "available": true, "highlighted": true, "pressed": true, "on": false,
            })
        );
        assert_eq!(
            json["keys"][29],
            serde_json::json!({
                "text": "Caps", "kind": "caps", "row": 3, "column": 0, "span": 3,
                "available": true, "highlighted": false, "pressed": false, "on": true,
            })
        );
        let said = |at: usize| {
            let key = &json["keys"][at];
            (key["text"].clone(), key["kind"].clone(), key["available"].clone())
        };
        assert_eq!(said(37), ("Backspace".into(), "backspace".into(), false.into()));
        assert_eq!(said(38), ("#+=".into(), "symbols".into(), true.into()));
        assert_eq!(said(39), ("Space".into(), "space".into(), true.into()));

        let json = serde_json::to_value(Keyboard::new(Layout::Phone).view(Takes::ANYTHING)).unwrap();
        assert_eq!((&json["layout"], &json["columns"], &json["rows"]), (&"phone".into(), &3.into(), &4.into()));
        assert_eq!(json["keys"].as_array().unwrap().len(), 12);
        assert_eq!(json["keys"][9], serde_json::json!({
            "text": ".", "kind": "character", "row": 3, "column": 0, "span": 1,
            "available": true, "highlighted": false, "pressed": false, "on": false,
        }));
        assert_eq!((&json["keys"][10]["text"], &json["keys"][11]["kind"]), (&"0".into(), &"backspace".into()));
    }
}
