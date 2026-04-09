mod codegen;
mod error;
mod lexer;
mod parser;

use std::env;
use std::fs;
use std::io::stdout;

use ariadne::{Color, Label, Report, ReportKind, Source};

use crate::codegen::Codegen;
use crate::parser::parse;

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
    let mut codegen = Codegen::new(tree, stdout());
    codegen.generate().unwrap();
}
