use std::ops::{Deref, DerefMut};

use chumsky::prelude::*;

use crate::{
    AbstractLabelExpression, AcceptanceSignature, AtomicProposition, Id, Label, LabelExpression,
    StateConjunction,
    value::{self, HoaErr, sym, tok},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(unused)]
pub struct Alphabet(pub Vec<AtomicProposition>);

/// A state as parsed, before implicit labels and state labels are resolved.
#[derive(Clone, Debug)]
pub(crate) struct RawState {
    label: Option<Label>,
    id: Id,
    comment: Option<String>,
    acc: Option<AcceptanceSignature>,
    edges: Vec<RawEdge>,
}

/// An edge as parsed; the label is missing for implicit-label edges and state-label states.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RawEdge(Option<Label>, StateConjunction, Option<AcceptanceSignature>);

/// Represents an edge in a HOA automaton. It contains the [`crate::LabelExpression`], the
/// [`StateConjunction`] and the [`AcceptanceSignature`] of the edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge(
    pub(crate) Label,
    pub(crate) StateConjunction,
    pub(crate) AcceptanceSignature,
);

impl Edge {
    /// Returns the label of the edge.
    pub fn label(&self) -> &Label {
        &self.0
    }

    /// Gives mutable access to the label of the edge.
    pub fn label_mut(&mut self) -> &mut Label {
        &mut self.0
    }

    /// Returns the state conjunction of the edge.
    pub fn state_conjunction(&self) -> &StateConjunction {
        &self.1
    }

    /// Returns the acceptance signature of the edge.
    pub fn acceptance_signature(&self) -> &AcceptanceSignature {
        &self.2
    }

    /// Tries to get the target (singular) of the transition. Returns `None` if the
    /// transition does not have a singular target.
    pub fn target(&self) -> Option<Id> {
        self.1.get_singleton()
    }

    /// Builds an edge from its parts.
    pub fn from_parts(
        label_expression: Label,
        state_conjunction: StateConjunction,
        acceptance_signature: AcceptanceSignature,
    ) -> Self {
        Self(label_expression, state_conjunction, acceptance_signature)
    }
}

/// Represents a state in a HOA automaton. It contains the [`Id`] of the state, an optional
/// comment and a list of outgoing edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State(
    pub(crate) Id,
    pub(crate) Option<String>,
    pub(crate) Vec<Edge>,
);

impl State {
    /// Constructs a new state from its parts.
    pub fn from_parts(id: Id, comment: Option<String>, edges: Vec<Edge>) -> Self {
        Self(id, comment, edges)
    }

    /// Extracts the id of the state.
    pub fn id(&self) -> Id {
        self.0
    }

    /// Extracts the comment of the state, if present.
    pub fn comment(&self) -> Option<&str> {
        self.1.as_deref()
    }

    /// Extracts the edges of the state.
    pub fn edges(&self) -> &[Edge] {
        &self.2
    }

    /// Gives mutable access to the edges of this state.
    pub fn edges_mut(&mut self) -> &mut [Edge] {
        &mut self.2
    }
}

impl RawState {
    /// Resolves implicit labels and the state label into explicit edge labels.
    /// `num_aps` is the number of atomic propositions declared in the header.
    ///
    /// - A labelled state must have unlabelled edges, each of which gets the state label.
    /// - An unlabelled state with unlabelled edges must have exactly `2^num_aps` edges;
    ///   edge `i` gets the label in which AP `j` is true iff bit `j` of `i` is set.
    /// - Mixing labelled and unlabelled edges is an error.
    pub(crate) fn resolve(self, num_aps: u16) -> Result<State, String> {
        let RawState {
            label: state_label,
            id,
            comment,
            acc: state_acc,
            edges,
        } = self;
        let labelled = edges.iter().filter(|e| e.0.is_some()).count();
        if state_label.is_some() && labelled > 0 {
            return Err(format!(
                "State {id} has a state label, so its edges must not have labels"
            ));
        }
        if labelled > 0 && labelled < edges.len() {
            return Err(format!("State {id} mixes labelled and unlabelled edges"));
        }
        let implicit = state_label.is_none() && labelled == 0 && !edges.is_empty();
        if implicit && (num_aps >= 32 || edges.len() != 1usize << num_aps) {
            return Err(format!(
                "State {id} has implicit edge labels, so it needs exactly 2^{num_aps} edges but has {}",
                edges.len()
            ));
        }

        let out_edges = edges
            .into_iter()
            .enumerate()
            .map(|(i, RawEdge(label, conj, acc))| {
                let label = match (label, &state_label) {
                    (Some(l), _) => l,
                    (None, Some(l)) => l.clone(),
                    (None, None) => implicit_label(i, num_aps),
                };
                let acc = match (&state_acc, acc) {
                    (None, None) => AcceptanceSignature(Vec::new()),
                    (Some(acc), None) => acc.clone(),
                    (None, Some(acc)) => acc,
                    (Some(left), Some(right)) => AcceptanceSignature(
                        left.iter().cloned().chain(right.iter().cloned()).collect(),
                    ),
                };
                Edge(label, conj, acc)
            })
            .collect();
        Ok(State(id, comment, out_edges))
    }
}

/// The label of the `i`-th implicit edge: AP `j` is positive iff bit `j` of `i` is set.
fn implicit_label(i: usize, num_aps: u16) -> Label {
    let mut literals: Vec<_> = (0..num_aps)
        .map(|j| {
            let ap = AbstractLabelExpression::Integer(j);
            if i >> j & 1 == 1 {
                ap
            } else {
                AbstractLabelExpression::Negated(Box::new(ap))
            }
        })
        .collect();
    Label(LabelExpression::Abstract(match literals.len() {
        0 => AbstractLabelExpression::Boolean(true),
        1 => literals.remove(0),
        _ => AbstractLabelExpression::Conjunction(literals),
    }))
}

pub fn label<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, Label, extra::Err<X>> + Clone {
    sym('[')
        .ignore_then(value::label_expression())
        .then_ignore(sym(']'))
        .map(LabelExpression::Abstract)
        .map(Label)
}

fn edge<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, RawEdge, extra::Err<X>> {
    label()
        .or_not()
        .then(value::state_conjunction())
        .then(value::acceptance_signature().or_not())
        .map(|((label, state_conjunction), acceptance_signature)| {
            RawEdge(label, state_conjunction, acceptance_signature)
        })
}

pub(crate) fn state<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, RawState, extra::Err<X>> {
    tok("State:")
        .ignore_then(
            label()
                .or_not()
                .then(value::integer())
                .then(value::text().or_not())
                .then(value::acceptance_signature().or_not()),
        )
        .then(edge().repeated().collect::<Vec<_>>())
        .map(|((((label, id), comment), acc), edges)| RawState {
            label,
            id,
            comment,
            acc,
            edges,
        })
}

/// Represents the body of a HOA automaton. In essence, this is just a vector of [`State`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Body(Vec<State>);

impl<'a> IntoIterator for &'a Body {
    type Item = &'a State;

    type IntoIter = std::slice::Iter<'a, State>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl Body {
    /// Constructs a new empty body.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn parser<'a, X: HoaErr<'a>>()
    -> impl Parser<'a, &'a str, Vec<RawState>, extra::Err<X>> {
        tok("--BODY--")
            .ignore_then(state().repeated().collect())
            .then_ignore(tok("--END--"))
    }
}

impl From<Vec<State>> for Body {
    fn from(value: Vec<State>) -> Self {
        Body(value)
    }
}

impl Deref for Body {
    type Target = Vec<State>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Body {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use chumsky::{Parser, prelude::end};

    use crate::{AbstractLabelExpression, Edge, Label, StateConjunction, label::AnonymousAbstract};

    use super::State;

    pub fn in_tags(input: &str) -> String {
        format!("--BODY--\n{}\n--END--", input)
    }

    #[cfg(test)]
    pub fn process_body(input: &str) -> Result<Vec<State>, ()> {
        use crate::{body::Body, print_error_report};

        let ast = crate::value::ws()
            .ignore_then(Body::parser::<chumsky::error::Rich<char, crate::value::Span>>())
            .then_ignore(end())
            .parse(input)
            .into_result()
            .map_err(|errors| print_error_report(input, errors))?;
        // the tests only use explicit labels, so the AP count does not matter
        ast.into_iter()
            .map(|s| s.resolve(0).map_err(|_| ()))
            .collect()
    }

    #[test]
    fn two_explicit_transitions_with_comment() {
        let hoa = r#"State: 0 "a U b"   /* An example of named state */
        [0 & !1] 0 {0}
        [1] 1 {0}"#;
        let t0 = Edge::from_parts(
            Label(crate::LabelExpression::Abstract(
                AbstractLabelExpression::Conjunction(vec![
                    AbstractLabelExpression::Integer(0),
                    AbstractLabelExpression::Negated(Box::new(AbstractLabelExpression::Integer(1))),
                ]),
            )),
            StateConjunction(vec![0]),
            crate::AcceptanceSignature(vec![0]),
        );
        let t1 = Edge::from_parts(
            AnonymousAbstract::var_label(1),
            StateConjunction(vec![1]),
            crate::AcceptanceSignature(vec![0]),
        );
        let q0 = State::from_parts(0, Some("a U b".to_string()), vec![t0, t1]);
        assert_eq!(process_body(&in_tags(hoa)), Ok(vec![q0]));
    }

    #[test]
    fn one_transition_label_true() {
        let hoa = r#"
            State: 1
            [t] 1 {1}
        "#;
        let t0 = Edge::from_parts(
            AnonymousAbstract::top_label(),
            StateConjunction(vec![1]),
            crate::AcceptanceSignature(vec![1]),
        );
        let q0 = State::from_parts(1, None, vec![t0]);
        assert_eq!(process_body(&in_tags(hoa)), Ok(vec![q0]));
    }

    #[test]
    fn no_transition_state() {
        let hoa = r#"State: 1"#;
        let q0 = State::from_parts(1, None, vec![]);
        assert_eq!(process_body(&in_tags(hoa)), Ok(vec![q0]));
    }
}
