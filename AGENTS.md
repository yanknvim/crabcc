# Agent Guidelines for crabcc

This document is for agentic coding assistants working in this repository. It
summarizes build/test commands and coding conventions based on the current
codebase.

## Project Snapshot

- Language: Rust (Edition 2024)
- Target: RISC-V assembly generation
- Parser: Chumsky (token-based), lexer: Logos
- Pipeline: Lexer -> Parser/AST -> Codegen

## Commands

### Build
```bash
cargo build
cargo build --release
```

### Test
```bash
# Run all Rust unit tests
cargo test

# Run a single test by name
cargo test parse_respects_precedence

# Run all tests in a module
cargo test parser::
```

### Formatting / Lint
```bash
cargo fmt
cargo fmt --check
cargo clippy
cargo clippy -- -D warnings
```

### Integration Tests (manual)
There is no built-in integration runner in this repo. If needed, run a single
C test manually with a RISC-V toolchain:
```bash
cargo run -- tests/return_42.c > /tmp/out.S
riscv64-none-elf-gcc -o /tmp/out /tmp/out.S
spike pk /tmp/out
echo $?  # match tests/return_42.expect
```
Requires: riscv64-none-elf-gcc, spike, pk.

## Repository Layout

```
src/
  main.rs       CLI entry; reads source, parses, runs codegen
  lexer.rs      Logos lexer + Token display
  parser.rs     Chumsky parser + AST
  error.rs      ParseError formatting for Ariadne
  codegen.rs    RISC-V assembly generation

tests/          C source tests with .expect exit codes
scripts/        helper scripts (if added later)
```

## Code Style Guidelines

### General
- Follow rustfmt defaults.
- Prefer explicit, small helper functions over large monolithic blocks.
- Keep public API surface minimal (expose only what other modules need).

### Imports
- Order: std -> external crates -> internal modules.
- Group related imports.
- Avoid glob imports.

Example:
```rust
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use chumsky::prelude::*;
use logos::Logos;

use crate::parser::{Op, Tree};
```

### Naming
- Types: PascalCase (`Tree`, `Codegen`).
- Functions/vars: snake_case (`gen_stmt`, `stack_offset`).
- Constants: SCREAMING_SNAKE_CASE.
- Use descriptive names for AST variants and helpers.

### AST / Parser
- AST enums derive `Debug` and `Clone`.
- Prefer `match` over nested `if let` chains.
- `parser.rs` keeps AST + parser logic together; keep helpers close to use.
- Use Chumsky combinators (`ignore_then`, `then_ignore`) to discard syntax
  tokens while preserving semantic nodes.

### Error Handling
- Parsing: return `Result<Tree, Vec<ParseError>>`.
- `ParseError` formatting lives in `src/error.rs`.
- Runtime/compiler errors: `panic!` with a clear message.
- I/O: propagate with `?` where possible.

### Codegen
- Use 4-space indentation in emitted assembly.
- Keep register usage consistent:
  - temporaries: t0, t1
  - args/returns: a0-a7
- Stack frame size must be 16-byte aligned (RISC-V ABI).
- Offsets are negative relative to fp.

### Tests
- Unit tests live at file bottom in `#[cfg(test)] mod tests`.
- Use focused, descriptive test names.
- Prefer helper functions for repeated patterns (`parse_one`).

## Notes for Agents

- This repo previously referenced Pest, but current code uses Chumsky + Logos.
- `AGENTS.md` is authoritative for agent workflow; update it if conventions
  change.
- There are no Cursor or Copilot rule files in this repo currently.
