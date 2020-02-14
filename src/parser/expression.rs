use crate::lexer::Token;
use std::convert::TryFrom;
use std::fmt::{Debug, Error, Formatter};
use BooleanAtom::*;
use BooleanExpression::*;

type StartStates = Vec<usize>;

#[derive(Debug)]
pub enum BooleanAtom<'a> {
    BooleanValue(bool),
    IntegerValue(usize),
    AliasName(&'a str),
}

#[derive(Debug)]
pub enum BooleanExpression<'a> {
    Atom(BooleanAtom<'a>),
    Negation(Box<BooleanExpression<'a>>),
    Conjunction(Box<BooleanExpression<'a>>, Box<BooleanExpression<'a>>),
    Disjunction(Box<BooleanExpression<'a>>, Box<BooleanExpression<'a>>),
}

#[derive(Debug)]
pub enum AcceptanceIdent {
    Fin(usize),
    FinNeg(usize),
    Inf(usize),
    InfNeg(usize),
}

#[derive(Debug)]
pub enum AcceptanceCondition {
    Atom(AcceptanceIdent),
    Conjunction(Box<AcceptanceCondition>, Box<AcceptanceCondition>),
    Disjunction(Box<AcceptanceCondition>, Box<AcceptanceCondition>),
    BooleanValue(bool),
}

impl<'a> From<BooleanAtom<'a>> for BooleanExpression<'a> {
    fn from(atom: BooleanAtom<'a>) -> Self {
        Atom(atom)
    }
}

impl<'a> BooleanAtom<'a> {
    pub fn btrue() -> BooleanAtom<'a> {
        BooleanValue(true)
    }

    pub fn bfalse() -> BooleanAtom<'a> {
        BooleanValue(false)
    }

    pub fn bint(int: usize) -> BooleanAtom<'a> {
        IntegerValue(int)
    }

    pub fn balias(name: &'a str) -> BooleanAtom<'a> {
        AliasName(name)
    }
}

impl<'a> BooleanExpression<'a> {
    pub fn and(self, other: BooleanExpression<'a>) -> BooleanExpression<'a> {
        Conjunction(Box::new(self), Box::new(other))
    }

    pub fn or(self, other: BooleanExpression<'a>) -> BooleanExpression<'a> {
        Disjunction(Box::new(self), Box::new(other))
    }

    pub fn not(self) -> BooleanExpression<'a> {
        Negation(Box::new(self))
    }
}

impl std::fmt::Display for AcceptanceIdent {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            AcceptanceIdent::Fin(int) => write!(f, "Fin({})", int),
            AcceptanceIdent::FinNeg(int) => write!(f, "Fin(!{})", int),
            AcceptanceIdent::Inf(int) => write!(f, "Inf({})", int),
            AcceptanceIdent::InfNeg(int) => write!(f, "Inf(!{})", int),
        }
    }
}

impl std::fmt::Display for AcceptanceCondition {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            AcceptanceCondition::Atom(atom) => write!(f, "{}", atom),
            AcceptanceCondition::Conjunction(left, right) => write!(f, "({} & {})", left, right),
            AcceptanceCondition::Disjunction(left, right) => write!(f, "({} | {})", left, right),
            AcceptanceCondition::BooleanValue(b) => write!(
                f,
                "{}",
                match b {
                    true => "t",
                    false => "f",
                }
            ),
        }
    }
}

impl<'a> std::fmt::Display for BooleanAtom<'a> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            BooleanValue(b) => match b {
                true => write!(f, "t"),
                false => write!(f, "f"),
            },
            IntegerValue(int) => write!(f, "{}", int),
            AliasName(name) => write!(f, "@{}", name),
        }
    }
}

impl<'a> std::fmt::Display for BooleanExpression<'a> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Atom(atom) => write!(f, "{}", atom),
            Negation(subexpr) => write!(f, "!{}", *subexpr),
            Conjunction(left, right) => write!(f, "({} & {})", *left, *right),
            Disjunction(left, right) => write!(f, "({} | {})", *left, *right),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_test() {
        let a: BooleanExpression = BooleanAtom::balias("asdf").into();
        let t: BooleanExpression = BooleanAtom::btrue().into();

        let be = a.not().or(t.not());
        println!("{}", be);
    }

    #[test]
    fn expression_methods_construction_test() {
        let t: BooleanExpression = BooleanAtom::btrue().into();
        let f: BooleanExpression = BooleanAtom::bfalse().into();

        let be = t.not().and(f);
        println!("{}", be);
    }

    #[test]
    fn expression_construction_test() {
        let be = Conjunction(
            Box::new(Atom(BooleanValue(true))),
            Box::new(Atom(BooleanValue(false))),
        );
        println!("{}", be);
    }
}