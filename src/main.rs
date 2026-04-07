mod codegen;
mod parser;

use std::env;
use std::io::stdout;

use crate::codegen::Codegen;
use crate::parser::parse;
use pest::Parser;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() != 2 {
        panic!("invalid number of args");
    }

    let tree = parse(&args[1]);
    let mut codegen = Codegen::new(tree, stdout());
    codegen.generate().unwrap();
}
