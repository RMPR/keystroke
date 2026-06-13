//! Lesson definitions for the structured learning path.

pub struct Lesson {
    pub title: &'static str,
    pub description: &'static str,
    pub text: &'static str,
}

pub const LESSONS: &[Lesson] = &[
    Lesson {
        title: "Lesson 1 — Home Row (Left Hand)",
        description: "Place your left fingers on A S D F. Keep your eyes on the screen, not on the keyboard. The F key has a small bump so your index finger can find it without looking.",
        text: "asdf fdsa asdf fdsa aaaa ssss dddd ffff",
    },
    Lesson {
        title: "Lesson 2 — Home Row (Right Hand)",
        description: "Place your right fingers on J K L ;. The J key also has a bump. Always return your fingers to the home row after pressing other keys.",
        text: "jkl; ;lkj jkl; ;lkj jjjj kkkk llll ;;;;",
    },
    Lesson {
        title: "Lesson 3 — Home Row Together",
        description: "Practice the entire home row using both hands. Try to keep a steady rhythm rather than rushing.",
        text: "ask dad sad lad fall jak ada lass jaffa",
    },
    Lesson {
        title: "Lesson 4 — Top Row",
        description: "Reach up from the home row to type Q W E R T Y U I O P. Your fingers should return to the home row after each press.",
        text: "the quiet fox quickly types ten quirky words",
    },
    Lesson {
        title: "Lesson 5 — Bottom Row",
        description: "Reach down from the home row to type Z X C V B N M. The bottom row uses the same fingers as the keys directly above them on the home row.",
        text: "zoo can vex many brave zebras my common box",
    },
    Lesson {
        title: "Lesson 6 — The Space Bar",
        description: "Use either thumb to tap the space bar. Most people prefer the right thumb. Keep your other fingers on the home row while doing so.",
        text: "a b c d e f g h i j k l m n o p q r s",
    },
    Lesson {
        title: "Lesson 7 — Capital Letters",
        description: "Hold the Shift key with the pinky on the opposite hand to type capital letters. For an A, hold Right Shift; for an L, hold Left Shift.",
        text: "The Quick Brown Fox Jumps Over The Lazy Dog",
    },
    Lesson {
        title: "Lesson 8 — Punctuation",
        description: "Practice common punctuation marks: comma, period, semicolon, question mark. Your right hand handles most punctuation.",
        text: "Hello, world. How are you? I am fine, thanks.",
    },
];
