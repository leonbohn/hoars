use std::ops::{Deref, DerefMut};

use chumsky::{prelude::*, select};

use crate::{
    AbstractLabelExpression, AcceptanceCondition, AcceptanceInfo, AcceptanceName, AliasName, Id,
    Property, Token,
    lexer::{Err, TokInput},
    format::{AtomicProposition, StateConjunction},
    value,
};

/// Represents a header item in a HOA file, for more information on each
/// element, see the [HOA format specification](https://adl.github.io/hoaf/).
/// The multiplicity of each element is given in parenthesis.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HeaderItem {
    /// The version of the HOA format.
    Version(String),
    /// (0|1) State header, gives the number of states in the automaton.
    States(Id),
    /// (>=0) Gives a conjunction of states that are the start states of the
    /// automaton. May be specified multiple times.
    Start(StateConjunction),
    /// (1) Gives the atomic propositions of the automaton.
    AP(Vec<AtomicProposition>),
    /// (>=0) Allows the introduction of an alias for a label expression.
    Alias(AliasName, AbstractLabelExpression),
    /// (1) Gives the acceptance condition of the automaton.
    Acceptance(Id, AcceptanceCondition),
    /// (>=0) Gives the acceptance sets of the automaton.
    AcceptanceName(AcceptanceName, Vec<AcceptanceInfo>),
    /// (0|1) Correspond to tool name and optional version number.
    Tool(String, Option<String>),
    /// (0|1) Correspond to the name of the automaton.
    Name(String),
    /// (>=0) Gives the properties of the automaton.
    Properties(Vec<Property>),
    /// A header item this crate does not know. The spec allows ignoring those
    /// whose name starts with a lowercase letter; they are kept to allow round-tripping.
    Unknown(String, Vec<UnknownValue>),
}

/// A value of an [`HeaderItem::Unknown`] header item.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UnknownValue {
    /// An integer.
    Int(String),
    /// A quoted string (without the quotes).
    Text(String),
    /// An identifier.
    Identifier(String),
}

impl HeaderItem {
    pub fn count_states(&self) -> Option<usize> {
        if let HeaderItem::States(i) = self {
            Some(*i as usize)
        } else {
            None
        }
    }

    pub fn try_acceptance_name(&self) -> Option<(&AcceptanceName, &[AcceptanceInfo])> {
        if let HeaderItem::AcceptanceName(x, y) = self {
            Some((x, y))
        } else {
            None
        }
    }

    pub fn count_acceptance_sets(&self) -> Option<usize> {
        if let HeaderItem::Acceptance(n, _) = self {
            Some(*n as usize)
        } else {
            None
        }
    }
}

impl HeaderItem {
    /// Creates a new version 1 header item.
    pub fn v1() -> Self {
        HeaderItem::Version("v1".to_string())
    }
}

fn item<'a, I: TokInput<'a>>() -> impl Parser<'a, I, HeaderItem, Err<'a>> {
    let states = just(Token::Header("States"))
        .ignore_then(value::integer())
        .map(HeaderItem::States);

    let acceptance_name = just(Token::Header("acc-name"))
        .ignore_then(value::identifier())
        .map(AcceptanceName::from)
        .then(value::acceptance_info().repeated().collect::<Vec<_>>())
        .map(|(name, info)| HeaderItem::AcceptanceName(name, info));

    let start = just(Token::Header("Start"))
        .ignore_then(value::integer().separated_by(just(Token::Op('&'))).collect())
        .map(|conjunction| HeaderItem::Start(StateConjunction(conjunction)));

    let aps = just(Token::Header("AP"))
        .ignore_then(value::integer())
        .then(value::text().repeated().collect::<Vec<_>>())
        .try_map(|(num, prop_labels), span| {
            if (num as usize) == prop_labels.len() {
                Ok(HeaderItem::AP(prop_labels))
            } else {
                Err(Rich::custom(
                    span,
                    format!("Expected {} aps but got {}", num, prop_labels.len()),
                ))
            }
        });

    let acceptance = just(Token::Header("Acceptance"))
        .ignore_then(value::integer())
        .then(value::acceptance_condition())
        .map(|(count, condition)| HeaderItem::Acceptance(count, condition));

    let alias = just(Token::Header("Alias"))
        .ignore_then(value::alias_name())
        .then(value::label_expression())
        .map(|(aname, expression)| HeaderItem::Alias(AliasName(aname), expression));

    let name = just(Token::Header("name"))
        .ignore_then(value::text())
        .map(HeaderItem::Name);

    let tool = just(Token::Header("tool"))
        .ignore_then(value::text())
        .then(value::text().or_not())
        .map(|(tool, version)| HeaderItem::Tool(tool, version));

    let properties = just(Token::Header("properties"))
        .ignore_then(
            value::identifier()
                .map(Property::from)
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map(HeaderItem::Properties);

    const KNOWN: [&str; 9] = [
        "HOA",
        "States",
        "Start",
        "AP",
        "Alias",
        "Acceptance",
        "acc-name",
        "tool",
        "name",
    ];
    let unknown = select! { Token::Header(name) if !KNOWN.contains(&name) && name != "properties" => name.to_string() }
        .then(
            select! {
                Token::Int(i) => UnknownValue::Int(i.to_string()),
                Token::Text(t) => UnknownValue::Text(t.into_owned()),
                Token::Identifier(i) => UnknownValue::Identifier(i.to_string()),
            }
            .repeated()
            .collect::<Vec<_>>(),
        )
        .map(|(name, values)| {
            if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                tracing::warn!("ignoring unknown header item {name}");
            }
            HeaderItem::Unknown(name, values)
        });

    choice((
        states,
        acceptance,
        acceptance_name,
        start,
        aps,
        alias,
        name,
        tool,
        properties,
        unknown,
    ))
}

/// Represents the header of a HOA file, consists of a set of [`HeaderItem`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Header(Vec<HeaderItem>);

impl From<Vec<HeaderItem>> for Header {
    fn from(value: Vec<HeaderItem>) -> Self {
        Self(value)
    }
}

impl<'a> IntoIterator for &'a Header {
    type Item = &'a HeaderItem;

    type IntoIter = std::slice::Iter<'a, HeaderItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl Header {
    /// Construts a new header parser.
    pub fn parser<'a, I: TokInput<'a>>() -> impl Parser<'a, I, Self, Err<'a>> {
        let version = just(Token::Header("HOA"))
            .ignore_then(value::identifier())
            .map(HeaderItem::Version);
        version
            .then(item().repeated().collect::<Vec<_>>())
            .map(|(version, headers)| Header(std::iter::once(version).chain(headers).collect()))
    }

    /// Constructs a new header from a vector of header items.
    pub fn from_vec(value: Vec<HeaderItem>) -> Self {
        Self(value)
    }

    /// Returns the version of the format.
    pub fn get_version(&self) -> Option<String> {
        self.iter().find_map(|item| match item {
            HeaderItem::Version(version) => Some(version.clone()),
            _ => None,
        })
    }

    pub fn count_states(&self) -> Option<usize> {
        self.iter().find_map(|i| i.count_states())
    }

    pub fn acceptance_name(&self) -> Option<AcceptanceName> {
        self.iter()
            .find_map(|i| i.try_acceptance_name())
            .map(|(name, _)| name.clone())
    }
}

impl Deref for Header {
    type Target = Vec<HeaderItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Header {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    pub fn process_header(input: &str) -> Result<Vec<HeaderItem>, String> {
        use chumsky::{input::Stream, prelude::Input};

        use crate::build_error_report;

        let with_hoa = format!("HOA: v1\n{}", input);
        let tokens = crate::input::lex(&with_hoa).map_err(|e| e.to_string())?;
        let eoi = (with_hoa.len()..with_hoa.len()).into();
        Header::parser()
            .then_ignore(end())
            .parse(Stream::from_iter(tokens).map(eoi, |(t, s)| (t, s)))
            .into_result()
            .map(|header| header.0)
            .map_err(|errors| build_error_report(&with_hoa, errors))
    }

    use tracing::error;

    use super::*;

    fn assert_header(input: &str, cmp: &[HeaderItem]) {
        match process_header(input) {
            Ok(res) => assert_eq!(res, cmp),
            Err(err) => {
                error!("{}", err);
                unreachable!()
            }
        }
    }

    fn assert_fails(input: &str) {
        assert!(process_header(input).is_err())
    }

    #[test]
    fn property() {
        assert_header(
            r#"properties: trans-labels state-labels"#,
            &[
                HeaderItem::Version("v1".to_string()),
                HeaderItem::Properties(vec![Property::TransLabels, Property::StateLabels]),
            ],
        );
        assert_header(
            "properties: trans-labels semi-deterministic",
            &[
                HeaderItem::Version("v1".into()),
                HeaderItem::Properties(vec![
                    Property::TransLabels,
                    Property::Other("semi-deterministic".into()),
                ]),
            ],
        );
        assert_header(
            "acc-name: Rabin-like 1",
            &[
                HeaderItem::Version("v1".into()),
                HeaderItem::AcceptanceName(
                    AcceptanceName::Other("Rabin-like".into()),
                    vec![AcceptanceInfo::Int(1)],
                ),
            ],
        );
        assert_fails(r#"properties: "#);
    }

    #[test]
    fn tool_and_name() {
        assert_header(
            r#"
                tool: "ltl-translate" "1.2-alpha"
                name: "BA for GFa & GFb"
            "#,
            &[
                HeaderItem::Version("v1".to_string()),
                HeaderItem::Tool("ltl-translate".to_string(), Some("1.2-alpha".to_string())),
                HeaderItem::Name("BA for GFa & GFb".to_string()),
            ],
        );
        assert_fails("tool: ltl-translate \"1.2-alpha\"");
    }

    #[test]
    fn unknown_items() {
        assert_header(
            "controllable-AP: 0 1\nspot.state-player: 0 1 1\nFoo: \"x\" bar",
            &[
                HeaderItem::Version("v1".into()),
                HeaderItem::Unknown(
                    "controllable-AP".into(),
                    vec![UnknownValue::Int("0".into()), UnknownValue::Int("1".into())],
                ),
                HeaderItem::Unknown(
                    "spot.state-player".into(),
                    vec![
                        UnknownValue::Int("0".into()),
                        UnknownValue::Int("1".into()),
                        UnknownValue::Int("1".into()),
                    ],
                ),
                HeaderItem::Unknown(
                    "Foo".into(),
                    vec![
                        UnknownValue::Text("x".into()),
                        UnknownValue::Identifier("bar".into()),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn unknown_items_round_trip() {
        let hoa = "HOA: v1\nStates: 1\nStart: 0\nAP: 0\nAcceptance: 0 t\ncontrollable-AP: 0 1\nspot.state-player: 0\n--BODY--\nState: 0\n--END--";
        let aut = crate::parse_hoa_automata(hoa).remove(0);
        let out = crate::output::to_hoa(&aut);
        assert!(out.contains("controllable-AP: 0 1\nspot.state-player: 0\n--BODY--"));
        assert_eq!(
            crate::parse_hoa_automata(&out).remove(0).header(),
            aut.header()
        );
    }

    #[test]
    fn ariadne() {
        assert_header(
            "acc-name: Rabin 3",
            &[
                HeaderItem::Version("v1".to_string()),
                HeaderItem::AcceptanceName(AcceptanceName::Rabin, vec![AcceptanceInfo::Int(3)]),
            ],
        );
        assert_header(
            "Start: 0 & 7",
            &[
                HeaderItem::Version("v1".to_string()),
                HeaderItem::Start(StateConjunction(vec![0, 7])),
            ],
        );
    }

    #[test]
    fn aps() {
        assert_header(
            r#"AP: 3 "a" "proc@state" "a[x] >= 2""#,
            &[
                HeaderItem::Version("v1".to_string()),
                HeaderItem::AP(vec![
                    "a".to_string(),
                    "proc@state".to_string(),
                    "a[x] >= 2".to_string(),
                ]),
            ],
        )
    }

    #[test]
    fn alias() {
        use AbstractLabelExpression::*;
        let alias = |x| HeaderItem::Alias(AliasName("a".to_string()), x);
        let v1 = HeaderItem::Version("v1".to_string());
        assert_header("Alias: @a 0", &[v1.clone(), alias(Integer(0))]);
        assert_header(
            "Alias: @a 0 & 1",
            &[v1.clone(), alias(Conjunction(vec![Integer(0), Integer(1)]))],
        );
        // & binds stronger
        assert_header(
            "Alias: @a 1 | 2 & 0",
            &[
                v1.clone(),
                alias(Disjunction(vec![
                    Integer(1),
                    Conjunction(vec![Integer(2), Integer(0)]),
                ])),
            ],
        );
        assert_header(
            "Alias: @a (0 | 1) & 2",
            &[
                v1.clone(),
                alias(Conjunction(vec![
                    Disjunction(vec![Integer(0), Integer(1)]),
                    Integer(2),
                ])),
            ],
        );
        assert_header(
            "Alias: @a !@b",
            &[v1, alias(Negated(Box::new(Alias("b".to_string()))))],
        );
    }
}
