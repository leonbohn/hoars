use chumsky::prelude::*;

pub type Span = std::ops::Range<usize>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Token {
    Int(String),
    Text(String),
    Identifier(String),
    Alias(String),
    Header(String),
    Op(char),
    Paren(char),
    BodyStart,
    BodyEnd,
    Abort,
    Fin,
    Inf,
}

impl std::fmt::Display for Token {
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

pub fn tokenizer() -> impl Parser<char, Vec<(Token, Span)>, Error = Simple<char>> {
    let int = text::int(10).map(Token::Int);

    // C-like strings: `\"` and `\\` are decoded, any other escape is kept verbatim
    // (backslash included).
    let escape = just('\\').ignore_then(any().map(|c| match c {
        '"' | '\\' => c.to_string(),
        c => format!("\\{c}"),
    }));
    let str_ = just('"')
        .ignore_then(
            escape
                .or(filter(|c: &char| *c != '"' && *c != '\\').map(|c| c.to_string()))
                .repeated(),
        )
        .then_ignore(just('"'))
        .map(|parts| Token::Text(parts.concat()));

    let op = one_of("!|&").map(Token::Op);

    let paren = one_of(r#"(){}[]"#).map(Token::Paren);

    let raw_ident = filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .chain(filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').repeated())
        .collect::<String>();

    let ident = raw_ident.map(|ident: String| match ident.as_str() {
        "Fin" => Token::Fin,
        "Inf" => Token::Inf,
        _ => Token::Identifier(ident),
    });

    let alias = just('@').ignore_then(raw_ident).map(Token::Alias);

    // Spot emits header names such as `spot.state-player`, so `.` is allowed here only.
    let header = filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .chain(
            filter(|c: &char| c.is_ascii_alphanumeric() || matches!(*c, '_' | '-' | '.'))
                .repeated(),
        )
        .collect::<String>()
        .then_ignore(just(':'))
        .map(Token::Header);

    let body = just("--BODY--").to(Token::BodyStart);
    let end = just("--END--").to(Token::BodyEnd);
    let abort = just("--ABORT--").to(Token::Abort);

    let token = int
        .or(abort)
        .or(end)
        .or(body)
        .or(header)
        .or(str_)
        .or(op)
        .or(paren)
        .or(alias)
        .or(ident);

    // comments nest: `/*a/*b*/c*/` is a single comment
    let comment = recursive(|comment| {
        just("/*")
            .ignore_then(comment.or(just("*/").not().ignored()).repeated())
            .then_ignore(just("*/"))
            .ignored()
    })
    .padded();

    token
        .map_with_span(|tok, span| (tok, span))
        .padded_by(comment.repeated())
        .padded()
        .repeated()
}
