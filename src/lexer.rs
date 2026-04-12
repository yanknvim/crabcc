use std::fmt;

use logos::Logos;

#[derive(Logos, Clone, Copy, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")]
#[logos(error = String)]
pub enum Token<'a> {
    #[regex(r"//.*\n?", logos::skip, allow_greedy = true)]
    #[regex(r"/\*(?:[^*]|\*[^/])*\*/", logos::skip, allow_greedy = true)]
    #[token("int")]
    Int,
    #[token("char")]
    Char,

    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,

    #[token("sizeof")]
    Sizeof,

    #[token("return")]
    Return,
    #[token(";")]
    Semicolon,

    #[token(",")]
    Comma,

    #[regex("[0-9]+", |lex| lex.slice().parse::<usize>().unwrap())]
    Number(usize),
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident(&'a str),
    // match a double-quoted string including possible escapes, strip surrounding quotes
    #[regex(r#""([^"\\]|\\.)*""#, |lex| &lex.slice()[1..lex.slice().len()-1])]
    String(&'a str),

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Asterisk,
    #[token("/")]
    Slash,

    #[token("&")]
    And,

    #[token("==")]
    Eq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    Gte,
    #[token(">")]
    Gt,
    #[token("<=")]
    Lte,
    #[token("<")]
    Lt,

    #[token("=")]
    Assign,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,

    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,

    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Int => write!(f, "`int`"),
            Token::Char => write!(f, "`char`"),
            Token::If => write!(f, "`if`"),
            Token::Else => write!(f, "`else`"),
            Token::While => write!(f, "`while`"),
            Token::For => write!(f, "`for`"),
            Token::Sizeof => write!(f, "`sizeof`"),
            Token::Return => write!(f, "`return`"),
            Token::Semicolon => write!(f, "`;`"),
            Token::Comma => write!(f, "`,`"),
            Token::Number(value) => write!(f, "number `{value}`"),
            Token::Ident(name) => write!(f, "identifier `{name}`"),
            Token::String(inner) => write!(f, "stringLiteral `{inner}`"),
            Token::Plus => write!(f, "`+`"),
            Token::Minus => write!(f, "`-`"),
            Token::Asterisk => write!(f, "`*`"),
            Token::Slash => write!(f, "`/`"),
            Token::And => write!(f, "`&`"),
            Token::Eq => write!(f, "`==`"),
            Token::NotEq => write!(f, "`!=`"),
            Token::Gte => write!(f, "`>=`"),
            Token::Gt => write!(f, "`>`"),
            Token::Lte => write!(f, "`<=`"),
            Token::Lt => write!(f, "`<`"),
            Token::Assign => write!(f, "`=`"),
            Token::LParen => write!(f, "`(`"),
            Token::RParen => write!(f, "`)`"),
            Token::LBrace => write!(f, "`{{`"),
            Token::RBrace => write!(f, "`}}`"),
            Token::LBracket => write!(f, "`[`"),
            Token::RBracket => write!(f, "`]`"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Token;
    use logos::Logos;

    fn lex<'a>(source: &'a str) -> Vec<Token<'a>> {
        Token::lexer(source)
            .map(|token| token.unwrap_or_else(|err| panic!("lexer error: {}", err)))
            .collect()
    }

    #[test]
    fn lex_keywords_ident_number() {
        let tokens = lex("int x = 42;");
        assert_eq!(
            tokens,
            vec![
                Token::Int,
                Token::Ident("x"),
                Token::Assign,
                Token::Number(42),
                Token::Semicolon
            ]
        );
    }

    #[test]
    fn lex_operators_and_delimiters() {
        let tokens = lex("a==b!=c<=d>=e<f>g&(h+i);");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("a"),
                Token::Eq,
                Token::Ident("b"),
                Token::NotEq,
                Token::Ident("c"),
                Token::Lte,
                Token::Ident("d"),
                Token::Gte,
                Token::Ident("e"),
                Token::Lt,
                Token::Ident("f"),
                Token::Gt,
                Token::Ident("g"),
                Token::And,
                Token::LParen,
                Token::Ident("h"),
                Token::Plus,
                Token::Ident("i"),
                Token::RParen,
                Token::Semicolon
            ]
        );
    }

    #[test]
    fn lex_sizeof_keyword() {
        let tokens = lex("sizeof x;");
        assert_eq!(
            tokens,
            vec![Token::Sizeof, Token::Ident("x"), Token::Semicolon]
        );
    }
}
