//! Keyboard-layout definitions and OS-level auto-detection.
//!
//! Each layout describes the characters produced by every key in every shift
//! state, together with the finger that should press that key. This lets us
//! highlight the correct key on the virtual keyboard, and find the correct
//! finger, even when the user's logical keyboard layout differs from the
//! standard US QWERTY one.

use crate::finger::Finger;

/// A single key on the virtual keyboard.
#[derive(Debug, Clone)]
pub struct LayoutKey {
    /// Human-visible label printed on the key cap (e.g. `"A"`, `"&\n1"`, `"Tab"`).
    pub label: &'static str,
    /// Character produced when this key is pressed without Shift,
    /// or `'\0'` if the key is non-printable (Shift, Tab, ...).
    pub unshifted: char,
    /// Character produced when this key is pressed with Shift,
    /// or `'\0'` if Shift doesn't change the output.
    pub shifted: char,
    /// Visual width of this key as a multiple of the unit key width.
    pub width: f32,
    /// Finger that should press this key.
    pub finger: Finger,
}

/// A keyboard layout description.
pub struct KeyboardLayout {
    /// Human-readable name (e.g. `"US English (QWERTY)"`).
    pub name: &'static str,
    /// Windows Keyboard Layout Identifier, e.g. `"00000409"`.
    pub klid: &'static str,
    /// Rows of keys, top to bottom.
    pub rows: &'static [&'static [LayoutKey]],
}

impl KeyboardLayout {
    /// Returns the finger that should press `c` on this layout, or
    /// [`Finger::None`] if the character isn't produced by any single key.
    pub fn finger_for_char(&self, c: char) -> Finger {
        for row in self.rows {
            for k in row.iter() {
                if k.unshifted == c || k.shifted == c {
                    return k.finger;
                }
            }
        }
        Finger::None
    }
}

// -- short helper used while declaring the layouts --------------------------

const NIL: char = '\0';

const fn k(label: &'static str, u: char, s: char, w: f32, f: Finger) -> LayoutKey {
    LayoutKey {
        label,
        unshifted: u,
        shifted: s,
        width: w,
        finger: f,
    }
}

use Finger::{
    LeftIndex, LeftMiddle, LeftPinky, LeftRing, RightIndex, RightMiddle, RightPinky, RightRing,
    RightThumb,
};

// ---------------------------------------------------------------------------
//  US English (QWERTY) — fallback / default
// ---------------------------------------------------------------------------

static US_ROWS: &[&[LayoutKey]] = &[
    &[
        k("`", '`', '~', 1.0, LeftPinky),
        k("1", '1', '!', 1.0, LeftPinky),
        k("2", '2', '@', 1.0, LeftRing),
        k("3", '3', '#', 1.0, LeftMiddle),
        k("4", '4', '$', 1.0, LeftIndex),
        k("5", '5', '%', 1.0, LeftIndex),
        k("6", '6', '^', 1.0, RightIndex),
        k("7", '7', '&', 1.0, RightIndex),
        k("8", '8', '*', 1.0, RightMiddle),
        k("9", '9', '(', 1.0, RightRing),
        k("0", '0', ')', 1.0, RightPinky),
        k("-", '-', '_', 1.0, RightPinky),
        k("=", '=', '+', 1.0, RightPinky),
        k("⌫", NIL, NIL, 2.0, RightPinky),
    ],
    &[
        k("Tab", NIL, NIL, 1.5, LeftPinky),
        k("Q", 'q', 'Q', 1.0, LeftPinky),
        k("W", 'w', 'W', 1.0, LeftRing),
        k("E", 'e', 'E', 1.0, LeftMiddle),
        k("R", 'r', 'R', 1.0, LeftIndex),
        k("T", 't', 'T', 1.0, LeftIndex),
        k("Y", 'y', 'Y', 1.0, RightIndex),
        k("U", 'u', 'U', 1.0, RightIndex),
        k("I", 'i', 'I', 1.0, RightMiddle),
        k("O", 'o', 'O', 1.0, RightRing),
        k("P", 'p', 'P', 1.0, RightPinky),
        k("[", '[', '{', 1.0, RightPinky),
        k("]", ']', '}', 1.0, RightPinky),
        k("\\", '\\', '|', 1.5, RightPinky),
    ],
    &[
        k("Caps", NIL, NIL, 1.75, LeftPinky),
        k("A", 'a', 'A', 1.0, LeftPinky),
        k("S", 's', 'S', 1.0, LeftRing),
        k("D", 'd', 'D', 1.0, LeftMiddle),
        k("F", 'f', 'F', 1.0, LeftIndex),
        k("G", 'g', 'G', 1.0, LeftIndex),
        k("H", 'h', 'H', 1.0, RightIndex),
        k("J", 'j', 'J', 1.0, RightIndex),
        k("K", 'k', 'K', 1.0, RightMiddle),
        k("L", 'l', 'L', 1.0, RightRing),
        k(";", ';', ':', 1.0, RightPinky),
        k("'", '\'', '"', 1.0, RightPinky),
        k("Enter", NIL, NIL, 2.25, RightPinky),
    ],
    &[
        k("Shift", NIL, NIL, 2.25, LeftPinky),
        k("Z", 'z', 'Z', 1.0, LeftPinky),
        k("X", 'x', 'X', 1.0, LeftRing),
        k("C", 'c', 'C', 1.0, LeftMiddle),
        k("V", 'v', 'V', 1.0, LeftIndex),
        k("B", 'b', 'B', 1.0, LeftIndex),
        k("N", 'n', 'N', 1.0, RightIndex),
        k("M", 'm', 'M', 1.0, RightIndex),
        k(",", ',', '<', 1.0, RightMiddle),
        k(".", '.', '>', 1.0, RightRing),
        k("/", '/', '?', 1.0, RightPinky),
        k("Shift", NIL, NIL, 2.75, RightPinky),
    ],
    &[
        k("Ctrl", NIL, NIL, 1.5, LeftPinky),
        k("Alt", NIL, NIL, 1.25, LeftPinky),
        k("Space", ' ', ' ', 8.0, RightThumb),
        k("Alt", NIL, NIL, 1.25, RightPinky),
        k("Ctrl", NIL, NIL, 1.5, RightPinky),
    ],
];

pub static US: KeyboardLayout = KeyboardLayout {
    name: "US English (QWERTY)",
    klid: "00000409",
    rows: US_ROWS,
};

// ---------------------------------------------------------------------------
//  UK English (QWERTY)
//   Differs from US in a few symbol placements (2/", 3/£, '/@).
// ---------------------------------------------------------------------------

static UK_ROWS: &[&[LayoutKey]] = &[
    &[
        k("`", '`', '¬', 1.0, LeftPinky),
        k("1", '1', '!', 1.0, LeftPinky),
        k("2", '2', '"', 1.0, LeftRing),
        k("3", '3', '£', 1.0, LeftMiddle),
        k("4", '4', '$', 1.0, LeftIndex),
        k("5", '5', '%', 1.0, LeftIndex),
        k("6", '6', '^', 1.0, RightIndex),
        k("7", '7', '&', 1.0, RightIndex),
        k("8", '8', '*', 1.0, RightMiddle),
        k("9", '9', '(', 1.0, RightRing),
        k("0", '0', ')', 1.0, RightPinky),
        k("-", '-', '_', 1.0, RightPinky),
        k("=", '=', '+', 1.0, RightPinky),
        k("⌫", NIL, NIL, 2.0, RightPinky),
    ],
    US_ROWS[1],
    &[
        k("Caps", NIL, NIL, 1.75, LeftPinky),
        k("A", 'a', 'A', 1.0, LeftPinky),
        k("S", 's', 'S', 1.0, LeftRing),
        k("D", 'd', 'D', 1.0, LeftMiddle),
        k("F", 'f', 'F', 1.0, LeftIndex),
        k("G", 'g', 'G', 1.0, LeftIndex),
        k("H", 'h', 'H', 1.0, RightIndex),
        k("J", 'j', 'J', 1.0, RightIndex),
        k("K", 'k', 'K', 1.0, RightMiddle),
        k("L", 'l', 'L', 1.0, RightRing),
        k(";", ';', ':', 1.0, RightPinky),
        k("'", '\'', '@', 1.0, RightPinky),
        k("Enter", NIL, NIL, 2.25, RightPinky),
    ],
    US_ROWS[3],
    US_ROWS[4],
];

pub static UK: KeyboardLayout = KeyboardLayout {
    name: "UK English (QWERTY)",
    klid: "00000809",
    rows: UK_ROWS,
};

// ---------------------------------------------------------------------------
//  French (AZERTY)
// ---------------------------------------------------------------------------

static FR_ROWS: &[&[LayoutKey]] = &[
    &[
        k("²", '²', '³', 1.0, LeftPinky),
        k("& 1", '&', '1', 1.0, LeftPinky),
        k("é 2", 'é', '2', 1.0, LeftRing),
        k("\" 3", '"', '3', 1.0, LeftMiddle),
        k("' 4", '\'', '4', 1.0, LeftIndex),
        k("( 5", '(', '5', 1.0, LeftIndex),
        k("- 6", '-', '6', 1.0, RightIndex),
        k("è 7", 'è', '7', 1.0, RightIndex),
        k("_ 8", '_', '8', 1.0, RightMiddle),
        k("ç 9", 'ç', '9', 1.0, RightRing),
        k("à 0", 'à', '0', 1.0, RightPinky),
        k(") °", ')', '°', 1.0, RightPinky),
        k("= +", '=', '+', 1.0, RightPinky),
        k("⌫", NIL, NIL, 2.0, RightPinky),
    ],
    &[
        k("Tab", NIL, NIL, 1.5, LeftPinky),
        k("A", 'a', 'A', 1.0, LeftPinky),
        k("Z", 'z', 'Z', 1.0, LeftRing),
        k("E", 'e', 'E', 1.0, LeftMiddle),
        k("R", 'r', 'R', 1.0, LeftIndex),
        k("T", 't', 'T', 1.0, LeftIndex),
        k("Y", 'y', 'Y', 1.0, RightIndex),
        k("U", 'u', 'U', 1.0, RightIndex),
        k("I", 'i', 'I', 1.0, RightMiddle),
        k("O", 'o', 'O', 1.0, RightRing),
        k("P", 'p', 'P', 1.0, RightPinky),
        k("^", '^', '¨', 1.0, RightPinky),
        k("$", '$', '£', 1.0, RightPinky),
        k("*", '*', 'µ', 1.5, RightPinky),
    ],
    &[
        k("Caps", NIL, NIL, 1.75, LeftPinky),
        k("Q", 'q', 'Q', 1.0, LeftPinky),
        k("S", 's', 'S', 1.0, LeftRing),
        k("D", 'd', 'D', 1.0, LeftMiddle),
        k("F", 'f', 'F', 1.0, LeftIndex),
        k("G", 'g', 'G', 1.0, LeftIndex),
        k("H", 'h', 'H', 1.0, RightIndex),
        k("J", 'j', 'J', 1.0, RightIndex),
        k("K", 'k', 'K', 1.0, RightMiddle),
        k("L", 'l', 'L', 1.0, RightRing),
        k("M", 'm', 'M', 1.0, RightPinky),
        k("ù %", 'ù', '%', 1.0, RightPinky),
        k("Enter", NIL, NIL, 2.25, RightPinky),
    ],
    &[
        k("Shift", NIL, NIL, 2.25, LeftPinky),
        k("W", 'w', 'W', 1.0, LeftPinky),
        k("X", 'x', 'X', 1.0, LeftRing),
        k("C", 'c', 'C', 1.0, LeftMiddle),
        k("V", 'v', 'V', 1.0, LeftIndex),
        k("B", 'b', 'B', 1.0, LeftIndex),
        k("N", 'n', 'N', 1.0, RightIndex),
        k(", ?", ',', '?', 1.0, RightIndex),
        k("; .", ';', '.', 1.0, RightMiddle),
        k(": /", ':', '/', 1.0, RightRing),
        k("! §", '!', '§', 1.0, RightPinky),
        k("Shift", NIL, NIL, 2.75, RightPinky),
    ],
    US_ROWS[4],
];

pub static FR: KeyboardLayout = KeyboardLayout {
    name: "French (AZERTY)",
    klid: "0000040C",
    rows: FR_ROWS,
};

// ---------------------------------------------------------------------------
//  German (QWERTZ)
// ---------------------------------------------------------------------------

static DE_ROWS: &[&[LayoutKey]] = &[
    &[
        k("^ °", '^', '°', 1.0, LeftPinky),
        k("1 !", '1', '!', 1.0, LeftPinky),
        k("2 \"", '2', '"', 1.0, LeftRing),
        k("3 §", '3', '§', 1.0, LeftMiddle),
        k("4 $", '4', '$', 1.0, LeftIndex),
        k("5 %", '5', '%', 1.0, LeftIndex),
        k("6 &", '6', '&', 1.0, RightIndex),
        k("7 /", '7', '/', 1.0, RightIndex),
        k("8 (", '8', '(', 1.0, RightMiddle),
        k("9 )", '9', ')', 1.0, RightRing),
        k("0 =", '0', '=', 1.0, RightPinky),
        k("ß ?", 'ß', '?', 1.0, RightPinky),
        k("´ `", '´', '`', 1.0, RightPinky),
        k("⌫", NIL, NIL, 2.0, RightPinky),
    ],
    &[
        k("Tab", NIL, NIL, 1.5, LeftPinky),
        k("Q", 'q', 'Q', 1.0, LeftPinky),
        k("W", 'w', 'W', 1.0, LeftRing),
        k("E", 'e', 'E', 1.0, LeftMiddle),
        k("R", 'r', 'R', 1.0, LeftIndex),
        k("T", 't', 'T', 1.0, LeftIndex),
        k("Z", 'z', 'Z', 1.0, RightIndex),
        k("U", 'u', 'U', 1.0, RightIndex),
        k("I", 'i', 'I', 1.0, RightMiddle),
        k("O", 'o', 'O', 1.0, RightRing),
        k("P", 'p', 'P', 1.0, RightPinky),
        k("Ü", 'ü', 'Ü', 1.0, RightPinky),
        k("+ *", '+', '*', 1.0, RightPinky),
        k("# '", '#', '\'', 1.5, RightPinky),
    ],
    &[
        k("Caps", NIL, NIL, 1.75, LeftPinky),
        k("A", 'a', 'A', 1.0, LeftPinky),
        k("S", 's', 'S', 1.0, LeftRing),
        k("D", 'd', 'D', 1.0, LeftMiddle),
        k("F", 'f', 'F', 1.0, LeftIndex),
        k("G", 'g', 'G', 1.0, LeftIndex),
        k("H", 'h', 'H', 1.0, RightIndex),
        k("J", 'j', 'J', 1.0, RightIndex),
        k("K", 'k', 'K', 1.0, RightMiddle),
        k("L", 'l', 'L', 1.0, RightRing),
        k("Ö", 'ö', 'Ö', 1.0, RightPinky),
        k("Ä", 'ä', 'Ä', 1.0, RightPinky),
        k("Enter", NIL, NIL, 2.25, RightPinky),
    ],
    &[
        k("Shift", NIL, NIL, 2.25, LeftPinky),
        k("Y", 'y', 'Y', 1.0, LeftPinky),
        k("X", 'x', 'X', 1.0, LeftRing),
        k("C", 'c', 'C', 1.0, LeftMiddle),
        k("V", 'v', 'V', 1.0, LeftIndex),
        k("B", 'b', 'B', 1.0, LeftIndex),
        k("N", 'n', 'N', 1.0, RightIndex),
        k("M", 'm', 'M', 1.0, RightIndex),
        k(", ;", ',', ';', 1.0, RightMiddle),
        k(". :", '.', ':', 1.0, RightRing),
        k("- _", '-', '_', 1.0, RightPinky),
        k("Shift", NIL, NIL, 2.75, RightPinky),
    ],
    US_ROWS[4],
];

pub static DE: KeyboardLayout = KeyboardLayout {
    name: "German (QWERTZ)",
    klid: "00000407",
    rows: DE_ROWS,
};

// ---------------------------------------------------------------------------
//  Spanish (Spain) — QWERTY with extra ñ and accent keys
// ---------------------------------------------------------------------------

static ES_ROWS: &[&[LayoutKey]] = &[
    &[
        k("º ª", 'º', 'ª', 1.0, LeftPinky),
        k("1 !", '1', '!', 1.0, LeftPinky),
        k("2 \"", '2', '"', 1.0, LeftRing),
        k("3 ·", '3', '·', 1.0, LeftMiddle),
        k("4 $", '4', '$', 1.0, LeftIndex),
        k("5 %", '5', '%', 1.0, LeftIndex),
        k("6 &", '6', '&', 1.0, RightIndex),
        k("7 /", '7', '/', 1.0, RightIndex),
        k("8 (", '8', '(', 1.0, RightMiddle),
        k("9 )", '9', ')', 1.0, RightRing),
        k("0 =", '0', '=', 1.0, RightPinky),
        k("' ?", '\'', '?', 1.0, RightPinky),
        k("¡ ¿", '¡', '¿', 1.0, RightPinky),
        k("⌫", NIL, NIL, 2.0, RightPinky),
    ],
    US_ROWS[1],
    &[
        k("Caps", NIL, NIL, 1.75, LeftPinky),
        k("A", 'a', 'A', 1.0, LeftPinky),
        k("S", 's', 'S', 1.0, LeftRing),
        k("D", 'd', 'D', 1.0, LeftMiddle),
        k("F", 'f', 'F', 1.0, LeftIndex),
        k("G", 'g', 'G', 1.0, LeftIndex),
        k("H", 'h', 'H', 1.0, RightIndex),
        k("J", 'j', 'J', 1.0, RightIndex),
        k("K", 'k', 'K', 1.0, RightMiddle),
        k("L", 'l', 'L', 1.0, RightRing),
        k("Ñ", 'ñ', 'Ñ', 1.0, RightPinky),
        k("´ ¨", '´', '¨', 1.0, RightPinky),
        k("Enter", NIL, NIL, 2.25, RightPinky),
    ],
    US_ROWS[3],
    US_ROWS[4],
];

pub static ES: KeyboardLayout = KeyboardLayout {
    name: "Spanish (QWERTY)",
    klid: "0000040A",
    rows: ES_ROWS,
};

// ---------------------------------------------------------------------------
//  Norwegian (QWERTY) — adds Å, Ø, Æ; same finger layout as US for letters.
// ---------------------------------------------------------------------------

static NO_ROWS: &[&[LayoutKey]] = &[
    &[
        k("| §", '|', '§', 1.0, LeftPinky),
        k("1 !", '1', '!', 1.0, LeftPinky),
        k("2 \"", '2', '"', 1.0, LeftRing),
        k("3 #", '3', '#', 1.0, LeftMiddle),
        k("4 ¤", '4', '¤', 1.0, LeftIndex),
        k("5 %", '5', '%', 1.0, LeftIndex),
        k("6 &", '6', '&', 1.0, RightIndex),
        k("7 /", '7', '/', 1.0, RightIndex),
        k("8 (", '8', '(', 1.0, RightMiddle),
        k("9 )", '9', ')', 1.0, RightRing),
        k("0 =", '0', '=', 1.0, RightPinky),
        k("+ ?", '+', '?', 1.0, RightPinky),
        k("\\ `", '\\', '`', 1.0, RightPinky),
        k("⌫", NIL, NIL, 2.0, RightPinky),
    ],
    &[
        k("Tab", NIL, NIL, 1.5, LeftPinky),
        k("Q", 'q', 'Q', 1.0, LeftPinky),
        k("W", 'w', 'W', 1.0, LeftRing),
        k("E", 'e', 'E', 1.0, LeftMiddle),
        k("R", 'r', 'R', 1.0, LeftIndex),
        k("T", 't', 'T', 1.0, LeftIndex),
        k("Y", 'y', 'Y', 1.0, RightIndex),
        k("U", 'u', 'U', 1.0, RightIndex),
        k("I", 'i', 'I', 1.0, RightMiddle),
        k("O", 'o', 'O', 1.0, RightRing),
        k("P", 'p', 'P', 1.0, RightPinky),
        k("Å", 'å', 'Å', 1.0, RightPinky),
        k("¨ ^", '¨', '^', 1.0, RightPinky),
        k("' *", '\'', '*', 1.5, RightPinky),
    ],
    &[
        k("Caps", NIL, NIL, 1.75, LeftPinky),
        k("A", 'a', 'A', 1.0, LeftPinky),
        k("S", 's', 'S', 1.0, LeftRing),
        k("D", 'd', 'D', 1.0, LeftMiddle),
        k("F", 'f', 'F', 1.0, LeftIndex),
        k("G", 'g', 'G', 1.0, LeftIndex),
        k("H", 'h', 'H', 1.0, RightIndex),
        k("J", 'j', 'J', 1.0, RightIndex),
        k("K", 'k', 'K', 1.0, RightMiddle),
        k("L", 'l', 'L', 1.0, RightRing),
        k("Ø", 'ø', 'Ø', 1.0, RightPinky),
        k("Æ", 'æ', 'Æ', 1.0, RightPinky),
        k("Enter", NIL, NIL, 2.25, RightPinky),
    ],
    &[
        k("Shift", NIL, NIL, 1.25, LeftPinky),
        k("< >", '<', '>', 1.0, LeftPinky),
        k("Z", 'z', 'Z', 1.0, LeftPinky),
        k("X", 'x', 'X', 1.0, LeftRing),
        k("C", 'c', 'C', 1.0, LeftMiddle),
        k("V", 'v', 'V', 1.0, LeftIndex),
        k("B", 'b', 'B', 1.0, LeftIndex),
        k("N", 'n', 'N', 1.0, RightIndex),
        k("M", 'm', 'M', 1.0, RightIndex),
        k(", ;", ',', ';', 1.0, RightMiddle),
        k(". :", '.', ':', 1.0, RightRing),
        k("- _", '-', '_', 1.0, RightPinky),
        k("Shift", NIL, NIL, 2.75, RightPinky),
    ],
    US_ROWS[4],
];

pub static NO: KeyboardLayout = KeyboardLayout {
    name: "Norwegian (QWERTY)",
    klid: "00000414",
    rows: NO_ROWS,
};

// ---------------------------------------------------------------------------
//  Layout lookup
// ---------------------------------------------------------------------------

const ALL_LAYOUTS: &[&KeyboardLayout] = &[&US, &UK, &FR, &DE, &ES, &NO];

/// Map a Windows KLID (or the language-id portion of one) to a known layout.
/// Falls back to US if the layout is unknown.
fn layout_for_klid(klid: &str) -> &'static KeyboardLayout {
    // Try exact match first.
    for layout in ALL_LAYOUTS {
        if klid.eq_ignore_ascii_case(layout.klid) {
            return layout;
        }
    }
    // KLIDs are 8 hex chars; the lower 16 bits are the primary-language id.
    // We also accept matches against just the lower 4 chars so that variant
    // layouts (e.g. Canadian French "00000C0C") map to a sensible default.
    if klid.len() >= 4 {
        let tail = &klid[klid.len() - 4..];
        for layout in ALL_LAYOUTS {
            if layout.klid.ends_with(&tail.to_ascii_uppercase())
                || layout.klid.ends_with(&tail.to_ascii_lowercase())
            {
                return layout;
            }
        }
        // Handle common language families by primary language id.
        match tail.to_ascii_lowercase().as_str() {
            "040c" | "080c" | "0c0c" | "100c" | "140c" => return &FR, // French variants
            "0407" | "0807" | "0c07" | "1007" | "1407" => return &DE, // German variants
            "040a" | "080a" | "0c0a" | "100a" => return &ES,          // Spanish variants
            "0409" | "0809" | "0c09" | "1009" => return &US,          // English variants
            "0414" | "0814" => return &NO, // Norwegian variants (Bokmål / Nynorsk)
            _ => {}
        }
    }
    &US
}

/// The KLID Windows currently reports for the active input thread, e.g.
/// `"00000409"`. Returns `None` if we can't query it (non-Windows or error).
#[cfg(windows)]
fn detect_klid() -> Option<String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayoutNameW;
    // KL_NAMELENGTH is 9 (8 hex chars + trailing NUL).
    let mut buf = [0u16; 9];
    let ok = unsafe { GetKeyboardLayoutNameW(buf.as_mut_ptr()) };
    if ok == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

#[cfg(not(windows))]
fn detect_klid() -> Option<String> {
    None
}

/// Result of an auto-detection attempt.
pub struct Detected {
    /// Layout to render and use for finger lookup.
    pub layout: &'static KeyboardLayout,
    /// KLID reported by the OS (or `None` if unavailable / non-Windows).
    pub detected_klid: Option<String>,
    /// True if the detected KLID matched a known layout exactly; false if we
    /// had to fall back.
    pub is_exact_match: bool,
}

/// Auto-detect the user's active keyboard layout. Returns US QWERTY as the
/// fallback when detection fails or the layout isn't in our table.
pub fn detect() -> Detected {
    let klid = detect_klid();
    match &klid {
        Some(k) => {
            let layout = layout_for_klid(k);
            let exact = ALL_LAYOUTS
                .iter()
                .any(|l| k.eq_ignore_ascii_case(l.klid) && std::ptr::eq(layout, *l));
            Detected {
                layout,
                detected_klid: klid,
                is_exact_match: exact,
            }
        }
        None => Detected {
            layout: &US,
            detected_klid: None,
            is_exact_match: false,
        },
    }
}
