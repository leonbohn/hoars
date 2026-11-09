use std::ops::Deref;

use crate::AliasError;

/// Newtype wrapper around an [`AbstractLabelExpression`], implements [`Deref`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub AbstractLabelExpression);

impl Deref for Label {
    type Target = AbstractLabelExpression;

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
    /// The highest AP index used in this expression, if any. Aliases are not followed, so call
    /// [`crate::HoaRepresentation::unalias`] first to also account for them.
    pub fn max_ap(&self) -> Option<u16> {
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
}

/// Only used by test helpers in this crate, not meaningful for users.
#[deprecated(since = "0.3.0", note = "an internal constant, define your own limit")]
pub const MAX_APS: u8 = 6;

/// The truth table of `expr` over `num_aps` propositions: entry `v` is its value when AP `i` is
/// true iff bit `i` of `v` is set. Panics on aliases.
#[cfg(test)]
pub(crate) fn truth_table(expr: &AbstractLabelExpression, num_aps: u16) -> Vec<bool> {
    fn eval(e: &AbstractLabelExpression, v: usize) -> bool {
        match e {
            AbstractLabelExpression::Boolean(b) => *b,
            AbstractLabelExpression::Integer(i) => v >> i & 1 == 1,
            AbstractLabelExpression::Alias(a) => panic!("unresolved alias @{a}"),
            AbstractLabelExpression::Negated(e) => !eval(e, v),
            AbstractLabelExpression::Conjunction(es) => es.iter().all(|e| eval(e, v)),
            AbstractLabelExpression::Disjunction(es) => es.iter().any(|e| eval(e, v)),
        }
    }
    (0..1usize << num_aps).map(|v| eval(expr, v)).collect()
}

#[cfg(test)]
pub(crate) struct Anonymous;
#[cfg(test)]
pub(crate) type AnonymousAbstract = Anonymous;

#[cfg(test)]
#[allow(dead_code)]
impl Anonymous {
    pub fn top_label() -> Label {
        Label(AbstractLabelExpression::Boolean(true))
    }
    pub fn var_label(n: u16) -> Label {
        Label(AbstractLabelExpression::Integer(n))
    }
    pub fn not_var_label(n: u16) -> Label {
        Label(AbstractLabelExpression::Negated(Box::new(
            AbstractLabelExpression::Integer(n),
        )))
    }
}
