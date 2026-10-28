//! Filter expression parser and evaluator for FlowLens
//!
//! Supports DSL expressions like:
//!   GET /api/* AND status>=200 AND status<400
//!   POST host=github.com PID=1234

use thiserror::Error;
use crate::models::TrafficRow;

#[derive(Debug, Clone)]
pub enum FilterExpr {
    Method(String),
    Host(String),
    StatusCodeRange(u16, u16),
    PathGlob(glob::Pattern),
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
    Not(Box<FilterExpr>),
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected end of input")]
    UnexpectedEnd,
    #[error("unexpected token: {0}")]
    UnexpectedToken(String),
    #[error("invalid range: {0}")]
    InvalidRange(String),
    #[error("glob parse error: {0}")]
    GlobError(#[from] glob::PatternError),
    #[error("unknown field: {0}")]
    UnknownField(String),
    #[error("invalid integer: {0}")]
    InvalidInt(#[from] std::num::ParseIntError),
}

impl ParseError {
    pub fn unexpected(s: &str) -> Self {
        ParseError::UnexpectedToken(s.to_string())
    }
}

pub fn parse_filter(expr: &str) -> Result<FilterExpr, ParseError> {
    let tokens = tokenize(expr)?;
    if tokens.is_empty() {
        return Err(ParseError::UnexpectedEnd);
    }
    let (consumed, expr) = parse_or(&tokens, 0)?;
    if consumed != tokens.len() {
        return Err(ParseError::UnexpectedToken(format!("trailing tokens after position {consumed}")));
    }
    Ok(expr)
}

#[derive(Debug, Clone)]
enum Token {
    Word(String), Eq, Ne, Le, Lt, Ge, Gt, And, Or, Not, LPar, RPar,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' => tokens.push(Token::LPar),
            ')' => tokens.push(Token::RPar),
            '=' => tokens.push(Token::Eq),
            '!' if chars.peek() == Some(&'=') => { chars.next(); tokens.push(Token::Ne); }
            '<' if chars.peek() == Some(&'=') => { chars.next(); tokens.push(Token::Le); }
            '<' => tokens.push(Token::Lt),
            '>' if chars.peek() == Some(&'=') => { chars.next(); tokens.push(Token::Ge); }
            '>' => tokens.push(Token::Gt),
            'a'..='z' | 'A'..='Z' | '_' | '.' | '-' => {
                let mut word = String::from(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '.' || nc == '-' { word.push(nc); chars.next(); } else { break; }
                }
                match word.to_uppercase().as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    "NOT" => tokens.push(Token::Not),
                    _ => tokens.push(Token::Word(word)),
                }
            }
            '/' | '*' => {
                let mut glob_str = String::from(c);
                while let Some(&nc) = chars.peek() {
                    if !nc.is_whitespace() { glob_str.push(nc); chars.next(); } else { break; }
                }
                tokens.push(Token::Word(glob_str));
            }
            '0'..='9' => {
                let mut num = String::from(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() { num.push(nc); chars.next(); } else { break; }
                }
                tokens.push(Token::Word(num));
            }
            ' ' | '\t' | '\n' | '\r' => {}
            _ => return Err(ParseError::unexpected(&format!("{c}"))),
        }
    }
    Ok(tokens)
}

fn parse_or(tokens: &[Token], pos: usize) -> Result<(usize, FilterExpr), ParseError> {
    let (mut pos, mut left) = parse_and(tokens, pos)?;
    while pos < tokens.len() && matches!(&tokens[pos], Token::Or) {
        let (next_pos, right) = parse_and(tokens, pos + 1)?;
        pos = next_pos;
        left = FilterExpr::Or(vec![left, right]);
    }
    Ok((pos, left))
}

fn parse_and(tokens: &[Token], pos: usize) -> Result<(usize, FilterExpr), ParseError> {
    let (mut pos, mut left) = parse_not(tokens, pos)?;
    loop {
        // explicit AND
        if pos < tokens.len() && matches!(&tokens[pos], Token::And) {
            let (next_pos, right) = parse_not(tokens, pos + 1)?;
            pos = next_pos;
            left = FilterExpr::And(vec![left, right]);
            continue;
        }
        // implicit AND: next token starts a new predicate (Word/LPar/Not)
        if pos < tokens.len() && matches!(&tokens[pos], Token::Word(_) | Token::LPar | Token::Not) {
            let (next_pos, right) = parse_not(tokens, pos)?;
            pos = next_pos;
            left = FilterExpr::And(vec![left, right]);
            continue;
        }
        break;
    }
    Ok((pos, left))
}

fn parse_not(tokens: &[Token], pos: usize) -> Result<(usize, FilterExpr), ParseError> {
    if pos < tokens.len() && matches!(&tokens[pos], Token::Not) {
        let pos = pos + 1;
        let (next_pos, inner) = parse_not(tokens, pos)?;
        return Ok((next_pos, FilterExpr::Not(Box::new(inner))));
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[Token], pos: usize) -> Result<(usize, FilterExpr), ParseError> {
    if pos >= tokens.len() { return Err(ParseError::UnexpectedEnd); }
    match &tokens[pos] {
        Token::LPar => {
            let (pos, expr) = parse_or(tokens, pos + 1)?;
            if pos < tokens.len() && matches!(&tokens[pos], Token::RPar) {
                Ok((pos + 1, expr))
            } else { Err(ParseError::UnexpectedEnd) }
        }
        _ => parse_predicate(tokens, pos),
    }
}

fn parse_predicate(tokens: &[Token], pos: usize) -> Result<(usize, FilterExpr), ParseError> {
    if pos >= tokens.len() { return Err(ParseError::UnexpectedEnd); }
    let word = match &tokens[pos] {
        Token::Word(w) => w.clone(),
        _ => return Err(ParseError::unexpected(&format!("{:?}", tokens[pos]))),
    };
    let pos = pos + 1;

    if word.eq_ignore_ascii_case("status") && pos < tokens.len() {
        let op = &tokens[pos];
        let val = match &tokens.get(pos + 1) {
            Some(Token::Word(v)) => v.parse::<u16>().map_err(|_| ParseError::InvalidRange(v.clone()))?,
            _ => return Err(ParseError::InvalidRange("expected status value".into())),
        };
        let pos = pos + 2;
        match op {
            Token::Gt => Ok((pos, FilterExpr::StatusCodeRange(val.saturating_add(1), u16::MAX))),
            Token::Ge => Ok((pos, FilterExpr::StatusCodeRange(val, u16::MAX))),
            Token::Lt => Ok((pos, FilterExpr::StatusCodeRange(0, val.saturating_sub(1)))),
            Token::Le => Ok((pos, FilterExpr::StatusCodeRange(0, val))),
            Token::Eq => Ok((pos, FilterExpr::StatusCodeRange(val, val))),
            _ => Err(ParseError::unexpected(&format!("{op:?}"))),
        }
    } else if word.eq_ignore_ascii_case("host") {
        if pos < tokens.len() && matches!(&tokens[pos], Token::Eq) {
            let pos = pos + 1;
            if let Some(Token::Word(host)) = tokens.get(pos) {
                return Ok((pos + 1, FilterExpr::Host(host.clone())));
            }
        }
        Ok((pos, FilterExpr::Host(word)))
    } else if word.starts_with('/') || word.contains('*') {
        let pattern = glob::Pattern::new(&word)?;
        Ok((pos, FilterExpr::PathGlob(pattern)))
    } else if word.len() <= 7 && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        Ok((pos, FilterExpr::Method(word)))
    } else {
        Err(ParseError::UnknownField(word))
    }
}

pub fn eval_filter(row: &TrafficRow, filter: &FilterExpr) -> bool {
    match filter {
        FilterExpr::Method(m) => row.method.to_uppercase() == m.to_uppercase(),
        FilterExpr::Host(h) => row.src_addr.contains(h) || row.dst_addr.contains(h),
        FilterExpr::StatusCodeRange(lo, hi) => {
            let code = parse_status_code(&row.status);
            code.map(|c| c >= *lo && c <= *hi).unwrap_or(false)
        }
        FilterExpr::PathGlob(p) => p.matches(&row.path),
        FilterExpr::And(exprs) => exprs.iter().all(|e| eval_filter(row, e)),
        FilterExpr::Or(exprs) => exprs.iter().any(|e| eval_filter(row, e)),
        FilterExpr::Not(inner) => !eval_filter(row, inner),
    }
}

fn parse_status_code(status: &str) -> Option<u16> {
    status.trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row(method: &str, path: &str, status: &str) -> TrafficRow {
        TrafficRow {
            timestamp: 0, event_type: "http".into(),
            src_addr: "127.0.0.1:1234".into(), dst_addr: "93.184.216.34:443".into(),
            method: method.into(), path: path.into(), status: status.into(),
            duration_ms: 0, body_size: 0,
        }
    }

    #[test]
    fn test_parse_get_method() {
        let expr = parse_filter("GET").unwrap();
        assert!(matches!(expr, FilterExpr::Method(ref m) if m == "GET"));
    }

    #[test]
    fn test_parse_path_glob() {
        let expr = parse_filter("/api/*").unwrap();
        assert!(matches!(expr, FilterExpr::PathGlob(_)));
    }

    #[test]
    fn test_parse_status_ge() {
        let expr = parse_filter("status>=200").unwrap();
        assert!(matches!(expr, FilterExpr::StatusCodeRange(200, u16::MAX)));
    }

    #[test]
    fn test_parse_status_lt() {
        let expr = parse_filter("status<400").unwrap();
        assert!(matches!(expr, FilterExpr::StatusCodeRange(0, 399)));
    }

    #[test]
    fn test_parse_and_expr() {
        let expr = parse_filter("GET /api/* AND status>=200").unwrap();
        assert!(matches!(expr, FilterExpr::And(_)));
    }

    #[test]
    fn test_parse_or_expr() {
        let expr = parse_filter("GET OR POST").unwrap();
        assert!(matches!(expr, FilterExpr::Or(_)));
    }

    #[test]
    fn test_parse_not_expr() {
        let expr = parse_filter("NOT GET").unwrap();
        assert!(matches!(expr, FilterExpr::Not(_)));
    }

    #[test]
    fn test_parse_host_eq() {
        let expr = parse_filter("host=github.com").unwrap();
        assert!(matches!(expr, FilterExpr::Host(ref h) if h == "github.com"));
    }

    #[test]
    fn test_eval_method_match() {
        let row = mk_row("GET", "/api/users", "200 OK");
        assert!(eval_filter(&row, &parse_filter("GET").unwrap()));
    }

    #[test]
    fn test_eval_method_no_match() {
        let row = mk_row("POST", "/api/users", "201 Created");
        assert!(!eval_filter(&row, &parse_filter("GET").unwrap()));
    }

    #[test]
    fn test_eval_status_range_pass() {
        let row = mk_row("GET", "/api", "200 OK");
        assert!(eval_filter(&row, &parse_filter("status>=200 AND status<300").unwrap()));
    }

    #[test]
    fn test_eval_status_range_fail() {
        let row = mk_row("GET", "/api", "500 Error");
        assert!(!eval_filter(&row, &parse_filter("status>=200 AND status<300").unwrap()));
    }

    #[test]
    fn test_eval_and_complex() {
        let row = mk_row("GET", "/api/users", "200 OK");
        assert!(eval_filter(&row, &parse_filter("GET /api/* AND status>=200").unwrap()));
    }

    #[test]
    fn test_eval_or() {
        let row = mk_row("POST", "/api", "201 Created");
        assert!(eval_filter(&row, &parse_filter("GET OR POST").unwrap()));
    }

    #[test]
    fn test_eval_not() {
        let row = mk_row("POST", "/api", "201 Created");
        assert!(eval_filter(&row, &parse_filter("NOT GET").unwrap()));
    }

    #[test]
    fn test_parse_error_unknown_field() {
        assert!(parse_filter("FOO=bar").is_err());
    }

    #[test]
    fn test_parse_error_empty() {
        assert!(parse_filter("").is_err());
    }

    #[test]
    fn test_parse_parenthesized() {
        let expr = parse_filter("(GET OR POST) AND /api/*").unwrap();
        assert!(matches!(expr, FilterExpr::And(_)));
    }
}

