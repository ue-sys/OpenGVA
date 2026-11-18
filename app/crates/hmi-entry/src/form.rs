//! Forms: what a form is, and how a configuration describes one.
//!
//! A form is a title and some fields, each with a title of its own and a box
//! to enter into. What the form is for, and what its
//! fields mean, is not known here: a field is free text or a number, of some
//! length, and has a name for whatever the form is for to know its value by.
//! A number is its digits, as they are entered: what is 007 to the crew is
//! not made 7 here, since it may as well be a code as a quantity.

use serde::Serialize;

/// As many fields as a form can have: what its page has room for, a row to
/// each. `entry.css` lays the page out, and has rows of that height.
pub const MOST_FIELDS: usize = 8;
/// The longest a field can be: what its box holds on one line.
pub const LONGEST: usize = 28;

/// What a field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Free text: letters, digits, spaces and punctuation.
    Text,
    /// A whole number: digits, and nothing else.
    Number,
    /// A number that need not be whole: digits, and a decimal point among
    /// them. There is no sign: it is not less than nothing.
    Decimal,
}

impl Kind {
    /// Whether a character can go on the end of what a field of this kind
    /// holds.
    pub fn takes(self, held: &str, character: char) -> bool {
        match self {
            Kind::Text => character == ' ' || character.is_ascii_graphic(),
            Kind::Number => character.is_ascii_digit(),
            // One point at the most.
            Kind::Decimal => character.is_ascii_digit() || (character == '.' && !held.contains('.')),
        }
    }

    /// What a field of this kind holds, as it is taken once the crew has
    /// done with it: text without the spaces at its ends, which nobody can
    /// see, and a number without a point that nothing follows.
    pub fn accepted(self, held: &str) -> &str {
        match self {
            Kind::Text => held.trim(),
            Kind::Number => held,
            Kind::Decimal => held.strip_suffix('.').unwrap_or(held),
        }
    }
}

/// A field of a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// What its value goes by, where what is entered is sent: one word.
    pub name: String,
    /// What the crew know it as, written beside it.
    pub title: String,
    pub kind: Kind,
    /// The most characters it holds.
    pub length: usize,
}

impl Field {
    /// Read a field as a configuration describes it: its name - one word -
    /// its kind, `text`, `number` or `decimal`, the most characters it holds,
    /// and then its title.
    ///
    /// ```text
    /// description  text    24  Description
    /// quantity     number   4  Quantity
    /// frequency    decimal  7  Frequency (MHz)
    /// ```
    pub fn parse(said: &str) -> Result<Self, String> {
        let mut words = said.split_whitespace();
        let (Some(name), Some(kind), Some(length)) = (words.next(), words.next(), words.next()) else {
            return Err("a field is its name, its kind, how many characters it holds and then its title".to_string());
        };
        let kind = match kind {
            "text" => Kind::Text,
            "number" => Kind::Number,
            "decimal" => Kind::Decimal,
            _ => return Err(format!("a field's kind is 'text', 'number' or 'decimal', not '{kind}'")),
        };
        let length = length.parse().map_err(|_| format!("'{length}' is not how many characters a field holds"))?;
        let field = Field { name: name.to_string(), title: words.collect::<Vec<_>>().join(" "), kind, length };
        field.fault().map_or(Ok(field), Err)
    }

    /// Why this cannot be a field, if it cannot.
    pub fn fault(&self) -> Option<String> {
        if !one_word(&self.name) {
            Some("a field's name is one word".to_string())
        } else if self.title.trim().is_empty() {
            Some(format!("'{}' has no title", self.name))
        } else if self.length == 0 {
            Some(format!("'{}' holds no characters: one at the least", self.name))
        } else if self.length > LONGEST {
            Some(format!("'{}' is longer than its box: {LONGEST} characters at the most", self.name))
        } else {
            None
        }
    }
}

/// A form: something for the crew to fill in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// What it goes by, where what is entered into it is sent: one word.
    pub name: String,
    /// What the crew know it as, written at the top of its page.
    pub title: String,
    /// Its fields, in the order they are filled in: down the page.
    pub fields: Vec<Field>,
}

impl Form {
    /// Read a form as a configuration describes it - its name, one word, and
    /// then its title - with the fields the configuration gives it.
    ///
    /// ```text
    /// load  Load Record
    /// ```
    pub fn parse(said: &str, fields: Vec<Field>) -> Result<Self, String> {
        let (name, title) = said.trim().split_once(char::is_whitespace).unwrap_or((said.trim(), ""));
        let form = Form {
            name: name.to_string(),
            title: title.split_whitespace().collect::<Vec<_>>().join(" "),
            fields,
        };
        form.fault().map_or(Ok(form), Err)
    }

    /// Why this cannot be a form, if it cannot.
    pub fn fault(&self) -> Option<String> {
        if !one_word(&self.name) || self.title.trim().is_empty() {
            return Some("a form is its name, one word, and then its title".to_string());
        }
        if self.fields.is_empty() {
            return Some("no 'field'".to_string());
        }
        if self.fields.len() > MOST_FIELDS {
            return Some(format!("{} fields, and room on the page for {MOST_FIELDS}", self.fields.len()));
        }
        let mut names = Vec::new();
        for field in &self.fields {
            if let Some(fault) = field.fault() {
                return Some(fault);
            }
            if names.contains(&&field.name) {
                return Some(format!("'{}' is there twice", field.name));
            }
            names.push(&field.name);
        }
        None
    }
}

fn one_word(name: &str) -> bool {
    !name.is_empty() && !name.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, kind: Kind, length: usize, title: &str) -> Field {
        Field { name: name.to_string(), title: title.to_string(), kind, length }
    }

    #[test]
    fn a_configuration_gives_a_field_its_name_kind_length_and_title() {
        assert_eq!(
            Field::parse("description  text    24  Description"),
            Ok(field("description", Kind::Text, 24, "Description"))
        );
        // A title can be several words; what is between them is one space.
        assert_eq!(
            Field::parse("weight\tnumber 5 Weight   (kg)"),
            Ok(field("weight", Kind::Number, 5, "Weight (kg)"))
        );
        assert_eq!(
            Field::parse("frequency decimal 7 Frequency (MHz)"),
            Ok(field("frequency", Kind::Decimal, 7, "Frequency (MHz)"))
        );
    }

    #[test]
    fn a_field_that_cannot_be_right_says_why() {
        let error = |said: &str| Field::parse(said).unwrap_err();
        assert!(error("").contains("its name, its kind"));
        assert!(error("quantity number").contains("its name, its kind"));
        assert!(error("quantity digits 4 Quantity").contains("not 'digits'"));
        assert!(error("quantity number four Quantity").contains("'four'"));
        assert!(error("quantity number -4 Quantity").contains("'-4'"));
        // A field has a title.
        assert!(error("quantity number 4").contains("'quantity' has no title"));
        assert!(error("quantity number 0 Quantity").contains("one at the least"));
        // A field is one line of its box, whatever its kind.
        for kind in ["text", "number", "decimal"] {
            assert!(Field::parse(&format!("note {kind} {LONGEST} Note")).is_ok());
            assert!(error(&format!("note {kind} {} Note", LONGEST + 1)).contains("28 characters"), "{kind}");
        }
    }

    #[test]
    fn a_field_takes_free_text_or_a_number() {
        for character in ['A', 'z', '0', ' ', '.', '/', '~'] {
            assert!(Kind::Text.takes("a.b", character), "{character:?}");
        }
        for character in ['\n', '\t', '\u{e9}', '\u{2013}'] {
            assert!(!Kind::Text.takes("", character), "{character:?}");
        }
        assert!(('0'..='9').all(|digit| Kind::Number.takes("", digit) && Kind::Decimal.takes("1.5", digit)));
        for character in ['A', ' ', '.', '-', '+'] {
            assert!(!Kind::Number.takes("", character), "{character:?}");
        }
        // A number that need not be whole has a point, and one only. It has no sign.
        assert!(Kind::Decimal.takes("", '.') && Kind::Decimal.takes("45", '.'));
        assert!(!Kind::Decimal.takes("45.2", '.') && !Kind::Decimal.takes(".", '.'));
        for character in ['A', ' ', ',', '-', '+'] {
            assert!(!Kind::Decimal.takes("", character), "{character:?}");
        }
    }

    #[test]
    fn what_is_entered_is_taken_without_what_is_no_part_of_it() {
        assert_eq!(Kind::Text.accepted("  A B  "), "A B");
        assert_eq!(Kind::Text.accepted("   "), "");
        // A number is its digits as they were entered: the noughts stay.
        assert_eq!(Kind::Number.accepted("007"), "007");
        assert_eq!(Kind::Decimal.accepted("45.250"), "45.250");
        assert_eq!(Kind::Decimal.accepted(".5"), ".5");
        // A point that nothing follows is not part of one.
        assert_eq!(Kind::Decimal.accepted("45."), "45");
        assert_eq!(Kind::Decimal.accepted("."), "");
    }

    #[test]
    fn a_form_is_its_name_its_title_and_its_fields() {
        let fields =
            vec![field("description", Kind::Text, 24, "Description"), field("quantity", Kind::Number, 4, "Quantity")];
        let form = Form::parse("load   Load  Record", fields.clone()).unwrap();
        assert_eq!((form.name.as_str(), form.title.as_str()), ("load", "Load Record"));
        assert_eq!(form.fields, fields);
    }

    #[test]
    fn a_form_that_cannot_be_right_says_why() {
        let one = || vec![field("a", Kind::Text, 4, "A")];
        let error = |said: &str, fields| Form::parse(said, fields).unwrap_err();
        assert!(error("", one()).contains("its name, one word, and then its title"));
        assert!(error("load", one()).contains("and then its title"));
        assert!(error("load Load Record", Vec::new()).contains("no 'field'"));
        assert!(error("load Load Record", [one(), one()].concat()).contains("'a' is there twice"));

        // A row of the page to a field, and there are eight.
        let fields = |count| (0..count).map(|n| field(&format!("f{n}"), Kind::Number, 2, "F")).collect::<Vec<_>>();
        assert!(Form::parse("load Load Record", fields(MOST_FIELDS)).is_ok());
        assert!(error("load Load Record", fields(MOST_FIELDS + 1)).contains("9 fields"));

        // One put together some other way is held to the same.
        let mut form = Form::parse("load Load Record", one()).unwrap();
        form.fields[0].length = 0;
        assert!(form.fault().unwrap().contains("one at the least"));
        form.fields[0] = field("two words", Kind::Text, 4, "A");
        assert!(form.fault().unwrap().contains("one word"));
    }
}
