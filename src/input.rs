use chumsky::Parser;

use crate::{Diagnostic, FromHoaError, HoaRepresentation, Token, lexer};

/// Lexes `input` into tokens with their (byte) spans. The parser itself does not use the
/// tokens, only [`crate::parse_hoa_automata`] and [`crate::first_automaton_split_position`] do,
/// to find where automata begin and end.
pub(crate) fn lex(input: &str) -> Result<Vec<(Token<'_>, lexer::Span)>, FromHoaError> {
    let tokens = lexer::tokenizer()
        .parse(input)
        .into_result()
        .map_err(|errors| {
            FromHoaError::LexerError(
                errors
                    .into_iter()
                    .map(|e| {
                        let found = input[e.span().start..].chars().next();
                        Diagnostic {
                            span: e.span().into_range(),
                            message: match found {
                                Some(c) => format!("unexpected character {c:?}"),
                                None => "unexpected end of input".to_string(),
                            },
                            expected: Vec::new(),
                            found: found.map(String::from),
                        }
                    })
                    .collect(),
            )
        })?;
    Ok(tokens)
}

pub fn from_hoa(value: &str) -> Result<HoaRepresentation, FromHoaError> {
    parse_str(value)
}

/// Parses exactly one automaton from `input` (text, comments and whitespace included).
pub(crate) fn parse_str(input: &str) -> Result<HoaRepresentation, FromHoaError> {
    // First attempt without error tracking (much faster); only if it fails, look closer to
    // produce the diagnostics.
    match HoaRepresentation::parser::<chumsky::error::EmptyErr>()
        .parse(input)
        .into_result()
    {
        Ok(aut) => aut.and_then(|aut| aut.validate().map(|()| aut)),
        Err(_) => Err(diagnose(input)),
    }
}

/// The error for an `input` that failed to parse: a lexical error, an abort, or the parse error.
fn diagnose(input: &str) -> FromHoaError {
    match lex(input) {
        Err(e) => e,
        Ok(tokens) if tokens.iter().any(|(t, _)| *t == Token::Abort) => FromHoaError::Abort,
        Ok(_) => FromHoaError::ParserError(
            match HoaRepresentation::parser::<chumsky::error::Rich<char, lexer::Span>>()
                .parse(input)
                .into_result()
            {
                Err(errors) => errors.iter().map(Diagnostic::from_rich).collect(),
                Ok(_) => vec![Diagnostic {
                    span: 0..input.len(),
                    message: "parsing failed".to_string(),
                    expected: Vec::new(),
                    found: None,
                }],
            },
        ),
    }
}
