use biodivine_lib_bdd::{Bdd, BddVariable, BddVariableSet};
use std::ops::Deref;

use crate::{AliasError, LabelError};

/// Newtype wrapper around a [`crate::LabelExpression`], implements [`Deref`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub LabelExpression);

impl Deref for Label {
    type Target = LabelExpression;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A label expression as written in the file, over atomic propositions given by index.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum AbstractLabelExpression {
    /// `t` or `f`.
    Boolean(bool),
    /// The atomic proposition with this index.
    Integer(u16),
    /// A reference to an alias (`@name`, stored without the `@`); see
    /// [`crate::HoaRepresentation::unalias`].
    Alias(String),
    /// Negation of an expression.
    Negated(Box<AbstractLabelExpression>),
    /// Conjunction (`&`), true when empty.
    Conjunction(Vec<AbstractLabelExpression>),
    /// Disjunction (`|`), false when empty.
    Disjunction(Vec<AbstractLabelExpression>),
}

impl AbstractLabelExpression {
    /// Converts this expression into a [`Bdd`], using the variables of `vs`.
    ///
    /// The variable for AP `i` is `BddVariable::from_index(i)`, so `vs` must have at
    /// least as many variables as the largest AP identifier used in this expression.
    pub fn try_into_bdd(&self, vs: &BddVariableSet) -> Result<Bdd, LabelError> {
        match self {
            AbstractLabelExpression::Boolean(b) => Ok(match b {
                true => vs.mk_true(),
                false => vs.mk_false(),
            }),
            AbstractLabelExpression::Integer(i) => {
                if *i < vs.num_vars() {
                    Ok(vs.mk_var(BddVariable::from_index(*i as usize)))
                } else {
                    Err(LabelError::ApOutOfRange {
                        ap: *i,
                        count: vs.num_vars(),
                    })
                }
            }
            AbstractLabelExpression::Alias(name) => Err(LabelError::UnresolvedAlias(name.clone())),
            AbstractLabelExpression::Negated(e) => Ok(e.try_into_bdd(vs)?.not()),
            AbstractLabelExpression::Conjunction(cs) => cs
                .iter()
                .try_fold(vs.mk_true(), |acc, c| Ok(acc.and(&c.try_into_bdd(vs)?))),
            AbstractLabelExpression::Disjunction(ds) => ds
                .iter()
                .try_fold(vs.mk_false(), |acc, d| Ok(acc.or(&d.try_into_bdd(vs)?))),
        }
    }

    /// The highest AP index used in this expression, if any.
    pub(crate) fn max_ap(&self) -> Option<u16> {
        match self {
            Self::Integer(i) => Some(*i),
            Self::Boolean(_) | Self::Alias(_) => None,
            Self::Negated(e) => e.max_ap(),
            Self::Conjunction(es) | Self::Disjunction(es) => {
                es.iter().filter_map(|e| e.max_ap()).max()
            }
        }
    }

    /// Replaces every alias reference by its (recursively substituted) definition.
    /// `stack` holds the aliases currently being expanded and is used to detect cycles.
    pub(crate) fn substitute(
        &self,
        defs: &std::collections::HashMap<String, AbstractLabelExpression>,
        stack: &mut Vec<String>,
    ) -> Result<Self, AliasError> {
        let many = |es: &[Self], stack: &mut Vec<String>| {
            es.iter()
                .map(|e| e.substitute(defs, stack))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match self {
            Self::Alias(name) => {
                if stack.contains(name) {
                    return Err(AliasError::Cyclic(name.clone()));
                }
                let def = defs.get(name).ok_or(AliasError::Undefined(name.clone()))?;
                stack.push(name.clone());
                let out = def.substitute(defs, stack);
                stack.pop();
                out?
            }
            Self::Negated(e) => Self::Negated(Box::new(e.substitute(defs, stack)?)),
            Self::Conjunction(es) => Self::Conjunction(many(es, stack)?),
            Self::Disjunction(es) => Self::Disjunction(many(es, stack)?),
            Self::Boolean(_) | Self::Integer(_) => self.clone(),
        })
    }

    /// Converts this expression into a [`Bdd`] over `num_aps` anonymous variables.
    pub fn try_into_prop(&self, num_aps: u16) -> Result<HoaExpression, LabelError> {
        let var_set = BddVariableSet::new_anonymous(num_aps);
        self.try_into_bdd(&var_set)
    }
}

/// The number of variables the `AnonymousAbstract` helpers build their BDDs over.
const ANONYMOUS_APS: u8 = 6;

/// Only used by test helpers in this crate, not meaningful for users.
#[deprecated(since = "0.3.0", note = "an internal constant, define your own limit")]
pub const MAX_APS: u8 = ANONYMOUS_APS;

/// Typedef for an expression used by a [`crate::HoaRepresentation`].
pub type HoaExpression = Bdd;

/// The expression of a [`Label`]: either as parsed or already converted to a BDD.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum LabelExpression {
    /// A BDD over the atomic propositions.
    Expression(HoaExpression),
    /// An expression as parsed.
    Abstract(AbstractLabelExpression),
}

impl LabelExpression {
    /// Converts the expression into a BDD over `num_aps` variables.
    pub fn try_into_hoa_expression(self, num_aps: u16) -> Result<HoaExpression, LabelError> {
        match self {
            LabelExpression::Expression(b) => Ok(b),
            LabelExpression::Abstract(b) => b.try_into_prop(num_aps),
        }
    }
}

#[cfg(test)]
#[allow(unused)]
pub(crate) struct Anonymous<const PARSED: bool = false>;
#[cfg(test)]
#[allow(unused)]
pub(crate) type AnonymousParsed = Anonymous<true>;
#[cfg(test)]
#[allow(unused)]
pub(crate) type AnonymousAbstract = Anonymous<false>;

#[cfg(test)]
#[allow(unused)]
use AbstractLabelExpression::*;

#[cfg(test)]
#[allow(unused)]
#[allow(dead_code)]
impl Anonymous<false> {
    pub fn var_expr(n: u16) -> LabelExpression {
        assert!(n < ANONYMOUS_APS as u16);
        LabelExpression::Abstract(AbstractLabelExpression::Integer(n))
    }
    pub fn not_var_expr(n: u16) -> LabelExpression {
        assert!(n < ANONYMOUS_APS as u16);
        LabelExpression::Abstract(Negated(Box::new(Integer(n))))
    }
    pub fn top_label() -> Label {
        Label(LabelExpression::Abstract(AbstractLabelExpression::Boolean(
            true,
        )))
    }
    pub fn var_label(n: u16) -> Label {
        assert!(n < ANONYMOUS_APS as u16);
        Label(Self::var_expr(n))
    }
    pub fn not_var_label(n: u16) -> Label {
        assert!(n < ANONYMOUS_APS as u16);
        Label(Self::not_var_expr(n))
    }
}

#[cfg(test)]
#[allow(unused)]
#[allow(dead_code)]
impl Anonymous<true> {
    pub fn var_expr(n: usize) -> LabelExpression {
        assert!(n < ANONYMOUS_APS as usize);
        LabelExpression::Expression(Self::var(n))
    }
    pub fn var_label(n: usize) -> Label {
        assert!(n < ANONYMOUS_APS as usize);
        Label(Self::var_expr(n))
    }
    pub fn var(n: usize) -> Bdd {
        assert!(n < ANONYMOUS_APS as usize);
        BddVariableSet::new_anonymous(ANONYMOUS_APS as u16).mk_var(BddVariable::from_index(n))
    }
    pub fn not_var(n: usize) -> Bdd {
        assert!(n < ANONYMOUS_APS as usize);
        BddVariableSet::new_anonymous(ANONYMOUS_APS as u16).mk_not_var(BddVariable::from_index(n))
    }
    pub fn top() -> Bdd {
        BddVariableSet::new_anonymous(ANONYMOUS_APS as u16).mk_true()
    }
    pub fn top_expr() -> LabelExpression {
        LabelExpression::Expression(Self::top())
    }
    pub fn top_label() -> Label {
        Label(Self::top_expr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(expr: &AbstractLabelExpression, num_aps: u16) -> f64 {
        expr.try_into_prop(num_aps)
            .expect("label should convert")
            .cardinality()
    }

    fn int(i: u16) -> Box<AbstractLabelExpression> {
        Box::new(Integer(i))
    }

    fn not(e: Box<AbstractLabelExpression>) -> AbstractLabelExpression {
        Negated(e)
    }

    #[test]
    fn composite_expressions_are_converted() {
        // 0&!1 | !0&1
        let xor = Disjunction(vec![
            Conjunction(vec![Integer(0), not(int(1))]),
            Conjunction(vec![not(int(0)), Integer(1)]),
        ]);
        assert_eq!(2.0, card(&xor, 2));

        // (0|1)&!1 == 0&!1
        let expr = Conjunction(vec![Disjunction(vec![Integer(0), Integer(1)]), not(int(1))]);
        assert_eq!(1.0, card(&expr, 2));

        // 0&t == 0
        let expr = Conjunction(vec![Integer(0), Boolean(true)]);
        assert_eq!(2.0, card(&expr, 2));

        // 0&!0 == f
        let expr = Conjunction(vec![Integer(0), not(int(0))]);
        assert_eq!(0.0, card(&expr, 2));

        assert_eq!(4.0, card(&Boolean(true), 2));
        assert_eq!(0.0, card(&Boolean(false), 2));
    }

    #[test]
    fn out_of_bounds_ap_is_an_error() {
        let expr = Conjunction(vec![Integer(0), not(int(7))]);
        let expected = LabelError::ApOutOfRange { ap: 7, count: 2 };
        assert_eq!(expr.try_into_prop(2), Err(expected.clone()));
        assert_eq!(Integer(7).try_into_prop(2), Err(expected.clone()));
        assert_eq!(
            LabelExpression::Abstract(Integer(7)).try_into_hoa_expression(2),
            Err(expected)
        );
    }

    #[test]
    fn unresolved_alias_is_an_error() {
        let expr = AbstractLabelExpression::Alias("x".into());
        assert_eq!(
            expr.try_into_prop(2),
            Err(LabelError::UnresolvedAlias("x".into()))
        );
        assert_eq!(
            expr.try_into_prop(2).unwrap_err().to_string(),
            "alias @x is not resolved, call `HoaRepresentation::unalias` first"
        );
    }
}
