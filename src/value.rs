use chumsky::{prelude::*, select};

use crate::{
    AbstractLabelExpression, AcceptanceAtom, AcceptanceCondition, AcceptanceInfo,
    AcceptanceSignature, HoaBool, Id, StateConjunction, Token,
    lexer::{Err, TokInput},
};

#[allow(unused)]
pub fn header<'a, I: TokInput<'a>>() -> impl Parser<'a, I, String, Err<'a>> + Clone {
    select! {
        Token::Header(hdr) => hdr.to_string(),
    }
}

pub fn boolean<'a, I: TokInput<'a>>() -> impl Parser<'a, I, bool, Err<'a>> + Clone {
    select! {
        Token::Identifier("t") => true,
        Token::Identifier("f") => false,
    }
}

pub fn integer<'a, I: TokInput<'a>>() -> impl Parser<'a, I, Id, Err<'a>> + Clone {
    select! {
        Token::Int(n) => n,
    }
    .try_map(|n: &str, span| {
        n.parse::<Id>()
            .map_err(|_| Rich::custom(span, "integer out of range"))
    })
}

pub fn text<'a, I: TokInput<'a>>() -> impl Parser<'a, I, String, Err<'a>> + Clone {
    select! {
        Token::Text(txt) => txt.into_owned(),
    }
}

pub fn identifier<'a, I: TokInput<'a>>() -> impl Parser<'a, I, String, Err<'a>> + Clone {
    select! { Token::Identifier(ident) => ident.to_string() }
}

pub fn alias_name<'a, I: TokInput<'a>>() -> impl Parser<'a, I, String, Err<'a>> + Clone {
    select! { Token::Alias(aname) => aname.to_string() }
}

pub fn state_conjunction<'a, I: TokInput<'a>>()
-> impl Parser<'a, I, StateConjunction, Err<'a>> + Clone {
    integer()
        .separated_by(just(Token::Op('&')))
        .at_least(1)
        .collect()
        .map(StateConjunction)
}

pub fn acceptance_signature<'a, I: TokInput<'a>>()
-> impl Parser<'a, I, AcceptanceSignature, Err<'a>> + Clone {
    integer()
        .repeated()
        .collect()
        .delimited_by(just(Token::Paren('{')), just(Token::Paren('}')))
        .map(AcceptanceSignature)
}

pub fn acceptance_info<'a, I: TokInput<'a>>()
-> impl Parser<'a, I, AcceptanceInfo, Err<'a>> + Clone {
    identifier()
        .map(AcceptanceInfo::Identifier)
        .or(integer().map(AcceptanceInfo::Int))
}

fn ap_index<'a, I: TokInput<'a>>()
-> impl Parser<'a, I, AbstractLabelExpression, Err<'a>> + Clone {
    integer().try_map(|i, span| {
        u16::try_from(i)
            .map(AbstractLabelExpression::Integer)
            .map_err(|_| Rich::custom(span, "AP identifier out of range"))
    })
}

pub fn label_expression<'a, I: TokInput<'a>>()
-> impl Parser<'a, I, AbstractLabelExpression, Err<'a>> + Clone {
    recursive(|label_expression| {
        let value = boolean()
            .map(AbstractLabelExpression::Boolean)
            .or(ap_index())
            .or(alias_name().map(AbstractLabelExpression::Alias));

        let atom = value
            .or(label_expression.delimited_by(just(Token::Paren('(')), just(Token::Paren(')'))));

        let unary = just(Token::Op('!'))
            .or_not()
            .then(atom)
            .map(|(negated, expr)| {
                if negated.is_some() {
                    AbstractLabelExpression::Negated(Box::new(expr))
                } else {
                    expr
                }
            });

        let conjunction = unary
            .separated_by(just(Token::Op('&')))
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|mut acc| match acc.len() {
                1 => acc.remove(0),
                _ => AbstractLabelExpression::Conjunction(acc),
            });

        conjunction
            .separated_by(just(Token::Op('|')))
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|mut acc| match acc.len() {
                1 => acc.remove(0),
                _ => AbstractLabelExpression::Disjunction(acc),
            })
    })
}

pub fn acceptance_condition<'a, I: TokInput<'a>>()
-> impl Parser<'a, I, AcceptanceCondition, Err<'a>> + Clone {
    recursive(|acceptance_condition| {
        let atom_value = just(Token::Op('!'))
            .or_not()
            .then(integer())
            .map(|(negated, integer)| {
                if negated.is_none() {
                    AcceptanceAtom::Positive(integer)
                } else {
                    AcceptanceAtom::Negative(integer)
                }
            });

        let fin_inf_atom = just(Token::Fin)
            .to(AcceptanceCondition::Fin as fn(_) -> _)
            .or(just(Token::Inf).to(AcceptanceCondition::Inf as fn(_) -> _))
            .then(atom_value.delimited_by(just(Token::Paren('(')), just(Token::Paren(')'))))
            .map(|(f, atom)| f(atom));

        let atom = boolean()
            .map(HoaBool)
            .map(AcceptanceCondition::Boolean)
            .or(fin_inf_atom)
            .or(acceptance_condition
                .delimited_by(just(Token::Paren('(')), just(Token::Paren(')'))));

        let conjunction = atom.clone().foldl(
            just(Token::Op('&'))
                .to(AcceptanceCondition::And as fn(_, _) -> _)
                .then(atom)
                .repeated(),
            |lhs, (f, rhs)| f(Box::new(lhs), Box::new(rhs)),
        );

        conjunction.clone().foldl(
            just(Token::Op('|'))
                .to(AcceptanceCondition::Or as fn(_, _) -> _)
                .then(conjunction)
                .repeated(),
            |lhs, (f, rhs)| f(Box::new(lhs), Box::new(rhs)),
        )
    })
}
