use chumsky::error::Rich;
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use typed_arena::Arena;

use crate::error::ParseError;
use crate::lexer::Token;
use crate::types::Type;

#[derive(Debug)]
pub enum Tree<'a> {
    Program(Vec<&'a Tree<'a>>),
    BinOp(Op, &'a Tree<'a>, &'a Tree<'a>),
    Assign(&'a Tree<'a>, &'a Tree<'a>),
    Block(Vec<&'a Tree<'a>>),
    FuncDef(Type, &'a str, Vec<(Type, &'a str)>, &'a Tree<'a>),
    If(&'a Tree<'a>, &'a Tree<'a>, Option<&'a Tree<'a>>),
    While(&'a Tree<'a>, &'a Tree<'a>),
    For(
        Option<&'a Tree<'a>>,
        Option<&'a Tree<'a>>,
        Option<&'a Tree<'a>>,
        &'a Tree<'a>,
    ),

    Integer(i64),
    String(&'a str),
    Var(&'a str),
    Indexed(&'a Tree<'a>, &'a Tree<'a>),
    VarDeclare(Type, &'a str),
    Addr(&'a Tree<'a>),
    Deref(&'a Tree<'a>),

    Sizeof(&'a Tree<'a>),

    Call(&'a str, Vec<&'a Tree<'a>>),
    Return(&'a Tree<'a>),
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

type ParserErr<'src> = extra::Err<Rich<'src, Token<'src>, SimpleSpan>>;

fn expr_parser<'src, 'arena, I>(
    arena: &'arena Arena<Tree<'arena>>,
) -> impl Parser<'src, I, &Tree<'arena>, ParserErr<'src>> + 'arena + Clone
where
    I: Input<'src, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'arena,
{
    let ident_name = select! { Token::Ident(ident) => ident };
    let int_lit = select! { Token::Number(n) => &*arena.alloc(Tree::Integer(n as i64)) };
    let string_lit = select! { Token::String(s) => &*arena.alloc(Tree::String(s)) };

    recursive(|expr| {
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
            .map(|(name, args)| &*arena.alloc(Tree::Call(name, args.unwrap_or_default())));

        let array_index = just(Token::LBracket)
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RBracket));

        let primary_expr = choice((
            call_expr,
            int_lit,
            string_lit,
            ident_name.map(|name| &*arena.alloc(Tree::Var(name))),
            just(Token::LParen)
                .ignore_then(expr.clone())
                .then_ignore(just(Token::RParen)),
        ))
        .then(array_index.or_not())
        .map(|(prim, index)| match index {
            Some(i) => &*arena.alloc(Tree::Indexed(prim, i)),
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
                    .map(|expr| &*arena.alloc(Tree::Sizeof(expr))),
                unary_operator
                    .then(unary.clone())
                    .map(|(op, expr)| match op {
                        UnaryOp::Plus => expr,
                        UnaryOp::Minus => {
                            &*arena.alloc(Tree::BinOp(Op::Sub, &Tree::Integer(0), expr))
                        }
                        UnaryOp::Addr => &*arena.alloc(Tree::Addr(expr)),
                        UnaryOp::Deref => &*arena.alloc(Tree::Deref(expr)),
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
                &*arena.alloc(Tree::BinOp(op, &lhs, &rhs))
            });

        let add_expr = mul_expr
            .clone()
            .foldl(add_op.then(mul_expr).repeated(), |lhs, (op, rhs)| {
                &*arena.alloc(Tree::BinOp(op, &lhs, &rhs))
            });

        let relational_expr = add_expr
            .clone()
            .foldl(relational_op.then(add_expr).repeated(), |lhs, (op, rhs)| {
                &*arena.alloc(Tree::BinOp(op, &lhs, &rhs))
            });

        let equality_expr = relational_expr.clone().foldl(
            equality_op.then(relational_expr).repeated(),
            |lhs, (op, rhs)| &*arena.alloc(Tree::BinOp(op, &lhs, &rhs)),
        );

        let assign_rhs = just(Token::Assign).ignore_then(expr.clone());

        equality_expr
            .clone()
            .then(assign_rhs.or_not())
            .map(|(lhs, rhs)| match rhs {
                Some(rhs) => &*arena.alloc(Tree::Assign(lhs, rhs)),
                None => lhs,
            })
    })
}

fn type_parser<'src, I>() -> impl Parser<'src, I, Type, ParserErr<'src>> + Clone
where
    I: Input<'src, Token = Token<'src>, Span = SimpleSpan>,
{
    let base_type = choice((
        just::<Token<'_>, I, ParserErr<'src>>(Token::Int).to(Type::Int),
        just(Token::Char).to(Type::Char),
    ));

    base_type
        .then(just(Token::Asterisk).repeated().collect::<Vec<_>>())
        .map(|(ty, stars)| stars.iter().fold(ty, |ty, _| Type::Ptr(Box::new(ty))))
}

fn stmt_parser<'src, 'arena, I>(
    arena: &'arena Arena<Tree<'arena>>,
) -> impl Parser<'src, I, &Tree<'arena>, ParserErr<'src>> + 'arena + Clone
where
    I: Input<'src, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'arena,
{
    let ident_name = select! { Token::Ident(ident) => ident };

    let array_size = just::<Token<'_>, I, ParserErr<'src>>(Token::LBracket)
        .ignore_then(select! { Token::Number(n) => n })
        .then_ignore(just(Token::RBracket))
        .or_not();

    let var_decl = type_parser()
        .then(ident_name)
        .then(array_size)
        .then_ignore(just(Token::Semicolon))
        .map(|((ty, name), size)| {
            let ty = match size {
                Some(size) => Type::Array(Box::new(ty), size),
                None => ty,
            };
            &*arena.alloc(Tree::VarDeclare(ty, name))
        });

    recursive(|stmt| {
        let block_stmt = just(Token::LBrace)
            .ignore_then(
                choice((stmt.clone(), var_decl.clone()))
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::RBrace))
            .map(|stmts: Vec<&Tree<'arena>>| {
                let stmts_ref = stmts
                    .iter()
                    .map(|s| *s as &Tree<'arena>)
                    .collect::<Vec<_>>();
                &*arena.alloc(Tree::Block(stmts_ref))
            });

        let return_stmt = just(Token::Return)
            .ignore_then(expr_parser(arena))
            .then_ignore(just(Token::Semicolon))
            .map(|expr| &*arena.alloc(Tree::Return(&expr)));

        let if_stmt = just(Token::If)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser(arena))
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .then(just(Token::Else).ignore_then(stmt.clone()).or_not())
            .map(|((cond, then), other)| &*arena.alloc(Tree::If(cond, then, other)));

        let while_stmt = just(Token::While)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser(arena))
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .map(|(cond, body)| &*arena.alloc(Tree::While(cond, body)));

        let for_stmt = just(Token::For)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser(arena).or_not())
            .then_ignore(just(Token::Semicolon))
            .then(expr_parser(arena).or_not())
            .then_ignore(just(Token::Semicolon))
            .then(expr_parser(arena).or_not())
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .map(|(((init, cond), update), body)| {
                &*arena.alloc(Tree::For(init, cond, update, body))
            });

        let expr_stmt = expr_parser(arena).then_ignore(just(Token::Semicolon));

        choice((
            block_stmt,
            if_stmt,
            while_stmt,
            for_stmt,
            return_stmt,
            expr_stmt,
        ))
    })
}

fn parser<'src, 'arena, I>(
    arena: &'arena Arena<Tree<'arena>>,
) -> impl Parser<'src, I, &Tree<'arena>, ParserErr<'src>> + 'arena + Clone
where
    I: Input<'src, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'arena,
{
    let ident_name = select! { Token::Ident(ident) => ident };

    let param_name = type_parser().then(ident_name);
    let param_list = param_name
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>();

    let array_size = just::<Token<'_>, I, ParserErr<'src>>(Token::LBracket)
        .ignore_then(select! { Token::Number(n) => n })
        .then_ignore(just(Token::RBracket))
        .or_not();

    let var_decl = type_parser()
        .then(ident_name)
        .then(array_size)
        .then_ignore(just(Token::Semicolon))
        .map(|((ty, name), size)| {
            let ty = match size {
                Some(size) => Type::Array(Box::new(ty), size),
                None => ty,
            };
            &*arena.alloc(Tree::VarDeclare(ty, name))
        });

    let func_def = type_parser()
        .then(ident_name)
        .then(
            just(Token::LParen)
                .ignore_then(param_list.or_not())
                .then_ignore(just(Token::RParen)),
        )
        .then(
            just(Token::LBrace)
                .ignore_then(
                    choice((stmt_parser(arena), var_decl.clone()))
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(just(Token::RBrace))
                .map(|stmts: Vec<&Tree<'arena>>| {
                    let stmts_ref = stmts
                        .iter()
                        .map(|s| *s as &Tree<'arena>)
                        .collect::<Vec<_>>();
                    &*arena.alloc(Tree::Block(stmts_ref))
                }),
        )
        .map(|(((ty, name), params), body)| {
            &*arena.alloc(Tree::FuncDef(ty, name, params.unwrap(), body))
        });

    choice((func_def, var_decl, stmt_parser(arena)))
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map(|items| &*arena.alloc(Tree::Program(items)))
}

pub fn parse<'src, 'arena>(
    arena: &'arena Arena<Tree<'arena>>,
    tokens: &'src [(Token<'src>, SimpleSpan)],
    eoi: SimpleSpan,
) -> Result<&'arena Tree<'arena>, Vec<ParseError>> 
where 
    'src: 'arena,
{
    let input = tokens.split_token_span::<Token<'src>, _>(eoi);
    
    let parser = parser(arena);
    match parser.parse(input).into_result() {
        Ok(tree) => Ok(tree),
        Err(errors) => Err(errors.into_iter().map(ParseError::from_rich).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Op, Tree};
    use crate::types::Type;

    use typed_arena::Arena;

    fn parse_one<'arena>(
        arena: &'arena Arena<Tree<'arena>>,
        source: &'arena str,
    ) -> &'arena Tree<'arena> {
        match parse(arena, source).unwrap() {
            Tree::Program(trees) => {
                assert_eq!(trees.len(), 1, "expected one top-level item");
                trees[0]
            }
            _ => panic!("top-level tree must be Program"),
        }
    }

    fn parse_func<'arena>(
        arena: &'arena Arena<Tree<'arena>>,
        source: &'arena str,
    ) -> (
        Type,
        &'arena str,
        Vec<(Type, &'arena str)>,
        &'arena Tree<'arena>,
    ) {
        match parse_one(arena, source) {
            Tree::FuncDef(ty, name, params, body) => (ty.clone(), *name, params.clone(), body),
            _ => panic!("expected function definition"),
        }
    }

    fn expect_block<'arena>(tree: &'arena Tree<'arena>) -> &'arena Vec<&'arena Tree<'arena>> {
        match tree {
            Tree::Block(stmts) => stmts,
            _ => panic!("expected block body"),
        }
    }

    #[test]
    fn parse_respects_precedence() {
        let arena = Arena::new();
        let (ty, name, params, body) = parse_func(&arena, "int main(){ return 1+2*3; }");
        assert_eq!(ty, Type::Int);
        assert_eq!(*name, "main");
        assert!(params.is_empty());
        let stmts = expect_block(body);
        assert_eq!(stmts.len(), 1);
        match stmts[0] {
            Tree::Return(expr) => match expr {
                Tree::BinOp(Op::Add, lhs, rhs) => {
                    assert!(matches!(lhs, Tree::Integer(1)));
                    match rhs {
                        Tree::BinOp(Op::Mul, mul_lhs, mul_rhs) => {
                            assert!(matches!(mul_lhs, Tree::Integer(2)));
                            assert!(matches!(mul_rhs, Tree::Integer(3)));
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
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ if (1) return 2; else return 3; }");
        let stmts = expect_block(body);
        match stmts[0] {
            Tree::If(cond, then_branch, else_branch) => {
                assert!(matches!(cond, Tree::Integer(1)));
                assert!(matches!(then_branch, Tree::Return(_)));
                assert!(else_branch.is_some());
            }
            _ => panic!("expected if stmt"),
        }
    }

    #[test]
    fn parse_while_and_for() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(
            &arena,
            "int main(){ int i; while(i) i=i-1; for(i=0;i<3;i=i+1) i; }",
        );
        let stmts = expect_block(body);
        assert_eq!(stmts.len(), 3);
        assert!(matches!(stmts[0], Tree::VarDeclare(_, _)));
        assert!(matches!(stmts[1], Tree::While(_, _)));
        assert!(matches!(stmts[2], Tree::For(_, _, _, _)));
    }

    #[test]
    fn parse_array_decl() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ int a[10]; }");
        let stmts = expect_block(body);

        match stmts[0] {
            Tree::VarDeclare(ty, name) => {
                assert_eq!(*name, "a");
                assert_eq!(*ty, Type::Array(Box::new(Type::Int), 10));
            }
            _ => panic!("expected array declaration"),
        }
    }

    #[test]
    fn parse_array_index_expr() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ return a[3]; }");
        let stmts = expect_block(body);

        match stmts[0] {
            Tree::Return(expr) => match expr {
                Tree::Indexed(inner, index) => {
                    assert!(matches!(index, Tree::Integer(3)));
                    assert!(matches!(inner, Tree::Var(name) if *name == "a"));
                }
                _ => panic!("expected indexed expression"),
            },
            _ => panic!("expected return statement"),
        }
    }

    #[test]
    fn parse_array_index_assignment() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ a[1] = 2; }");
        let stmts = expect_block(body);

        match stmts[0] {
            Tree::Assign(lhs, rhs) => {
                match lhs {
                    Tree::Indexed(inner, index) => {
                        assert!(matches!(index, Tree::Integer(1)));
                        assert!(matches!(inner, Tree::Var(name) if *name == "a"));
                    }
                    _ => panic!("expected indexed lvalue"),
                }
                assert!(matches!(rhs, Tree::Integer(2)));
            }
            _ => panic!("expected assignment statement"),
        }
    }

    #[test]
    fn parse_call_and_unary() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ return *&foo(1,2); }");
        let stmts = expect_block(body);
        match stmts[0] {
            Tree::Return(expr) => match expr {
                Tree::Deref(inner) => match inner {
                    Tree::Addr(call) => match call {
                        Tree::Call(name, args) => {
                            assert_eq!(*name, "foo");
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
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ a=b=1; }");
        let stmts = expect_block(body);
        match stmts[0] {
            Tree::Assign(lhs, rhs) => {
                assert!(matches!(lhs, Tree::Var(name) if *name == "a"));
                match rhs {
                    Tree::Assign(inner_lhs, inner_rhs) => {
                        assert!(matches!(inner_lhs, Tree::Var(name) if *name == "b"));
                        assert!(matches!(inner_rhs, Tree::Integer(1)));
                    }
                    _ => panic!("expected nested assignment"),
                }
            }
            _ => panic!("expected assignment stmt"),
        }
    }

    #[test]
    fn parse_relational_then_equality() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ return 1 < 2 == 0; }");
        let stmts = expect_block(body);
        match stmts[0] {
            Tree::Return(expr) => match expr {
                Tree::BinOp(Op::Eq, lhs, rhs) => {
                    assert!(matches!(rhs, Tree::Integer(0)));
                    match lhs {
                        Tree::BinOp(Op::Lt, rel_lhs, rel_rhs) => {
                            assert!(matches!(rel_lhs, Tree::Integer(1)));
                            assert!(matches!(rel_rhs, Tree::Integer(2)));
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
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ for(;;) return 1; }");
        let stmts = expect_block(body);
        match stmts[0] {
            Tree::For(init, cond, update, body) => {
                assert!(init.is_none());
                assert!(cond.is_none());
                assert!(update.is_none());
                assert!(matches!(body, Tree::Return(_)));
            }
            _ => panic!("expected for stmt"),
        }
    }

    #[test]
    fn parse_sizeof_unary() {
        let arena = Arena::new();
        let (_, _, _, body) = parse_func(&arena, "int main(){ return sizeof 1 + sizeof x; }");
        let stmts = expect_block(body);
        match stmts[0] {
            Tree::Return(expr) => match expr {
                Tree::BinOp(Op::Add, lhs, rhs) => {
                    assert!(matches!(lhs, Tree::Sizeof(_)));
                    assert!(matches!(rhs, Tree::Sizeof(_)));
                }
                _ => panic!("expected add in return"),
            },
            _ => panic!("expected return stmt"),
        }
    }

    #[test]
    fn parse_global_decl() {
        let arena = Arena::new();
        let tree = parse_one(&arena, "int g;");
        match tree {
            Tree::VarDeclare(ty, name) => {
                assert_eq!(*ty, Type::Int);
                assert_eq!(*name, "g");
            }
            _ => panic!("expected global var declaration"),
        }
    }

    #[test]
    fn parse_program_with_global_and_func() {
        let arena = Arena::new();
        let tree = parse(&arena, "int g; int main(){ return g; }").unwrap();
        match tree {
            Tree::Program(trees) => {
                assert_eq!(trees.len(), 2);
                match trees[0] {
                    Tree::VarDeclare(ty, name) => {
                        assert_eq!(*ty, Type::Int);
                        assert_eq!(*name, "g");
                    }
                    _ => panic!("expected first top-level to be var declare"),
                }
                match trees[1] {
                    Tree::FuncDef(_, name, _, _) => assert_eq!(*name, "main"),
                    _ => panic!("expected second top-level to be function"),
                }
            }
            _ => panic!("expected program"),
        }
    }
}
