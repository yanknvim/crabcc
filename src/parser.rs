use pest::{Parser, iterators::Pair};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "./parser.pest"]
pub struct CParser;

#[derive(Debug, Clone)]
pub enum Tree {
    BinOp(Op, Box<Tree>, Box<Tree>),
    Assign(Box<Tree>, Box<Tree>),
    Integer(i64),
    Var(String),
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
    inner.map(|p| parse_stmt(p)).collect::<Vec<_>>()
}

fn parse_stmt(pair: Pair<Rule>) -> Tree {
    match pair.as_rule() {
        Rule::stmt => parse_stmt(pair.into_inner().next().unwrap()),
        Rule::return_stmt => {
            let mut inner = pair.into_inner();
            Tree::Return(Box::new(parse_assign(inner.next().unwrap())))
        }
        Rule::expr_stmt => {
            let mut inner = pair.into_inner();
            parse_assign(inner.next().unwrap())
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
    let operand = parse_primary(inner.next().unwrap());
    match op {
        "-" => Tree::BinOp(Op::Sub, Box::new(Tree::Integer(0)), Box::new(operand)),
        _ => operand,
    }
}

fn parse_primary(pair: Pair<Rule>) -> Tree {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::integer => Tree::Integer(inner.as_str().parse().unwrap()),
        Rule::ident => Tree::Var(inner.as_str().to_string()),
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
}
