use std::fmt::{self, Display};
use std::ops::Range;

use crate::Id;

/// One lexer or parser error. The span refers to byte offsets into the parsed input; use
/// [`FromHoaError::render`] to get a report with source snippets.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Diagnostic {
    /// Byte range of the offending input.
    pub span: Range<usize>,
    /// Human readable description (no colour codes).
    pub message: String,
    /// What the parser would have accepted instead (may be empty).
    pub expected: Vec<String>,
    /// The unexpected token, or `None` for custom errors and end of input.
    pub found: Option<String>,
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (at bytes {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

impl Diagnostic {
    /// Converts a chumsky error; `T` is the token type (`char` for the text parser).
    pub(crate) fn from_rich<T: Display>(e: &chumsky::error::Rich<'_, T>) -> Self {
        use chumsky::error::RichReason;
        let span = e.span().into_range();
        match e.reason() {
            RichReason::Custom(msg) => Diagnostic {
                span,
                message: msg.clone(),
                expected: Vec::new(),
                found: None,
            },
            _ => {
                let found = e.found().map(|t| t.to_string());
                let expected: Vec<_> = e.expected().map(|p| p.to_string()).collect();
                let what = if found.is_some() {
                    "Unexpected token in input"
                } else {
                    "Unexpected end of input"
                };
                let message = if expected.is_empty() {
                    format!("{what}, expected something else")
                } else {
                    format!("{what}, expected {}", expected.join(", "))
                };
                Diagnostic {
                    span,
                    message,
                    expected,
                    found,
                }
            }
        }
    }
}

fn join(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Represents the different types of error that can be encountered when parsing a [`crate::HoaRepresentation`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FromHoaError {
    /// The version string does not match, we only support v1.
    UnsupportedVersion(String),
    /// The input could not be lexed.
    LexerError(Vec<Diagnostic>),
    /// The input could not be parsed.
    ParserError(Vec<Diagnostic>),
    /// Abort token was encountered.
    Abort,
    /// The automaton parsed but violates a rule of the format.
    Invalid(ValidationError),
}

/// A violation of the HOA format found by [`crate::HoaRepresentation::validate`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ValidationError {
    /// The header does not start with the version (`HOA:`).
    MissingVersion,
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
            Self::MissingVersion => write!(f, "missing `HOA:` version"),
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

/// An `@name` reference that [`crate::HoaRepresentation::unalias`] cannot substitute.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AliasError {
    /// There is no `Alias:` item for this name.
    Undefined(String),
    /// The name is part of a cycle, i.e. it is defined in terms of itself.
    Cyclic(String),
}

impl Display for AliasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undefined(name) => write!(f, "alias @{name} is not defined"),
            Self::Cyclic(name) => write!(f, "alias @{name} is defined in terms of itself"),
        }
    }
}

impl std::error::Error for AliasError {}

/// A violation found by [`crate::HoaRepresentation::verify`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum VerifyError {
    /// The body defines this state more than once.
    DuplicateState(Id),
    /// `States:` disagrees with the number of states in the body.
    StateCountMismatch {
        /// The number stated in the header.
        declared: usize,
        /// The number of states in the body.
        actual: usize,
    },
}

impl Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateState(state) => write!(f, "state {state} is defined more than once"),
            Self::StateCountMismatch { declared, actual } => {
                write!(f, "`States:` says {declared} but the body defines {actual}")
            }
        }
    }
}

impl std::error::Error for VerifyError {}

/// All violations found by [`crate::HoaRepresentation::verify`], in the order found.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VerifyErrors(Vec<VerifyError>);

impl VerifyErrors {
    pub(crate) fn new(errors: Vec<VerifyError>) -> Self {
        Self(errors)
    }
}

impl std::ops::Deref for VerifyErrors {
    type Target = [VerifyError];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for VerifyErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let messages: Vec<_> = self.0.iter().map(ToString::to_string).collect();
        write!(f, "{}", messages.join("; "))
    }
}

impl std::error::Error for VerifyErrors {}

impl Display for FromHoaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FromHoaError::UnsupportedVersion(version) => {
                write!(f, "Unsupported HOA version ({})", version)
            }
            FromHoaError::Abort => write!(f, "Abort token encountered"),
            FromHoaError::Invalid(err) => write!(f, "Invalid automaton: {}", err),
            FromHoaError::LexerError(rep) => write!(f, "Lexer error: {}", join(rep)),
            FromHoaError::ParserError(rep) => write!(f, "Parser error: {}", join(rep)),
        }
    }
}

impl std::error::Error for FromHoaError {}

impl std::error::Error for ValidationError {}

impl FromHoaError {
    /// Renders the error against the `src` it was produced from. Lexer and parser errors
    /// become ariadne reports with source snippets, other errors their `Display` text.
    /// `color` selects ANSI colours; use `false` for files and logs.
    pub fn render(&self, src: &str, color: bool) -> String {
        use ariadne::{Color, Fmt, Report, ReportKind, Source};
        let (Self::LexerError(diagnostics) | Self::ParserError(diagnostics)) = self else {
            return self.to_string();
        };
        let config = ariadne::Config::default()
            .with_index_type(ariadne::IndexType::Byte)
            .with_color(color);
        let mut out = Vec::new();
        for d in diagnostics {
            let label = match &d.found {
                Some(found) => format!("Unexpected token {found}"),
                None => d.message.clone(),
            };
            let label = if color {
                label.fg(Color::Red).to_string()
            } else {
                label
            };
            let report = Report::build(ReportKind::Error, d.span.clone())
                .with_config(config)
                .with_message(&d.message)
                .with_label(
                    ariadne::Label::new(d.span.clone())
                        .with_message(label)
                        .with_color(Color::Red),
                )
                .finish();
            // writing to a Vec cannot fail
            let _ = report.write(Source::from(src), &mut out);
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use crate::HoaRepresentation;

    #[test]
    fn errors_are_plain_and_boxable() {
        let src = "HOA: v1\nStates: x\n";
        let err = HoaRepresentation::try_from(src).unwrap_err();
        assert!(!err.to_string().contains('\x1b'));
        assert!(!err.render(src, false).contains('\x1b'));
        assert!(err.render(src, true).contains('\x1b'));
        let boxed: Result<HoaRepresentation, Box<dyn std::error::Error>> =
            (|| Ok(HoaRepresentation::try_from(src)?))();
        assert!(boxed.is_err());
    }

    #[test]
    fn conversion_errors_are_std_errors() {
        let boxed = |e: Box<dyn std::error::Error>| e.to_string();
        assert!(
            boxed(Box::new(crate::AliasError::Undefined("a".into()))).contains("@a is not defined")
        );
        assert!(boxed(Box::new(crate::VerifyError::DuplicateState(1))).contains("state 1"));
        let errors = crate::VerifyErrors::new(vec![
            crate::VerifyError::DuplicateState(1),
            crate::VerifyError::StateCountMismatch {
                declared: 2,
                actual: 1,
            },
        ]);
        assert_eq!(
            boxed(Box::new(errors)).as_str(),
            "state 1 is defined more than once; `States:` says 2 but the body defines 1"
        );
    }
}
