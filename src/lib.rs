//! This crate provides a parser for the HOA format.
#![warn(missing_docs)]
mod body;
mod error;
mod format;
mod header;
/// Entry points for parsing HOA text.
pub mod input;
mod lexer;
/// Writing HOA, see [`output::to_hoa`].
pub mod output;
mod value;

/// Label expressions on edges and aliases.
pub mod label;
#[allow(deprecated)]
pub use label::MAX_APS;
pub use label::{AbstractLabelExpression, Label};

use tracing::warn;

use std::fmt::Debug;

use chumsky::{Parser, prelude::end};

pub use format::*;

pub use body::{Body, Edge, State};
pub use error::{AliasError, Diagnostic, FromHoaError, ValidationError, VerifyError, VerifyErrors};
pub use header::{Header, HeaderItem, UnknownValue};

use itertools::Itertools;
use lexer::Token;

/// The type of identifier used for states.
pub type Id = u32;

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
pub type Aliases = Vec<(AliasName, AbstractLabelExpression)>;

impl HoaRepresentation {
    /// Adds the given state.
    pub fn add_state(&mut self, state: State) {
        self.body.push(state);
    }

    /// Returns the version of the HOA file.
    pub fn version(&self) -> String {
        self.header.version.clone()
    }

    /// Returns the header of the HOA file.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Gives mutable access to the header.
    pub fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    /// Returns the body of the HOA file.
    pub fn body(&self) -> &Body {
        &self.body
    }

    /// Gives mutable access to the body.
    pub fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    /// Parses a HOA automaton from a string. The result is `Err` if the header is
    /// malformed (repeated items), the automaton is not validated further, see
    /// [`HoaRepresentation::validate`].
    pub fn parser<'a, X: value::HoaErr<'a>>()
    -> impl Parser<'a, &'a str, Result<Self, FromHoaError>, chumsky::extra::Err<X>> {
        value::ws()
            .ignore_then(Header::parser())
            .then(Body::parser())
            .then_ignore(end())
            .try_map(|(header, raw_states), span| {
                let header = match header {
                    Ok(header) => header,
                    Err(e) => return Ok(Err(FromHoaError::Invalid(e))),
                };
                let num_aps = u16::try_from(header.aps().len())
                    .map_err(|_| X::custom(span, "too many atomic propositions".to_string()))?;
                let states = raw_states
                    .into_iter()
                    .map(|s| s.resolve(num_aps))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| X::custom(span, e))?;
                Ok(Ok(HoaRepresentation::from_parts(header, states.into())))
            })
    }

    /// Creates a new HOA automaton from the given header and body.
    pub fn from_parts(header: Header, body: Body) -> Self {
        Self { header, body }
    }

    /// Verifies that the automaton is well-formed. This means that
    /// - the number of states is set correctly
    /// - all states are defined exactly once
    pub fn verify(&self) -> Result<(), VerifyErrors> {
        use VerifyError::*;
        let mut errors = Vec::new();
        // `body_mut`/`add_state` can leave the body unsorted, so sort the ids instead of relying on it
        let mut states: Vec<_> = self.body().iter().map(|s| s.id()).collect();
        states.sort_unstable();
        for pair in states.windows(2) {
            if pair[0] == pair[1] {
                errors.push(DuplicateState(pair[0]));
            }
        }
        if let Some(num_states) = self.num_states()
            && states.len() != num_states
        {
            errors.push(StateCountMismatch {
                declared: num_states,
                actual: states.len(),
            });
        }
        match errors.is_empty() {
            true => Ok(()),
            false => Err(VerifyErrors::new(errors)),
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

        let Some((num_sets, condition)) = self.header.acceptance() else {
            return invalid(MissingAcceptance);
        };
        let num_aps = self.aps().len();
        let num_states = self.header.states();

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
        for conj in self.header.start() {
            conj.0.iter().copied().try_for_each(check_state)?;
        }
        for (_, expr) in self.header.aliases() {
            check_label(expr)?;
        }
        for state in self.body.iter() {
            check_state(state.id())?;
            if let Some(acc) = state.acceptance() {
                acc.0.iter().copied().try_for_each(check_set)?;
            }
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
                check_label(&edge.label().0)?;
            }
        }
        Ok(())
    }

    /// Returns the number of states in the automaton, if the header states it.
    pub fn num_states(&self) -> Option<usize> {
        self.header.count_states()
    }

    /// Returns the start state conjunctions of the automaton, which may be empty.
    pub fn start(&self) -> Vec<&StateConjunction> {
        self.header.start().iter().collect()
    }

    /// Returns the set of all atomic propositions in the automaton, which is empty
    /// when the header has no `AP:` item.
    pub fn aps(&self) -> &[String] {
        self.header.aps()
    }

    /// Counts the number of atomic propositions in the automaton.
    ///
    /// # Panics
    ///
    /// If there are more than [`u16::MAX`] atomic propositions, which label expressions
    /// cannot refer to anyway.
    pub fn num_aps(&self) -> u16 {
        u16::try_from(self.aps().len())
            .expect("more than u16::MAX atomic propositions are not supported")
    }

    /// Returns the acceptance condition of the automaton, if the header has one.
    pub fn acceptance(&self) -> Option<HoaAcceptance> {
        let (sets, condition) = self.header.acceptance()?;
        Some((sets as usize, condition.clone()))
    }

    /// Returns the aliases of the automaton.
    pub fn aliases(&self) -> Vec<(AliasName, AbstractLabelExpression)> {
        self.alias_iter()
            .map(|(name, expr)| (name.clone(), expr.clone()))
            .collect()
    }

    /// Iterates over the `Alias:` header items without cloning them.
    pub fn alias_iter(&self) -> impl Iterator<Item = (&AliasName, &AbstractLabelExpression)> {
        self.header
            .aliases()
            .iter()
            .map(|(name, expr)| (name, expr))
    }

    /// Replaces all alias references (`@name`) in the aliases and in the edge labels by
    /// their definitions. The `Alias:` header items are kept. Fails on undefined or
    /// cyclic aliases; on error the automaton is left unchanged.
    ///
    /// Parsing does not unalias, so call this before interpreting labels.
    pub fn unalias(&mut self) -> Result<(), AliasError> {
        let raw: std::collections::HashMap<_, _> = self
            .alias_iter()
            .map(|(name, expr)| (name.0.clone(), expr.clone()))
            .collect();
        // walk the aliases in file order so that a reported cycle does not depend on map order
        let defs = self
            .alias_iter()
            .map(|(name, expr)| {
                Ok((
                    name.0.clone(),
                    expr.substitute(&raw, &mut vec![name.0.clone()])?,
                ))
            })
            .collect::<Result<std::collections::HashMap<_, _>, AliasError>>()?;
        let mut body = self.body.clone();
        for edge in body.iter_mut().flat_map(|s| s.edges_mut()) {
            edge.0 = Label(edge.label().0.substitute(&defs, &mut vec![])?);
        }
        for (name, expr) in &mut self.header.aliases {
            *expr = defs[&name.0].clone();
        }
        self.body = body;
        Ok(())
    }

    /// Returns the acceptance name of the automaton, if the header has one.
    pub fn acceptance_name(&self) -> Option<(&AcceptanceName, &Vec<AcceptanceInfo>)> {
        self.header
            .acc_name
            .as_ref()
            .map(|(name, info)| (name, info))
    }

    /// Adds a header item to the automaton, see [`Header::push`].
    pub fn add_header_item(&mut self, item: HeaderItem) -> Result<(), ValidationError> {
        self.header.push(item)
    }
}

impl Default for HoaRepresentation {
    fn default() -> Self {
        HoaRepresentation::from_parts(Header::default(), Body::new())
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
        // chunks are never empty; error reports refer to the text of the chunk
        let text = &input[chunk[0].1.start..chunk[chunk.len() - 1].1.end];
        match input::parse_str(text) {
            Ok(aut) => out.push(aut),
            Err(e) => warn!("Error when parsing automaton: {}", e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
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
        // printing flattens nested conjunctions, so compare the labels semantically
        let reparsed = HoaRepresentation::try_from(printed.as_str()).unwrap();
        let tables = |a: &HoaRepresentation| {
            a.body()[0]
                .edges()
                .iter()
                .map(|e| crate::label::truth_table(&e.label().0, 2))
                .collect::<Vec<_>>()
        };
        assert_eq!(tables(&aut), tables(&reparsed));
        // the automaton before unaliasing still refers to the alias
        assert_eq!(
            before.body()[0].edges()[0].label().0,
            crate::AbstractLabelExpression::Alias("y".into())
        );
    }

    #[test]
    fn unalias_rejects_undefined_and_cyclic_aliases() {
        let undefined = ALIASED.replace("[@x & !0]", "[@z]");
        let mut aut = HoaRepresentation::try_from(undefined.as_str()).unwrap();
        let before = aut.clone();
        assert_eq!(
            aut.unalias().unwrap_err(),
            crate::AliasError::Undefined("z".into())
        );
        assert_eq!(aut, before);

        let cyclic = ALIASED.replace("Alias: @x 0 & 1", "Alias: @x @y");
        let mut aut = HoaRepresentation::try_from(cyclic.as_str()).unwrap();
        // the first alias in file order closes the cycle
        assert_eq!(
            aut.unalias().unwrap_err(),
            crate::AliasError::Cyclic("x".into())
        );
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
        assert!(aut.header().name() == Some("a\"b\\c\\n"));
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
    fn unclosed_nested_comments_are_linear_time() {
        // found by fuzzing: backtracking over unclosed `/*` was exponential
        let input = "/*/*\"\"/*#{//*/*/".repeat(200);
        let start = std::time::Instant::now();
        assert!(HoaRepresentation::try_from(input.as_str()).is_err());
        assert!(super::parse_hoa_automata(&input).is_empty());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
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
                .unknown()
                .iter()
                .any(|(n, _)| n == "foo--ABORT--")
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
    fn verify_reports_duplicate_states() {
        let mut aut = HoaRepresentation::try_from(
            "HOA: v1\nStates: 3\nAcceptance: 0 t\n--BODY--\nState: 0\nState: 1\nState: 2\n--END--",
        )
        .unwrap();
        assert_eq!(Ok(()), aut.verify());
        let dup = aut.body()[0].clone();
        aut.add_state(dup);
        assert!(aut.body().windows(2).all(|w| w[0].id() <= w[1].id()));
        let err = aut.verify().unwrap_err();
        assert_eq!(
            err.to_string(),
            "state 0 is defined more than once; `States:` says 3 but the body defines 4"
        );
        assert_eq!(err[0], crate::VerifyError::DuplicateState(0));
    }

    #[test]
    fn verify_reports_state_count_mismatch() {
        let aut = HoaRepresentation::try_from(
            "HOA: v1\nStates: 3\nAcceptance: 0 t\n--BODY--\nState: 0\n--END--",
        )
        .unwrap();
        assert_eq!(
            aut.verify().unwrap_err().to_vec(),
            vec![crate::VerifyError::StateCountMismatch {
                declared: 3,
                actual: 1
            }]
        );
    }

    /// Every file in `hoa/` (not the benchmarks) parses, round-trips and converts to BDDs.
    #[test]
    fn fixtures_parse_round_trip_and_convert() {
        fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|n| n != "bench") {
                        collect(&path, out);
                    }
                } else if path.extension().is_some_and(|e| e == "hoa") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        collect(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("hoa"),
            &mut files,
        );
        assert!(files.len() >= 20, "found only {} fixtures", files.len());
        for path in files {
            let name = path.display();
            let text = std::fs::read_to_string(&path).unwrap();
            // some files hold several automata, and parse_hoa_automata skips failures
            let auts = crate::parse_hoa_automata(&text);
            assert_eq!(auts.len(), text.matches("--END--").count(), "{name}");
            for mut aut in auts {
                let printed = crate::output::to_hoa(&aut);
                let reparsed = HoaRepresentation::try_from(printed.as_str())
                    .unwrap_or_else(|e| panic!("{name}: reprint does not parse: {e}\n{printed}"));
                assert_eq!(reparsed, aut, "{name}: round trip changed the automaton");
                aut.unalias().unwrap_or_else(|e| panic!("{name}: {e}"));
                for state in aut.body().iter() {
                    for edge in state.edges() {
                        let l = &edge.label().0;
                        assert!(
                            l.max_ap().is_none_or(|ap| ap < aut.num_aps()),
                            "{name}: label {l} uses an unknown AP"
                        );
                    }
                }
            }
        }
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
        let hoa_aut = HoaRepresentation::try_from(contents).unwrap();

        let header = Header::from_items(vec![
            HeaderItem::Version("v1".to_string()),
            HeaderItem::AP(vec!["a".to_string()]),
            HeaderItem::States(3),
            HeaderItem::Start(StateConjunction(vec![0])),
            HeaderItem::AcceptanceName(AcceptanceName::Buchi, vec![]),
            HeaderItem::Acceptance(1, AcceptanceCondition::Inf(AcceptanceAtom::Positive(0))),
        ])
        .unwrap();
        let q0 = State::from_parts(
            0,
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
        )
        .with_acceptance(Some(AcceptanceSignature(vec![0])));
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
            HoaRepresentation::from_parts(header, Body::from(vec![q0, q1, q2]))
        )
    }
}
