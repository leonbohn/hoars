use chumsky::Parser;

use crate::{FromHoaError, HoaRepresentation, Token, build_error_report, lexer};

/// Lexes `input` into tokens with their (byte) spans. The parser itself does not use the
/// tokens, only [`crate::parse_hoa_automata`] and [`crate::first_automaton_split_position`] do,
/// to find where automata begin and end.
pub(crate) fn lex(input: &str) -> Result<Vec<(Token<'_>, lexer::Span)>, FromHoaError> {
    let tokens = lexer::tokenizer()
        .parse(input)
        .into_result()
        .map_err(|errors| {
            build_error_report::<char>(
                input,
                errors.into_iter().map(|e| {
                    let found = input[e.span().start..].chars().next();
                    chumsky::error::Rich::custom(
                        *e.span(),
                        match found {
                            Some(c) => format!("unexpected character {c:?}"),
                            None => "unexpected end of input".to_string(),
                        },
                    )
                }),
            )
        })
        .map_err(FromHoaError::LexerError)?;
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
        Ok(aut) => aut.validate().map(|()| aut),
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
                Err(errors) => build_error_report(input, errors),
                Ok(_) => "parsing failed".to_string(),
            },
        ),
    }
}
