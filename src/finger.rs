//! Finger identification used by the touch-typing methodology.
//!
//! The actual character-to-finger mapping is layout-dependent and lives in
//! `crate::keyboard_layout`.

/// Index of each finger, matching the order used in the Slint UI.
/// `None` is represented by -1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[allow(dead_code)] // some variants only used when keyboard layouts assign them
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
