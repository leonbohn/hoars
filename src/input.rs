use chumsky::{Parser, input::Stream, prelude::Input};

use crate::{FromHoaError, HoaRepresentation, Token, build_error_report, lexer};

/// Lexes `input` into tokens with their (byte) spans.
pub(crate) fn lex(input: &str) -> Result<Vec<(Token<'_>, lexer::Span)>, FromHoaError> {
    let start = std::time::Instant::now();
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
    let eoi: lexer::Span = (input.len()..input.len()).into();
    let stream = || Stream::from_iter(tokens.iter().cloned()).map(eoi, |(t, s)| (t, s));
    // First attempt without error tracking (much faster); only if it fails, parse again with
    // `Rich` errors to produce the diagnostics.
    let out = match HoaRepresentation::parser::<_, chumsky::error::EmptyErr>()
        .parse(stream())
        .into_result()
    {
        Ok(aut) => aut.validate().map(|()| aut),
        Err(_) => Err(FromHoaError::ParserError(
            match HoaRepresentation::parser::<_, chumsky::error::Rich<Token, lexer::Span>>()
                .parse(stream())
                .into_result()
            {
                Err(errors) => build_error_report(input, errors),
                Ok(_) => "parsing failed".to_string(),
            },
        )),
    };
    tracing::info!("Actual parsing took {}µs", start.elapsed().as_micros());
    out
}
