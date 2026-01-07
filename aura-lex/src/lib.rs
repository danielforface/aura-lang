#![forbid(unsafe_code)]

mod lexer;
mod token;

pub use lexer::{LexError, Lexer};
pub use token::{Token, TokenKind};

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lex_int_literals_with_bases_and_underscores() {
		let src = "val a = 1_000\nval b = 0b1010_0110\nval c = 0o755\nval d = 0xDEAD_BEEF\n";
		let tokens = Lexer::new(src).lex().unwrap();
		let ints: Vec<u64> = tokens
			.iter()
			.filter_map(|t| match &t.kind {
				TokenKind::Int(n) => Some(*n),
				_ => None,
			})
			.collect();
		assert_eq!(ints, vec![1000, 0b1010_0110, 0o755, 0xDEAD_BEEF]);
	}

	#[test]
	fn lex_rejects_bad_int_underscore_placement() {
		let err = Lexer::new("val x = 0x_DEAD\n").lex().unwrap_err();
		assert!(err.message.contains("invalid integer literal"));
	}

	#[test]
	fn lex_string_escapes_are_strict() {
		let tokens = Lexer::new("val s = \"a\\n\\t\\r\\\\\\\"\"\n").lex().unwrap();
		let s = tokens
			.iter()
			.find_map(|t| match &t.kind {
				TokenKind::String(s) => Some(s.clone()),
				_ => None,
			})
			.unwrap();
		assert_eq!(s, "a\n\t\r\\\"");
	}

	#[test]
	fn lex_string_unicode_escape() {
		let tokens = Lexer::new("val s = \"\\u{41}\\u{1f}\\u{7E}\"\n").lex().unwrap();
		let s = tokens
			.iter()
			.find_map(|t| match &t.kind {
				TokenKind::String(s) => Some(s.clone()),
				_ => None,
			})
			.unwrap();
		assert_eq!(s, "A\u{001F}~");
	}

	#[test]
	fn lex_rejects_unknown_string_escape() {
		let err = Lexer::new("val s = \"\\q\"\n").lex().unwrap_err();
		assert!(err.message.contains("invalid string literal"));
	}
}
