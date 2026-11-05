use chumsky::{Parser, Stream};

use crate::{FromHoaError, HoaRepresentation, Token, build_error_report, lexer};

/// Lexes `input` into tokens with their (character) spans.
pub(crate) fn lex(input: &str) -> Result<Vec<(Token, lexer::Span)>, FromHoaError> {
    let start = std::time::Instant::now();
    let tokens = lexer::tokenizer()
        .parse(input)
        .map_err(|error_list| {
            build_error_report(
                input,
                error_list.into_iter().map(|err| err.map(|c| c.to_string())),
            )
        })
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
pub(crate) fn parse_tokens(
    input: &str,
    tokens: Vec<(Token, lexer::Span)>,
) -> Result<HoaRepresentation, FromHoaError> {
    let length = input.chars().count();
    let start = std::time::Instant::now();
    let out = HoaRepresentation::parser()
        .parse(Stream::from_iter(length..length + 1, tokens.into_iter()))
        .map_err(|error_list| {
            build_error_report(
                input,
                error_list.into_iter().map(|err| err.map(|c| c.to_string())),
            )
        })
        .map_err(FromHoaError::ParserError);
    tracing::info!("Actual parsing took {}µs", start.elapsed().as_micros());
    out
}
