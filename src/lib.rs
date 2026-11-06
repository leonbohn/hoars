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

use chumsky::Parser;
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
    /// The automaton parsed but violates a rule of the format.
    Invalid(ValidationError),
}

/// A violation of the HOA format found by [`HoaRepresentation::validate`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ValidationError {
    /// There is no `Acceptance:` header item.
    MissingAcceptance,
    /// A header item that may appear at most once appears several times.
    DuplicateHeader(&'static str),
    /// An acceptance set is not below the count given in `Acceptance:`.
    AcceptanceSetOutOfRange {
        /// The offending acceptance set.
        set: Id,
        /// The number of acceptance sets.
        count: Id,
    },
    /// A state is not below the count given in `States:`.
    StateOutOfRange {
        /// The offending state.
        state: Id,
        /// The number of states.
        count: Id,
    },
    /// An atomic proposition is not below the count given in `AP:`.
    ApOutOfRange {
        /// The offending atomic proposition.
        ap: u16,
        /// The number of atomic propositions (0 if there is no `AP:` item).
        count: usize,
    },
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingAcceptance => write!(f, "missing `Acceptance:` header item"),
            Self::DuplicateHeader(name) => write!(f, "`{name}:` appears more than once"),
            Self::AcceptanceSetOutOfRange { set, count } => {
                write!(f, "acceptance set {set} used but only {count} declared")
            }
            Self::StateOutOfRange { state, count } => {
                write!(f, "state {state} used but only {count} states declared")
            }
            Self::ApOutOfRange { ap, count } => {
                write!(f, "AP {ap} used but only {count} declared")
            }
        }
    }
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
            FromHoaError::Invalid(err) => write!(f, "Invalid automaton: {}", err),
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
    pub fn parser<'a, I: lexer::TokInput<'a>, X: lexer::HoaErr<'a, I>>()
    -> impl Parser<'a, I, Self, chumsky::extra::Err<X>> {
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
                    .map_err(|_| X::custom(span, "too many atomic propositions".to_string()))?;
                let states = raw_states
                    .into_iter()
                    .map(|s| s.resolve(num_aps))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| X::custom(span, e))?;
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

    /// Checks the automaton against the rules of the format: the version is `v1` or
    /// `v1.x`, `Acceptance:` appears exactly once and `States:`, `AP:`, `acc-name:`,
    /// `tool:` and `name:` at most once, and all states, acceptance sets and atomic
    /// propositions that are used lie within the declared counts. This is run by the
    /// parser; use [`HoaRepresentation::verify`] for the checks on the body's states.
    pub fn validate(&self) -> Result<(), FromHoaError> {
        use ValidationError::*;
        let invalid = |e| Err(FromHoaError::Invalid(e));

        let version = self.version();
        let minor = version.strip_prefix("v1.");
        if version != "v1" && !minor.is_some_and(|m| m.split('.').all(|d| !d.is_empty())) {
            return Err(FromHoaError::UnsupportedVersion(version));
        }

        let count = |pred: fn(&HeaderItem) -> bool| self.header.iter().filter(|i| pred(i)).count();
        for (name, n) in [
            ("States", count(|i| matches!(i, HeaderItem::States(_)))),
            ("AP", count(|i| matches!(i, HeaderItem::AP(_)))),
            (
                "acc-name",
                count(|i| matches!(i, HeaderItem::AcceptanceName(..))),
            ),
            ("tool", count(|i| matches!(i, HeaderItem::Tool(..)))),
            ("name", count(|i| matches!(i, HeaderItem::Name(_)))),
            (
                "Acceptance",
                count(|i| matches!(i, HeaderItem::Acceptance(..))),
            ),
        ] {
            if n > 1 {
                return invalid(DuplicateHeader(name));
            }
        }
        let Some((num_sets, condition)) = self.header.iter().find_map(|i| match i {
            HeaderItem::Acceptance(n, c) => Some((*n, c)),
            _ => None,
        }) else {
            return invalid(MissingAcceptance);
        };
        let num_aps = self.aps().len();
        let num_states = self.header.iter().find_map(|i| match i {
            HeaderItem::States(n) => Some(*n),
            _ => None,
        });

        let check_set = |set: Id| match set < num_sets {
            true => Ok(()),
            false => invalid(AcceptanceSetOutOfRange {
                set,
                count: num_sets,
            }),
        };
        let check_state = |state: Id| match num_states {
            Some(count) if state >= count => invalid(StateOutOfRange { state, count }),
            _ => Ok(()),
        };
        let check_label = |label: &AbstractLabelExpression| match label
            .max_ap()
            .filter(|ap| usize::from(*ap) >= num_aps)
        {
            Some(ap) => invalid(ApOutOfRange { ap, count: num_aps }),
            None => Ok(()),
        };

        condition.sets().into_iter().try_for_each(check_set)?;
        for item in self.header.iter() {
            match item {
                HeaderItem::Start(conj) => conj.0.iter().copied().try_for_each(check_state)?,
                HeaderItem::Alias(_, expr) => check_label(expr)?,
                _ => {}
            }
        }
        for state in self.body.iter() {
            check_state(state.id())?;
            for edge in state.edges() {
                edge.state_conjunction()
                    .0
                    .iter()
                    .copied()
                    .try_for_each(check_state)?;
                edge.acceptance_signature()
                    .0
                    .iter()
                    .copied()
                    .try_for_each(check_set)?;
                if let LabelExpression::Abstract(expr) = &edge.label().0 {
                    check_label(expr)?;
                }
            }
        }
        Ok(())
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

fn build_error_report<'a, T: std::fmt::Display + 'a>(
    input: &str,
    errs: impl IntoIterator<Item = chumsky::error::Rich<'a, T>>,
) -> String {
    use chumsky::error::RichReason;
    errs.into_iter()
        .map(|e| {
            let span = e.span().into_range();
            let report = ariadne::Report::build(ReportKind::Error, span.clone())
                .with_config(ariadne::Config::default().with_index_type(ariadne::IndexType::Byte));
            let (message, label) = match e.reason() {
                RichReason::Custom(msg) => (msg.clone(), msg.clone()),
                _ => {
                    let found = e.found().map(|t| t.to_string());
                    let expected = e.expected().map(|p| p.to_string()).collect::<Vec<_>>();
                    (
                        format!(
                            "{}, expected {}",
                            if found.is_some() {
                                "Unexpected token in input"
                            } else {
                                "Unexpected end of input"
                            },
                            if expected.is_empty() {
                                "something else".to_string()
                            } else {
                                expected.join(", ")
                            }
                        ),
                        format!("Unexpected token {}", found.unwrap_or("end of file".into())),
                    )
                }
            };
            let report = report.with_message(message).with_label(
                ariadne::Label::new(span)
                    .with_message(label.fg(Color::Red))
                    .with_color(Color::Red),
            );

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
fn print_error_report<'a, T: std::fmt::Display + 'a>(
    input: &str,
    errs: impl IntoIterator<Item = chumsky::error::Rich<'a, T>>,
) {
    tracing::error!("{}", build_error_report(input, errs))
}

/// Splits a token stream into automata: each returned chunk ends with a `--END--`, a
/// trailing chunk without `--END--` is returned last. An `--ABORT--` discards everything
/// since the previous boundary.
fn split_automata(tokens: Vec<(Token<'_>, lexer::Span)>) -> Vec<Vec<(Token<'_>, lexer::Span)>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for (token, span) in tokens {
        match token {
            Token::Abort => current.clear(),
            Token::BodyEnd => {
                current.push((token, span));
                chunks.push(std::mem::take(&mut current));
            }
            _ => current.push((token, span)),
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Returns the byte index just past the `--END--` that terminates the first
/// automaton in `input` that was not aborted.
///
/// An `--ABORT--` discards the automaton it terminates, so aborted automata
/// never contribute a position; the search continues after the abort. Returns
/// `None` if no `--END--` follows the last abort, or if `input` cannot be lexed.
/// Only real tokens count, so `--END--` or `--ABORT--` inside a string or a comment
/// is ignored.
pub fn first_automaton_split_position(input: &str) -> Option<usize> {
    let end = split_automata(input::lex(input).ok()?)
        .into_iter()
        .find(|chunk| matches!(chunk.last(), Some((Token::BodyEnd, _))))?
        .pop()?
        .1
        .end;
    Some(end)
}

/// Parses all automata in `input`. Automata that are aborted or that fail to parse are
/// skipped (failures are logged). If `input` cannot be lexed, nothing is returned.
pub fn parse_hoa_automata(input: &str) -> Vec<HoaRepresentation> {
    let tokens = match input::lex(input) {
        Ok(tokens) => tokens,
        Err(e) => {
            warn!("Error when lexing automata: {}", e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for chunk in split_automata(tokens) {
        if !chunk.iter().any(|(t, _)| *t == Token::BodyStart) {
            continue;
        }
        match input::parse_tokens(input, chunk) {
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

    const MINIMAL: &str = "HOA: v1\nAcceptance: 0 t\n--BODY--\n--END--";

    #[test]
    fn escaped_quote_in_string() {
        let hoa = MINIMAL.replace("v1", "v1\nname: \"a\\\"b\\\\c\\n\"");
        let aut = HoaRepresentation::try_from(hoa.as_str()).unwrap();
        assert!(
            aut.header()
                .iter()
                .any(|i| *i == HeaderItem::Name("a\"b\\c\\n".into()))
        );
        // the writer escapes again, so it round-trips
        let printed = crate::output::to_hoa(&aut);
        assert_eq!(HoaRepresentation::try_from(printed.as_str()).unwrap(), aut);
    }

    #[test]
    fn nested_comment() {
        let hoa = MINIMAL.replace("v1", "v1 /*a/*b*/c*/ /* d */");
        assert!(HoaRepresentation::try_from(hoa.as_str()).is_ok());
        let unterminated = MINIMAL.replace("v1", "v1 /*a/*b*/c");
        assert!(HoaRepresentation::try_from(unterminated.as_str()).is_err());
    }

    #[test]
    fn abort_inside_string_is_not_an_abort() {
        let hoa = MINIMAL.replace("v1", "v1\nname: \"--ABORT-- --END--\"");
        assert!(HoaRepresentation::try_from(hoa.as_str()).is_ok());
        assert_eq!(super::parse_hoa_automata(&hoa).len(), 1);
        assert_eq!(super::first_automaton_split_position(&hoa), Some(hoa.len()));
    }

    #[test]
    fn abort_in_header_followed_by_valid_automaton() {
        let input = format!("HOA: v1\nStates: 3\n--ABORT--\n{MINIMAL}");
        let auts = super::parse_hoa_automata(&input);
        assert_eq!(auts.len(), 1);
        assert_eq!(auts[0], HoaRepresentation::try_from(MINIMAL).unwrap());
        assert!(matches!(
            HoaRepresentation::try_from(input.as_str()),
            Err(super::FromHoaError::Abort)
        ));
    }

    #[test]
    fn abort_suffix_is_an_identifier() {
        let hoa = MINIMAL.replace("v1", "v1\nfoo--ABORT--: 1\nacc-name: foo--ABORT--");
        let aut = HoaRepresentation::try_from(hoa.as_str()).unwrap();
        assert!(
            aut.header()
                .iter()
                .any(|i| matches!(i, HeaderItem::Unknown(n, _) if n == "foo--ABORT--"))
        );
        assert_eq!(super::parse_hoa_automata(&hoa).len(), 1);
    }

    fn validation_error(hoa: &str) -> crate::FromHoaError {
        HoaRepresentation::try_from(hoa).unwrap_err()
    }

    fn invalid(e: crate::ValidationError) -> crate::FromHoaError {
        crate::FromHoaError::Invalid(e)
    }

    const VALID: &str = "HOA: v1\nStates: 2\nStart: 0\nAP: 1 \"a\"\nAcceptance: 2 Inf(0) | Fin(1)\n\
         --BODY--\nState: 0\n[0] 1 {1}\nState: 1\n[!0] 0 {0}\n--END--";

    #[test]
    fn validation_accepts_valid_automaton_and_minor_versions() {
        assert!(HoaRepresentation::try_from(VALID).is_ok());
        for v in ["v1.1", "v1.10.2"] {
            let hoa = VALID.replace("v1", v);
            assert!(HoaRepresentation::try_from(hoa.as_str()).is_ok(), "{v}");
        }
    }

    #[test]
    fn unsupported_versions() {
        for v in ["v2", "v1.", "v10", "v2.1", "v1x"] {
            let hoa = VALID.replace("HOA: v1", &format!("HOA: {v}"));
            assert_eq!(
                validation_error(&hoa),
                crate::FromHoaError::UnsupportedVersion(v.into()),
                "{v}"
            );
        }
    }

    #[test]
    fn missing_acceptance() {
        let hoa = VALID.replace("Acceptance: 2 Inf(0) | Fin(1)\n", "");
        assert_eq!(
            validation_error(&hoa),
            invalid(crate::ValidationError::MissingAcceptance)
        );
    }

    #[test]
    fn duplicate_headers() {
        for (name, extra) in [
            ("States", "States: 2"),
            ("AP", "AP: 0"),
            ("acc-name", "acc-name: all\nacc-name: all"),
            ("tool", "tool: \"a\"\ntool: \"b\""),
            ("name", "name: \"a\"\nname: \"b\""),
            ("Acceptance", "Acceptance: 0 t"),
        ] {
            let hoa = VALID.replace("--BODY--", &format!("{extra}\n--BODY--"));
            assert_eq!(
                validation_error(&hoa),
                invalid(crate::ValidationError::DuplicateHeader(name)),
                "{name}"
            );
        }
    }

    #[test]
    fn acceptance_set_out_of_range() {
        for hoa in [
            VALID.replace("Fin(1)", "Fin(2)"),
            VALID.replace("{1}", "{2}"),
        ] {
            assert_eq!(
                validation_error(&hoa),
                invalid(crate::ValidationError::AcceptanceSetOutOfRange { set: 2, count: 2 })
            );
        }
    }

    #[test]
    fn state_out_of_range() {
        for (hoa, state) in [
            (VALID.replace("Start: 0", "Start: 2"), 2),
            (VALID.replace("[0] 1", "[0] 3"), 3),
            (VALID.replace("State: 1", "State: 4"), 4),
        ] {
            assert_eq!(
                validation_error(&hoa),
                invalid(crate::ValidationError::StateOutOfRange { state, count: 2 })
            );
        }
        // without `States:` any state id is fine
        let hoa = VALID.replace("States: 2\n", "").replace("[0] 1", "[0] 3");
        assert!(HoaRepresentation::try_from(hoa.as_str()).is_ok());
    }

    #[test]
    fn ap_out_of_range() {
        for hoa in [
            VALID.replace("[!0]", "[!1]"),
            VALID.replace("AP: 1 \"a\"\n", "AP: 1 \"a\"\nAlias: @x 1\n"),
        ] {
            assert_eq!(
                validation_error(&hoa),
                invalid(crate::ValidationError::ApOutOfRange { ap: 1, count: 1 })
            );
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
