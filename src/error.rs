use chumsky::error::Rich;
use chumsky::error::RichPattern;
use chumsky::error::RichReason;
use chumsky::span::SimpleSpan;

use crate::lexer::Token;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub span: SimpleSpan,
    pub message: String,
}

impl ParseError {
    pub fn from_rich<'a>(error: Rich<'a, Token<'a>, SimpleSpan>) -> Self {
        let span = *error.span();
        let message = format_error_message(&error);
        Self { span, message }
    }
}

fn format_error_message<'a>(error: &Rich<'a, Token<'a>, SimpleSpan>) -> String {
    match error.reason() {
        RichReason::ExpectedFound { .. } => {
            let found = format_found(error);
            if let Some(expected) = format_expected(error) {
                format!("expected {expected}, found {found}")
            } else {
                format!("found {found}")
            }
        }
        RichReason::Custom(message) => message.to_string(),
    }
}

fn format_expected<'a>(error: &Rich<'a, Token<'a>, SimpleSpan>) -> Option<String> {
    let mut expected = Vec::new();
    for pattern in error.expected() {
        match pattern {
            RichPattern::EndOfInput => expected.push("end of input".to_string()),
            _ => expected.push(pattern.to_string()),
        }
    }

    expected.sort();
    expected.dedup();
    if expected.is_empty() {
        None
    } else {
        Some(expected.join(", "))
    }
}

fn format_found<'a>(error: &Rich<'a, Token<'a>, SimpleSpan>) -> String {
    error
        .found()
        .map(|token| token.to_string())
        .unwrap_or_else(|| "end of input".to_string())
}
