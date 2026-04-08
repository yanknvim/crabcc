mod codegen;
mod parser;

use std::env;
use std::fs;
use std::io::stdout;

use crate::codegen::Codegen;
use crate::parser::parse;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() != 2 {
        panic!("invalid number of args");
    }

    let source_path = &args[1];
    let source = fs::read_to_string(source_path).expect("failed to read source file");
    let tree = parse(&source);
    let mut codegen = Codegen::new(tree, stdout());
    codegen.generate().unwrap();
}
