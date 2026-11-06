use chumsky::prelude::*;

use crate::{
    AbstractLabelExpression, AcceptanceCondition, AcceptanceInfo, AcceptanceName, AliasName, Id,
    Property, ValidationError,
    format::{AtomicProposition, StateConjunction},
    value::{self, HoaErr, tok},
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
    /// The number of states if this is a `States:` item.
    pub fn count_states(&self) -> Option<usize> {
        if let HeaderItem::States(i) = self {
            Some(*i as usize)
        } else {
            None
        }
    }

    /// The name and parameters if this is an `acc-name:` item.
    pub fn try_acceptance_name(&self) -> Option<(&AcceptanceName, &[AcceptanceInfo])> {
        if let HeaderItem::AcceptanceName(x, y) = self {
            Some((x, y))
        } else {
            None
        }
    }

    /// The number of acceptance sets if this is an `Acceptance:` item.
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

fn item<'a, X: HoaErr<'a>>() -> impl Parser<'a, &'a str, HeaderItem, extra::Err<X>> {
    let states = tok("States:")
        .ignore_then(value::integer())
        .map(HeaderItem::States);

    let acceptance_name = tok("acc-name:")
        .ignore_then(value::identifier())
        .map(AcceptanceName::from)
        .then(value::acceptance_info().repeated().collect::<Vec<_>>())
        .map(|(name, info)| HeaderItem::AcceptanceName(name, info));

    let start = tok("Start:")
        .ignore_then(value::state_conjunction())
        .map(HeaderItem::Start);

    let aps = tok("AP:")
        .ignore_then(value::integer())
        .then(value::text().repeated().collect::<Vec<_>>())
        .try_map(|(num, prop_labels), span| {
            if (num as usize) == prop_labels.len() {
                Ok(HeaderItem::AP(prop_labels))
            } else {
                Err(X::custom(
                    span,
                    format!("Expected {} aps but got {}", num, prop_labels.len()),
                ))
            }
        });

    let acceptance = tok("Acceptance:")
        .ignore_then(value::integer())
        .then(value::acceptance_condition())
        .map(|(count, condition)| HeaderItem::Acceptance(count, condition));

    let alias = tok("Alias:")
        .ignore_then(value::alias_name())
        .then(value::label_expression())
        .map(|(aname, expression)| HeaderItem::Alias(AliasName(aname), expression));

    let name = tok("name:")
        .ignore_then(value::text())
        .map(HeaderItem::Name);

    let tool = tok("tool:")
        .ignore_then(value::text())
        .then(value::text().or_not())
        .map(|(tool, version)| HeaderItem::Tool(tool, version));

    let properties = tok("properties:")
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
    let unknown = value::header_name()
        .filter(|name: &&str| !KNOWN.contains(name) && *name != "properties")
        .map(str::to_string)
        .then(
            choice((
                value::int_text().map(|i: &str| UnknownValue::Int(i.to_string())),
                value::text().map(UnknownValue::Text),
                value::identifier().map(UnknownValue::Identifier),
            ))
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

/// The header of a HOA file. The items are validated once, when the header is built: all
/// items that may appear at most once are stored in their own field, so the accessors are
/// plain field reads. Items that may be repeated (`Start:`, `Alias:`, `properties:`) are
/// collected, and unknown items are kept in [`Header::unknown`].
///
/// [`Header::items`] lists the header in a fixed order (version, `name:`, `tool:`,
/// `States:`, `Start:`, `AP:`, `Alias:`, `Acceptance:`, `acc-name:`, `properties:`, unknown
/// items), which is also the order the writer uses; the order of the input is not kept, and
/// several `properties:` lines are joined into one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub(crate) version: String,
    pub(crate) states: Option<Id>,
    pub(crate) start: Vec<StateConjunction>,
    pub(crate) aps: Option<Vec<AtomicProposition>>,
    pub(crate) aliases: Vec<(AliasName, AbstractLabelExpression)>,
    pub(crate) acceptance: Option<(Id, AcceptanceCondition)>,
    pub(crate) acc_name: Option<(AcceptanceName, Vec<AcceptanceInfo>)>,
    pub(crate) tool: Option<(String, Option<String>)>,
    pub(crate) name: Option<String>,
    pub(crate) properties: Vec<Property>,
    pub(crate) unknown: Vec<(String, Vec<UnknownValue>)>,
}

impl Default for Header {
    /// An empty header of version `v1`.
    fn default() -> Self {
        Self::new("v1")
    }
}

impl Header {
    /// Creates a header that only has the given version, e.g. `v1`.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            states: None,
            start: Vec::new(),
            aps: None,
            aliases: Vec::new(),
            acceptance: None,
            acc_name: None,
            tool: None,
            name: None,
            properties: Vec::new(),
            unknown: Vec::new(),
        }
    }

    /// Parses a header, i.e. `HOA:` and all following header items.
    pub fn parser<'a, X: HoaErr<'a>>()
    -> impl Parser<'a, &'a str, Result<Self, ValidationError>, extra::Err<X>> {
        raw_parser().map(Self::from_items)
    }

    /// Builds a header from its items. The first item must be the version, further versions
    /// are an error, as is a repetition of an item that may appear at most once.
    pub fn from_items(items: Vec<HeaderItem>) -> Result<Self, ValidationError> {
        let mut items = items.into_iter();
        let Some(HeaderItem::Version(version)) = items.next() else {
            return Err(ValidationError::MissingVersion);
        };
        let mut header = Self::new(version);
        items.try_for_each(|item| header.push(item))?;
        Ok(header)
    }

    /// Adds an item. Fails if `item` may appear at most once and the header already has one.
    /// `Start:`, `Alias:` and `properties:` are appended, unknown items are kept in order.
    pub fn push(&mut self, item: HeaderItem) -> Result<(), ValidationError> {
        fn once<T>(slot: &mut Option<T>, name: &'static str, v: T) -> Result<(), ValidationError> {
            if slot.is_some() {
                return Err(ValidationError::DuplicateHeader(name));
            }
            *slot = Some(v);
            Ok(())
        }
        match item {
            HeaderItem::Version(_) => return Err(ValidationError::DuplicateHeader("HOA")),
            HeaderItem::States(n) => once(&mut self.states, "States", n)?,
            HeaderItem::Start(conj) => self.start.push(conj),
            HeaderItem::AP(aps) => once(&mut self.aps, "AP", aps)?,
            HeaderItem::Alias(name, expr) => self.aliases.push((name, expr)),
            HeaderItem::Acceptance(n, cond) => once(&mut self.acceptance, "Acceptance", (n, cond))?,
            HeaderItem::AcceptanceName(name, info) => {
                once(&mut self.acc_name, "acc-name", (name, info))?
            }
            HeaderItem::Tool(name, version) => once(&mut self.tool, "tool", (name, version))?,
            HeaderItem::Name(name) => once(&mut self.name, "name", name)?,
            HeaderItem::Properties(mut props) => self.properties.append(&mut props),
            HeaderItem::Unknown(name, values) => self.unknown.push((name, values)),
        }
        Ok(())
    }

    /// The header as a list of items, in the order described at [`Header`].
    pub fn items(&self) -> Vec<HeaderItem> {
        let mut out = vec![HeaderItem::Version(self.version.clone())];
        out.extend(self.name.clone().map(HeaderItem::Name));
        out.extend(self.tool.clone().map(|(n, v)| HeaderItem::Tool(n, v)));
        out.extend(self.states.map(HeaderItem::States));
        out.extend(self.start.iter().cloned().map(HeaderItem::Start));
        out.extend(self.aps.clone().map(HeaderItem::AP));
        out.extend(
            self.aliases
                .iter()
                .cloned()
                .map(|(n, e)| HeaderItem::Alias(n, e)),
        );
        out.extend(
            self.acceptance
                .clone()
                .map(|(n, c)| HeaderItem::Acceptance(n, c)),
        );
        out.extend(
            self.acc_name
                .clone()
                .map(|(n, i)| HeaderItem::AcceptanceName(n, i)),
        );
        if !self.properties.is_empty() {
            out.push(HeaderItem::Properties(self.properties.clone()));
        }
        out.extend(
            self.unknown
                .iter()
                .cloned()
                .map(|(n, v)| HeaderItem::Unknown(n, v)),
        );
        out
    }

    /// Returns the version of the format.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the version of the format.
    pub fn get_version(&self) -> Option<&str> {
        Some(&self.version)
    }

    /// Returns the number given in `States:`, if any.
    pub fn states(&self) -> Option<Id> {
        self.states
    }

    /// Returns the number given in `States:`, if any.
    pub fn count_states(&self) -> Option<usize> {
        self.states.map(|n| n as usize)
    }

    /// Returns the `Start:` items.
    pub fn start(&self) -> &[StateConjunction] {
        &self.start
    }

    /// Returns the atomic propositions, empty without an `AP:` item.
    pub fn aps(&self) -> &[AtomicProposition] {
        self.aps.as_deref().unwrap_or_default()
    }

    /// Returns the `Alias:` items.
    pub fn aliases(&self) -> &[(AliasName, AbstractLabelExpression)] {
        &self.aliases
    }

    /// Returns the number of acceptance sets and the condition of `Acceptance:`, if present.
    pub fn acceptance(&self) -> Option<(Id, &AcceptanceCondition)> {
        self.acceptance.as_ref().map(|(n, c)| (*n, c))
    }

    /// Returns the name and parameters of `acc-name:`, if present.
    pub fn acceptance_name_info(&self) -> Option<(&AcceptanceName, &[AcceptanceInfo])> {
        self.acc_name.as_ref().map(|(n, i)| (n, i.as_slice()))
    }

    /// Returns the name of `acc-name:`, if present.
    pub fn acceptance_name(&self) -> Option<AcceptanceName> {
        self.acc_name.as_ref().map(|(n, _)| n.clone())
    }

    /// Returns the tool and its optional version, if present.
    pub fn tool(&self) -> Option<(&str, Option<&str>)> {
        self.tool.as_ref().map(|(t, v)| (t.as_str(), v.as_deref()))
    }

    /// Returns the name of the automaton, if present.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns all properties.
    pub fn properties(&self) -> &[Property] {
        &self.properties
    }

    /// Returns the header items this crate does not know.
    pub fn unknown(&self) -> &[(String, Vec<UnknownValue>)] {
        &self.unknown
    }
}

/// Parses `HOA:` and the header items without combining them.
pub(crate) fn raw_parser<'a, X: HoaErr<'a>>()
-> impl Parser<'a, &'a str, Vec<HeaderItem>, extra::Err<X>> {
    tok("HOA:")
        .ignore_then(value::identifier())
        .map(HeaderItem::Version)
        .then(item().repeated().collect::<Vec<_>>())
        .map(|(version, headers)| std::iter::once(version).chain(headers).collect())
}

#[cfg(test)]
mod tests {
    pub fn process_header(input: &str) -> Result<Vec<HeaderItem>, String> {
        let with_hoa = format!("HOA: v1\n{}", input);
        crate::value::ws()
            .ignore_then(raw_parser::<chumsky::error::Rich<char, crate::value::Span>>())
            .then_ignore(end())
            .parse(&with_hoa)
            .into_result()
            .map_err(|errors| {
                errors
                    .iter()
                    .map(|e| crate::Diagnostic::from_rich(e).to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
    }

    use tracing::error;

    use super::*;

    #[test]
    fn start_needs_a_state() {
        assert_fails("Start:");
    }

    #[test]
    fn header_is_validated_and_canonical() {
        let mut header = Header::new("v1");
        header.push(HeaderItem::States(2)).unwrap();
        assert_eq!(
            header.push(HeaderItem::States(3)),
            Err(ValidationError::DuplicateHeader("States"))
        );
        assert_eq!(
            header.push(HeaderItem::Version("v1".into())),
            Err(ValidationError::DuplicateHeader("HOA"))
        );
        assert_eq!(header.states(), Some(2));
        header.push(HeaderItem::Name("n".into())).unwrap();
        assert_eq!(
            header.items(),
            vec![
                HeaderItem::Version("v1".into()),
                HeaderItem::Name("n".into()),
                HeaderItem::States(2)
            ]
        );
        assert_eq!(
            Header::from_items(vec![HeaderItem::States(1)]),
            Err(ValidationError::MissingVersion)
        );
    }

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
