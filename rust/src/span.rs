// fon-parser/src/span.rs

#![allow(clippy::missing_const_for_fn)]

/// A half-open byte range in the original UTF-8 source.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}
