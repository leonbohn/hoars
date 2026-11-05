//! This crate provides a parser for the HOA format.
// #![warn(missing_docs)]
mod body;
mod format;
mod header;
pub mod input;
mod lexer;
pub mod output;
mod value;

pub mod label;
pub use label::{
    AbstractLabelExpression, HoaAlphabet, HoaExpression, HoaRepr, HoaSymbol, Label,
    LabelExpression, MAX_APS,
};

use tracing::warn;

use std::fmt::{Debug, Display};

use ariadne::{Color, Fmt, ReportKind, Source};

#[allow(unused_imports)]
use chumsky::prelude::*;
pub use format::*;

use chumsky::{Parser, prelude::Simple};
pub use format::{
    AcceptanceCondition, AcceptanceInfo, AcceptanceName, AcceptanceSignature, AliasName, Property,
};

pub use body::{Body, Edge, State};
pub use header::{Header, HeaderItem, UnknownValue};

use itertools::Itertools;
use lexer::Token;

/// The type of identifier used for states.
pub type Id = u32;

/// Represents the different types of error that can be encountered when parsing a [`HoaRepresentation`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FromHoaError {
    /// The version string does not match, we only support v1.
    UnsupportedVersion(String),
    /// Encapsulates that an unsupported acceptance condition was used.
    UnsupportedAcceptanceCondition,
    /// An error occurred when parsing the acceptance condition.
    ParseAcceptanceCondition(String),
    /// There was an error in the body.
    UnsupportedBody,
    /// Lexer encountered an error, contains detailed report.
    LexerError(String),
    /// Parser encountered an error, contains detailed report.
    ParserError(String),
    /// Abort token was encountered.
    Abort,
}

impl Display for FromHoaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FromHoaError::UnsupportedVersion(version) => {
                write!(f, "Unsupported HOA version ({})", version)
            }
            FromHoaError::UnsupportedAcceptanceCondition => {
                write!(f, "Unsupported acceptance condition")
            }
            FromHoaError::UnsupportedBody => write!(f, "Unsupported body"),
            FromHoaError::ParseAcceptanceCondition(message) => {
                write!(f, "Could not parse acceptance condition: {}", message)
            }
            FromHoaError::Abort => write!(f, "Abort token encountered"),
            FromHoaError::LexerError(rep) => write!(f, "Lexer error: {}", rep),
            FromHoaError::ParserError(rep) => write!(f, "Parser error: {}", rep),
        }
    }
}

/// Represents a parsed HOA automaton. It consists of a the version string,
/// a [`Header`] and a [`Body`].
/// The header contains all the information about the automaton (e.g. the number of states, the
/// acceptance condition, aliases etc.) and the body contains the actual transitions.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HoaRepresentation {
    header: Header,
    body: Body,
}

/// Represents an acceptance condition as it is encoded in a HOA automaton.
pub type HoaAcceptance = (usize, AcceptanceCondition);

/// Stores information on aliases, it holds a vector of pairs of alias
/// names and label expression. This can be used to unalias an automaton.
pub type Aliases = Vec<(AliasName, LabelExpression)>;

impl HoaRepresentation {
    /// Adds the given state.
    pub fn add_state(&mut self, state: State) {
        self.body.push(state);
    }

    /// Returns the version of the HOA file.
    pub fn version(&self) -> String {
        self.header.get_version().expect("Version must be set!")
    }

    /// Returns the header of the HOA file.
    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    /// Returns the body of the HOA file.
    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    /// Parses a HOA automaton from a string.
    #[allow(clippy::result_large_err)]
    pub fn parser() -> impl Parser<Token, Self, Error = Simple<Token>> {
        Header::parser()
            .then(Body::parser())
            .then_ignore(end())
            .try_map(|(header, raw_states), span| {
                let num_aps = header
                    .iter()
                    .find_map(|i| match i {
                        HeaderItem::AP(aps) => Some(aps.len()),
                        _ => None,
                    })
                    .unwrap_or(0);
                let num_aps = u16::try_from(num_aps)
                    .map_err(|_| Simple::custom(span.clone(), "too many atomic propositions"))?;
                let states = raw_states
                    .into_iter()
                    .map(|s| s.resolve(num_aps))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| Simple::custom(span, e))?;
                Ok(HoaRepresentation::from_parts(header, states.into()))
            })
    }

    /// Creates a new HOA automaton from the given version, header and
    /// body. This function will also unalias the automaton.
    pub fn from_parts(header: Header, body: Body) -> Self {
        let mut out = Self { header, body };
        out.body.sort_by_key(|x| x.0);
        out
    }

    /// Verifies that the automaton is well-formed. This means that
    /// - the number of states is set correctly
    /// - all states are defined exactly once
    pub fn verify(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        let mut states = Vec::new();
        for state in self.body().iter() {
            if states.contains(&state.id()) {
                errors.push(format!("State {} is defined more than once!", state.id()));
            }
            states.push(state.id());
        }
        if let Some(num_states) = self.num_states()
            && states.len() != num_states
        {
            errors.push(format!(
                "The number of states is set to {} but there are {} states!",
                num_states,
                states.len()
            ));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    /// Returns the number of states in the automaton, if the header states it.
    pub fn num_states(&self) -> Option<usize> {
        self.header().iter().find_map(|item| match item {
            HeaderItem::States(id) => Some(*id as usize),
            _ => None,
        })
    }

    /// Returns the start state conjunctions of the automaton, which may be empty.
    pub fn start(&self) -> Vec<&StateConjunction> {
        self.header()
            .iter()
            .filter_map(|item| match item {
                HeaderItem::Start(start) => Some(start),
                _ => None,
            })
            .collect()
    }

    /// Returns the set of all atomic propositions in the automaton, which is empty
    /// when the header has no `AP:` item.
    pub fn aps(&self) -> &[String] {
        self.header()
            .iter()
            .find_map(|item| match item {
                HeaderItem::AP(aps) => Some(aps.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Counts the number of atomic propositions in the automaton.
    ///
    /// # Panics
    ///
    /// If there are more than [`u16::MAX`] atomic propositions, which BDDs cannot
    /// represent anyway.
    pub fn num_aps(&self) -> u16 {
        u16::try_from(self.aps().len())
            .expect("more than u16::MAX atomic propositions are not supported")
    }

    /// Returns the acceptance condition of the automaton, if the header has one.
    pub fn acceptance(&self) -> Option<HoaAcceptance> {
        self.header().iter().find_map(|item| match item {
            HeaderItem::Acceptance(acceptance_sets, condition) => {
                Some((*acceptance_sets as usize, condition.clone()))
            }
            _ => None,
        })
    }

    /// Returns the aliases of the automaton.
    pub fn aliases(&self) -> Vec<(AliasName, AbstractLabelExpression)> {
        self.header()
            .iter()
            .filter_map(|item| match item {
                HeaderItem::Alias(name, expr) => Some((name.clone(), expr.clone())),
                _ => None,
            })
            .collect()
    }

    /// Replaces all alias references (`@name`) in the aliases and in the edge labels by
    /// their definitions. The `Alias:` header items are kept. Fails on undefined or
    /// cyclic aliases; on error the automaton is left unchanged.
    ///
    /// Parsing does not unalias, so call this before converting labels to BDDs.
    pub fn unalias(&mut self) -> Result<(), String> {
        let raw: std::collections::HashMap<_, _> = self
            .aliases()
            .into_iter()
            .map(|(name, expr)| (name.0, expr))
            .collect();
        let defs = raw
            .iter()
            .map(|(name, expr)| {
                Ok((
                    name.clone(),
                    expr.substitute(&raw, &mut vec![name.clone()])?,
                ))
            })
            .collect::<Result<std::collections::HashMap<_, _>, String>>()?;
        let mut body = self.body.clone();
        for edge in body.iter_mut().flat_map(|s| s.edges_mut()) {
            if let LabelExpression::Abstract(expr) = &edge.label().0 {
                edge.0 = Label(LabelExpression::Abstract(
                    expr.substitute(&defs, &mut vec![])?,
                ));
            }
        }
        for item in self.header.iter_mut() {
            if let HeaderItem::Alias(name, expr) = item {
                *expr = defs[&name.0].clone();
            }
        }
        self.body = body;
        Ok(())
    }

    /// Returns the acceptance name of the automaton, if the header has one.
    pub fn acceptance_name(&self) -> Option<(&AcceptanceName, &Vec<AcceptanceInfo>)> {
        self.header().iter().find_map(|item| match item {
            HeaderItem::AcceptanceName(name, info) => Some((name, info)),
            _ => None,
        })
    }

    /// Adds a header item to the automaton.
    pub fn add_header_item(&mut self, item: HeaderItem) {
        self.header.push(item);
    }
}

impl Default for HoaRepresentation {
    fn default() -> Self {
        HoaRepresentation::from_parts(vec![HeaderItem::Version("v1".into())].into(), vec![].into())
    }
}

impl TryFrom<&str> for HoaRepresentation {
    type Error = FromHoaError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        input::from_hoa(value)
    }
}

impl std::fmt::Display for AbstractLabelExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `&` binds tighter than `|`, and `!` applies to a single atom, so a
        // disjunction inside a conjunction and a non-atom inside a negation
        // have to be parenthesised to keep the meaning.
        match self {
            AbstractLabelExpression::Boolean(b) => match b {
                true => write!(f, "t"),
                false => write!(f, "f"),
            },
            AbstractLabelExpression::Integer(i) => write!(f, "{i}"),
            AbstractLabelExpression::Alias(name) => write!(f, "@{name}"),
            AbstractLabelExpression::Negated(expr) => {
                if is_atom(expr) {
                    write!(f, "!{}", expr)
                } else {
                    write!(f, "!({})", expr)
                }
            }
            AbstractLabelExpression::Conjunction(conjuncts) => {
                if conjuncts.is_empty() {
                    return write!(f, "t");
                }
                for (i, conjunct) in conjuncts.iter().enumerate() {
                    if i > 0 {
                        write!(f, " & ")?;
                    }
                    if matches!(conjunct, AbstractLabelExpression::Disjunction(_)) {
                        write!(f, "({})", conjunct)?;
                    } else {
                        write!(f, "{}", conjunct)?;
                    }
                }
                Ok(())
            }
            AbstractLabelExpression::Disjunction(disjuncts) => {
                if disjuncts.is_empty() {
                    return write!(f, "f");
                }
                write!(f, "{}", disjuncts.iter().format(" | "))
            }
        }
    }
}

/// Whether `expr` can be used as the argument of a negation without parentheses.
fn is_atom(expr: &AbstractLabelExpression) -> bool {
    matches!(
        expr,
        AbstractLabelExpression::Boolean(_)
            | AbstractLabelExpression::Integer(_)
            | AbstractLabelExpression::Alias(_)
    )
}

fn build_error_report<I: Iterator<Item = Simple<String>>>(input: &str, errs: I) -> String {
    errs.into_iter()
        .map(|e| {
            let report = ariadne::Report::build(ReportKind::Error, e.span());

            let report = match e.reason() {
                chumsky::error::SimpleReason::Unexpected => report
                    .with_message(format!(
                        "{}, expected {}",
                        if e.found().is_some() {
                            "Unexpected token in input"
                        } else {
                            "Unexpected end of input"
                        },
                        if e.expected().len() == 0 {
                            "something else".to_string()
                        } else {
                            e.expected()
                                .map(|expected| match expected {
                                    Some(expected) => expected.to_string(),
                                    None => "end of input".to_string(),
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    ))
                    .with_label(
                        ariadne::Label::new(e.span())
                            .with_message(format!(
                                "Unexpected token {}",
                                e.found()
                                    .unwrap_or(&"end of file".to_string())
                                    .fg(Color::Red)
                            ))
                            .with_color(Color::Red),
                    ),
                chumsky::error::SimpleReason::Unclosed { span, delimiter } => report
                    .with_message(format!(
                        "Unclosed delimiter {}",
                        delimiter.fg(Color::Yellow)
                    ))
                    .with_label(
                        ariadne::Label::new(span.clone())
                            .with_message(format!(
                                "Unclosed delimiter {}",
                                delimiter.fg(Color::Yellow)
                            ))
                            .with_color(Color::Yellow),
                    )
                    .with_label(
                        ariadne::Label::new(e.span())
                            .with_message(format!(
                                "Must be closed before this {}",
                                e.found()
                                    .unwrap_or(&"end of file".to_string())
                                    .fg(Color::Red)
                            ))
                            .with_color(Color::Red),
                    ),
                chumsky::error::SimpleReason::Custom(msg) => report.with_message(msg).with_label(
                    ariadne::Label::new(e.span())
                        .with_message(format!("{}", msg.fg(Color::Red)))
                        .with_color(Color::Red),
                ),
            };

            let mut report_output = Vec::new();
            report
                .finish()
                .write(Source::from(input), &mut report_output)
                .unwrap();

            std::str::from_utf8(&report_output)
                .unwrap_or("Could not parse error report")
                .to_string()
        })
        .join("\n")
}

#[cfg(test)]
fn print_error_report<I: Iterator<Item = Simple<String>>>(input: &str, errs: I) {
    tracing::error!("{}", build_error_report(input, errs))
}

/// Returns the byte index just past the `--END--` that terminates the first
/// automaton in `input` that was not aborted.
///
/// An `--ABORT--` discards the automaton it terminates, so aborted automata
/// never contribute a position; the search continues after the abort. Returns
/// `None` if no `--END--` follows the last abort.
pub fn first_automaton_split_position(input: &str) -> Option<usize> {
    const ENDLEN: usize = "--END--".len();
    const ABORTLEN: usize = "--ABORT--".len();

    let mut cursor = 0;
    loop {
        let rest = &input[cursor..];
        let end = cursor + rest.find("--END--")?;
        match rest.find("--ABORT--") {
            Some(abort) if cursor + abort < end => cursor += abort + ABORTLEN,
            _ => return Some(end + ENDLEN),
        }
    }
}

pub fn parse_hoa_automata(input: &str) -> Vec<HoaRepresentation> {
    let mut out = Vec::new();
    for hoa_aut in input.split_inclusive("--END--") {
        if !hoa_aut.contains("--BODY--") {
            continue;
        }
        match hoa_aut.try_into() {
            Ok(aut) => out.push(aut),
            Err(e) => warn!("Error when parsing automaton: {}", e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tracing::error;

    use crate::{
        AcceptanceAtom, AcceptanceCondition, AcceptanceName, AcceptanceSignature, Body, HeaderItem,
        HoaBool, HoaRepresentation, StateConjunction,
        body::{Edge, State},
        header::Header,
        label::AnonymousAbstract,
    };

    const ALIASED: &str = "HOA: v1
States: 1
Start: 0
AP: 2 \"a\" \"b\"
Alias: @x 0 & 1
Alias: @y !@x | 0
Acceptance: 1 Inf(0)
--BODY--
State: 0
[@y] 0 {0}
[@x & !0] 0
--END--";

    #[test]
    fn aliases_in_edges_and_aliases_unalias_convert_and_round_trip() {
        let mut aut = HoaRepresentation::try_from(ALIASED).unwrap();
        let printed = crate::output::to_hoa(&aut);
        assert_eq!(HoaRepresentation::try_from(printed.as_str()).unwrap(), aut);
        assert!(printed.contains("[@y] 0"), "{printed}");

        let before = aut.clone();
        aut.unalias().unwrap();
        let printed = crate::output::to_hoa(&aut);
        assert!(printed.contains("Alias: @y !(0 & 1) | 0"), "{printed}");
        assert!(printed.contains("[!(0 & 1) | 0] 0 {0}"), "{printed}");
        // printing flattens nested conjunctions, so compare the labels as BDDs
        let reparsed = HoaRepresentation::try_from(printed.as_str()).unwrap();
        let vs = biodivine_lib_bdd::BddVariableSet::new_anonymous(2);
        let bdds = |a: &HoaRepresentation| {
            a.body()[0]
                .edges()
                .iter()
                .map(|e| {
                    let crate::LabelExpression::Abstract(l) = &e.label().0 else {
                        panic!()
                    };
                    l.try_into_bdd(&vs).unwrap()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(bdds(&aut), bdds(&reparsed));
        // unresolved aliases do not convert
        let crate::LabelExpression::Abstract(l) = &before.body()[0].edges()[0].label().0 else {
            panic!()
        };
        assert!(l.try_into_bdd(&vs).is_err());
    }

    #[test]
    fn unalias_rejects_undefined_and_cyclic_aliases() {
        let undefined = ALIASED.replace("[@x & !0]", "[@z]");
        let mut aut = HoaRepresentation::try_from(undefined.as_str()).unwrap();
        let before = aut.clone();
        assert!(aut.unalias().unwrap_err().contains("@z is not defined"));
        assert_eq!(aut, before);

        let cyclic = ALIASED.replace("Alias: @x 0 & 1", "Alias: @x @y");
        let mut aut = HoaRepresentation::try_from(cyclic.as_str()).unwrap();
        assert!(aut.unalias().unwrap_err().contains("itself"));
    }

    fn with_body(body: &str) -> String {
        format!(
            "HOA: v1\nStates: 2\nStart: 0\nAP: 2 \"a\" \"b\"\nAcceptance: 1 Inf(0)\n--BODY--\n{body}\n--END--"
        )
    }

    #[test]
    fn implicit_labels_equal_explicit_labels() {
        let implicit = with_body("State: 0\n1 {0}\n0\n1\n1\nState: 1\n0\n0\n0\n0");
        let explicit = with_body(
            "State: 0\n[!0 & !1] 1 {0}\n[0 & !1] 0\n[!0 & 1] 1\n[0 & 1] 1\n\
             State: 1\n[!0 & !1] 0\n[0 & !1] 0\n[!0 & 1] 0\n[0 & 1] 0",
        );
        assert_eq!(
            HoaRepresentation::try_from(implicit.as_str()).unwrap(),
            HoaRepresentation::try_from(explicit.as_str()).unwrap()
        );
    }

    #[test]
    fn state_labels_equal_transition_labels() {
        let state_labels = with_body("State: [0] 0\n0 {0}\n1\nState: [!0 & 1] 1\n0");
        let explicit = with_body("State: 0\n[0] 0 {0}\n[0] 1\nState: 1\n[!0 & 1] 0");
        assert_eq!(
            HoaRepresentation::try_from(state_labels.as_str()).unwrap(),
            HoaRepresentation::try_from(explicit.as_str()).unwrap()
        );
    }

    #[test]
    fn invalid_label_combinations_are_errors() {
        for (body, msg) in [
            ("State: 0\n[0] 0\n1", "mixes labelled and unlabelled"),
            ("State: [0] 0\n[1] 0", "must not have labels"),
            ("State: 0\n0\n1\n0", "exactly 2^2 edges"),
        ] {
            let err = HoaRepresentation::try_from(with_body(body).as_str())
                .unwrap_err()
                .to_string();
            assert!(err.contains(msg), "{body}: {err}");
        }
    }

    #[test]
    fn integer_overflow_is_a_parse_error() {
        let contents = "HOA: v1\nStates: 99999999999\n--BODY--\n--END--";
        let err = HoaRepresentation::try_from(contents)
            .unwrap_err()
            .to_string();
        assert!(err.contains("integer out of range"), "{err}");
    }

    #[test]
    fn out_of_range_ap_identifier_is_a_parse_error() {
        let contents =
            "HOA: v1\nAP: 1 \"a\"\nAcceptance: 0 t\n--BODY--\nState: 0\n[65536] 0\n--END--";
        let err = HoaRepresentation::try_from(contents)
            .unwrap_err()
            .to_string();
        assert!(err.contains("AP identifier out of range"), "{err}");
    }

    #[test]
    fn accessors_work_without_optional_headers() {
        let contents = "HOA: v1\nAcceptance: 0 t\n--BODY--\n--END--";
        let aut = HoaRepresentation::try_from(contents).expect("minimal automaton parses");

        assert_eq!("v1", aut.version());
        assert_eq!(None, aut.num_states());
        assert!(aut.start().is_empty());
        assert!(aut.aps().is_empty());
        assert_eq!(0, aut.num_aps());
        assert_eq!(
            Some((0, AcceptanceCondition::Boolean(HoaBool(true)))),
            aut.acceptance()
        );
        assert_eq!(None, aut.acceptance_name());
        assert!(aut.header().acceptance_name().is_none());
        assert!(aut.aliases().is_empty());
        assert_eq!(Ok(()), aut.verify());
    }

    #[test]
    fn accessors_work_on_default_automaton() {
        let aut = HoaRepresentation::default();
        assert_eq!("v1", aut.version());
        assert_eq!(None, aut.num_states());
        assert!(aut.start().is_empty());
        assert!(aut.aps().is_empty());
        assert_eq!(0, aut.num_aps());
        assert_eq!(None, aut.acceptance());
        assert_eq!(None, aut.acceptance_name());
        assert_eq!(Ok(()), aut.verify());
    }

    #[test]
    fn first_automaton_split_and_abort() {
        let contents = "HOA: v1\n--END--\nHOA: v1\n--ABORT--\nHOA: v1\n--END--\n";

        let first = super::first_automaton_split_position(contents);
        assert_eq!(first, Some(15));
    }

    #[test]
    fn first_automaton_abort_before_end() {
        let contents = "--ABORT--\n--END--";

        let first = super::first_automaton_split_position(contents);
        assert_eq!(first, Some(contents.len()));
    }

    #[test]
    fn first_automaton_multiple_aborts() {
        let contents = "--ABORT--\nHOA: v1\n--ABORT--\nHOA: v1\n--END--";

        let first = super::first_automaton_split_position(contents);
        assert_eq!(first, Some(contents.len()));
    }

    #[test]
    fn first_automaton_abort_and_no_end() {
        assert_eq!(super::first_automaton_split_position("--ABORT--\n"), None);
        assert_eq!(
            super::first_automaton_split_position("--ABORT--\n--ABORT--\n"),
            None
        );
        assert_eq!(super::first_automaton_split_position("HOA: v1\n"), None);
    }

    #[test]
    fn real_test_1() {
        let contents = r#"HOA: v1
             AP: 1 "a"
             States: 3
             Start: 0
             acc-name: Buchi
             Acceptance: 1 Inf(0)
             --BODY--
             State: 0 {0}
              [0] 1
              [!0]  2
             State: 1  /* former state 0 */
              [0] 1
              [!0] 2
             State: 2  /* former state 1 */
              [0] 1
              [!0] 2
             --END--
             "#;
        let hoa_aut = HoaRepresentation::try_from(contents);

        if let Err(err) = hoa_aut {
            error!("Encountered parsing error\n{}", err);
            return;
        }

        let header = Header::from_vec(vec![
            HeaderItem::Version("v1".to_string()),
            HeaderItem::AP(vec!["a".to_string()]),
            HeaderItem::States(3),
            HeaderItem::Start(StateConjunction(vec![0])),
            HeaderItem::AcceptanceName(AcceptanceName::Buchi, vec![]),
            HeaderItem::Acceptance(1, AcceptanceCondition::Inf(AcceptanceAtom::Positive(0))),
        ]);
        let q0 = State::from_parts(
            0,
            None,
            vec![
                Edge::from_parts(
                    AnonymousAbstract::var_label(0),
                    StateConjunction(vec![1]),
                    AcceptanceSignature(vec![0]),
                ),
                Edge::from_parts(
                    AnonymousAbstract::not_var_label(0),
                    StateConjunction(vec![2]),
                    AcceptanceSignature(vec![0]),
                ),
            ],
        );
        let q1 = State::from_parts(
            1,
            None,
            vec![
                Edge::from_parts(
                    AnonymousAbstract::var_label(0),
                    StateConjunction(vec![1]),
                    AcceptanceSignature(vec![]),
                ),
                Edge::from_parts(
                    AnonymousAbstract::not_var_label(0),
                    StateConjunction(vec![2]),
                    AcceptanceSignature(vec![]),
                ),
            ],
        );
        let q2 = State::from_parts(
            2,
            None,
            vec![
                Edge::from_parts(
                    AnonymousAbstract::var_label(0),
                    StateConjunction(vec![1]),
                    AcceptanceSignature(vec![]),
                ),
                Edge::from_parts(
                    AnonymousAbstract::not_var_label(0),
                    StateConjunction(vec![2]),
                    AcceptanceSignature(vec![]),
                ),
            ],
        );
        assert_eq!(
            hoa_aut,
            Ok(HoaRepresentation::from_parts(
                header,
                Body::from(vec![q0, q1, q2])
            ))
        )
    }
}
