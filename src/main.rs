mod codegen;
mod error;
mod lexer;
mod parser;
mod sema;
mod types;

use std::env;
use std::fs;
use std::io::stdout;

use ariadne::{Color, Label, Report, ReportKind, Source};

use crate::codegen::Codegen;
use crate::parser::parse;
use crate::types::TypeChecker;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() != 2 {
        panic!("invalid number of args");
    }

    let source_path = &args[1];
    let source = fs::read_to_string(source_path).expect("failed to read source file");
    let tree = match parse(&source) {
        Ok(tree) => tree,
        Err(errors) => {
            for error in errors {
                let span = error.span;
                Report::build(ReportKind::Error, (), span.start)
                    .with_message("parse error")
                    .with_label(
                        Label::new(span.start..span.end)
                            .with_message(error.message)
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(Source::from(&source))
                    .expect("failed to write parse error");
            }
            std::process::exit(1);
        }
    };
    let mut checker = TypeChecker::new();
    let typed_tree = checker.check(&tree);
    let lowered_tree = sema::lower(typed_tree);
    let mut codegen = Codegen::new(
        checker.globals().clone(),
        checker.strings().clone(),
        stdout(),
    );
    codegen.generate(&lowered_tree).unwrap();
}
