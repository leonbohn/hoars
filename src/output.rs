use std::fmt::Display;

use biodivine_lib_bdd::Bdd;
use itertools::Itertools;

use crate::{
    AcceptanceAtom, AcceptanceCondition, AcceptanceInfo, AcceptanceName, AcceptanceSignature,
    AliasName, Edge, HeaderItem, HoaBool, HoaRepresentation, Label, LabelExpression, Property,
    State, StateConjunction, UnknownValue,
};

/// Quotes a string for use in HOA, escaping backslashes and double quotes.
fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Renders a [`Bdd`] as a HOA label expression in disjunctive normal form.
fn bdd_to_expression(bdd: &Bdd) -> String {
    if bdd.is_true() {
        return "t".to_string();
    }
    if bdd.is_false() {
        return "f".to_string();
    }
    bdd.sat_clauses()
        .map(|clause| {
            let literals = clause
                .to_values()
                .into_iter()
                .map(|(var, value)| match value {
                    true => var.to_index().to_string(),
                    false => format!("!{}", var.to_index()),
                })
                .join(" & ");
            match literals.is_empty() {
                true => "t".to_string(),
                false => literals,
            }
        })
        .join(" | ")
}

pub fn to_hoa(aut: &HoaRepresentation) -> String {
    aut.header()
        .into_iter()
        .map(|header_item| header_item.to_string())
        .chain(std::iter::once("--BODY--".to_string()))
        .chain(aut.body().into_iter().map(|state| state.to_string()))
        .chain(std::iter::once("--END--".to_string()))
        .join("\n")
}

impl Display for HeaderItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeaderItem::Version(version) => write!(f, "HOA: {}", version),
            HeaderItem::States(number) => write!(f, "States: {}", number),
            HeaderItem::Start(state_conj) => write!(f, "Start: {}", state_conj),
            HeaderItem::AP(aps) => write!(
                f,
                "AP: {} {}",
                aps.len(),
                aps.iter().map(|ap| quote(ap)).join(" ")
            ),
            HeaderItem::Alias(alias_name, alias_expression) => {
                write!(f, "Alias: {} {}", alias_name, alias_expression)
            }
            HeaderItem::Acceptance(number_sets, condition) => {
                write!(f, "Acceptance: {} {}", number_sets, condition)
            }
            HeaderItem::AcceptanceName(identifier, vec_info) => {
                write!(f, "acc-name: {} {}", identifier, vec_info.iter().join(" "))
            }
            HeaderItem::Tool(name, version) => {
                write!(f, "tool: {}", quote(name))?;
                if let Some(version) = version {
                    write!(f, " {}", quote(version))?;
                }
                Ok(())
            }
            HeaderItem::Name(name) => write!(f, "name: {}", quote(name)),
            HeaderItem::Properties(properties) => {
                write!(f, "properties: {}", properties.iter().join(" "))
            }
            HeaderItem::Unknown(name, values) => {
                write!(f, "{name}:")?;
                for value in values {
                    match value {
                        UnknownValue::Int(s) | UnknownValue::Identifier(s) => write!(f, " {s}")?,
                        UnknownValue::Text(s) => write!(f, " {}", quote(s))?,
                    }
                }
                Ok(())
            }
        }
    }
}

impl Display for AcceptanceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptanceInfo::Int(integer) => write!(f, "{}", integer),
            AcceptanceInfo::Identifier(identifier) => write!(f, "{}", identifier),
        }
    }
}

impl Display for Property {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Property::Other(other) => other,
                Property::StateLabels => "state-labels",
                Property::TransLabels => "trans-labels",
                Property::ImplicitLabels => "implicit-labels",
                Property::ExplicitLabels => "explicit-labels",
                Property::StateAcceptance => "state-acc",
                Property::TransitionAcceptance => "trans-acc",
                Property::UniversalBranching => "univ-branch",
                Property::NoUniversalBranching => "no-univ-branch",
                Property::Deterministic => "deterministic",
                Property::Complete => "complete",
                Property::Unambiguous => "unambiguous",
                Property::StutterInvariant => "stutter-invariant",
                Property::Weak => "weak",
                Property::VeryWeak => "very-weak",
                Property::InherentlyWeak => "inherently-weak",
                Property::Terminal => "terminal",
                Property::Tight => "tight",
                Property::Colored => "colored",
            }
        )
    }
}

impl Display for AcceptanceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AcceptanceName::Other(other) => other,
                AcceptanceName::Buchi => "Buchi",
                AcceptanceName::GeneralizedBuchi => "generalized-Buchi",
                AcceptanceName::CoBuchi => "co-Buchi",
                AcceptanceName::GeneralizedCoBuchi => "generalized-co-Buchi",
                AcceptanceName::Streett => "Streett",
                AcceptanceName::Rabin => "Rabin",
                AcceptanceName::GeneralizedRabin => "generalized-Rabin",
                AcceptanceName::Parity => "parity",
                AcceptanceName::All => "all",
                AcceptanceName::None => "none",
            }
        )
    }
}

impl Display for AcceptanceAtom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptanceAtom::Positive(id) => write!(f, "{}", id),
            AcceptanceAtom::Negative(id) => write!(f, "!{}", id),
        }
    }
}

impl Display for HoaBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.0 { "t" } else { "f" })
    }
}

impl Display for AcceptanceCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptanceCondition::Fin(id) => write!(f, "Fin({})", id),
            AcceptanceCondition::Inf(id) => write!(f, "Inf({})", id),
            AcceptanceCondition::And(left, right) => write!(f, "({} & {})", left, right),
            AcceptanceCondition::Or(left, right) => write!(f, "({} | {})", left, right),
            AcceptanceCondition::Boolean(val) => write!(f, "{}", val),
        }
    }
}

impl Display for AliasName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl Display for StateConjunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.iter().map(|s| s.to_string()).join(" & "))
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            LabelExpression::Abstract(expr) => write!(f, "[{}]", expr),
            LabelExpression::Expression(bdd) => write!(f, "[{}]", bdd_to_expression(bdd)),
        }
    }
}

impl Display for AcceptanceSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
        write!(f, "{{{}}}", self.0.iter().join(" "))
    }
}

impl Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.0, self.1)?;
        if !self.2.is_empty() {
            write!(f, " {}", self.2)?;
        }
        Ok(())
    }
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State: {}", self.0)?;
        if let Some(comment) = &self.1 {
            write!(f, " {}", quote(comment))?;
        }
        for edge in &self.2 {
            write!(f, "\n{}", edge)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AbstractLabelExpression;

    fn parse(text: &str) -> HoaRepresentation {
        text.try_into()
            .unwrap_or_else(|err| panic!("could not parse {text:?}: {err}"))
    }

    fn round_trip(aut: &HoaRepresentation) -> String {
        let printed = to_hoa(aut);
        let reparsed = parse(&printed);
        assert_eq!(
            *aut, reparsed,
            "round trip changed the automaton:\n{printed}"
        );
        printed
    }

    fn label(expr: AbstractLabelExpression) -> String {
        expr.to_string()
    }

    fn not(expr: AbstractLabelExpression) -> Box<AbstractLabelExpression> {
        Box::new(AbstractLabelExpression::Negated(Box::new(expr)))
    }

    #[test]
    fn round_trip_fixture_files() {
        let mut seen = 0;
        let mut dirs = vec![std::path::PathBuf::from("hoa")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("hoa/ must exist") {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                let contents = std::fs::read_to_string(&path).unwrap();
                for aut in crate::parse_hoa_automata(&contents) {
                    round_trip(&aut);
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "no automata were parsed from hoa/");
    }

    #[test]
    fn round_trip_sample() {
        let sample = r#"HOA: v1
AP: 2 "a" "b"
Acceptance: 1 Inf(0)
properties: unambiguous
name: "x"
Alias: @a (0 | 1) & 1
--BODY--
State: 0
[0 & !1] 0 {0}
--END--"#;
        let printed = round_trip(&parse(sample));
        assert!(printed.contains("properties: unambiguous"), "{printed}");
        assert!(printed.contains("name: \"x\""), "{printed}");
        assert!(printed.contains("Alias: @a (0 | 1) & 1"), "{printed}");
        assert!(printed.contains("[0 & !1] 0 {0}"), "{printed}");
    }

    #[test]
    fn label_parenthesisation() {
        use AbstractLabelExpression::{Boolean, Conjunction, Disjunction, Integer};

        let disjunction = Disjunction(vec![Integer(0), Integer(1)]);
        assert_eq!("0 | 1", label(disjunction.clone()));
        assert_eq!(
            "(0 | 1) & 1",
            label(Conjunction(vec![disjunction.clone(), Integer(1)]))
        );
        assert_eq!("!(0 | 1)", label(*not(disjunction)));
        assert_eq!(
            "!(0 & 1)",
            label(*not(Conjunction(vec![Integer(0), Integer(1)])))
        );
        assert_eq!("!(!0)", label(*not(*not(Integer(0)))));
        assert_eq!("!t", label(*not(Boolean(true))));
        assert_eq!("0", label(Integer(0)));
    }

    #[test]
    fn bdd_labels_are_printed_as_dnf() {
        let vs = biodivine_lib_bdd::BddVariableSet::new_anonymous(2);
        let bdd = vs.mk_var(biodivine_lib_bdd::BddVariable::from_index(0));
        assert_eq!("[0]", Label(LabelExpression::Expression(bdd)).to_string());
        assert_eq!(
            "[f]",
            Label(LabelExpression::Expression(vs.mk_false())).to_string()
        );
        assert_eq!(
            "[t]",
            Label(LabelExpression::Expression(vs.mk_true())).to_string()
        );
        let or = vs
            .mk_var(biodivine_lib_bdd::BddVariable::from_index(0))
            .or(&vs.mk_not_var(biodivine_lib_bdd::BddVariable::from_index(1)));
        let printed = Label(LabelExpression::Expression(or.clone())).to_string();
        // The printed DNF must parse back to the same BDD.
        let text = format!(
            "HOA: v1\nAP: 2 \"a\" \"b\"\nAcceptance: 1 Inf(0)\n--BODY--\nState: 0\n{printed} 0\n--END--"
        );
        let aut = parse(&text);
        let LabelExpression::Abstract(expr) = &aut.body()[0].2[0].0.0 else {
            panic!("expected an abstract label")
        };
        assert_eq!(or, expr.try_into_bdd(&vs).unwrap());
    }
}
