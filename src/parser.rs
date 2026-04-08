use pest::{Parser, iterators::Pair};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "./parser.pest"]
pub struct CParser;

#[derive(Debug, Clone)]
pub enum Tree {
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

impl Tree {
    pub fn children(&self) -> Box<dyn Iterator<Item = &Tree> + '_> {
        match self {
            Tree::Assign(_, rhs) | Tree::Return(rhs) => Box::new(std::iter::once(rhs.as_ref())),
            Tree::BinOp(_, lhs, rhs) => Box::new(vec![lhs.as_ref(), rhs.as_ref()].into_iter()),
            Tree::Block(stmts) => Box::new(stmts.iter()),
            Tree::If(cond, a, Some(b)) => {
                Box::new(vec![cond.as_ref(), a.as_ref(), b.as_ref()].into_iter())
            }
            Tree::If(cond, a, None) => Box::new(vec![cond.as_ref(), a.as_ref()].into_iter()),
            Tree::While(cond, stmt) => Box::new(vec![cond.as_ref(), stmt.as_ref()].into_iter()),
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

#[derive(Debug, Clone)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    GreaterThan,
    GreaterThanOrEq,
    LessThan,
    LessThanOrEq,
}

pub fn parse(s: &str) -> Vec<Tree> {
    let mut pairs = CParser::parse(Rule::program, s).expect("parse error");

    let pair = pairs.next().unwrap();
    parse_program(pair)
}

fn parse_program(pair: Pair<Rule>) -> Vec<Tree> {
    let inner = pair.into_inner();
    inner
        .filter_map(|p| match p.as_rule() {
            Rule::func_def => {
                let mut inner = p.into_inner();
                let name = inner.next().unwrap().as_str().to_string();
                let mut params = Vec::new();
                let mut body = None;

                for item in inner {
                    match item.as_rule() {
                        Rule::params => {
                            params = item
                                .into_inner()
                                .map(|ident| ident.as_str().to_string())
                                .collect();
                        }
                        Rule::block => {
                            body = Some(Box::new(parse_block(item)));
                        }
                        _ => unreachable!(),
                    }
                }

                Some(Tree::FuncDef(
                    name,
                    params,
                    body.expect("missing function body"),
                ))
            }
            Rule::stmt
            | Rule::block
            | Rule::if_stmt
            | Rule::while_stmt
            | Rule::for_stmt
            | Rule::return_stmt
            | Rule::expr_stmt => Some(parse_stmt(p)),
            _ => None,
        })
        .collect::<Vec<_>>()
}

fn parse_block(pair: Pair<Rule>) -> Tree {
    let inner = pair.into_inner();
    Tree::Block(
        inner
            .map(|p| match p.as_rule() {
                Rule::stmt => parse_stmt(p),
                Rule::var_declare => parse_var_declare(p),
                _ => unreachable!(),
            })
            .collect(),
    )
}

fn parse_var_declare(pair: Pair<Rule>) -> Tree {
    let mut inner = pair.into_inner();
    Tree::VarDeclare(inner.next().unwrap().to_string())
}

fn parse_stmt(pair: Pair<Rule>) -> Tree {
    match pair.as_rule() {
        Rule::stmt => parse_stmt(pair.into_inner().next().unwrap()),
        Rule::block => parse_block(pair),
        Rule::if_stmt => {
            let mut inner = pair.into_inner();
            let cond = parse_expr(inner.next().unwrap());

            let stmt_a = parse_stmt(inner.next().unwrap());
            let stmt_b = inner.next().map(parse_stmt);

            Tree::If(Box::new(cond), Box::new(stmt_a), stmt_b.map(Box::new))
        }
        Rule::while_stmt => {
            let mut inner = pair.into_inner();
            let cond = parse_expr(inner.next().unwrap());
            let stmt = parse_stmt(inner.next().unwrap());

            Tree::While(Box::new(cond), Box::new(stmt))
        }
        Rule::for_stmt => {
            let mut init = None;
            let mut cond = None;
            let mut update = None;
            let mut stmt = None;

            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::init => {
                        init = Some(Box::new(parse_expr(inner.into_inner().next().unwrap())))
                    }
                    Rule::cond => {
                        cond = Some(Box::new(parse_expr(inner.into_inner().next().unwrap())))
                    }
                    Rule::update => {
                        update = Some(Box::new(parse_expr(inner.into_inner().next().unwrap())))
                    }
                    Rule::stmt => {
                        stmt = Some(Box::new(parse_stmt(inner.into_inner().next().unwrap())))
                    }
                    _ => unreachable!(),
                }
            }

            Tree::For(init, cond, update, stmt.unwrap())
        }
        Rule::return_stmt => {
            let mut inner = pair.into_inner();
            Tree::Return(Box::new(parse_expr(inner.next().unwrap())))
        }
        Rule::expr_stmt => {
            let mut inner = pair.into_inner();
            parse_expr(inner.next().unwrap())
        }
        _ => panic!("unexpected syntax: {:?}", pair),
    }
}

fn parse_assign(pair: Pair<Rule>) -> Tree {
    let mut inner = pair.into_inner();
    let mut lhs = parse_expr(inner.next().unwrap());

    if let Some(rhs) = inner.next() {
        lhs = Tree::Assign(Box::new(lhs), Box::new(parse_assign(rhs)));
    }

    lhs
}

fn parse_expr(pair: Pair<Rule>) -> Tree {
    match pair.as_rule() {
        Rule::expr => parse_assign(pair.into_inner().next().unwrap()),
        Rule::assign => parse_assign(pair),
        Rule::relational | Rule::equality | Rule::add | Rule::mul => {
            let mut inner = pair.into_inner();
            let mut lhs = parse_expr(inner.next().unwrap());

            while let (Some(op), Some(rhs)) = (inner.next(), inner.next()) {
                let op = parse_op(op.as_str());
                let rhs = parse_expr(rhs);

                lhs = Tree::BinOp(op, Box::new(lhs), Box::new(rhs));
            }

            lhs
        }
        Rule::unary => parse_unary(pair),
        Rule::primary => parse_primary(pair),
        _ => unreachable!("{:?}", pair.as_rule()),
    }
}

fn parse_unary(pair: Pair<Rule>) -> Tree {
    let mut inner = pair.into_inner();
    let head = inner.next().unwrap();

    if head.as_rule() == Rule::primary {
        return parse_primary(head);
    }

    let op = head.as_str();
    let operand = parse_unary(inner.next().unwrap());
    match op {
        "-" => Tree::BinOp(Op::Sub, Box::new(Tree::Integer(0)), Box::new(operand)),
        "+" => operand,
        "&" => Tree::Addr(Box::new(operand)),
        "*" => Tree::Deref(Box::new(operand)),
        _ => unreachable!(),
    }
}

fn parse_primary(pair: Pair<Rule>) -> Tree {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::integer => Tree::Integer(inner.as_str().parse().unwrap()),
        Rule::ident => Tree::Var(inner.as_str().to_string()),
        Rule::call => {
            let mut inner = inner.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let mut args = Vec::new();

            if let Some(args_pair) = inner.next() {
                args = args_pair.into_inner().map(parse_expr).collect();
            }

            Tree::Call(name, args)
        }
        Rule::expr => parse_expr(inner),
        _ => unreachable!(),
    }
}

fn parse_op(s: &str) -> Op {
    match s {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" => Op::Mul,
        "/" => Op::Div,

        "==" => Op::Eq,
        "!=" => Op::NotEq,

        ">" => Op::GreaterThan,
        ">=" => Op::GreaterThanOrEq,
        "<" => Op::LessThan,
        "<=" => Op::LessThanOrEq,
        other => panic!("unexpected char: {}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(s: &str) -> Tree {
        let trees = parse(s);
        assert_eq!(trees.len(), 1, "expected single statement, got {trees:?}");
        trees.into_iter().next().unwrap()
    }

    fn parse_one_stmt(s: &str) -> Tree {
        parse_one(&format!("{{{s}}}"))
    }

    fn assert_tree_eq(actual: &Tree, expected: &Tree) {
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }

    #[test]
    fn parse_respects_precedence() {
        let tree = parse_one("1+2*3;");
        let expected = Tree::BinOp(
            Op::Add,
            Box::new(Tree::Integer(1)),
            Box::new(Tree::BinOp(
                Op::Mul,
                Box::new(Tree::Integer(2)),
                Box::new(Tree::Integer(3)),
            )),
        );

        assert!(matches!(tree, Tree::BinOp(Op::Add, _, _)));
        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_parentheses_grouping() {
        let tree = parse_one("(1+2)*3;");
        let expected = Tree::BinOp(
            Op::Mul,
            Box::new(Tree::BinOp(
                Op::Add,
                Box::new(Tree::Integer(1)),
                Box::new(Tree::Integer(2)),
            )),
            Box::new(Tree::Integer(3)),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_unary_minus_as_zero_sub() {
        let tree = parse_one("-a;");
        let expected = Tree::BinOp(
            Op::Sub,
            Box::new(Tree::Integer(0)),
            Box::new(Tree::Var("a".to_string())),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_assignment_is_right_associative() {
        let tree = parse_one("a=b=1;");
        let expected = Tree::Assign(
            Box::new(Tree::Var("a".to_string())),
            Box::new(Tree::Assign(
                Box::new(Tree::Var("b".to_string())),
                Box::new(Tree::Integer(1)),
            )),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_add_is_left_associative() {
        let tree = parse_one("10-3-2;");
        let expected = Tree::BinOp(
            Op::Sub,
            Box::new(Tree::BinOp(
                Op::Sub,
                Box::new(Tree::Integer(10)),
                Box::new(Tree::Integer(3)),
            )),
            Box::new(Tree::Integer(2)),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_unary_plus_is_noop() {
        let tree = parse_one("+a;");
        let expected = Tree::Var("a".to_string());

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_relational_operator() {
        let tree = parse_one("1<2;");
        let expected = Tree::BinOp(
            Op::LessThan,
            Box::new(Tree::Integer(1)),
            Box::new(Tree::Integer(2)),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_equality_operator() {
        let tree = parse_one("a==b;");
        let expected = Tree::BinOp(
            Op::Eq,
            Box::new(Tree::Var("a".to_string())),
            Box::new(Tree::Var("b".to_string())),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_mixed_precedence_expression() {
        let tree = parse_one("a+1<b*2;");
        let expected = Tree::BinOp(
            Op::LessThan,
            Box::new(Tree::BinOp(
                Op::Add,
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::Integer(1)),
            )),
            Box::new(Tree::BinOp(
                Op::Mul,
                Box::new(Tree::Var("b".to_string())),
                Box::new(Tree::Integer(2)),
            )),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_return_statement() {
        let tree = parse_one("return a+1;");
        let expected = Tree::Return(Box::new(Tree::BinOp(
            Op::Add,
            Box::new(Tree::Var("a".to_string())),
            Box::new(Tree::Integer(1)),
        )));

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_if_with_stmt_body() {
        let tree = parse_one("if(1==1) return 2; else return 3;");
        let expected = Tree::If(
            Box::new(Tree::BinOp(
                Op::Eq,
                Box::new(Tree::Integer(1)),
                Box::new(Tree::Integer(1)),
            )),
            Box::new(Tree::Return(Box::new(Tree::Integer(2)))),
            Some(Box::new(Tree::Return(Box::new(Tree::Integer(3))))),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_while_with_stmt_body() {
        let tree = parse_one("while(a<3) a=a+1;");
        let expected = Tree::While(
            Box::new(Tree::BinOp(
                Op::LessThan,
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::Integer(3)),
            )),
            Box::new(Tree::Assign(
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::BinOp(
                    Op::Add,
                    Box::new(Tree::Var("a".to_string())),
                    Box::new(Tree::Integer(1)),
                )),
            )),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_for_with_stmt_body() {
        let tree = parse_one("for(a=0; a<3; a=a+1) return a;");
        let expected = Tree::For(
            Some(Box::new(Tree::Assign(
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::Integer(0)),
            ))),
            Some(Box::new(Tree::BinOp(
                Op::LessThan,
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::Integer(3)),
            ))),
            Some(Box::new(Tree::Assign(
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::BinOp(
                    Op::Add,
                    Box::new(Tree::Var("a".to_string())),
                    Box::new(Tree::Integer(1)),
                )),
            ))),
            Box::new(Tree::Return(Box::new(Tree::Var("a".to_string())))),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_if_without_else() {
        let tree = parse_one("if(a!=0) return a;");
        let expected = Tree::If(
            Box::new(Tree::BinOp(
                Op::NotEq,
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::Integer(0)),
            )),
            Box::new(Tree::Return(Box::new(Tree::Var("a".to_string())))),
            None,
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_while_with_block_body() {
        let tree = parse_one("while(a<1){a=a+1;}");
        let expected = Tree::While(
            Box::new(Tree::BinOp(
                Op::LessThan,
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::Integer(1)),
            )),
            Box::new(Tree::Block(vec![Tree::Assign(
                Box::new(Tree::Var("a".to_string())),
                Box::new(Tree::BinOp(
                    Op::Add,
                    Box::new(Tree::Var("a".to_string())),
                    Box::new(Tree::Integer(1)),
                )),
            )])),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_for_with_missing_parts() {
        let tree = parse_one("for(;;) return 0;");
        let expected = Tree::For(
            None,
            None,
            None,
            Box::new(Tree::Return(Box::new(Tree::Integer(0)))),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_function_definition() {
        let tree = parse_one("int add(int a,int b){int a1; int b1; return a+b;}");
        let expected = Tree::FuncDef(
            "add".to_string(),
            vec!["a".to_string(), "b".to_string()],
            Box::new(Tree::Block(vec![
                Tree::VarDeclare("a1".to_string()),
                Tree::VarDeclare("b1".to_string()),
                Tree::Return(Box::new(Tree::BinOp(
                    Op::Add,
                    Box::new(Tree::Var("a".to_string())),
                    Box::new(Tree::Var("b".to_string())),
                ))),
            ])),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_main_function_definition() {
        let tree = parse_one("int main(){int result; return 0;}");
        let expected = Tree::FuncDef(
            "main".to_string(),
            vec![],
            Box::new(Tree::Block(vec![
                Tree::VarDeclare("result".to_string()),
                Tree::Return(Box::new(Tree::Integer(0))),
            ])),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_allows_newlines() {
        let tree = parse_one("int main()\n{\nint result;\nreturn 0;\n}\n");
        let expected = Tree::FuncDef(
            "main".to_string(),
            vec![],
            Box::new(Tree::Block(vec![
                Tree::VarDeclare("result".to_string()),
                Tree::Return(Box::new(Tree::Integer(0))),
            ])),
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_call_expression() {
        let tree = parse_one("add(1,2+3);");
        let expected = Tree::Call(
            "add".to_string(),
            vec![
                Tree::Integer(1),
                Tree::BinOp(
                    Op::Add,
                    Box::new(Tree::Integer(2)),
                    Box::new(Tree::Integer(3)),
                ),
            ],
        );

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_addr_of_expression() {
        let tree = parse_one_stmt("int a; &a;");
        let expected = Tree::Block(vec![
            Tree::VarDeclare("a".to_string()),
            Tree::Addr(Box::new(Tree::Var("a".to_string()))),
        ]);

        assert_tree_eq(&tree, &expected);
    }

    #[test]
    fn parse_deref_expression() {
        let tree = parse_one_stmt("int a; *a;");
        let expected = Tree::Block(vec![
            Tree::VarDeclare("a".to_string()),
            Tree::Deref(Box::new(Tree::Var("a".to_string()))),
        ]);

        assert_tree_eq(&tree, &expected);
    }
}
