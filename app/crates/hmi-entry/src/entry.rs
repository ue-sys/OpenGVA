//! Data entry: a form being filled in.
//!
//! The cursor is on one of the form's fields, and is moved from field to
//! field. Enter, or a touch on a field, begins data entry
//! there: the on-screen keyboard comes up, and what it types goes into the
//! field the cursor is on. From there four things can be done:
//! go on to the next field, go back to the one before, save
//! what has been changed and leave, or leave without saving. Until it is
//! saved, nothing that has been entered is anyone's but the page's: leaving
//! without saving puts every field back as it was last saved.
//!
//! While data is being entered a dialog is open, and
//! F13 cancels it. That would lose what has been changed, so
//! where anything has been, the crew is asked first whether it is meant:
//! a second dialog, which Enter accepts and F13 cancels in
//! its turn.
//!
//! There is one place to type, the end of the field: a character goes on
//! there and Backspace takes one off.

use crate::form::{Form, Kind};
use crate::keyboard::{Direction, Keyboard, KeyboardView, Layout, Stroke, Takes};
use serde::{Serialize, Serializer};

/// As many of a form's fields as are shown while the keyboard is up: what
/// the page has room for above it. `entry.css` gives the keyboard the rest.
pub const FIELDS_BESIDE_KEYBOARD: usize = 2;

/// What the buttons do on a form's page.
///
/// Along the bottom they are the common functions, doing here what they do
/// everywhere: F17 and F18 move what there is to move - the cursor, or the
/// keyboard's highlight while that is up - F20 enters and F13
/// cancels. Down the sides, the fields' buttons are at the top,
/// where the fields are, and the keyboard's at the bottom, where it is: the
/// button that goes left on the left, the one that goes right on the right.
/// Space and Backspace have buttons of their own because they are wanted
/// most, and would otherwise each be a journey for the highlight. They are
/// above the one that goes right, which is beside Enter: the highlight goes
/// round, so one hand at that corner can do all of the typing.
pub mod button {
    pub const PREVIOUS_FIELD: u8 = 1;
    pub const LEFT: u8 = 6;
    pub const NEXT_FIELD: u8 = 7;
    pub const SAVE: u8 = 8;
    pub const SPACE: u8 = 10;
    pub const BACKSPACE: u8 = 11;
    pub const RIGHT: u8 = 12;
    /// F13, while data is being entered.
    pub const CANCEL: u8 = 13;
    pub const UP: u8 = 17;
    pub const DOWN: u8 = 18;
    pub const ENTER: u8 = 20;
}

/// What a finger has come down on, of what a form's page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touched {
    /// A field, by its number ([`FieldRow::number`]).
    Field(usize),
    /// A key of the keyboard, by its place among the keys as the page lists
    /// them.
    Key(usize),
}

/// What a form held when the crew saved it, for whatever the form is for to
/// be told.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Saved {
    /// The form, by the name its configuration gives it.
    pub form: String,
    /// What each field holds, by the field's name, in the form's order:
    /// what was entered, as it was entered, or `None` where nothing was. A
    /// number is its digits - "007" is not 7.
    #[serde(serialize_with = "by_name")]
    pub fields: Vec<(String, Option<String>)>,
}

fn by_name<S: Serializer>(fields: &[(String, Option<String>)], to: S) -> Result<S::Ok, S::Error> {
    to.collect_map(fields.iter().map(|(name, value)| (name, value)))
}

/// Where a form's data stands, for the page to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// Nothing has been saved yet, and nothing is being entered.
    Empty,
    /// What the fields show is what was last saved.
    Saved,
    /// Data is being entered, and nothing has been changed yet.
    Entering,
    /// Data is being entered, and what the fields show is not what is saved.
    Changed,
}

/// A form's page as it is to be drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormPage {
    pub title: String,
    pub standing: Standing,
    /// The field the cursor is on, counted from 1...
    pub field: usize,
    /// ...of this many.
    pub fields: usize,
    /// The fields shown: all of them, or while the keyboard is up as many as
    /// there is room for beside it.
    pub rows: Vec<FieldRow>,
    /// The on-screen keyboard, while data is being entered: what it types
    /// goes into the field the cursor is on.
    pub keyboard: Option<KeyboardView>,
    /// Whether the crew is being asked if what has been changed is to be
    /// discarded: a dialog, in the keyboard's place, which Enter accepts and
    /// F13 cancels.
    pub discard: bool,
}

/// A field of the form as it is to be drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldRow {
    /// Which of the form's fields it is, counted from 1: what a touch on it
    /// is told of by ([`Touched::Field`]).
    pub number: usize,
    pub title: String,
    /// Text is put at the left of its box, and a number - whole or not - at
    /// the right.
    pub kind: Kind,
    /// The most characters it holds: how long its box is.
    pub length: usize,
    /// What it holds: as it was last saved, but for what has been entered
    /// since data entry began.
    pub value: String,
    /// Whether the cursor is on it.
    pub cursor: bool,
}

/// A form being filled in.
#[derive(Debug, Clone)]
pub struct Entry {
    form: Form,
    /// What each field held when the form was last saved: nothing, to begin
    /// with.
    saved: Vec<String>,
    /// What each field holds now.
    held: Vec<String>,
    /// The field the cursor is on: its place among the form's.
    cursor: usize,
    /// The keyboard, while data is being entered.
    keyboard: Option<Keyboard>,
    /// Whether the crew, having asked to leave data entry with something
    /// changed, is being asked if that is to be discarded.
    discarding: bool,
    /// Whether the form has ever been saved.
    has_saved: bool,
    /// What has been saved and not yet been asked for.
    told: Vec<Saved>,
}

impl Entry {
    /// A form to be filled in: every field empty, the cursor on the first.
    /// `Err` says why the form cannot be one ([`Form::fault`]).
    pub fn new(form: Form) -> Result<Self, String> {
        if let Some(fault) = form.fault() {
            return Err(fault);
        }
        let empty = vec![String::new(); form.fields.len()];
        Ok(Entry {
            saved: empty.clone(),
            held: empty,
            form,
            cursor: 0,
            keyboard: None,
            discarding: false,
            has_saved: false,
            told: Vec::new(),
        })
    }

    /// Whether data is being entered: the keyboard is up, and a dialog open.
    pub fn entering(&self) -> bool {
        self.keyboard.is_some()
    }

    /// F`n` has been pressed.
    pub fn press(&mut self, n: u8) {
        use button::*;
        let last = self.form.fields.len() - 1;

        // Not entering: the cursor goes from field to field, and Enter
        // begins on the one it is on.
        if self.keyboard.is_none() {
            match n {
                PREVIOUS_FIELD | UP => self.cursor = self.cursor.saturating_sub(1),
                NEXT_FIELD | DOWN => self.cursor = (self.cursor + 1).min(last),
                ENTER => self.begin(),
                _ => {}
            }
            return;
        }

        // Asked whether what has been changed is to be discarded: Enter
        // accepts that and F13 cancels it, and nothing
        // else answers until one of them has.
        if self.discarding {
            match n {
                ENTER => self.leave(),
                CANCEL => self.discarding = false,
                _ => {}
            }
            return;
        }

        // Entering: what is done to the form...
        match n {
            PREVIOUS_FIELD if self.cursor > 0 => return self.go_to(self.cursor - 1),
            NEXT_FIELD if self.cursor < last => return self.go_to(self.cursor + 1),
            SAVE => return self.save(),
            CANCEL => return self.cancel(),
            _ => {}
        }
        // ...and what is done to the keyboard, and through it to the field.
        let takes = self.takes();
        let Some(keyboard) = &mut self.keyboard else { return };
        // A key still shown as pressed, the letting go of its button having
        // gone astray, is let go now.
        keyboard.release();
        match n {
            UP => keyboard.step(Direction::Up),
            DOWN => keyboard.step(Direction::Down),
            LEFT => keyboard.step(Direction::Left),
            RIGHT => keyboard.step(Direction::Right),
            _ => {}
        }
        let typed = match n {
            ENTER => keyboard.select(takes),
            SPACE => keyboard.strike(Stroke::Character(' '), takes),
            BACKSPACE => keyboard.strike(Stroke::Backspace, takes),
            _ => None,
        };
        self.typed(typed);
    }

    /// F`n` has been let go: a key it was selecting is no longer pressed.
    pub fn release(&mut self, n: u8) {
        if matches!(n, button::ENTER | button::SPACE | button::BACKSPACE) {
            self.lift();
        }
    }

    /// A finger has come down on a field or on a key. A field touched is one
    /// to enter data into, as one the cursor is taken to and Enter pressed
    /// on; a key touched is a key selected. Neither is to be
    /// touched through a dialog.
    pub fn touch(&mut self, touched: Touched) {
        if self.discarding {
            return;
        }
        match touched {
            Touched::Field(number) => {
                let Some(field) = number.checked_sub(1).filter(|&field| field < self.form.fields.len()) else {
                    return;
                };
                if self.keyboard.is_none() {
                    self.cursor = field;
                    self.begin();
                } else if field != self.cursor {
                    self.go_to(field);
                }
            }
            Touched::Key(key) => {
                let takes = self.takes();
                let typed = self.keyboard.as_mut().and_then(|keyboard| keyboard.touch(key, takes));
                self.typed(typed);
            }
        }
    }

    /// The finger has gone.
    pub fn lift(&mut self) {
        if let Some(keyboard) = &mut self.keyboard {
            keyboard.release();
        }
    }

    /// What the crew has saved since this was last asked, for whatever the
    /// form is for to be told.
    pub fn take_saved(&mut self) -> Vec<Saved> {
        std::mem::take(&mut self.told)
    }

    /// The labels of the buttons that a form's page gives something to do
    /// beyond what they do everywhere: each with the number of its button,
    /// and whether it can do anything now. One that cannot is shown as
    /// unavailable.
    pub fn labels(&self) -> impl Iterator<Item = (u8, &'static str, bool)> {
        use button::*;
        let entering = self.entering();
        let takes = self.takes();
        let text = self.form.fields[self.cursor].kind == Kind::Text;
        // Nothing down the sides answers while the crew is being asked
        // whether to discard.
        let asked = self.discarding;
        [
            (PREVIOUS_FIELD, "Previous\nField", self.cursor > 0),
            (LEFT, "Left", entering),
            (NEXT_FIELD, "Next\nField", self.cursor + 1 < self.form.fields.len()),
            (SAVE, "Save", entering && self.changed()),
            (SPACE, "Space", entering && text && takes.character),
            (BACKSPACE, "Backspace", entering && takes.backspace),
            (RIGHT, "Right", entering),
        ]
        .map(|(n, text, able)| (n, text, able && !asked))
        .into_iter()
        // F13 is 'Cancel' while a dialog is open.
        .chain(entering.then_some((CANCEL, "Cancel", true)))
    }

    /// The page to draw.
    pub fn page(&self) -> FormPage {
        // With the keyboard up there is room for a few of the fields: the
        // one being entered into and those after it, or at the end of the
        // form the last ones.
        let shown = if self.entering() { FIELDS_BESIDE_KEYBOARD } else { self.form.fields.len() };
        let first = self.cursor.min(self.form.fields.len().saturating_sub(shown));
        let rows = (1..)
            .zip(&self.form.fields)
            .zip(&self.held)
            .skip(first)
            .take(shown)
            .map(|((number, field), held)| FieldRow {
                number,
                title: field.title.clone(),
                kind: field.kind,
                length: field.length,
                value: held.clone(),
                cursor: number == self.cursor + 1,
            })
            .collect();
        FormPage {
            title: self.form.title.clone(),
            standing: match (self.entering(), self.changed(), self.has_saved) {
                (true, true, _) => Standing::Changed,
                (true, false, _) => Standing::Entering,
                (false, _, true) => Standing::Saved,
                (false, _, false) => Standing::Empty,
            },
            field: self.cursor + 1,
            fields: self.form.fields.len(),
            rows,
            // The question has the keyboard's place while it is being asked:
            // there is nothing to type until it is answered.
            keyboard: match &self.keyboard {
                Some(keyboard) if !self.discarding => Some(keyboard.view(self.takes())),
                _ => None,
            },
            discard: self.discarding,
        }
    }

    /// Bring up the keyboard for the field the cursor is on.
    fn begin(&mut self) {
        self.keyboard = Some(Keyboard::new(match self.form.fields[self.cursor].kind {
            Kind::Text => Layout::Qwerty,
            Kind::Number | Kind::Decimal => Layout::Phone,
        }));
    }

    /// Accept what the field the cursor is on holds, and go to another:
    /// the keyboard is that field's from here.
    fn go_to(&mut self, field: usize) {
        self.accept();
        self.cursor = field;
        self.begin();
    }

    /// Take what has been entered into the field the cursor is on as its
    /// value ([`Kind::accepted`]).
    fn accept(&mut self) {
        let held = &mut self.held[self.cursor];
        *held = self.form.fields[self.cursor].kind.accepted(held).to_string();
    }

    /// Save what has been changed and leave data entry. With
    /// nothing changed there is nothing to save, and nothing is done.
    fn save(&mut self) {
        if !self.changed() {
            return;
        }
        self.accept();
        self.saved = self.held.clone();
        self.has_saved = true;
        // A field nothing was entered into holds nothing, not text that is
        // no characters long.
        let entered = |saved: &String| (!saved.is_empty()).then(|| saved.clone());
        let fields = self.form.fields.iter().zip(&self.saved);
        let fields = fields.map(|(field, saved)| (field.name.clone(), entered(saved))).collect();
        self.told.push(Saved { form: self.form.name.clone(), fields });
        self.keyboard = None;
    }

    /// F13: leave data entry without saving. That loses what
    /// has been changed, and a button is easily pressed by mistake: with
    /// anything changed, the crew is asked whether it is meant.
    fn cancel(&mut self) {
        if self.changed() {
            self.lift();
            self.discarding = true;
        } else {
            self.leave();
        }
    }

    /// Leave data entry, every field as it was last saved.
    fn leave(&mut self) {
        self.held = self.saved.clone();
        self.keyboard = None;
        self.discarding = false;
    }

    /// Whether any field holds something other than what was last saved.
    fn changed(&self) -> bool {
        let mut fields = self.form.fields.iter().zip(&self.held).zip(&self.saved);
        fields.any(|((field, held), saved)| field.kind.accepted(held) != saved)
    }

    /// What the field the cursor is on can take.
    fn takes(&self) -> Takes {
        let (field, held) = (&self.form.fields[self.cursor], &self.held[self.cursor]);
        let length = held.chars().count();
        Takes { character: length < field.length, point: field.kind.takes(held, '.'), backspace: length > 0 }
    }

    /// Do to the field the cursor is on what the keyboard has typed.
    fn typed(&mut self, stroke: Option<Stroke>) {
        let field = &self.form.fields[self.cursor];
        let held = &mut self.held[self.cursor];
        match stroke {
            Some(Stroke::Character(character)) => {
                if field.kind.takes(held, character) && held.chars().count() < field.length {
                    held.push(character);
                }
            }
            Some(Stroke::Backspace) => {
                held.pop();
            }
            // No key of a form's keyboard types these: going from field to
            // field and saving are the form's own to do.
            Some(Stroke::Enter | Stroke::Tab | Stroke::BackTab) | None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::button::*;
    use super::*;
    use crate::form::Field;
    use crate::keyboard::Key;

    fn form(fields: &[&str]) -> Entry {
        let fields = fields.iter().map(|said| Field::parse(said).unwrap()).collect();
        Entry::new(Form::parse("load Load Record", fields).unwrap()).unwrap()
    }

    /// The form of the example: a field of free text and a number.
    fn load() -> Entry {
        form(&["description text 12 Description", "quantity number 4 Quantity"])
    }

    /// Press these buttons one after another, letting each go.
    fn press(entry: &mut Entry, buttons: &[u8]) {
        for &n in buttons {
            entry.press(n);
            entry.release(n);
        }
    }

    fn values(entry: &Entry) -> Vec<String> {
        entry.page().rows.into_iter().map(|row| row.value).collect()
    }

    fn keys(entry: &Entry) -> Vec<Key> {
        entry.page().keyboard.expect("the keyboard is up").keys
    }

    /// What is written on the keys that are `which`, in their order.
    fn said(entry: &Entry, which: impl Fn(&Key) -> bool) -> Vec<String> {
        keys(entry).into_iter().filter(which).map(|key| key.text).collect()
    }

    /// What is written on the key the highlight is on.
    fn highlighted(entry: &Entry) -> String {
        said(entry, |key| key.highlighted).remove(0)
    }

    /// Touch the keys that say these things, one after another.
    fn touch(entry: &mut Entry, texts: &[&str]) {
        for text in texts {
            let key = keys(entry).iter().position(|key| key.text == *text);
            entry.touch(Touched::Key(key.unwrap_or_else(|| panic!("no key says {text}"))));
            entry.lift();
        }
    }

    /// Select the key that says `text` with the buttons alone: the highlight
    /// along each row in turn until it is on it, and then Enter.
    fn select(entry: &mut Entry, text: &str) {
        for _ in 0..5 {
            for _ in 0..10 {
                if highlighted(entry) == text {
                    return press(entry, &[ENTER]);
                }
                press(entry, &[RIGHT]);
            }
            press(entry, &[DOWN]);
        }
        panic!("no key says {text}");
    }

    /// The label the form gives F`n`, and whether it can do anything.
    fn label(entry: &Entry, n: u8) -> Option<(&'static str, bool)> {
        entry.labels().find(|&(button, ..)| button == n).map(|(_, text, able)| (text, able))
    }

    fn able(entry: &Entry, n: u8) -> bool {
        label(entry, n).unwrap().1
    }

    #[test]
    fn the_cursor_goes_from_field_to_field() {
        let mut entry = load();
        let cursor = |entry: &Entry| {
            let page = entry.page();
            let on: Vec<usize> = page.rows.iter().filter(|row| row.cursor).map(|row| row.number).collect();
            assert_eq!(on, [page.field], "the cursor is on one field");
            (page.field, page.fields)
        };
        assert_eq!(cursor(&entry), (1, 2));
        // F17 and F18, as they move any selection: to the end, and no farther.
        press(&mut entry, &[DOWN]);
        assert_eq!(cursor(&entry), (2, 2));
        press(&mut entry, &[DOWN]);
        assert_eq!(cursor(&entry), (2, 2));
        press(&mut entry, &[UP, UP]);
        assert_eq!(cursor(&entry), (1, 2));
        // And the fields' own buttons.
        press(&mut entry, &[NEXT_FIELD]);
        assert_eq!(cursor(&entry), (2, 2));
        press(&mut entry, &[PREVIOUS_FIELD]);
        assert_eq!(cursor(&entry), (1, 2));
        assert!(!entry.entering(), "moving the cursor is not entering data");
    }

    #[test]
    fn enter_begins_on_the_field_and_then_selects_the_key_the_highlight_is_on() {
        let mut entry = load();
        assert_eq!((entry.page().keyboard, entry.page().standing), (None, Standing::Empty));

        press(&mut entry, &[ENTER]);
        assert!(entry.entering());
        assert_eq!(entry.page().standing, Standing::Entering);
        assert_eq!(highlighted(&entry), "G");
        assert_eq!(values(&entry), ["", ""], "bringing the keyboard up types nothing");

        press(&mut entry, &[ENTER]);
        assert_eq!(values(&entry), ["G", ""]);
        assert_eq!(entry.page().standing, Standing::Changed);
        // The keyboard's highlight is what F17 and F18 move now, not the cursor.
        press(&mut entry, &[UP, ENTER, DOWN, DOWN, ENTER]);
        assert_eq!(values(&entry), ["GTV", ""]);
        assert_eq!(entry.page().field, 1);
    }

    #[test]
    fn text_is_entered_on_the_qwerty_keyboard_and_a_number_on_the_phone_pad() {
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        assert_eq!(entry.page().keyboard.unwrap().layout, Layout::Qwerty);
        touch(&mut entry, &["W", "Caps", "a", "t", "e", "r", "Space", "2", "0", "#+=", "/", "ABC", "Caps", "L"]);
        assert_eq!(values(&entry), ["Water 20/L", ""]);

        press(&mut entry, &[NEXT_FIELD]);
        assert_eq!(entry.page().keyboard.unwrap().layout, Layout::Phone);
        assert_eq!(highlighted(&entry), "5");
        touch(&mut entry, &["4", "8", "0", "Backspace"]);
        assert_eq!(values(&entry), ["Water 20/L", "48"]);

        // Which is which, for the page to put text at the left and a number at the right.
        let kinds: Vec<Kind> = entry.page().rows.iter().map(|row| row.kind).collect();
        assert_eq!(kinds, [Kind::Text, Kind::Number]);
    }

    #[test]
    fn a_form_is_filled_in_and_saved_with_the_buttons_alone() {
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        for key in ["W", "A", "T", "E", "R"] {
            select(&mut entry, key);
        }
        press(&mut entry, &[SPACE]);
        for key in ["2", "0", "L", "L"] {
            select(&mut entry, key);
        }
        press(&mut entry, &[BACKSPACE, NEXT_FIELD]);
        select(&mut entry, "4");
        select(&mut entry, "8");
        assert_eq!(values(&entry), ["WATER 20L", "48"]);

        press(&mut entry, &[SAVE]);
        assert_eq!(entry.page().standing, Standing::Saved);
        assert_eq!(entry.take_saved().len(), 1);
    }

    #[test]
    fn a_value_is_accepted_and_the_next_field_gone_to_or_the_one_before() {
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        // a) What has been entered is the field's, without the spaces nobody can see...
        touch(&mut entry, &["Space", "A", "Space", "B", "Space"]);
        assert_eq!(values(&entry)[0], " A B ");
        press(&mut entry, &[NEXT_FIELD]);
        assert_eq!(values(&entry), ["A B", ""]);
        assert_eq!(entry.page().field, 2);
        // ...and the keyboard is the next field's. There is no field after the last.
        assert_eq!(entry.page().keyboard.unwrap().layout, Layout::Phone);
        touch(&mut entry, &["7"]);
        press(&mut entry, &[NEXT_FIELD]);
        assert_eq!(entry.page().field, 2);

        // b) Back, keeping what was entered here too. There is none before the first.
        press(&mut entry, &[PREVIOUS_FIELD]);
        assert_eq!(values(&entry), ["A B", "7"]);
        assert_eq!(entry.page().field, 1);
        assert_eq!(entry.page().keyboard.unwrap().layout, Layout::Qwerty);
        press(&mut entry, &[PREVIOUS_FIELD]);
        assert_eq!(entry.page().field, 1);
        assert!(entry.entering());
    }

    #[test]
    fn what_is_changed_is_saved_or_left_unsaved() {
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        touch(&mut entry, &["A"]);
        press(&mut entry, &[NEXT_FIELD]);
        touch(&mut entry, &["7"]);

        // c) Saved: data entry is over, and the fields show what was saved.
        press(&mut entry, &[SAVE]);
        assert!(!entry.entering());
        assert_eq!(values(&entry), ["A", "7"]);
        assert_eq!(entry.page().standing, Standing::Saved);
        assert_eq!(entry.page().field, 2, "the cursor is where it was");
        assert_eq!(entry.take_saved().len(), 1);

        // d) Left without saving - which is asked about first, something
        // having been changed, and Enter says is meant: every field is as it
        // was last saved, and nobody is told anything.
        press(&mut entry, &[ENTER]);
        touch(&mut entry, &["Backspace", "9", "9"]);
        press(&mut entry, &[PREVIOUS_FIELD]);
        touch(&mut entry, &["B"]);
        assert_eq!(values(&entry), ["AB", "99"]);
        assert_eq!(entry.page().standing, Standing::Changed);
        press(&mut entry, &[CANCEL, ENTER]);
        assert!(!entry.entering());
        assert_eq!(values(&entry), ["A", "7"]);
        assert_eq!(entry.page().standing, Standing::Saved);
        assert_eq!(entry.take_saved(), []);

        // With nothing changed there is nothing to save: Save does nothing,
        // and Cancel leaves without asking.
        press(&mut entry, &[ENTER, SAVE]);
        assert!(entry.entering());
        touch(&mut entry, &["Space"]);
        press(&mut entry, &[SAVE]);
        assert!(entry.entering(), "a space at the end is not a change");
        press(&mut entry, &[CANCEL]);
        assert!(!entry.entering());
        assert_eq!(entry.take_saved(), []);

        // A form never saved, left unsaved, is empty again.
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        touch(&mut entry, &["A"]);
        press(&mut entry, &[CANCEL, ENTER]);
        assert_eq!(values(&entry), ["", ""]);
        assert_eq!(entry.page().standing, Standing::Empty);
    }

    #[test]
    fn leaving_with_something_changed_is_asked_about_first() {
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        touch(&mut entry, &["W", "A"]);
        press(&mut entry, &[UP]);
        let before = entry.page();
        assert!(!before.discard);

        // F13: the question, in the keyboard's place. What was entered is still there.
        press(&mut entry, &[CANCEL]);
        let asked = entry.page();
        assert!(asked.discard && asked.keyboard.is_none());
        assert_eq!((asked.standing, asked.rows[0].value.as_str()), (Standing::Changed, "WA"));
        assert!(entry.entering(), "data entry is not over until it is answered");

        // It is a dialog. F13 is still 'Cancel', and
        // nothing down the sides answers - nor does a touch, on a field or
        // on where a key was.
        assert_eq!(label(&entry, CANCEL), Some(("Cancel", true)));
        assert!(entry.labels().all(|(n, _, able)| able == (n == CANCEL)));
        press(&mut entry, &[SAVE, NEXT_FIELD, SPACE, BACKSPACE, LEFT, UP, DOWN]);
        entry.touch(Touched::Field(2));
        entry.touch(Touched::Key(0));
        entry.lift();
        assert_eq!(entry.page(), asked);
        assert_eq!(entry.take_saved(), []);

        // F13 cancels the question: data entry goes on as it was, the
        // highlight where it had been put.
        press(&mut entry, &[CANCEL]);
        assert_eq!(entry.page(), before);
        assert_eq!(highlighted(&entry), "Q");

        // Asked again, Enter accepts it: what was changed is gone.
        press(&mut entry, &[CANCEL, ENTER]);
        assert!(!entry.entering());
        assert_eq!(entry.page(), load().page());

        // A key held when the question is asked is let go by it.
        press(&mut entry, &[ENTER]);
        entry.press(ENTER);
        press(&mut entry, &[CANCEL, CANCEL]);
        assert!(keys(&entry).iter().all(|key| !key.pressed));
    }

    #[test]
    fn f13_is_cancel_while_data_is_being_entered_and_not_otherwise() {
        let mut entry = load();
        assert_eq!(label(&entry, CANCEL), None, "F13 is whatever it is elsewhere");
        press(&mut entry, &[CANCEL]);
        assert_eq!(entry.page(), load().page(), "and does nothing to the form");

        press(&mut entry, &[ENTER]);
        assert_eq!(label(&entry, CANCEL), Some(("Cancel", true)));
        press(&mut entry, &[CANCEL]);
        assert_eq!(label(&entry, CANCEL), None);
    }

    #[test]
    fn the_labels_say_what_the_buttons_can_do_now() {
        let mut entry = load();
        let said = |entry: &Entry| entry.labels().filter(|&(n, ..)| n != CANCEL).collect::<Vec<_>>();
        // Not entering: the cursor can go on from the first field, and that is all.
        assert_eq!(
            said(&entry),
            [
                (1, "Previous\nField", false),
                (6, "Left", false),
                (7, "Next\nField", true),
                (8, "Save", false),
                (10, "Space", false),
                (11, "Backspace", false),
                (12, "Right", false),
            ]
        );
        press(&mut entry, &[DOWN]);
        assert_eq!((able(&entry, PREVIOUS_FIELD), able(&entry, NEXT_FIELD)), (true, false));

        // Entering a number: the highlight can be moved, and there is nothing yet to take away or to save.
        press(&mut entry, &[ENTER]);
        assert_eq!((able(&entry, LEFT), able(&entry, RIGHT)), (true, true));
        assert_eq!((able(&entry, BACKSPACE), able(&entry, SAVE)), (false, false));
        assert!(!able(&entry, SPACE), "a number has no spaces");
        touch(&mut entry, &["1"]);
        assert_eq!((able(&entry, BACKSPACE), able(&entry, SAVE)), (true, true));
        touch(&mut entry, &["Backspace"]);
        assert_eq!((able(&entry, BACKSPACE), able(&entry, SAVE)), (false, false), "it is as it was saved again");

        // Entering text: Space, until the field is full.
        press(&mut entry, &[PREVIOUS_FIELD]);
        assert_eq!((able(&entry, PREVIOUS_FIELD), able(&entry, NEXT_FIELD), able(&entry, SPACE)), (false, true, true));
        touch(&mut entry, &["A"; 12]);
        assert_eq!((able(&entry, SPACE), able(&entry, BACKSPACE)), (false, true));

        // The same buttons whatever is being done, and none of them a common function's but F13.
        let buttons = |entry: &Entry| entry.labels().map(|(n, ..)| n).collect::<Vec<_>>();
        assert_eq!(buttons(&entry), [1, 6, 7, 8, 10, 11, 12, 13]);
        assert_eq!(buttons(&load()), [1, 6, 7, 8, 10, 11, 12]);
    }

    #[test]
    fn a_field_holds_no_more_than_its_length() {
        let mut entry = load();
        press(&mut entry, &[DOWN, ENTER]);
        let available = |entry: &Entry| said(entry, |key| key.available);
        assert_eq!(available(&entry).len(), 10, "the digits, with nothing to take away");

        touch(&mut entry, &["1", "2", "3", "4"]);
        // Full, and the keys say so. What is pressed regardless goes nowhere.
        assert_eq!(available(&entry), ["Backspace"]);
        let five = keys(&entry).iter().position(|key| key.text == "5").unwrap();
        entry.touch(Touched::Key(five));
        press(&mut entry, &[ENTER]);
        assert_eq!(values(&entry)[1], "1234");
        assert!(keys(&entry).iter().all(|key| !key.pressed));

        press(&mut entry, &[BACKSPACE]);
        assert_eq!(values(&entry)[1], "123");
        assert_eq!(available(&entry).len(), 11);
    }

    #[test]
    fn a_key_selected_is_shown_as_pressed_and_what_it_types_in_the_field() {
        let pressed = |entry: &Entry| said(entry, |key| key.pressed);
        let mut entry = load();
        press(&mut entry, &[ENTER]);
        assert!(pressed(&entry).is_empty());

        // Enter: for as long as it is down.
        entry.press(ENTER);
        assert_eq!(pressed(&entry), ["G"]);
        assert_eq!(values(&entry)[0], "G");
        entry.release(ENTER);
        assert!(pressed(&entry).is_empty());

        // A finger: for as long as it is there.
        let h = keys(&entry).iter().position(|key| key.text == "H").unwrap();
        entry.touch(Touched::Key(h));
        assert_eq!(pressed(&entry), ["H"]);
        assert_eq!(values(&entry)[0], "GH");
        entry.lift();
        assert!(pressed(&entry).is_empty());

        // The buttons that stand for keys show the keys they stand for.
        entry.press(BACKSPACE);
        assert_eq!(pressed(&entry), ["Backspace"]);
        assert_eq!(values(&entry)[0], "G");
        entry.release(BACKSPACE);
        entry.press(SPACE);
        assert_eq!(pressed(&entry), ["Space"]);
        // One whose letting go is never heard of is let go by the next button.
        entry.press(LEFT);
        assert!(pressed(&entry).is_empty());
        assert_eq!(highlighted(&entry), "G", "H was touched, and the highlight went there");
    }

    #[test]
    fn a_touch_on_a_field_does_what_the_cursor_and_enter_do() {
        let mut entry = load();
        entry.touch(Touched::Field(2));
        assert!(entry.entering());
        assert_eq!((entry.page().field, entry.page().keyboard.unwrap().layout), (2, Layout::Phone));
        touch(&mut entry, &["7"]);

        // On another field while entering: as the button that goes to it.
        entry.touch(Touched::Field(1));
        assert_eq!((entry.page().field, entry.page().keyboard.unwrap().layout), (1, Layout::Qwerty));
        touch(&mut entry, &["A"]);
        assert_eq!(values(&entry), ["A", "7"]);
        // On the one being entered into: nothing, and the highlight stays where it is.
        entry.touch(Touched::Field(1));
        assert_eq!(values(&entry), ["A", "7"]);
        assert_eq!(highlighted(&entry), "A");

        // On what is not there: nothing.
        entry.touch(Touched::Field(0));
        entry.touch(Touched::Field(3));
        entry.touch(Touched::Key(99));
        assert_eq!(values(&entry), ["A", "7"]);
        assert_eq!(entry.page().field, 1);
        // A key is not there to be touched until the keyboard is.
        let mut entry = load();
        entry.touch(Touched::Key(0));
        entry.lift();
        assert_eq!(entry.page(), load().page());
    }

    #[test]
    fn the_fields_there_is_room_for_are_shown_beside_the_keyboard() {
        let mut entry = form(&["a text 4 A", "b text 4 B", "c number 4 C", "d text 4 D", "e text 4 E"]);
        let shown = |entry: &Entry| entry.page().rows.iter().map(|row| row.number).collect::<Vec<_>>();
        assert_eq!(shown(&entry), [1, 2, 3, 4, 5]);

        // The field being entered into, and the one after it...
        press(&mut entry, &[ENTER]);
        assert_eq!((shown(&entry), entry.page().field, entry.page().fields), (vec![1, 2], 1, 5));
        press(&mut entry, &[NEXT_FIELD, NEXT_FIELD]);
        assert_eq!((shown(&entry), entry.page().field), (vec![3, 4], 3));
        // ...or at the end of the form, the one before it.
        press(&mut entry, &[NEXT_FIELD, NEXT_FIELD]);
        assert_eq!((shown(&entry), entry.page().field), (vec![4, 5], 5));
        assert!(entry.page().rows[1].cursor);

        press(&mut entry, &[CANCEL]);
        assert_eq!(shown(&entry), [1, 2, 3, 4, 5]);

        // A form of one field has one to show.
        let mut entry = form(&["a text 4 A"]);
        press(&mut entry, &[ENTER]);
        assert_eq!(shown(&entry), [1]);
        assert_eq!((able(&entry, PREVIOUS_FIELD), able(&entry, NEXT_FIELD)), (false, false));
    }

    #[test]
    fn what_is_saved_is_told_once_by_the_names_of_the_form_and_its_fields() {
        let mut entry =
            form(&["serial number 6 Serial", "description text 12 Description", "quantity number 4 Quantity"]);
        press(&mut entry, &[ENTER]);
        touch(&mut entry, &["0", "0", "7"]);
        press(&mut entry, &[NEXT_FIELD]);
        touch(&mut entry, &["W", "A", "T", "E", "R"]);
        press(&mut entry, &[SAVE]);

        let told = entry.take_saved();
        assert_eq!(
            told,
            [Saved {
                form: "load".to_string(),
                fields: vec![
                    ("serial".to_string(), Some("007".to_string())),
                    ("description".to_string(), Some("WATER".to_string())),
                    ("quantity".to_string(), None),
                ],
            }]
        );
        // As it is written down: the fields in the form's order, each as it
        // was entered - a number is its digits, noughts and all - and one
        // that nothing was entered into as null.
        assert_eq!(
            serde_json::to_string(&told[0]).unwrap(),
            r#"{"form":"load","fields":{"serial":"007","description":"WATER","quantity":null}}"#
        );
        assert_eq!(entry.take_saved(), [], "told once");
        // The page shows the same.
        assert_eq!(values(&entry), ["007", "WATER", ""]);

        // Text that has been taken away again is nothing entered, too.
        press(&mut entry, &[ENTER]);
        touch(&mut entry, &["Backspace"; 5]);
        press(&mut entry, &[SAVE]);
        assert_eq!(
            serde_json::to_string(&entry.take_saved()[0]).unwrap(),
            r#"{"form":"load","fields":{"serial":"007","description":null,"quantity":null}}"#
        );
    }

    #[test]
    fn a_number_that_is_not_whole_has_a_point_and_one_only() {
        let mut entry = form(&["frequency decimal 6 Frequency (MHz)", "quantity number 4 Quantity"]);
        let point = |entry: &Entry| keys(entry).into_iter().find(|key| key.text == ".").unwrap().available;
        press(&mut entry, &[ENTER]);
        assert_eq!(entry.page().keyboard.unwrap().layout, Layout::Phone);
        assert!(point(&entry));

        touch(&mut entry, &["4", "5", "."]);
        assert!(!point(&entry), "It has its point");
        touch(&mut entry, &[".", "2", "5", "0"]);
        assert_eq!(values(&entry)[0], "45.250");
        assert_eq!(said(&entry, |key| key.available), ["Backspace"], "and is full");
        // With the point taken away again there can be another.
        touch(&mut entry, &["Backspace"; 4]);
        assert!(point(&entry));
        touch(&mut entry, &["."]);
        assert_eq!(values(&entry)[0], "45.");

        // A point that nothing follows is not part of the number; a whole
        // number has no point at all.
        press(&mut entry, &[NEXT_FIELD]);
        assert_eq!(values(&entry)[0], "45");
        assert!(!point(&entry));
        touch(&mut entry, &["7", "."]);
        assert_eq!(values(&entry)[1], "7");

        // It goes at the right of its box, as a whole number does.
        assert_eq!(entry.page().rows[0].kind, Kind::Decimal);
        // And is told as it was entered: 45.250 is not 45.25.
        press(&mut entry, &[PREVIOUS_FIELD]);
        touch(&mut entry, &[".", "2", "5", "0"]);
        press(&mut entry, &[SAVE]);
        assert_eq!(
            serde_json::to_string(&entry.take_saved()[0]).unwrap(),
            r#"{"form":"load","fields":{"frequency":"45.250","quantity":"7"}}"#
        );
    }

    #[test]
    fn a_form_that_cannot_be_right_is_not_one_to_fill_in() {
        let form = Form { name: "load".to_string(), title: "Load Record".to_string(), fields: Vec::new() };
        assert!(Entry::new(form).unwrap_err().contains("no 'field'"));
    }

    #[test]
    fn a_page_serialises_in_the_shape_the_ui_reads() {
        let mut entry = load();
        let json = serde_json::to_value(entry.page()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "title": "Load Record", "standing": "empty", "field": 1, "fields": 2,
                "rows": [
                    {"number": 1, "title": "Description", "kind": "text", "length": 12, "value": "", "cursor": true},
                    {"number": 2, "title": "Quantity", "kind": "number", "length": 4, "value": "", "cursor": false},
                ],
                "keyboard": null, "discard": false,
            })
        );

        press(&mut entry, &[DOWN, ENTER]);
        touch(&mut entry, &["4", "8"]);
        let json = serde_json::to_value(entry.page()).unwrap();
        assert_eq!((&json["standing"], &json["field"]), (&"changed".into(), &2.into()));
        assert_eq!(json["rows"][1]["value"], "48");
        assert_eq!((&json["keyboard"]["layout"], &json["keyboard"]["columns"]), (&"phone".into(), &3.into()));
        assert_eq!(json["keyboard"]["keys"][7]["highlighted"], true);
        assert_eq!(json["keyboard"]["keys"][11]["kind"], "backspace");

        press(&mut entry, &[CANCEL]);
        let json = serde_json::to_value(entry.page()).unwrap();
        assert_eq!((&json["discard"], &json["keyboard"]), (&true.into(), &serde_json::Value::Null));
        assert_eq!(json["rows"].as_array().unwrap().len(), 2);
        press(&mut entry, &[CANCEL]);

        press(&mut entry, &[SAVE]);
        let json = serde_json::to_value(entry.page()).unwrap();
        assert_eq!((&json["standing"], &json["keyboard"]), (&"saved".into(), &serde_json::Value::Null));
    }
}
