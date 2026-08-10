//! In-game Bible reader — Word at the center of the loop.

#[derive(Debug, Clone, Copy)]
pub struct Verse {
    pub ref_: &'static str,
    pub text: &'static str,
}

pub const VERSES: &[Verse] = &[
    Verse {
        ref_: "Ephesians 6:10",
        text: "Finally, be strong in the Lord and in the strength of his might.",
    },
    Verse {
        ref_: "Matthew 6:10",
        text: "Your kingdom come, your will be done, on earth as it is in heaven.",
    },
    Verse {
        ref_: "Psalm 23:1",
        text: "The Lord is my shepherd; I shall not want.",
    },
    Verse {
        ref_: "Joshua 1:9",
        text: "Be strong and courageous. Do not be frightened, and do not be dismayed, for the Lord your God is with you wherever you go.",
    },
    Verse {
        ref_: "Romans 8:37",
        text: "In all these things we are more than conquerors through him who loved us.",
    },
    Verse {
        ref_: "Psalm 18:2",
        text: "The Lord is my rock and my fortress and my deliverer.",
    },
    Verse {
        ref_: "Isaiah 40:31",
        text: "But they who wait for the Lord shall renew their strength.",
    },
    Verse {
        ref_: "John 1:5",
        text: "The light shines in the darkness, and the darkness has not overcome it.",
    },
];

pub struct BibleState {
    pub open: bool,
    pub index: usize,
}

impl Default for BibleState {
    fn default() -> Self {
        Self {
            open: false,
            index: 0,
        }
    }
}

impl BibleState {
    pub fn current(&self) -> &'static Verse {
        &VERSES[self.index % VERSES.len()]
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % VERSES.len();
    }

    pub fn prev(&mut self) {
        self.index = if self.index == 0 {
            VERSES.len() - 1
        } else {
            self.index - 1
        };
    }
}
