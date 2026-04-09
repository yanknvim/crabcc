use chumsky::error::Rich;
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use logos::Logos;

use crate::error::ParseError;
use crate::lexer::Token;

#[derive(Debug, Clone)]
pub enum Tree {
    Program(Vec<Tree>),
    BinOp(Op, Box<Tree>, Box<Tree>),
    Assign(Box<Tree>, Box<Tree>),
    Block(Vec<Tree>),
    FuncDef(String, Vec<String>, Box<Tree>),
    If(Box<Tree>, Box<Tree>, Option<Box<Tree>>),
    While(Box<Tree>, Box<Tree>),
    For(
        Option<Box<Tree>>,
        Option<Box<Tree>>,
        Option<Box<Tree>>,
        Box<Tree>,
    ),

    Integer(i64),
    Var(String),
    VarDeclare(String),
    Addr(Box<Tree>),
    Deref(Box<Tree>),

    Call(String, Vec<Tree>),
    Return(Box<Tree>),
}

#[derive(Debug, Clone)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone, Copy)]
enum UnaryOp {
    Plus,
    Minus,
    Addr,
    Deref,
}

impl Tree {
    pub fn children(&self) -> Box<dyn Iterator<Item = &Tree> + '_> {
        match self {
            Tree::Assign(lhs, rhs) => Box::new([lhs.as_ref(), rhs.as_ref()].into_iter()),
            Tree::Return(rhs) => Box::new(std::iter::once(rhs.as_ref())),
            Tree::BinOp(_, lhs, rhs) => Box::new([lhs.as_ref(), rhs.as_ref()].into_iter()),
            Tree::Program(trees) => Box::new(trees.iter()),
            Tree::Block(stmts) => Box::new(stmts.iter()),
            Tree::If(cond, a, Some(b)) => {
                Box::new([cond.as_ref(), a.as_ref(), b.as_ref()].into_iter())
            }
            Tree::If(cond, a, None) => Box::new([cond.as_ref(), a.as_ref()].into_iter()),
            Tree::While(cond, stmt) => Box::new([cond.as_ref(), stmt.as_ref()].into_iter()),
            Tree::For(init, cond, update, stmt) => Box::new(
                init.as_deref()
                    .into_iter()
                    .chain(cond.as_deref())
                    .chain(update.as_deref())
                    .chain(std::iter::once(stmt.as_ref())),
            ),
            Tree::FuncDef(_, _, body) => Box::new(std::iter::once(body.as_ref())),
            Tree::Call(_, args) => Box::new(args.iter()),
            _ => Box::new(std::iter::empty()),
        }
    }
}

fn parser<'a, I>() -> impl Parser<'a, I, Tree, extra::Err<Rich<'a, Token<'a>, SimpleSpan>>>
where
    I: Input<'a, Token = Token<'a>, Span = SimpleSpan>,
{
    let ident_name = select! { Token::Ident(ident) => ident.to_string() };
    let int_lit = select! { Token::Number(n) => Tree::Integer(n as i64) };

    let expr_parser = recursive(|expr| {
        let call_expr = ident_name
            .then(
                just(Token::LParen)
                    .ignore_then(
                        expr.clone()
                            .separated_by(just(Token::Comma))
                            .collect::<Vec<_>>()
                            .or_not(),
                    )
                    .then_ignore(just(Token::RParen)),
            )
            .map(|(name, args)| Tree::Call(name, args.unwrap_or_default()));

        let primary_expr = choice((
            call_expr,
            int_lit,
            ident_name.map(Tree::Var),
            just(Token::LParen)
                .ignore_then(expr.clone())
                .then_ignore(just(Token::RParen)),
        ));

        let unary_operator = choice((
            just(Token::Plus).to(UnaryOp::Plus),
            just(Token::Minus).to(UnaryOp::Minus),
            just(Token::And).to(UnaryOp::Addr),
            just(Token::Asterisk).to(UnaryOp::Deref),
        ));

        let unary_expr = recursive(|unary| {
            unary_operator
                .then(unary.clone())
                .map(|(op, expr)| match op {
                    UnaryOp::Plus => expr,
                    UnaryOp::Minus => {
                        Tree::BinOp(Op::Sub, Box::new(Tree::Integer(0)), Box::new(expr))
                    }
                    UnaryOp::Addr => Tree::Addr(Box::new(expr)),
                    UnaryOp::Deref => Tree::Deref(Box::new(expr)),
                })
                .or(primary_expr.clone())
        });

        let mul_op = choice((
            just(Token::Asterisk).to(Op::Mul),
            just(Token::Slash).to(Op::Div),
        ));
        let add_op = choice((
            just(Token::Plus).to(Op::Add),
            just(Token::Minus).to(Op::Sub),
        ));
        let relational_op = choice((
            just(Token::Gte).to(Op::Gte),
            just(Token::Gt).to(Op::Gt),
            just(Token::Lte).to(Op::Lte),
            just(Token::Lt).to(Op::Lt),
        ));
        let equality_op = choice((just(Token::Eq).to(Op::Eq), just(Token::NotEq).to(Op::NotEq)));

        let mul_expr = unary_expr
            .clone()
            .foldl(mul_op.then(unary_expr).repeated(), |lhs, (op, rhs)| {
                Tree::BinOp(op, Box::new(lhs), Box::new(rhs))
            });

        let add_expr = mul_expr
            .clone()
            .foldl(add_op.then(mul_expr).repeated(), |lhs, (op, rhs)| {
                Tree::BinOp(op, Box::new(lhs), Box::new(rhs))
            });

        let relational_expr = add_expr
            .clone()
            .foldl(relational_op.then(add_expr).repeated(), |lhs, (op, rhs)| {
                Tree::BinOp(op, Box::new(lhs), Box::new(rhs))
            });

        let equality_expr = relational_expr.clone().foldl(
            equality_op.then(relational_expr).repeated(),
            |lhs, (op, rhs)| Tree::BinOp(op, Box::new(lhs), Box::new(rhs)),
        );

        let assign_rhs = just(Token::Assign).ignore_then(expr.clone());

        equality_expr
            .clone()
            .then(assign_rhs.or_not())
            .map(|(lhs, rhs)| match rhs {
                Some(rhs) => Tree::Assign(Box::new(lhs), Box::new(rhs)),
                None => lhs,
            })
    });

    let var_decl = just(Token::Int)
        .ignore_then(ident_name)
        .then_ignore(just(Token::Semicolon))
        .map(Tree::VarDeclare);

    let stmt_parser = recursive(|stmt| {
        let block_stmt = just(Token::LBrace)
            .ignore_then(
                choice((stmt.clone(), var_decl))
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::RBrace))
            .map(Tree::Block);

        let return_stmt = just(Token::Return)
            .ignore_then(expr_parser.clone())
            .then_ignore(just(Token::Semicolon))
            .map(|expr| Tree::Return(Box::new(expr)));

        let if_stmt = just(Token::If)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser.clone())
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .then(just(Token::Else).ignore_then(stmt.clone()).or_not())
            .map(|((cond, then), other)| {
                Tree::If(Box::new(cond), Box::new(then), other.map(Box::new))
            });

        let while_stmt = just(Token::While)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser.clone())
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .map(|(cond, body)| Tree::While(Box::new(cond), Box::new(body)));

        let for_stmt = just(Token::For)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser.clone().or_not())
            .then_ignore(just(Token::Semicolon))
            .then(expr_parser.clone().or_not())
            .then_ignore(just(Token::Semicolon))
            .then(expr_parser.clone().or_not())
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .map(|(((init, cond), update), body)| {
                Tree::For(
                    init.map(Box::new),
                    cond.map(Box::new),
                    update.map(Box::new),
                    Box::new(body),
                )
            });

        let expr_stmt = expr_parser.clone().then_ignore(just(Token::Semicolon));

        choice((
            block_stmt,
            if_stmt,
            while_stmt,
            for_stmt,
            return_stmt,
            expr_stmt,
        ))
    });

    let param_name = just(Token::Int).ignore_then(ident_name);
    let param_list = param_name
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>();

    let func_def = just(Token::Int)
        .ignore_then(ident_name)
        .then(
            just(Token::LParen)
                .ignore_then(param_list.or_not())
                .then_ignore(just(Token::RParen)),
        )
        .then(
            just(Token::LBrace)
                .ignore_then(
                    choice((stmt_parser.clone(), var_decl))
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(just(Token::RBrace))
                .map(Tree::Block),
        )
        .map(|((name, params), body)| {
            Tree::FuncDef(name, params.unwrap_or_default(), Box::new(body))
        });

    choice((func_def, stmt_parser))
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map(Tree::Program)
}

pub fn parse(source: &str) -> Result<Tree, Vec<ParseError>> {
    let lexer = Token::lexer(source);
    let tokens: Vec<(Token<'_>, SimpleSpan)> = lexer
        .spanned()
        .map(|(token, span)| {
            let token = match token {
                Ok(token) => token,
                Err(err) => panic!("lexer error: {}", err),
            };
            (token, SimpleSpan::from(span))
        })
        .collect();

    let eoi = SimpleSpan::from(source.len()..source.len());
    let input = tokens.as_slice().split_token_span(eoi);

    match parser().parse(input).into_result() {
        Ok(tree) => Ok(tree),
        Err(errors) => Err(errors.into_iter().map(ParseError::from_rich).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Op, Tree, parse};

    fn parse_one(source: &str) -> Tree {
        match parse(source).unwrap() {
            Tree::Program(trees) => {
                assert_eq!(trees.len(), 1, "expected one top-level item");
                trees.into_iter().next().unwrap()
            }
            _ => panic!("top-level tree must be Program"),
        }
    }

    #[test]
    fn parse_respects_precedence() {
        let tree = parse_one("int main(){ return 1+2*3; }");
        match tree {
            Tree::FuncDef(name, params, body) => {
                assert_eq!(name, "main");
                assert!(params.is_empty());
                match *body {
                    Tree::Block(stmts) => {
                        assert_eq!(stmts.len(), 1);
                        match &stmts[0] {
                            Tree::Return(expr) => match &**expr {
                                Tree::BinOp(Op::Add, lhs, rhs) => {
                                    assert!(matches!(**lhs, Tree::Integer(1)));
                                    match &**rhs {
                                        Tree::BinOp(Op::Mul, mul_lhs, mul_rhs) => {
                                            assert!(matches!(**mul_lhs, Tree::Integer(2)));
                                            assert!(matches!(**mul_rhs, Tree::Integer(3)));
                                        }
                                        _ => panic!("expected multiply in rhs"),
                                    }
                                }
                                _ => panic!("expected add in return"),
                            },
                            _ => panic!("expected return stmt"),
                        }
                    }
                    _ => panic!("expected block body"),
                }
            }
            _ => panic!("expected function definition"),
        }
    }

    #[test]
    fn parse_if_else_block() {
        let tree = parse_one("int main(){ if (1) return 2; else return 3; }");
        match tree {
            Tree::FuncDef(_, _, body) => match *body {
                Tree::Block(stmts) => match &stmts[0] {
                    Tree::If(cond, then_branch, else_branch) => {
                        assert!(matches!(**cond, Tree::Integer(1)));
                        assert!(matches!(**then_branch, Tree::Return(_)));
                        assert!(else_branch.is_some());
                    }
                    _ => panic!("expected if stmt"),
                },
                _ => panic!("expected block body"),
            },
            _ => panic!("expected function definition"),
        }
    }

    #[test]
    fn parse_while_and_for() {
        let tree = parse_one("int main(){ int i; while(i) i=i-1; for(i=0;i<3;i=i+1) i; }");
        match tree {
            Tree::FuncDef(_, _, body) => match *body {
                Tree::Block(stmts) => {
                    assert_eq!(stmts.len(), 3);
                    assert!(matches!(stmts[0], Tree::VarDeclare(_)));
                    assert!(matches!(stmts[1], Tree::While(_, _)));
                    assert!(matches!(stmts[2], Tree::For(_, _, _, _)));
                }
                _ => panic!("expected block body"),
            },
            _ => panic!("expected function definition"),
        }
    }

    #[test]
    fn parse_call_and_unary() {
        let tree = parse_one("int main(){ return *&foo(1,2); }");
        match tree {
            Tree::FuncDef(_, _, body) => match *body {
                Tree::Block(stmts) => match &stmts[0] {
                    Tree::Return(expr) => match &**expr {
                        Tree::Deref(inner) => match &**inner {
                            Tree::Addr(call) => match &**call {
                                Tree::Call(name, args) => {
                                    assert_eq!(name, "foo");
                                    assert_eq!(args.len(), 2);
                                }
                                _ => panic!("expected call"),
                            },
                            _ => panic!("expected addr"),
                        },
                        _ => panic!("expected deref"),
                    },
                    _ => panic!("expected return stmt"),
                },
                _ => panic!("expected block body"),
            },
            _ => panic!("expected function definition"),
        }
    }
}
