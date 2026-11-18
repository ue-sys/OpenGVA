//! A keyboard laid out as the United States' are: which key types each
//! character of ASCII, and whether Shift is held with it.
//!
//! It is what the HMI's on-screen keyboard is to another program. A program
//! is not handed text: it is sent the keys a keyboard would have sent, by
//! their numbers, and makes characters of them itself - with the keymap the
//! display server gives it ([`KEYMAP`]), or, as Android does, with a layout
//! of its own that takes the numbers to be those of this keyboard. So the
//! numbers are the ones Linux's input layer has for the keys (`KEY_A` is
//! 30), Shift is a key that goes down and comes up, and what cannot be typed
//! on such a keyboard cannot be sent: there is a key for every character of
//! printable ASCII and for no other.
//!
//! This crate has no windowing or I/O dependencies so that it can be tested
//! on any machine with `cargo test -p us-keyboard`.

/// A key, by the number Linux's input layer has for it.
pub type Code = u32;

/// The left Shift key.
pub const SHIFT: Code = 42;
pub const BACKSPACE: Code = 14;
pub const TAB: Code = 15;
/// Return.
pub const ENTER: Code = 28;

/// Shift among a keymap's modifiers: XKB's first, whatever the keymap.
pub const SHIFT_MODIFIER: u32 = 1;

/// The keymap that says what these keys type, in XKB's text format: the one
/// its rules give for the layout "us" on Linux's keys. It is made of the
/// parts every display server has (xkeyboard-config), which is what makes it
/// the same keymap a keyboard plugged into the machine would be given.
pub const KEYMAP: &str = r#"xkb_keymap {
	xkb_keycodes { include "evdev+aliases(qwerty)" };
	xkb_types { include "complete" };
	xkb_compat { include "complete" };
	xkb_symbols { include "pc+us+inet(evdev)" };
};
"#;

/// The keys that type, a row at a time: the first key of a run of keys that
/// are numbered one after another, what they type, and what they type with
/// Shift.
const ROWS: [(Code, &str, &str); 5] = [
    (2, "1234567890-=", "!@#$%^&*()_+"),
    (16, "qwertyuiop[]", "QWERTYUIOP{}"),
    (30, "asdfghjkl;'`", "ASDFGHJKL:\"~"),
    (43, "\\zxcvbnm,./", "|ZXCVBNM<>?"),
    (57, " ", " "),
];

/// The key that types `character`, and whether Shift is held with it. `None`
/// for a character this keyboard has no key for: any that is not printable
/// ASCII.
pub fn key(character: char) -> Option<(Code, bool)> {
    ROWS.iter().find_map(|(first, plain, shifted)| {
        let found = |keys: &str| (*first..).zip(keys.chars()).find_map(|(code, types)| (types == character).then_some(code));
        found(plain).map(|code| (code, false)).or_else(|| found(shifted).map(|code| (code, true)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_character_of_printable_ascii_has_a_key_of_its_own_and_nothing_else_has_one() {
        let keys: Vec<(Code, bool)> = (' '..='~').map(|character| key(character).expect("a key")).collect();
        assert_eq!(keys.len(), 95);
        assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), 95, "no two characters on one key");
        for character in ['\n', '\t', '\u{7f}', 'é', '£', '€', '\0'] {
            assert_eq!(key(character), None, "{character:?}");
        }
    }

    #[test]
    fn the_keys_are_those_of_a_us_keyboard_by_linuxs_numbers() {
        // linux/input-event-codes.h: KEY_1 is 2, KEY_Q 16, KEY_A 30, KEY_Z 44.
        assert_eq!((key('1'), key('0'), key('q'), key('p')), (Some((2, false)), Some((11, false)), Some((16, false)), Some((25, false))));
        assert_eq!((key('a'), key('l'), key('z'), key('m')), (Some((30, false)), Some((38, false)), Some((44, false)), Some((50, false))));
        assert_eq!((key(' '), key('-'), key('/'), key('`')), (Some((57, false)), Some((12, false)), Some((53, false)), Some((41, false))));
        // A capital is its letter's key with Shift, and so are the marks
        // above the digits.
        for letter in 'a'..='z' {
            let (code, shift) = key(letter).unwrap();
            assert_eq!((key(letter.to_ascii_uppercase()), shift), (Some((code, true)), false));
        }
        assert_eq!((key('!'), key('@'), key('?'), key('|')), (Some((2, true)), Some((3, true)), Some((53, true)), Some((43, true))));
        // The keys that type nothing are none of these, nor Shift.
        let typing: BTreeSet<Code> = (' '..='~').filter_map(key).map(|(code, _)| code).collect();
        assert!([SHIFT, BACKSPACE, TAB, ENTER].iter().all(|code| !typing.contains(code)));
        assert_eq!(typing.len(), 48);
    }
}
