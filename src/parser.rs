use chumsky::error::Rich;
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use logos::Logos;

use crate::error::ParseError;
use crate::lexer::Token;
use crate::types::Type;

pub trait Phase {
    type XBinOp;
    type XAssign;

    type XInteger;
    type XStringLiteral;
    type XVar;

    type XAddr;
    type XDeref;
    type XIndexed;
    type XCall;
    type XReturn;
}

#[derive(Clone, Debug)]
pub struct Parsed;

#[derive(Clone, Debug)]
pub struct Typed;

#[derive(Clone, Debug)]
pub struct Lowered;

impl Phase for Parsed {
    type XBinOp = ();
    type XAssign = ();

    type XInteger = ();
    type XStringLiteral = ();
    type XVar = ();

    type XAddr = ();
    type XDeref = ();
    type XIndexed = ();
    type XCall = ();
    type XReturn = ();
}

impl Phase for Typed {
    type XBinOp = Type;
    type XAssign = Type;

    type XInteger = Type;
    type XStringLiteral = Type;
    type XVar = Type;

    type XAddr = Type;
    type XDeref = Type;
    type XIndexed = Type;
    type XCall = Type;
    type XReturn = Type;
}

impl Phase for Lowered {
    type XBinOp = Type;
    type XAssign = Type;

    type XInteger = Type;
    type XStringLiteral = Type;
    type XVar = Type;

    type XAddr = Type;
    type XDeref = Type;
    type XIndexed = Type;
    type XCall = Type;
    type XReturn = Type;
}

pub trait TypedTree {
    fn ty(&self) -> &Type;
}

impl TypedTree for Tree<Typed> {
    fn ty(&self) -> &Type {
        match self {
            Tree::BinOp(_, _, _, ty) => &ty,
            Tree::Assign(_, _, ty) => &ty,
            Tree::Integer(_, ty) => &ty,
            Tree::String(_, ty) => &ty,
            Tree::Var(_, ty) => &ty,
            Tree::Addr(_, ty) => &ty,
            Tree::Deref(_, ty) => &ty,
            Tree::Indexed(_, _, ty) => &ty,
            Tree::Call(_, _, ty) => &ty,
            Tree::Return(_, ty) => &ty,
            _ => panic!("Not typed"),
        }
    }
}

impl TypedTree for Tree<Lowered> {
    fn ty(&self) -> &Type {
        match self {
            Tree::BinOp(_, _, _, ty) => &ty,
            Tree::Assign(_, _, ty) => &ty,
            Tree::Integer(_, ty) => &ty,
            Tree::String(_, ty) => &ty,
            Tree::Var(_, ty) => &ty,
            Tree::Addr(_, ty) => &ty,
            Tree::Deref(_, ty) => &ty,
            Tree::Indexed(_, _, ty) => &ty,
            Tree::Call(_, _, ty) => &ty,
            Tree::Return(_, ty) => &ty,
            _ => panic!("Not typed"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Tree<P: Phase> {
    Program(Vec<Tree<P>>),
    BinOp(Op, Box<Tree<P>>, Box<Tree<P>>, P::XBinOp),
    Assign(Box<Tree<P>>, Box<Tree<P>>, P::XAssign),
    Block(Vec<Tree<P>>),
    FuncDef(Type, String, Vec<(Type, String)>, Box<Tree<P>>),
    If(Box<Tree<P>>, Box<Tree<P>>, Option<Box<Tree<P>>>),
    While(Box<Tree<P>>, Box<Tree<P>>),
    For(
        Option<Box<Tree<P>>>,
        Option<Box<Tree<P>>>,
        Option<Box<Tree<P>>>,
        Box<Tree<P>>,
    ),

    Integer(i64, P::XInteger),
    String(String, P::XStringLiteral),
    Var(String, P::XVar),
    Indexed(Box<Tree<P>>, Box<Tree<P>>, P::XIndexed),
    VarDeclare(Type, String),
    Addr(Box<Tree<P>>, P::XAddr),
    Deref(Box<Tree<P>>, P::XDeref),

    Sizeof(Box<Tree<P>>),

    Call(String, Vec<Tree<P>>, P::XCall),
    Return(Box<Tree<P>>, P::XReturn),
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

fn parser<'a, I>() -> impl Parser<'a, I, Tree<Parsed>, extra::Err<Rich<'a, Token<'a>, SimpleSpan>>>
where
    I: Input<'a, Token = Token<'a>, Span = SimpleSpan>,
{
    let ident_name = select! { Token::Ident(ident) => ident.to_string() };
    let int_lit = select! { Token::Number(n) => Tree::<Parsed>::Integer(n as i64, ()) };
    let string_lit = select! { Token::String(s) => Tree::<Parsed>::String(s.to_string(), ()) };

    let type_parser = choice((
        just(Token::Int).to(Type::Int),
        just(Token::Char).to(Type::Char),
    ))
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
            .map(|(name, args)| Tree::<Parsed>::Call(name, args.unwrap_or_default(), ()));

        let array_index = just(Token::LBracket)
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RBracket));

        let primary_expr = choice((
            call_expr,
            int_lit,
            string_lit,
            ident_name.map(|name| Tree::<Parsed>::Var(name, ())),
            just(Token::LParen)
                .ignore_then(expr.clone())
                .then_ignore(just(Token::RParen)),
        ))
        .then(array_index.or_not())
        .map(|(prim, index)| match index {
            Some(i) => Tree::<Parsed>::Indexed(Box::new(prim), Box::new(i), ()),
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
                    .map(|expr| Tree::<Parsed>::Sizeof(Box::new(expr))),
                unary_operator
                    .then(unary.clone())
                    .map(|(op, expr)| match op {
                        UnaryOp::Plus => expr,
                        UnaryOp::Minus => Tree::<Parsed>::BinOp(
                            Op::Sub,
                            Box::new(Tree::<Parsed>::Integer(0, ())),
                            Box::new(expr),
                            (),
                        ),
                        UnaryOp::Addr => Tree::<Parsed>::Addr(Box::new(expr), ()),
                        UnaryOp::Deref => Tree::<Parsed>::Deref(Box::new(expr), ()),
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
                Tree::<Parsed>::BinOp(op, Box::new(lhs), Box::new(rhs), ())
            });

        let add_expr = mul_expr
            .clone()
            .foldl(add_op.then(mul_expr).repeated(), |lhs, (op, rhs)| {
                Tree::<Parsed>::BinOp(op, Box::new(lhs), Box::new(rhs), ())
            });

        let relational_expr = add_expr
            .clone()
            .foldl(relational_op.then(add_expr).repeated(), |lhs, (op, rhs)| {
                Tree::<Parsed>::BinOp(op, Box::new(lhs), Box::new(rhs), ())
            });

        let equality_expr = relational_expr.clone().foldl(
            equality_op.then(relational_expr).repeated(),
            |lhs, (op, rhs)| Tree::<Parsed>::BinOp(op, Box::new(lhs), Box::new(rhs), ()),
        );

        let assign_rhs = just(Token::Assign).ignore_then(expr.clone());

        equality_expr
            .clone()
            .then(assign_rhs.or_not())
            .map(|(lhs, rhs)| match rhs {
                Some(rhs) => Tree::<Parsed>::Assign(Box::new(lhs), Box::new(rhs), ()),
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
            Tree::<Parsed>::VarDeclare(ty, name)
        });

    let stmt_parser = recursive(|stmt| {
        let block_stmt = just(Token::LBrace)
            .ignore_then(
                choice((stmt.clone(), var_decl.clone()))
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::RBrace))
            .map(Tree::<Parsed>::Block);

        let return_stmt = just(Token::Return)
            .ignore_then(expr_parser.clone())
            .then_ignore(just(Token::Semicolon))
            .map(|expr| Tree::<Parsed>::Return(Box::new(expr), ()));

        let if_stmt = just(Token::If)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser.clone())
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .then(just(Token::Else).ignore_then(stmt.clone()).or_not())
            .map(|((cond, then), other)| {
                Tree::<Parsed>::If(Box::new(cond), Box::new(then), other.map(Box::new))
            });

        let while_stmt = just(Token::While)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr_parser.clone())
            .then_ignore(just(Token::RParen))
            .then(stmt.clone())
            .map(|(cond, body)| Tree::<Parsed>::While(Box::new(cond), Box::new(body)));

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
                Tree::<Parsed>::For(
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
                    choice((stmt_parser.clone(), var_decl.clone()))
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(just(Token::RBrace))
                .map(Tree::<Parsed>::Block),
        )
        .map(|(((ty, name), params), body)| {
            Tree::<Parsed>::FuncDef(ty, name, params.unwrap(), Box::new(body))
        });

    choice((func_def, var_decl, stmt_parser))
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map(Tree::<Parsed>::Program)
}

pub fn parse(source: &str) -> Result<Tree<Parsed>, Vec<ParseError>> {
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
    use super::{parse, Op, Parsed, Tree};
    use crate::types::Type;

    fn parse_one(source: &str) -> Tree<Parsed> {
        match parse(source).unwrap() {
            Tree::Program(trees) => {
                assert_eq!(trees.len(), 1, "expected one top-level item");
                trees.into_iter().next().unwrap()
            }
            _ => panic!("top-level tree must be Program"),
        }
    }

    fn parse_func(source: &str) -> (Type, String, Vec<(Type, String)>, Tree<Parsed>) {
        match parse_one(source) {
            Tree::FuncDef(ty, name, params, body) => (ty, name, params, *body),
            _ => panic!("expected function definition"),
        }
    }

    fn expect_block(tree: &Tree<Parsed>) -> &Vec<Tree<Parsed>> {
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
            Tree::Return(expr, _) => match expr.as_ref() {
                Tree::BinOp(Op::Add, lhs, rhs, _) => {
                    assert!(matches!(lhs.as_ref(), Tree::Integer(1, _)));
                    match rhs.as_ref() {
                        Tree::BinOp(Op::Mul, mul_lhs, mul_rhs, _) => {
                            assert!(matches!(**mul_lhs, Tree::Integer(2, _)));
                            assert!(matches!(**mul_rhs, Tree::Integer(3, _)));
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
                assert!(matches!(**cond, Tree::Integer(1, _)));
                assert!(matches!(**then_branch, Tree::Return(..)));
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
                assert_eq!(*ty, Type::Array(Box::new(Type::Int), 10));
            }
            _ => panic!("expected array declaration"),
        }
    }

    #[test]
    fn parse_array_index_expr() {
        let (_, _, _, body) = parse_func("int main(){ return a[3]; }");
        let stmts = expect_block(&body);

        match &stmts[0] {
            Tree::Return(expr, _) => match expr.as_ref() {
                Tree::Indexed(inner, index) => {
                    assert!(matches!(index.as_ref(), Tree::Integer(3, _)));
                    assert!(matches!(inner.as_ref(), Tree::Var(name, _) if name == "a"));
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
            Tree::Assign(lhs, rhs, _) => {
                match lhs.as_ref() {
                    Tree::Indexed(inner, index, _) => {
                        assert!(matches!(index.as_ref(), Tree::Integer(1, _)));
                        assert!(matches!(inner.as_ref(), Tree::Var(name, _) if name == "a"));
                    }
                    _ => panic!("expected indexed lvalue"),
                }
                assert!(matches!(rhs.as_ref(), Tree::Integer(2, _)));
            }
            _ => panic!("expected assignment statement"),
        }
    }

    #[test]
    fn parse_call_and_unary() {
        let (_, _, _, body) = parse_func("int main(){ return *&foo(1,2); }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::Return(expr, _) => match expr.as_ref() {
                Tree::Deref(inner, _) => match &**inner {
                    Tree::Addr(call, _) => match &**call {
                        Tree::Call(name, args, _) => {
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
            Tree::Assign(lhs, rhs, _) => {
                assert!(matches!(lhs.as_ref(), Tree::Var(name, _) if name == "a"));
                match rhs.as_ref() {
                    Tree::Assign(inner_lhs, inner_rhs, _) => {
                        assert!(matches!(inner_lhs.as_ref(), Tree::Var(name, _) if name == "b"));
                        assert!(matches!(inner_rhs.as_ref(), Tree::Integer(1, _)));
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
            Tree::Return(expr, _) => match expr.as_ref() {
                Tree::BinOp(Op::Eq, lhs, rhs, _) => {
                    assert!(matches!(rhs.as_ref(), Tree::Integer(0, _)));
                    match lhs.as_ref() {
                        Tree::BinOp(Op::Lt, rel_lhs, rel_rhs, _) => {
                            assert!(matches!(rel_lhs.as_ref(), Tree::Integer(1, _)));
                            assert!(matches!(rel_rhs.as_ref(), Tree::Integer(2, _)));
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
                assert!(matches!(body.as_ref(), Tree::Return(..)));
            }
            _ => panic!("expected for stmt"),
        }
    }

    #[test]
    fn parse_sizeof_unary() {
        let (_, _, _, body) = parse_func("int main(){ return sizeof 1 + sizeof x; }");
        let stmts = expect_block(&body);
        match &stmts[0] {
            Tree::Return(expr, _) => match expr.as_ref() {
                Tree::BinOp(Op::Add, lhs, rhs, _) => {
                    assert!(matches!(lhs.as_ref(), Tree::Sizeof(_)));
                    assert!(matches!(rhs.as_ref(), Tree::Sizeof(_)));
                }
                _ => panic!("expected add in return"),
            },
            _ => panic!("expected return stmt"),
        }
    }

    #[test]
    fn parse_global_decl() {
        let tree = parse_one("int g;");
        match tree {
            Tree::VarDeclare(ty, name) => {
                assert_eq!(ty, Type::Int);
                assert_eq!(name, "g");
            }
            _ => panic!("expected global var declaration"),
        }
    }

    #[test]
    fn parse_program_with_global_and_func() {
        let tree = parse("int g; int main(){ return g; }").unwrap();
        match tree {
            Tree::Program(trees) => {
                assert_eq!(trees.len(), 2);
                match &trees[0] {
                    Tree::VarDeclare(ty, name) => {
                        assert_eq!(ty, &Type::Int);
                        assert_eq!(name, "g");
                    }
                    _ => panic!("expected first top-level to be var declare"),
                }
                match &trees[1] {
                    Tree::FuncDef(_, name, _, _) => assert_eq!(name, "main"),
                    _ => panic!("expected second top-level to be function"),
                }
            }
            _ => panic!("expected program"),
        }
    }
}
