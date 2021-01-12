mod expression;

pub use expression::*;

mod expression_parser;

use crate::consumer::HoaConsumer;
use crate::lexer::Token::*;
use crate::lexer::{
    alias_name_token, header_name_token, identifier_token, integer_token, string_token, HoaLexer,
    LexerError, PositionedToken, Token,
};

use crate::parser::expression_parser::{
    is_header_token, parse_acceptance_expression, parse_alias_expression, parse_state_conjunction,
};
use itertools::Itertools;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::Formatter;
use std::iter::Peekable;
use std::slice::Iter;
use ParserError::*;

#[derive(Debug)]
pub enum ParserError {
    MismatchingToken {
        expected: String,
        actual: String,
        context: String,
    },
    MissingToken {
        expected: String,
        context: String,
    },
    UnexpectedEnd {
        message: String,
    },
    ExpressionParsingError {
        expected: String,
        found: String,
    },
    LexingError {
        message: String,
    },
    UnknownToken {
        message: String,
    },
    ZeroAtomicPropositions,
}

/// The structure holding all relevant information for parsing a HOA encoded automaton.
#[allow(dead_code)]
pub struct HoaParser<'a, 'c, C: HoaConsumer> {
    /// the consumer which receives the automaton
    consumer: &'c mut C,
    /// a lexer that tokenizes the input
    lexer: HoaLexer,
    /// the actual input which is passed in when the parser is constructed. It also determines
    /// the lifetime of a parser.
    input: &'a [u8],
}

#[allow(dead_code)]
fn expect<S: Into<String>>(
    expected: Token,
    possible_token: Option<&PositionedToken>,
    context: S,
) -> Result<&PositionedToken, ParserError> {
    match possible_token {
        Some(actual) => {
            if expected != actual.token {
                Err(MismatchingToken {
                    expected: expected.to_string(),
                    actual: actual.token.to_string(),
                    context: context.into(),
                })
            } else {
                Ok(actual)
            }
        }
        None => Err(MissingToken {
            expected: expected.to_string(),
            context: context.into(),
        }),
    }
}

impl<'a> fmt::Display for ParserError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            MissingToken { expected, context } => {
                write!(f, "Necessary token {} is missing in {}", expected, context)
            }
            MismatchingToken {
                expected,
                actual,
                context,
            } => write!(
                f,
                "Syntax error, expected token {} but got {} in {}",
                expected, actual, context
            ),
            ZeroAtomicPropositions => write!(f, "At least one atomic proposition is needed"),
            UnknownToken { message } => write!(f, "Unexpected token {}", message),
            _ => write!(f, "unexpected end"),
        }
    }
}

impl<'a> From<LexerError> for ParserError {
    fn from(err: LexerError) -> Self {
        LexingError {
            message: err.to_string(),
        }
    }
}

fn is_state_token(token: &PositionedToken) -> bool {
    token.token == TokenState
}

fn is_end_token(token: &PositionedToken) -> bool {
    token.token == TokenEnd
}

impl<'a, 'c, C: HoaConsumer> HoaParser<'a, 'c, C> {
    #[allow(dead_code)]
    pub fn new(consumer: &'c mut C, input: &'a [u8]) -> Self {
        HoaParser {
            consumer,
            input,
            lexer: HoaLexer::try_from(input).ok().unwrap(),
        }
    }

    #[allow(dead_code)]
    fn handle_edges(
        &mut self,
        state_number: usize,
        state_label_expr: Option<&BooleanExpressionAlias>,
        tokens: Vec<&Token>,
    ) -> Result<(), ParserError> {
        if tokens.is_empty() {
            return Ok(());
        }

        let mut pos = 0usize;
        let mut label_expr = state_label_expr;
        let mut label_tokens = Vec::new();
        let label_expression;
        if let Some(TokenLbracket) = tokens.get(pos) {
            pos += 1;
            loop {
                let next_token = tokens.get(pos);
                pos += 1;
                if next_token.is_none() || Some(&&TokenRbracket) == next_token {
                    break;
                }
                label_tokens.push(*next_token.unwrap());
            }
            label_expression = parse_alias_expression(&label_tokens)?;
            label_expr = Some(&label_expression);
        }

        // extract the tokens that make up state-conj
        let mut conj_tokens = Vec::new();
        loop {
            let next_token = tokens.get(pos);
            if next_token.is_none() || Some(&&TokenLcurly) == next_token {
                break;
            }
            pos += 1;
            conj_tokens.push(*next_token.unwrap());
        }
        let state_conj = &parse_state_conjunction(&conj_tokens)?;

        // extract the acc-sig if present
        let mut acc_sig = None;
        let mut acc_sig_tokens = Vec::new();
        if let Some(TokenLcurly) = tokens.get(pos) {
            pos += 1;
            loop {
                let next_token = tokens.get(pos);
                pos += 1;
                if next_token.is_none() || Some(&&TokenRcurly) == next_token {
                    break;
                }
                acc_sig_tokens.push(next_token.unwrap().unwrap_int());
            }
            acc_sig = Some(&acc_sig_tokens);
        }

        if let Some(le) = label_expr {
            // we have a labelled edge
            self.consumer
                .add_edge_with_label(state_number, le, state_conj, acc_sig);
        } else {
            // we have an implicit edge
            self.consumer
                .add_edge_implicit(state_number, state_conj, acc_sig);
        }

        self.handle_edges(
            state_number,
            state_label_expr,
            tokens.iter().skip(pos).copied().collect(),
        )?;

        Ok(())
    }

    #[allow(dead_code)]
    fn handle_state(&mut self, tokens: Vec<&Token>) -> Result<(), ParserError> {
        let mut pos = 0usize;

        // extract state label (if present)
        let mut label_expr = None;
        let mut label_tokens = Vec::new();
        let label_expression;
        if let Some(TokenLbracket) = tokens.get(pos) {
            pos += 1;
            loop {
                let next_token = tokens.get(pos);
                if next_token.is_none() || Some(&&TokenRbracket) == next_token {
                    break;
                }
                label_tokens.push(*next_token.unwrap());
                pos += 1;
            }
            label_expression = parse_alias_expression(&label_tokens)?;
            label_expr = Some(&label_expression);
        }

        // extract state number
        let state_number = match tokens.get(pos) {
            Some(&number_token) if *number_token == integer_token() => number_token.unwrap_int(),
            Some(actual) => {
                return Err(MismatchingToken {
                    expected: "Integer (state identifier)".to_string(),
                    actual: actual.to_string(),
                    context: "state extraction".to_string(),
                })
            }
            _ => {
                return Err(MissingToken {
                    expected: "Integer (state identifier)".to_string(),
                    context: "state extraction".to_string(),
                })
            }
        };
        pos += 1;

        let state_label = match tokens.get(pos) {
            Some(TokenString(label)) => {
                pos += 1;
                Some(label)
            }
            _ => None,
        };

        let mut acc_sig = None;
        let mut acc_sig_tokens = Vec::new();
        if let Some(TokenLcurly) = tokens.get(pos) {
            pos += 1;
            loop {
                let next_token = tokens.get(pos);
                if next_token.is_none() || next_token == Some(&&TokenRcurly) {
                    break;
                }
                acc_sig_tokens.push(next_token.unwrap().unwrap_int());
            }
            acc_sig = Some(&acc_sig_tokens);
        }

        self.consumer
            .add_state(state_number, state_label, label_expr, acc_sig);

        self.handle_edges(
            state_number,
            label_expr,
            tokens.iter().copied().skip(pos).collect(),
        )?;

        self.consumer.notify_end_of_state(state_number);

        Ok(())
    }

    #[allow(dead_code)]
    pub fn automaton(&mut self) -> Result<(), ParserError> {
        let tokens = self.lexer.tokenize()?;
        let mut it = tokens.iter().peekable();

        // extractor function
        let header_info_extractor = |it: &mut Peekable<Iter<PositionedToken>>| {
            it.peeking_take_while(|token| !is_header_token(&token.token))
                .map(|token| token.token.unwap_str().clone())
                .collect()
        };

        // todo hoa token extraction
        let _hoa = expect(TokenHoa, it.next(), "HOA header extraction")?;
        let hoa_version = expect(identifier_token(), it.next(), "HOA version")?
            .token
            .unwap_str();
        self.consumer
            .notify_header_start(&String::from(hoa_version));

        'header_items: loop {
            let next = it.peek();
            match next {
                None => break 'header_items,
                Some(&token) => {
                    match token.token {
                        TokenStates => {
                            // consume token
                            expect(TokenStates, it.next(), "state number extraction")?;

                            // expect next token to be integer, consume it and unwrap the contained integer
                            self.consumer.set_number_of_states(
                                expect(TokenInt(0), it.next(), "state number extraction (int)")?
                                    .token
                                    .unwrap_int(),
                            );
                        }
                        TokenStart => {
                            // allocate a vec for the start states and consume the token
                            let mut start_states = Vec::new();
                            expect(TokenStart, it.next(), "initial state extraction")?;

                            // there has to be at least one state so as above we expect an int, consume and unwrap it
                            start_states.push(
                                expect(integer_token(), it.next(), "first initial state")?
                                    .token
                                    .unwrap_int(),
                            );

                            // loop through any further integer tokens to obtain all start states
                            'extract_start_states: loop {
                                match it.peek() {
                                    Some(state) if state.token == integer_token() => {
                                        start_states.push(
                                            expect(
                                                integer_token(),
                                                it.next(),
                                                "subsequent initial states",
                                            )?
                                            .token
                                            .unwrap_int(),
                                        );
                                    }
                                    _ => break 'extract_start_states,
                                }
                            }

                            self.consumer.add_start_states(start_states);
                            // todo needs testing...
                        }
                        TokenAp => {
                            expect(TokenAp, it.next(), "ap header")?;
                            let num_aps = expect(integer_token(), it.next(), "num_aps")?
                                .token
                                .unwrap_int();
                            if num_aps < 1 {
                                return Err(ZeroAtomicPropositions);
                            }

                            // allocate space and extract atomic propositions
                            let mut aps = Vec::new();
                            for _ in 0..num_aps {
                                aps.push(String::from(
                                    expect(string_token(), it.next(), "ap extraction")?
                                        .token
                                        .unwap_str(),
                                ));
                            }
                            self.consumer.set_aps(aps);
                        }
                        TokenAlias => {
                            expect(TokenAlias, it.next(), "alias header")?;

                            //extract alias name and label-expr
                            let alias_name = String::from(
                                expect(alias_name_token(), it.next(), "alias_name")?
                                    .token
                                    .unwap_str(),
                            );

                            let alias_expr_tokens: Vec<&Token> = it
                                .peeking_take_while(|token| !is_header_token(&token.token))
                                .map(|token| &token.token)
                                .collect();

                            let alias_expr = parse_alias_expression(&alias_expr_tokens)?;
                            self.consumer.add_alias(&alias_name, &alias_expr);
                        }
                        TokenAcceptance => {
                            // todo test
                            expect(TokenAcceptance, it.next(), "acceptance header")?;

                            let num_acceptance_sets =
                                expect(integer_token(), it.next(), "number of acceptance sets")?
                                    .token
                                    .unwrap_int();

                            let acceptance_expr_tokens: Vec<&Token> = it
                                .peeking_take_while(|token| !is_header_token(&token.token))
                                .map(|token| &token.token)
                                .collect();

                            let acceptance_expr =
                                parse_acceptance_expression(&acceptance_expr_tokens)?;

                            self.consumer
                                .set_acceptance_condition(num_acceptance_sets, &acceptance_expr);
                        }
                        TokenAccname => {
                            expect(TokenAccname, it.next(), "accname header")?;

                            let acc_name = expect(
                                identifier_token(),
                                it.next(),
                                "acceptance name extraction",
                            )?
                            .token
                            .unwap_str();

                            let extra_info_tokens: Vec<&Token> = it
                                .peeking_take_while(|token| !is_header_token(&token.token))
                                .map(|token| &token.token)
                                .collect();

                            let extra_info: Vec<_> = extra_info_tokens
                                .iter()
                                .map(|token| match token {
                                    TokenIdent(ident) => AccnameInfo::StringValue(ident.clone()),
                                    TokenInt(integer) => AccnameInfo::IntegerValue(*integer),
                                    TokenTrue => AccnameInfo::BooleanValue(true),
                                    TokenFalse => AccnameInfo::BooleanValue(false),
                                    _tkn => panic!(
                                        "should not be reached, expected ident, int, true or false"
                                    ),
                                })
                                .collect();
                            self.consumer.provide_acceptance_name(acc_name, &extra_info);
                        }
                        TokenTool => {
                            expect(TokenTool, it.next(), "token tool")?;
                            let tool_info: Vec<String> = header_info_extractor(&mut it);
                            self.consumer.set_tool(tool_info);
                        }
                        TokenName => {
                            expect(TokenName, it.next(), "token name")?;
                            let name_info = expect(string_token(), it.next(), "token name info")?
                                .token
                                .unwap_str();
                            self.consumer.set_name(name_info);
                        }
                        TokenProperties => {
                            expect(TokenProperties, it.next(), "token properties")?;
                            let properties_info: Vec<String> = header_info_extractor(&mut it);
                            self.consumer.add_properties(properties_info);
                        }
                        ref hdr if header_name_token() == *hdr => {
                            expect(header_name_token(), it.next(), "misc header")?;
                            let _unused_info: Vec<_> = it
                                .peeking_take_while(|token| !is_header_token(&token.token))
                                .collect();
                        }
                        TokenBody => {
                            expect(TokenBody, it.next(), "body token")?;
                            break 'header_items;
                        }
                        _ => unreachable!(
                            "this should not happen, known headers and header tokens are handled"
                        ),
                    }
                }
            }
        }

        'states: loop {
            match it.peek() {
                Some(token) if token.token == TokenState => {
                    expect(TokenState, it.next(), "state token")?;
                    let state_tokens: Vec<&Token> = it
                        .peeking_take_while(|token| !(is_state_token(token) || is_end_token(token)))
                        .map(|token| &token.token)
                        .collect();
                    self.handle_state(state_tokens)?
                }
                _ => {
                    // all states have been read
                    break 'states;
                }
            }
        }

        expect(TokenEnd, it.next(), "end of automaton")?;
        expect(TokenEof, it.next(), "end of file")?;

        // finally return unit type as we have not encountered an error
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consumer::PrintConsumer;

    #[test]
    fn real_automaton_test() {
        let contents = b"HOA: v1\nStates: 2\nAlias: @a 0\nAlias: @ab 0 & !1\nStart: \
        0\nacc-name: Rabin 1\nAcceptance: 2 (Fin(0) & Inf(1))\nAP: 2 \"a\" \"b\"\n--BODY--\nState: \
        0 \"a U b\"   /* An example of named state */\n  [0 & !1] 0 {0}\n  [1] 1 {0}\nState: 1\n  \
        [t] 1 {1}\n--END--\n\n";
        let mut hp = HoaParser::new(PrintConsumer {}, contents as &[u8]);

        match hp.automaton() {
            Ok(_) => println!("hooray"),
            Err(err) => println!("{}", err),
        }
    }

    #[test]
    fn trait_test() {
        let v = vec![1, 2, 3];
        let mut it = v.iter().peekable();
        println!("{:?}", it.peek().unwrap());
        println!("{:?}", it.peek().unwrap());
    }
}
