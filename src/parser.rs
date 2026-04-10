use chumsky::error::Rich;
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use logos::Logos;

use crate::error::ParseError;
use crate::lexer::Token;
use crate::types::Type;

#[derive(Debug, Clone)]
pub enum Tree {
    Program(Vec<Tree>),
    BinOp(Op, Box<Tree>, Box<Tree>),
    Assign(Box<Tree>, Box<Tree>),
    Block(Vec<Tree>),
    FuncDef(Type, String, Vec<(Type, String)>, Box<Tree>),
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
    Indexed(Box<Tree>, Box<Tree>),
    VarDeclare(Type, String),
    Addr(Box<Tree>),
    Deref(Box<Tree>),

    Sizeof(Box<Tree>),

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

fn parser<'a, I>() -> impl Parser<'a, I, Tree, extra::Err<Rich<'a, Token<'a>, SimpleSpan>>>
where
    I: Input<'a, Token = Token<'a>, Span = SimpleSpan>,
{
    let ident_name = select! { Token::Ident(ident) => ident.to_string() };
    let int_lit = select! { Token::Number(n) => Tree::Integer(n as i64) };

    let type_parser = just(Token::Int)
        .to(Type::Int)
        .foldl(just(Token::Asterisk).repeated(), |acc, _| {
            Type::Ptr(Box::new(acc))
        });

    let array_size = just(Token::LBracket)
        .ignore_then(select! { Token::Number(n) => n })
        .then_ignore(just(Token::RBracket))
        .or_not();

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

        let array_index = just(Token::LBracket)
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RBracket));

        let primary_expr = choice((
            call_expr,
            int_lit,
            ident_name.map(|name| Tree::Var(name)),
            just(Token::LParen)
                .ignore_then(expr.clone())
                .then_ignore(just(Token::RParen)),
        ))
        .then(array_index.or_not())
        .map(|(prim, index)| match index {
            Some(i) => Tree::Indexed(Box::new(prim), Box::new(i)),
            None => prim,
        });

        let unary_operator = choice((
            just(Token::Plus).to(UnaryOp::Plus),
            just(Token::Minus).to(UnaryOp::Minus),
            just(Token::And).to(UnaryOp::Addr),
            just(Token::Asterisk).to(UnaryOp::Deref),
        ));

        let unary_expr = recursive(|unary| {
            choice((
                just(Token::Sizeof)
                    .ignore_then(unary.clone())
                    .map(|expr| Tree::Sizeof(Box::new(expr))),
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
                    .or(primary_expr.clone()),
            ))
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

    let var_decl = type_parser
        .clone()
        .then(ident_name)
        .then(array_size)
        .then_ignore(just(Token::Semicolon))
        .map(|((ty, name), size)| {
            let ty = match size {
                Some(size) => Type::Array(Box::new(ty), size),
                None => ty,
            };
            Tree::VarDeclare(ty, name)
        });

    let stmt_parser = recursive(|stmt| {
        let block_stmt = just(Token::LBrace)
            .ignore_then(
                choice((stmt.clone(), var_decl.clone()))
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

    let param_name = type_parser.clone().then(ident_name);
    let param_list = param_name
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>();

    let func_def = type_parser
        .clone()
        .then(ident_name)
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
        .map(|(((ty, name), params), body)| {
            Tree::FuncDef(ty, name, params.unwrap(), Box::new(body))
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
    use super::{parse, Op, Tree};
    use crate::types::Type;

    fn parse_one(source: &str) -> Tree {
        match parse(source).unwrap() {
            Tree::Program(trees) => {
                assert_eq!(trees.len(), 1, "expected one top-level item");
                trees.into_iter().next().unwrap()
            }
            _ => panic!("top-level tree must be Program"),
        }
    }

    fn parse_func(source: &str) -> (Type, String, Vec<(Type, String)>, Tree) {
        match parse_one(source) {
            Tree::FuncDef(ty, name, params, body) => (ty, name, params, *body),
            _ => panic!("expected function definition"),
        }
    }

    fn expect_block(tree: &Tree) -> &Vec<Tree> {
        match tree {
            Tree::Block(stmts) => stmts,
            _ => panic!("expected block body"),
        }
    }

    #[test]
    fn parse_respects_precedence() {
        let (ty, name, params, body) = parse_func("int main(){ return 1+2*3; }");
        assert_eq!(ty, Type::Int);
        assert_eq!(name, "main");
        assert!(params.is_empty());
        let stmts = expect_block(&body);
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

    #[test]
    fn parse_if_else_block() {
        let (_, _, _, body) = parse_func("int main(){ if (1) return 2; else return 3; }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::If(cond, then_branch, else_branch) => {
                assert!(matches!(**cond, Tree::Integer(1)));
                assert!(matches!(**then_branch, Tree::Return(_)));
                assert!(else_branch.is_some());
            }
            _ => panic!("expected if stmt"),
        }
    }

    #[test]
    fn parse_while_and_for() {
        let (_, _, _, body) =
            parse_func("int main(){ int i; while(i) i=i-1; for(i=0;i<3;i=i+1) i; }");
        let stmts = expect_block(&body);
        assert_eq!(stmts.len(), 3);
        assert!(matches!(stmts[0], Tree::VarDeclare(_, _)));
        assert!(matches!(stmts[1], Tree::While(_, _)));
        assert!(matches!(stmts[2], Tree::For(_, _, _, _)));
    }

    #[test]
    fn parse_array_decl() {
        let (_, _, _, body) = parse_func("int main(){ int a[10]; }");
        let stmts = expect_block(&body);

        match &stmts[0] {
            Tree::VarDeclare(ty, name) => {
                assert_eq!(name, "a");
                assert_eq!(ty, &Type::Array(Box::new(Type::Int), 10));
            }
            _ => panic!("expected array declaration"),
        }
    }

    #[test]
    fn parse_array_index_expr() {
        let (_, _, _, body) = parse_func("int main(){ return a[3]; }");
        let stmts = expect_block(&body);

        match &stmts[0] {
            Tree::Return(expr) => match &**expr {
                Tree::Indexed(inner, index) => {
                    assert!(matches!(**index, Tree::Integer(3)));
                    assert!(matches!(**inner, Tree::Var(ref name) if name == "a"));
                }
                _ => panic!("expected indexed expression"),
            },
            _ => panic!("expected return statement"),
        }
    }

    #[test]
    fn parse_array_index_assignment() {
        let (_, _, _, body) = parse_func("int main(){ a[1] = 2; }");
        let stmts = expect_block(&body);

        match &stmts[0] {
            Tree::Assign(lhs, rhs) => {
                match &**lhs {
                    Tree::Indexed(inner, index) => {
                        assert!(matches!(**index, Tree::Integer(1)));
                        assert!(matches!(**inner, Tree::Var(ref name) if name == "a"));
                    }
                    _ => panic!("expected indexed lvalue"),
                }
                assert!(matches!(**rhs, Tree::Integer(2)));
            }
            _ => panic!("expected assignment statement"),
        }
    }

    #[test]
    fn parse_call_and_unary() {
        let (_, _, _, body) = parse_func("int main(){ return *&foo(1,2); }");
        let stmts = expect_block(&body);
        match &stmts[0] {
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
        }
    }

    #[test]
    fn parse_assignment_right_associative() {
        let (_, _, _, body) = parse_func("int main(){ a=b=1; }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::Assign(lhs, rhs) => {
                assert!(matches!(**lhs, Tree::Var(ref name) if name == "a"));
                match &**rhs {
                    Tree::Assign(inner_lhs, inner_rhs) => {
                        assert!(matches!(**inner_lhs, Tree::Var(ref name) if name == "b"));
                        assert!(matches!(**inner_rhs, Tree::Integer(1)));
                    }
                    _ => panic!("expected nested assignment"),
                }
            }
            _ => panic!("expected assignment stmt"),
        }
    }

    #[test]
    fn parse_relational_then_equality() {
        let (_, _, _, body) = parse_func("int main(){ return 1 < 2 == 0; }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::Return(expr) => match &**expr {
                Tree::BinOp(Op::Eq, lhs, rhs) => {
                    assert!(matches!(**rhs, Tree::Integer(0)));
                    match &**lhs {
                        Tree::BinOp(Op::Lt, rel_lhs, rel_rhs) => {
                            assert!(matches!(**rel_lhs, Tree::Integer(1)));
                            assert!(matches!(**rel_rhs, Tree::Integer(2)));
                        }
                        _ => panic!("expected relational in lhs"),
                    }
                }
                _ => panic!("expected equality in return"),
            },
            _ => panic!("expected return stmt"),
        }
    }

    #[test]
    fn parse_for_with_empty_clauses() {
        let (_, _, _, body) = parse_func("int main(){ for(;;) return 1; }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::For(init, cond, update, body) => {
                assert!(init.is_none());
                assert!(cond.is_none());
                assert!(update.is_none());
                assert!(matches!(**body, Tree::Return(_)));
            }
            _ => panic!("expected for stmt"),
        }
    }

    #[test]
    fn parse_sizeof_unary() {
        let (_, _, _, body) = parse_func("int main(){ return sizeof 1 + sizeof x; }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::Return(expr) => match &**expr {
                Tree::BinOp(Op::Add, lhs, rhs) => {
                    assert!(matches!(**lhs, Tree::Sizeof(_)));
                    assert!(matches!(**rhs, Tree::Sizeof(_)));
                }
                _ => panic!("expected add in return"),
            },
            _ => panic!("expected return stmt"),
        }
    }
}
