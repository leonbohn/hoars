use chumsky::{extra, input::InputRef, prelude::*, util::MaybeRef};

use crate::{
    AbstractLabelExpression, AcceptanceAtom, AcceptanceCondition, AcceptanceInfo,
    AcceptanceSignature, HoaBool, Id, StateConjunction,
};

/// Byte span in the input.
pub type Span = SimpleSpan;

/// Error types the parsers can run with: `EmptyErr` (fast, no information) for the first
/// attempt and `Rich` to produce diagnostics when that attempt failed.
pub trait HoaErr<'a>:
    'a
    + chumsky::error::Error<'a, &'a str>
    + chumsky::error::LabelError<'a, &'a str, MaybeRef<'a, char>>
{
    fn custom(span: Span, msg: String) -> Self;
}
impl<'a> HoaErr<'a> for chumsky::error::EmptyErr {
    fn custom(_: Span, _: String) -> Self {
        chumsky::error::EmptyErr::default()
    }
}
impl<'a> HoaErr<'a> for chumsky::error::Rich<'a, char, Span> {
    fn custom(span: Span, msg: String) -> Self {
        chumsky::error::Rich::custom(span, msg)
    }
}

/// Whitespace and (nested) `/* */` comments; every token parser consumes the layout behind
/// it, so the grammar never has to mention it.
pub(crate) fn ws<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, (), extra::Err<X>> + Clone {
    custom(|inp: &mut InputRef<'a, '_, &'a str, extra::Err<X>>| {
        let start = inp.cursor();
        let mut depth = 0usize;
        loop {
            let before = inp.save();
            match inp.next() {
                Some(c) if depth == 0 && c.is_whitespace() => {}
                Some('/') if inp.peek() == Some('*') => {
                    inp.skip();
                    depth += 1;
                }
                Some('*') if depth > 0 && inp.peek() == Some('/') => {
                    inp.skip();
                    depth -= 1;
                }
                Some(_) if depth > 0 => {}
                None if depth > 0 => {
                    return Err(X::custom(
                        inp.span_since(&start),
                        "unclosed comment".to_string(),
                    ));
                }
                _ => {
                    inp.rewind(before);
                    return Ok(());
                }
            }
        }
    })
}

/// A fixed keyword (`--BODY--`, `Fin`, a header name with its colon, ...).
pub(crate) fn tok<'a, X: HoaErr<'a>>(
    s: &'static str,
) -> impl Parser<'a, &'a str, (), extra::Err<X>> + Clone {
    just(s).ignored().then_ignore(ws())
}

/// A single punctuation character.
pub(crate) fn sym<'a, X: HoaErr<'a>>(
    c: char,
) -> impl Parser<'a, &'a str, (), extra::Err<X>> + Clone {
    just(c).ignored().then_ignore(ws())
}

fn ident_start(c: &char) -> bool {
    c.is_ascii_alphabetic() || *c == '_'
}

fn ident_raw<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, &'a str, extra::Err<X>> + Clone {
    any()
        .filter(ident_start)
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || matches!(*c, '_' | '-'))
                .repeated(),
        )
        .to_slice()
}

/// An identifier or `t`/`f`/`Fin`/`Inf`. A word directly followed by `:` or `.` is (the start
/// of) a header name such as `spot.state-player:`, not a word.
fn word<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, &'a str, extra::Err<X>> + Clone {
    // minor versions such as `v1.1`; the only identifiers containing a `.`
    let version = just('v')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit() || *c == '.')
                .repeated()
                .at_least(1),
        )
        .to_slice()
        .filter(|v: &&str| v.contains('.'));
    version
        .or(ident_raw())
        .then_ignore(one_of(":.").not())
        .then_ignore(ws())
}

/// A header name followed by its colon, for names that are not matched as keywords. Spot
/// emits names such as `spot.state-player`, so `.` is allowed here only.
pub fn header_name<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, &'a str, extra::Err<X>> + Clone
{
    any()
        .filter(ident_start)
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || matches!(*c, '_' | '-' | '.'))
                .repeated(),
        )
        .to_slice()
        .then_ignore(just(':'))
        .then_ignore(ws())
}

pub fn boolean<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, bool, extra::Err<X>> + Clone {
    word().try_map(|w: &str, span| match w {
        "t" => Ok(true),
        "f" => Ok(false),
        _ => Err(X::custom(span, "expected t or f".to_string())),
    })
}

/// A non-negative integer without leading zeros, as the raw text.
pub fn int_text<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, &'a str, extra::Err<X>> + Clone {
    one_of("123456789")
        .then(any().filter(char::is_ascii_digit).repeated())
        .ignored()
        .or(just('0').ignored())
        .to_slice()
        .then_ignore(ws())
}

pub fn integer<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, Id, extra::Err<X>> + Clone {
    int_text().try_map(|n: &str, span| {
        n.parse::<Id>()
            .map_err(|_| X::custom(span, "integer out of range".to_string()))
    })
}

/// A quoted string. `\"` and `\\` are decoded, any other escape is kept verbatim
/// (backslash included).
pub fn text<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, String, extra::Err<X>> + Clone {
    let plain = none_of("\"\\").repeated().to_slice().map(str::to_string);
    let escaped = just('\\')
        .ignore_then(any().map(|c| match c {
            '"' | '\\' => c.to_string(),
            c => format!("\\{c}"),
        }))
        .or(none_of("\"\\").map(|c: char| c.to_string()))
        .repeated()
        .collect::<Vec<String>>()
        .map(|parts| parts.concat());
    just('"')
        .ignore_then(
            plain
                .then_ignore(just('"'))
                .or(escaped.then_ignore(just('"'))),
        )
        .then_ignore(ws())
}

pub fn identifier<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, String, extra::Err<X>> + Clone {
    word()
        .filter(|w: &&str| !matches!(*w, "Fin" | "Inf"))
        .map(str::to_string)
}

pub fn alias_name<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, String, extra::Err<X>> + Clone {
    just('@')
        .ignore_then(ident_raw())
        .then_ignore(ws())
        .map(str::to_string)
}

pub fn state_conjunction<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, StateConjunction, extra::Err<X>> + Clone {
    integer()
        .separated_by(sym('&'))
        .at_least(1)
        .collect()
        .map(StateConjunction)
}

pub fn acceptance_signature<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, AcceptanceSignature, extra::Err<X>> + Clone {
    integer()
        .repeated()
        .collect()
        .delimited_by(sym('{'), sym('}'))
        .map(AcceptanceSignature)
}

pub fn acceptance_info<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, AcceptanceInfo, extra::Err<X>> + Clone {
    identifier()
        .map(AcceptanceInfo::Identifier)
        .or(integer().map(AcceptanceInfo::Int))
}

fn ap_index<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, AbstractLabelExpression, extra::Err<X>> + Clone {
    integer().try_map(|i, span| {
        u16::try_from(i)
            .map(AbstractLabelExpression::Integer)
            .map_err(|_| X::custom(span, "AP identifier out of range".to_string()))
    })
}

pub fn label_expression<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, AbstractLabelExpression, extra::Err<X>> + Clone {
    recursive(|label_expression| {
        let value = boolean()
            .map(AbstractLabelExpression::Boolean)
            .or(ap_index())
            .or(alias_name().map(AbstractLabelExpression::Alias));

        let atom = value.or(label_expression.delimited_by(sym('('), sym(')')));

        let unary = sym('!').or_not().then(atom).map(|(negated, expr)| {
            if negated.is_some() {
                AbstractLabelExpression::Negated(Box::new(expr))
            } else {
                expr
            }
        });

        let conjunction = unary
            .separated_by(sym('&'))
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|mut acc| match acc.len() {
                1 => acc.remove(0),
                _ => AbstractLabelExpression::Conjunction(acc),
            });

        conjunction
            .separated_by(sym('|'))
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|mut acc| match acc.len() {
                1 => acc.remove(0),
                _ => AbstractLabelExpression::Disjunction(acc),
            })
    })
}

pub fn acceptance_condition<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, AcceptanceCondition, extra::Err<X>> + Clone {
    recursive(|acceptance_condition| {
        let atom_value = sym('!').or_not().then(integer()).map(|(negated, integer)| {
            if negated.is_none() {
                AcceptanceAtom::Positive(integer)
            } else {
                AcceptanceAtom::Negative(integer)
            }
        });

        let fin_inf_atom = tok("Fin")
            .to(AcceptanceCondition::Fin as fn(_) -> _)
            .or(tok("Inf").to(AcceptanceCondition::Inf as fn(_) -> _))
            .then(atom_value.delimited_by(sym('('), sym(')')))
            .map(|(f, atom)| f(atom));

        let atom = boolean()
            .map(HoaBool)
            .map(AcceptanceCondition::Boolean)
            .or(fin_inf_atom)
            .or(acceptance_condition.delimited_by(sym('('), sym(')')));

        let conjunction = atom.clone().foldl(
            sym('&')
                .to(AcceptanceCondition::And as fn(_, _) -> _)
                .then(atom)
                .repeated(),
            |lhs, (f, rhs)| f(Box::new(lhs), Box::new(rhs)),
        );

        conjunction.clone().foldl(
            sym('|')
                .to(AcceptanceCondition::Or as fn(_, _) -> _)
                .then(conjunction)
                .repeated(),
            |lhs, (f, rhs)| f(Box::new(lhs), Box::new(rhs)),
        )
    })
}
