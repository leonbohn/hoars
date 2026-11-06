use std::borrow::Cow;

use chumsky::prelude::*;

/// Byte span in the input.
pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Token<'a> {
    Int(&'a str),
    Text(Cow<'a, str>),
    Identifier(&'a str),
    Alias(&'a str),
    Header(&'a str),
    Op(char),
    Paren(char),
    BodyStart,
    BodyEnd,
    Abort,
    Fin,
    Inf,
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Int(n) => write!(f, "{}", n),
            Token::Text(txt) => write!(f, "{}", txt),
            Token::Identifier(id) => write!(f, "{}", id),
            Token::Alias(alias) => write!(f, "@{}", alias),
            Token::Header(hdr) => write!(f, "{}:", hdr),
            Token::Op(o) => write!(f, "{}", o),
            Token::Paren(c) => write!(f, "{}", c),
            Token::Fin => write!(f, "Fin"),
            Token::Inf => write!(f, "Inf"),
            Token::BodyEnd => write!(f, "--END--"),
            Token::BodyStart => write!(f, "--BODY--"),
            Token::Abort => write!(f, "--ABORT--"),
        }
    }
}

/// Lexer. Its error type is `Cheap` on purpose: tracking what was expected at every failed
/// alternative made lexing 4-5x slower, and `input::lex` reports the offending character itself.
pub fn tokenizer<'a>() -> impl Parser<'a, &'a str, Vec<(Token<'a>, Span)>, extra::Err<Cheap<Span>>>
{
    let int = text::int(10).map(Token::Int);

    // C-like strings: `\"` and `\\` are decoded, any other escape is kept verbatim
    // (backslash included). Strings without escapes are borrowed from the input.
    let plain = just('"')
        .ignore_then(none_of("\"\\").repeated().to_slice())
        .then_ignore(just('"'))
        .map(|s: &str| Token::Text(Cow::Borrowed(s)));
    let escape = just('\\').ignore_then(any().map(|c| match c {
        '"' | '\\' => c.to_string(),
        c => format!("\\{c}"),
    }));
    let escaped = just('"')
        .ignore_then(
            escape
                .or(none_of("\"\\").map(|c: char| c.to_string()))
                .repeated()
                .collect::<Vec<String>>(),
        )
        .then_ignore(just('"'))
        .map(|parts| Token::Text(Cow::Owned(parts.concat())));
    let str_ = plain.or(escaped);

    let op = one_of("!|&").map(Token::Op);

    let paren = one_of("(){}[]").map(Token::Paren);

    let raw_ident = any()
        .filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .repeated(),
        )
        .to_slice();

    let ident = raw_ident.map(|ident: &str| match ident {
        "Fin" => Token::Fin,
        "Inf" => Token::Inf,
        _ => Token::Identifier(ident),
    });

    // minor versions such as `v1.1`; the only identifiers containing a `.`
    let version = just('v')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit() || *c == '.')
                .repeated()
                .at_least(1),
        )
        .to_slice()
        .filter(|v: &&str| v.contains('.'))
        .map(Token::Identifier);

    // unlike identifiers, alias names may start with a digit (Spot emits `@0`, `@1`, ...)
    let alias = just('@')
        .ignore_then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .repeated()
                .at_least(1)
                .to_slice(),
        )
        .map(Token::Alias);

    // Spot emits header names such as `spot.state-player`, so `.` is allowed here only.
    let header = any()
        .filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || matches!(*c, '_' | '-' | '.'))
                .repeated(),
        )
        .to_slice()
        .then_ignore(just(':'))
        .map(Token::Header);

    let body = just("--BODY--").to(Token::BodyStart);
    let end_ = just("--END--").to(Token::BodyEnd);
    let abort = just("--ABORT--").to(Token::Abort);

    // first characters of the alternatives are disjoint except for header, version, ident
    // (tried in that order), so the cheap single-character tokens come first
    let token = choice((
        op, paren, int, abort, end_, body, header, version, str_, alias, ident,
    ));

    // comments nest: `/*a/*b*/c*/` is a single comment
    let comment = recursive(|comment| {
        just("/*")
            .ignore_then(
                comment
                    .or(any().and_is(just("*/").not()).ignored())
                    .repeated(),
            )
            .then_ignore(just("*/"))
            .ignored()
    })
    .padded();

    token
        .map_with(|tok, e| (tok, e.span()))
        .padded_by(comment.repeated())
        .padded()
        .repeated()
        .collect()
}
