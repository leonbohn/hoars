use chumsky::{Parser, input::Stream, prelude::Input};

use crate::{FromHoaError, HoaRepresentation, Token, build_error_report, lexer};

/// Lexes `input` into tokens with their (byte) spans.
pub(crate) fn lex(input: &str) -> Result<Vec<(Token<'_>, lexer::Span)>, FromHoaError> {
    let start = std::time::Instant::now();
    let tokens = lexer::tokenizer()
        .parse(input)
        .into_result()
        .map_err(|errors| build_error_report(input, errors))
        .map_err(FromHoaError::LexerError)?;
    tracing::info!("Tokenization took {}µs", start.elapsed().as_micros());
    Ok(tokens)
}

pub fn from_hoa(value: &str) -> Result<HoaRepresentation, FromHoaError> {
    let tokens = lex(value)?;
    if tokens.iter().any(|(t, _)| *t == Token::Abort) {
        return Err(FromHoaError::Abort);
    }
    parse_tokens(value, tokens)
}

/// Parses already lexed `tokens` that stem from `input` (used for error reports).
pub(crate) fn parse_tokens<'a>(
    input: &'a str,
    tokens: Vec<(Token<'a>, lexer::Span)>,
) -> Result<HoaRepresentation, FromHoaError> {
    let start = std::time::Instant::now();
    let eoi = (input.len()..input.len()).into();
    let out = HoaRepresentation::parser()
        .parse(Stream::from_iter(tokens).map(eoi, |(t, s)| (t, s)))
        .into_result()
        .map_err(|errors| build_error_report(input, errors))
        .map_err(FromHoaError::ParserError)
        .and_then(|aut| aut.validate().map(|()| aut));
    tracing::info!("Actual parsing took {}µs", start.elapsed().as_micros());
    out
}
