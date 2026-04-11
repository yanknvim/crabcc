#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

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
use chumsky::span::SimpleSpan;
use logos::Logos;
use typed_arena::Arena;

use crate::codegen::Codegen;
use crate::lexer::Token;
use crate::parser::parse;
use crate::sema::TypeChecker;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let args: Vec<_> = env::args().collect();

    if args.len() != 2 {
        panic!("invalid number of args");
    }

    let source_path = &args[1];
    let source = fs::read_to_string(source_path).expect("failed to read source file");

    let arena = Arena::new();

    let eoi = SimpleSpan::from(source.len()..source.len());

    let lexer = Token::lexer(&source);
    let tokens: Vec<(Token, SimpleSpan)> = lexer
        .spanned()
        .map(|(token, span)| {
            let token = match token {
                Ok(token) => token,
                Err(err) => panic!("lexer error: {}", err),
            };
            (token, SimpleSpan::from(span))
        })
        .collect();

    let tree = match parse(&arena, tokens.as_slice(), eoi) {
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
    let typed_tree = checker.check(tree);
    let mut codegen = Codegen::new(
        typed_tree,
        checker.globals().clone(),
        checker.strings().clone(),
        stdout(),
    );
    codegen.generate().unwrap();
}
