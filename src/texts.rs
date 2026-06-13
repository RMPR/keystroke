//! Sample texts for the free typing test mode.
//!
//! Most passages use ASCII-only punctuation (straight quotes, hyphens) so they
//! can be typed accurately regardless of the user's keyboard layout. A few
//! language-specific passages (e.g. Norwegian) intentionally use the extra
//! letters available on that layout.

pub struct SampleText {
    pub title: &'static str,
    pub source: &'static str,
    pub content: &'static str,
}

pub const TEXTS: &[SampleText] = &[
    // ---- The Bible (English Standard Version) ----------------------------
    SampleText {
        title: "Genesis 1:1",
        source: "The Bible, ESV",
        content: "In the beginning, God created the heavens and the earth.",
    },
    SampleText {
        title: "Psalm 23:1",
        source: "The Bible, ESV",
        content: "The LORD is my shepherd; I shall not want.",
    },
    SampleText {
        title: "Proverbs 3:5",
        source: "The Bible, ESV",
        content:
            "Trust in the LORD with all your heart, and do not lean on your own understanding.",
    },
    SampleText {
        title: "John 3:16",
        source: "The Bible, ESV",
        content: "For God so loved the world, that he gave his only Son, that whoever believes in him should not perish but have eternal life.",
    },
    SampleText {
        title: "Philippians 4:13",
        source: "The Bible, ESV",
        content: "I can do all things through him who strengthens me.",
    },

    // ---- The Lord of the Rings -------------------------------------------
    SampleText {
        title: "The Hobbit (opening)",
        source: "J.R.R. Tolkien",
        content: "In a hole in the ground there lived a hobbit.",
    },
    SampleText {
        title: "Bilbo's poem",
        source: "The Fellowship of the Ring",
        content: "All that is gold does not glitter, not all those who wander are lost.",
    },
    SampleText {
        title: "Gandalf to Frodo",
        source: "The Fellowship of the Ring",
        content: "All we have to decide is what to do with the time that is given us.",
    },
    SampleText {
        title: "The Ring inscription",
        source: "The Fellowship of the Ring",
        content: "One Ring to rule them all, One Ring to find them, One Ring to bring them all and in the darkness bind them.",
    },

    // ---- The Chronicles of Narnia ----------------------------------------
    SampleText {
        title: "Opening line",
        source: "The Lion, the Witch and the Wardrobe",
        content: "Once there were four children whose names were Peter, Susan, Edmund and Lucy.",
    },
    SampleText {
        title: "Aslan to Lucy",
        source: "The Voyage of the Dawn Treader",
        content: "Courage, dear heart.",
    },
    SampleText {
        title: "Aslan is on the move",
        source: "The Lion, the Witch and the Wardrobe",
        content: "They say Aslan is on the move, perhaps has already landed.",
    },
    SampleText {
        title: "Reading fairy tales",
        source: "C.S. Lewis, dedication",
        content: "Some day you will be old enough to start reading fairy tales again.",
    },

    // ---- Norwegian (Bokmål) ----------------------------------------------
    SampleText {
        title: "Ja, vi elsker (åpning)",
        source: "Bjørnstjerne Bjørnson, Norges nasjonalsang",
        content: "Ja, vi elsker dette landet, som det stiger frem, furet, værbitt over vannet, med de tusen hjem.",
    },
    SampleText {
        title: "Peer Gynt",
        source: "Henrik Ibsen",
        content: "Tenke det; ønske det; ville det med; men gjøre det!",
    },
    SampleText {
        title: "Sult (åpning)",
        source: "Knut Hamsun",
        content: "Det var i den tid jeg gikk omkring og sultet i Kristiania, denne forunderlige by som ingen forlater før han har fått merker av den.",
    },
    SampleText {
        title: "Eventyr",
        source: "Norsk folkeeventyr",
        content: "Det var en gang en konge som hadde en datter, og hun var så vakker at det gikk gjetord om henne over alle land.",
    },
];

/// Returns the content of a pseudo-randomly chosen sample text.
///
/// Used by the LAN multiplayer race game to pick a passage. The selection is
/// derived from the current system time so it doesn't require a randomness
/// dependency, and is good enough for "don't always pick the same one".
pub fn pick_random_text() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let idx = (nanos as usize) % TEXTS.len();
    TEXTS[idx].content
}
