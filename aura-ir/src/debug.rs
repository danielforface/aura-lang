#![forbid(unsafe_code)]

use aura_ast::Span;

#[derive(Clone, Debug)]
pub struct DebugSource {
    pub file_name: String,
    line_starts: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    /// 1-based line number
    pub line: u32,
    /// 1-based column number
    pub col: u32,
}

impl DebugSource {
    pub fn new(file_name: String, text: &str) -> Self {
        let mut line_starts: Vec<usize> = Vec::new();
        line_starts.push(0);
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { file_name, line_starts }
    }

    pub fn line_col(&self, span: Span) -> LineCol {
        let off: usize = span.offset().into();

        // Find the last line start <= off.
        let line_idx = match self.line_starts.binary_search(&off) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };

        let line_start = self.line_starts.get(line_idx).copied().unwrap_or(0);
        let col0 = off.saturating_sub(line_start);

        LineCol {
            line: (line_idx as u32) + 1,
            col: (col0 as u32) + 1,
        }
    }
}
