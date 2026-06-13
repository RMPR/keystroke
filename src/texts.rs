//! Sample texts for the free typing test mode.

pub struct SampleText {
    pub title: &'static str,
    pub content: &'static str,
}

pub const TEXTS: &[SampleText] = &[
    SampleText {
        title: "Pangram",
        content: "The quick brown fox jumps over the lazy dog near the riverbank.",
    },
    SampleText {
        title: "Practice",
        content: "Practice makes perfect, but only when you practice the right way.",
    },
    SampleText {
        title: "Programming",
        content: "Programming is the art of telling a computer what you want it to do.",
    },
    SampleText {
        title: "Story",
        content: "In a quiet village a small cat learned to type at amazing speeds.",
    },
    SampleText {
        title: "Advice",
        content: "Slow is smooth, and smooth is fast. Focus on accuracy before speed.",
    },
];
