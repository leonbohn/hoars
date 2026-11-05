use std::{
    borrow::Borrow,
    fmt::Display,
    ops::{Deref, Rem},
};

use crate::Id;

/// Represents a conjunction over states of a HOA automaton, this
/// is mostly used as the initial state of the automaton.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct StateConjunction(pub(crate) Vec<crate::Id>);

impl StateConjunction {
    /// Attempts to get the singleton element of the state conjunction, if it exists.
    /// Returns `None` if the state conjunction is not a singleton.
    /// This is useful when dealing with non-alternating automata.
    pub fn get_singleton(&self) -> Option<Id> {
        if self.0.len() == 1 {
            Some(self.0[0])
        } else {
            None
        }
    }

    /// Creates a state conjunction containing a single id.
    pub fn singleton(id: Id) -> Self {
        Self(vec![id])
    }
}

/// An atomic proposition is named by a string.
pub type AtomicProposition = String;

/// Aliases are also named by a string.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct AliasName(pub(crate) String);

/// An acceptance atom can be used to build an acceptance condition,
/// each atom is either a positive or a negative acceptance set
/// identifier.
#[derive(Debug, PartialEq, Eq, Clone)]
#[allow(missing_docs)]
pub enum AcceptanceAtom {
    Positive(Id),
    Negative(Id),
}

/// An acceptance signature is a vector of acceptance set
/// identifiers, it is associated with an edge.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AcceptanceSignature(pub(crate) Vec<crate::Id>);

impl AcceptanceSignature {
    /// Tries to get the singleton element of the acceptance signature, if it exists.
    /// Returns `None` if the acceptance signature is not a singleton.
    pub fn get_singleton(&self) -> Option<Option<Id>> {
        if self.is_empty() {
            Some(None)
        } else if self.len() == 1 {
            Some(Some(self[0]))
        } else {
            None
        }
    }

    /// Creates an acceptance signature containing a single id.
    pub fn from_singleton(singleton: Id) -> Self {
        Self(vec![singleton])
    }

    /// Creates an empty acceptance signature.
    pub fn empty() -> Self {
        Self(vec![])
    }
}

impl Deref for AcceptanceSignature {
    type Target = Vec<crate::Id>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Represents a boolean value in the HOA format.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Ord, PartialOrd)]
pub struct HoaBool(pub bool);

/// An acceptance condition is a positive boolean expression over
/// [`AcceptanceAtom`]s.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AcceptanceCondition {
    /// Represents that the given atom should appear finitely often.
    Fin(AcceptanceAtom),
    /// The given atom should appear infinitely often.
    Inf(AcceptanceAtom),
    /// Represents a conjunction of two acceptance conditions.
    And(Box<AcceptanceCondition>, Box<AcceptanceCondition>),
    /// Represents a disjunction of two acceptance conditions.
    Or(Box<AcceptanceCondition>, Box<AcceptanceCondition>),
    /// A constant boolean value.
    Boolean(HoaBool),
}

impl AcceptanceCondition {
    /// All acceptance sets mentioned in this condition.
    pub(crate) fn sets(&self) -> Vec<Id> {
        match self {
            Self::Fin(a) | Self::Inf(a) => match a {
                AcceptanceAtom::Positive(i) | AcceptanceAtom::Negative(i) => vec![*i],
            },
            Self::And(l, r) | Self::Or(l, r) => {
                let mut sets = l.sets();
                sets.extend(r.sets());
                sets
            }
            Self::Boolean(_) => Vec::new(),
        }
    }

    fn parity_rec(current: u32, total: u32) -> Self {
        if current + 1 >= total {
            if current.rem(2) == 0 {
                AcceptanceCondition::id_inf(current)
            } else {
                AcceptanceCondition::id_fin(current)
            }
        } else if current.rem(2) == 0 {
            AcceptanceCondition::Or(
                Box::new(AcceptanceCondition::Inf(AcceptanceAtom::Positive(current))),
                Box::new(Self::parity_rec(current + 1, total)),
            )
        } else {
            AcceptanceCondition::And(
                Box::new(AcceptanceCondition::Fin(AcceptanceAtom::Positive(current))),
                Box::new(Self::parity_rec(current + 1, total)),
            )
        }
    }

    /// Creates a parity acceptance condition with the given number of priorities.
    pub fn parity(priorities: u32) -> Self {
        Self::parity_rec(0, priorities)
    }

    /// Creates a Buchi acceptance condition.
    pub fn buchi() -> Self {
        AcceptanceCondition::Inf(AcceptanceAtom::Positive(0))
    }

    /// Creates a conjunction of two acceptance conditions.
    pub fn and<C: Borrow<AcceptanceCondition>>(&self, other: C) -> Self {
        AcceptanceCondition::And(Box::new(self.clone()), Box::new(other.borrow().clone()))
    }

    /// Creates a disjunction of two acceptance conditions.
    pub fn or<C: Borrow<AcceptanceCondition>>(&self, other: C) -> Self {
        AcceptanceCondition::Or(Box::new(self.clone()), Box::new(other.borrow().clone()))
    }

    /// Creates an acceptance condition containing the given atom.
    pub fn atom<A: Borrow<AcceptanceAtom>>(atom: A) -> Self {
        AcceptanceCondition::Fin(atom.borrow().clone())
    }

    /// Creates an acceptance condition consisting of a positive atodm.
    pub fn id_fin(id: Id) -> Self {
        Self::Fin(AcceptanceAtom::Positive(id))
    }

    /// Creates an acceptance condition consisting of a negative atom.
    pub fn id_inf(id: Id) -> Self {
        Self::Inf(AcceptanceAtom::Positive(id))
    }
}

/// Represents the name of a type of acceptance condition.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AcceptanceName {
    /// Büchi acceptance, implies that there is only one
    /// acceptance set. Is satisfied if an edge/state of the
    /// acceptance set appears infinitely often.
    Buchi,
    /// Generalized Büchi consists of multiple Büchi acceptance
    /// conditions. It is satisfied if all of the Büchi conditions
    /// are satsified.
    GeneralizedBuchi,
    /// Co-Büchi conditions are dual to Büchi conditions, they are
    /// satisfied if no edge/state from the acceptance set appears
    /// infinitely often.
    CoBuchi,
    /// Generalized co-Büchi conditions are dual to generalized
    /// Büchi conditions, see [`AcceptanceName::GeneralizedBuchi`].
    GeneralizedCoBuchi,
    /// A Streett condition consists of a set of pairs of acceptance
    /// sets. It is dual to a [`AcceptanceName::Rabin`] condition
    /// and satisfied if for each pair (X, Y) in the condition holds
    /// that if X appears infinitely often, then Y also appears
    /// infinitely often.
    Streett,
    /// A Rabin condition is dual to a [`AcceptanceName::Streett`]
    /// condition, it also consists of a set of pairs (X,Y) of
    /// acceptance sets. It is satisfied if there exists a pair
    /// (X, Y) in the condition such that no set from X appears
    /// infinitely often and one set from Y appears infinitely
    /// often.
    Rabin,
    /// A generalized Rabin condition is a set of [`AcceptanceName::Rabin`]
    /// conditions and it is satisfied of all of the Rabin conditions
    /// are satisfied.
    GeneralizedRabin,
    /// A parity (or Mostowski) condition associates with each
    /// state/transition a priority, i.e. a non-negative integer.
    /// It is satisfied if the least priority that appears infinitely
    /// often is even. There is also max even, min odd and max odd
    /// variants of this condition, but they are all equivalent
    /// in terms of expressiveness.
    Parity,
    /// Represents a condition where everything is accepted.
    All,
    /// A condition where nothing is accepted.
    None,
    /// Any other name (e.g. Spot's `Rabin-like`), kept verbatim.
    Other(String),
}

impl AcceptanceName {
    pub fn is_parity(&self) -> bool {
        matches!(self, AcceptanceName::Parity)
    }
}

impl From<String> for AcceptanceName {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Buchi" => AcceptanceName::Buchi,
            "generalized-Buchi" => AcceptanceName::GeneralizedBuchi,
            "co-Buchi" => AcceptanceName::CoBuchi,
            "generalized-co-Buchi" => AcceptanceName::GeneralizedCoBuchi,
            "Streett" => AcceptanceName::Streett,
            "Rabin" => AcceptanceName::Rabin,
            "generalized-Rabin" => AcceptanceName::GeneralizedRabin,
            "parity" => AcceptanceName::Parity,
            "all" => AcceptanceName::All,
            "none" => AcceptanceName::None,
            _ => AcceptanceName::Other(value),
        }
    }
}

/// Represents properties of an automaton. For more information
/// see the documentation of the [HOA format](https://adl.github.io/hoaf/#properties).
#[derive(Debug, PartialEq, Eq, Clone)]
#[allow(missing_docs)]
pub enum Property {
    StateLabels,
    TransLabels,
    ImplicitLabels,
    ExplicitLabels,
    StateAcceptance,
    TransitionAcceptance,
    UniversalBranching,
    NoUniversalBranching,
    Deterministic,
    Complete,
    Unambiguous,
    StutterInvariant,
    Weak,
    VeryWeak,
    InherentlyWeak,
    Terminal,
    Tight,
    Colored,
    /// Any other property (e.g. Spot's `semi-deterministic`), kept verbatim.
    Other(String),
}

impl From<String> for Property {
    fn from(value: String) -> Self {
        match value.as_str() {
            "state-labels" => Property::StateLabels,
            "trans-labels" => Property::TransLabels,
            "implicit-labels" => Property::ImplicitLabels,
            "explicit-labels" => Property::ExplicitLabels,
            "state-acc" => Property::StateAcceptance,
            "trans-acc" => Property::TransitionAcceptance,
            "univ-branch" => Property::UniversalBranching,
            "no-univ-branch" => Property::NoUniversalBranching,
            "deterministic" => Property::Deterministic,
            "complete" => Property::Complete,
            "unambiguous" => Property::Unambiguous,
            "stutter-invariant" => Property::StutterInvariant,
            "weak" => Property::Weak,
            "very-weak" => Property::VeryWeak,
            "inherently-weak" => Property::InherentlyWeak,
            "terminal" => Property::Terminal,
            "tight" => Property::Tight,
            "colored" => Property::Colored,
            _ => Property::Other(value),
        }
    }
}

/// Used to give additional (human-readable) and optional
/// information about the acceptance condition, more information
/// can be obtained in the [HOA docs](https://adl.github.io/hoaf/#acc-name).
#[derive(Debug, PartialEq, Eq, Clone)]
#[allow(missing_docs)]
pub enum AcceptanceInfo {
    Int(crate::Id),
    Identifier(String),
}

impl AcceptanceInfo {
    /// Creates an [`AcceptanceInfo`] from a [`Display`]able.
    pub fn identifier<D: Display>(id: D) -> Self {
        AcceptanceInfo::Identifier(id.to_string())
    }

    /// Creates an [`AcceptanceInfo`] from an [`Id`].
    pub fn integer(id: Id) -> Self {
        AcceptanceInfo::Int(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::AcceptanceCondition;

    #[test]
    fn terminal_property_is_parsed() {
        // the spelling "terminal" is the one that the writer produces
        assert_eq!(
            super::Property::from("terminal".to_string()),
            super::Property::Terminal
        );
    }

    #[test]
    fn parity_acceptance_creator() {
        let parity_condition = super::AcceptanceCondition::parity(3);
        assert_eq!(
            parity_condition,
            AcceptanceCondition::id_inf(0)
                .or(AcceptanceCondition::id_fin(1).and(AcceptanceCondition::id_inf(2)))
        );
    }
}
