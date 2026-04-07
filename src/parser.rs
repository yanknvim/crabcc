use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "./parser.pest"]
pub struct CParser;

#[derive(Debug, Clone)]
pub enum Tree {
    BinOp(Op, Box<Tree>, Box<Tree>),
    Integer(i64),
    Var(String),
}

#[derive(Debug, Clone)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

pub fn parse(s: &str) -> Tree {
    let mut pairs = CParser::parse(Rule::expr, s).expect("parse error");

    let pair = pairs.next().unwrap();
    parse_expr(pair)
}

fn parse_expr(pair: Pair<Rule>) -> Tree {
    match pair.as_rule() {
        Rule::expr => parse_expr(pair.into_inner().next().unwrap()),
        Rule::add | Rule::mul => {
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
        other => panic!("unexpected char: {}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_respects_precedence() {
        let tree = parse("1+2*3");
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
        assert_eq!(format!("{tree:?}"), format!("{expected:?}"));
    }

    #[test]
    fn parse_parentheses_grouping() {
        let tree = parse("(1+2)*3");
        let expected = Tree::BinOp(
            Op::Mul,
            Box::new(Tree::BinOp(
                Op::Add,
                Box::new(Tree::Integer(1)),
                Box::new(Tree::Integer(2)),
            )),
            Box::new(Tree::Integer(3)),
        );

        assert_eq!(format!("{tree:?}"), format!("{expected:?}"));
    }

    #[test]
    fn parse_unary_minus_as_zero_sub() {
        let tree = parse("-a");
        let expected = Tree::BinOp(
            Op::Sub,
            Box::new(Tree::Integer(0)),
            Box::new(Tree::Var("a".to_string())),
        );

        assert_eq!(format!("{tree:?}"), format!("{expected:?}"));
    }
}
