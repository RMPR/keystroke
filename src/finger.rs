//! Finger-to-key mapping for the touch-typing methodology.
//!
//! Standard QWERTY US-layout finger assignments.

/// Index of each finger, matching the order used in the Slint UI.
/// `None` is represented by -1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[allow(dead_code)] // LeftThumb is part of the API even if no key maps to it
pub enum Finger {
    None = -1,
    LeftPinky = 0,
    LeftRing = 1,
    LeftMiddle = 2,
    LeftIndex = 3,
    LeftThumb = 4,
    RightThumb = 5,
    RightIndex = 6,
    RightMiddle = 7,
    RightRing = 8,
    RightPinky = 9,
}

impl Finger {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Returns the finger that should press the given character.
/// Capital letters and shifted symbols use the same finger as the unshifted
/// key (the opposite-hand pinky also presses Shift, but we don't model that
/// separately).
pub fn finger_for_char(c: char) -> Finger {
    let lower = c.to_ascii_lowercase();
    match lower {
        '`' | '~' | '1' | '!' | 'q' | 'a' | 'z' => Finger::LeftPinky,
        '2' | '@' | 'w' | 's' | 'x' => Finger::LeftRing,
        '3' | '#' | 'e' | 'd' | 'c' => Finger::LeftMiddle,
        '4' | '$' | '5' | '%' | 'r' | 't' | 'f' | 'g' | 'v' | 'b' => Finger::LeftIndex,
        ' ' => Finger::RightThumb, // either thumb works; pick one
        '6' | '^' | '7' | '&' | 'y' | 'u' | 'h' | 'j' | 'n' | 'm' => Finger::RightIndex,
        '8' | '*' | 'i' | 'k' | ',' | '<' => Finger::RightMiddle,
        '9' | '(' | 'o' | 'l' | '.' | '>' => Finger::RightRing,
        '0' | ')' | 'p' | ';' | ':' | '/' | '?' | '[' | '{' | ']' | '}' | '-' | '_' | '=' | '+'
        | '\'' | '"' | '\\' | '|' => Finger::RightPinky,
        _ => Finger::None,
    }
}
