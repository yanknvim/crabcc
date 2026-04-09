# Agent Guidelines for crabcc

This document is for agentic coding assistants working in this repository. It
summarizes build/test commands and coding conventions based on the current
codebase.

## Project Snapshot

- Language: Rust (Edition 2024)
- Domain: tiny C-like compiler targeting RISC-V assembly
- Pipeline: Lexer (Logos) -> Parser/AST (Chumsky) -> Type check -> Codegen
- Error reporting: Ariadne for parse errors

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

# Run a single test by name (substring match)
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
There is no built-in integration runner. To run a single C test with a RISC-V
toolchain:
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
  main.rs       CLI entry; reads source, parses, type-checks, runs codegen
  lexer.rs      Logos lexer + Token display
  parser.rs     Chumsky parser + AST
  error.rs      ParseError formatting for Ariadne
  sema.rs       Type checking + typed AST
  types.rs      Type definitions
  codegen.rs    RISC-V assembly generation

tests/          C source tests with .expect exit codes
scripts/        helper scripts (if added later)
```

## Code Style Guidelines

### General
- Follow rustfmt defaults.
- Prefer explicit, small helper functions over large monolithic blocks.
- Keep public API surface minimal (expose only what other modules need).
- Keep module responsibilities narrow (lexer/parser/sema/codegen split).

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

### Formatting
- rustfmt is the source of truth; do not hand-align with spaces.
- Keep lines readable; extract helpers instead of deep nesting.

### Naming
- Types: PascalCase (`Tree`, `TypedTree`, `Codegen`).
- Functions/vars: snake_case (`gen_stmt`, `stack_offset`).
- Constants: SCREAMING_SNAKE_CASE.
- Use descriptive names for AST variants and helpers.

### AST / Parser
- AST enums derive `Debug` and `Clone`.
- Prefer `match` over nested `if let` chains.
- `parser.rs` keeps AST + parser logic together; keep helpers close to use.
- Use Chumsky combinators (`ignore_then`, `then_ignore`) to discard syntax
  tokens while preserving semantic nodes.
- Parser returns `Result<Tree, Vec<ParseError>>` and does not emit diagnostics.

### Type Checking (sema.rs)
- Typed AST mirrors the untyped AST with `Type` attached where needed.
- Use `TypeChecker` methods for scoping, lookup, and validation.
- Validate lvalues (`Var` or `Deref`) before assignment.
- Keep panic messages short and specific (used as compiler errors).

### Error Handling
- Parsing: return `Result<Tree, Vec<ParseError>>`.
- `ParseError` formatting lives in `src/error.rs` and maps Chumsky Rich errors.
- Compiler/runtime errors: `panic!` with a clear message (no user recovery).
- I/O: propagate with `?` where possible; `main` uses `expect` for file read.

### Codegen
- Use 4-space indentation in emitted assembly.
- Keep register usage consistent:
  - temporaries: t0, t1
  - args/returns: a0-a7
- Stack frame size must be 16-byte aligned (RISC-V ABI).
- Offsets are negative relative to fp; keep stack_offset in bytes.
- Gen functions are split: `gen_stmt`, `gen_expr`, `gen_lvalue`.

### Tests
- Unit tests live at file bottom in `#[cfg(test)] mod tests`.
- Use focused, descriptive test names.
- Prefer helper functions for repeated patterns (`parse_one`, `typed_program`).
- Test failures should explain the expected shape of AST or typing.

## Cursor/Copilot Rules

- No `.cursor/rules/`, `.cursorrules`, or `.github/copilot-instructions.md`
  files were found in this repository.

## Notes for Agents

- This repo previously referenced Pest, but current code uses Chumsky + Logos.
- `AGENTS.md` is authoritative for agent workflow; update it if conventions
  change.
