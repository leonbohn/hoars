use chumsky::{Parser, Stream};

use crate::{FromHoaError, HoaRepresentation, build_error_report, lexer};

pub fn from_hoa(value: &str) -> Result<HoaRepresentation, FromHoaError> {
    if value.contains("--ABORT--") {
        return Err(FromHoaError::Abort);
    }
    let input = value;
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
