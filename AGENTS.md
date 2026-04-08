# Agent Guidelines for crabcc

This document provides coding guidelines and standards for agents working in the crabcc repository. crabcc is a C compiler written in Rust that targets RISC-V assembly.

## Project Overview

- **Language**: Rust (Edition 2024)
- **Target**: RISC-V assembly generation
- **Parser**: Pest parser generator
- **Architecture**: Parser → AST → Code Generator

## Build, Test, and Lint Commands

### Building
```bash
cargo build                    # Build in debug mode
cargo build --release          # Build optimized binary
```

### Testing
```bash
# Run all tests (Rust unit tests + C integration tests)
cargo make test               # Runs both Rust and C tests via Makefile.toml

# Run only Rust unit tests
cargo test                     # All unit tests
cargo test <test_name>         # Single test by name (e.g., cargo test parse_respects_precedence)
cargo test <module>::          # All tests in module (e.g., cargo test parser::)

# Run only C integration tests
bash scripts/run_tests.sh      # Requires riscv64-none-elf-gcc, spike, and pk
```

### Linting and Formatting
```bash
cargo fmt                      # Format code with rustfmt
cargo fmt --check              # Check formatting without modifying
cargo clippy                   # Run linter
cargo clippy -- -D warnings    # Treat warnings as errors
```

## Project Structure

```
src/
├── main.rs       # Entry point, handles CLI args
├── parser.rs     # Pest parser, AST definition
├── parser.pest   # Pest grammar file
└── codegen.rs    # RISC-V assembly code generator

tests/            # C test files with .expect files for exit codes
scripts/          # Test runner scripts
```

## Code Style Guidelines

### General Rust Style

- **Edition**: Use Rust 2024 edition features
- **Formatting**: Follow default `rustfmt` style
- **Line Length**: Default rustfmt limit (~100 chars)
- **Indentation**: 4 spaces (Rust standard)

### Imports

- Order: `std` → external crates → internal modules
- Group related imports together
- Use explicit imports rather than glob imports (`use std::io::{self, Write}`)

```rust
// Good example
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

use crate::parser::{Op, Tree};
```

### Types and Naming

- **Types**: PascalCase (`Tree`, `Codegen`, `CParser`)
- **Functions**: snake_case (`parse_expr`, `gen_stmt`, `collect_locals`)
- **Variables**: snake_case (`stack_offset`, `frame_size`, `current_frame_size`)
- **Constants**: SCREAMING_SNAKE_CASE (if needed)
- **Lifetimes**: Use explicit lifetimes when needed (`'_` for inference)

### Enums

- Use descriptive names for variants
- Prefer tuple variants for data (`BinOp(Op, Box<Tree>, Box<Tree>)`)
- Derive `Debug` and `Clone` for AST types

```rust
#[derive(Debug, Clone)]
pub enum Tree {
    BinOp(Op, Box<Tree>, Box<Tree>),
    Integer(i64),
    // ...
}
```

### Structs

- Derive `Debug` for all types
- Use public fields only when necessary
- Group related fields together

```rust
#[derive(Debug)]
pub struct Codegen<W: Write> {
    trees: Vec<Tree>,
    env: Vec<HashMap<String, i64>>,
    functions: HashMap<String, usize>,
    // ...
}
```

### Functions

- Keep functions focused and single-purpose
- Use `?` operator for error propagation with `io::Result<()>`
- Prefer early returns to reduce nesting
- Helper functions should be private unless needed externally

### Pattern Matching

- Use `let` guards for complex conditions in `if let` chains:
```rust
if let Tree::Assign(lhs, _) = tree
    && let Tree::Var(name) = &**lhs
{
    // ...
}
```

- Prefer exhaustive matching with `match` over multiple `if let`
- Use `unreachable!()` for impossible states with panic messages

### Error Handling

- **Parsing errors**: Use `expect("parse error")` with descriptive messages
- **Runtime errors**: Use `panic!()` with clear error messages for compiler errors
- **I/O errors**: Propagate with `?` and return `io::Result<()>`
- **Validation**: Panic early with descriptive messages (e.g., "Too much params: {}")

```rust
// Examples from codebase
panic!("invalid number of args");
panic!("Not declared variable: {}", name);
expect("failed to read source file")
```

### Comments

- Avoid obvious comments; code should be self-documenting
- Comment complex algorithms or non-obvious behavior
- Use `//` for inline comments
- No doc comments (`///`) currently in use; add if making public APIs

### Testing

- Place unit tests in `#[cfg(test)] mod tests` at bottom of files
- Use descriptive test names: `test_<what>_<expected_behavior>`
- Example: `parse_respects_precedence`, `parse_unary_minus_as_zero_sub`
- Create helper functions for common test patterns (`parse_one`, `assert_tree_eq`)
- Test edge cases and error conditions

### Module Organization

- Keep `pub` visibility minimal; expose only what's needed
- Public items: `Tree`, `Op`, `parse()` function, `Codegen` struct
- Private: Helper functions like `parse_expr`, `parse_stmt`, etc.

### Assembly Generation

- Use 4-space indentation for assembly instructions
- Keep register usage consistent (t0, t1 for temporaries; a0-a7 for args)
- Document complex code generation strategies if needed

## Common Patterns

### Tree Traversal
```rust
for child in tree.children() {
    Self::collect_locals(child, locals);
}
```

### Stack Operations
```rust
self.push("t0")?;
self.pop("t1")?;
```

### Scope Management
```rust
self.env.push(HashMap::new());
// ... code ...
self.env.pop();
```

## Important Notes

- The compiler uses Pest for parsing; grammar changes must be in `parser.pest`
- All code generation writes to a generic `Write` trait for testability
- Stack frame size must be 16-byte aligned for RISC-V ABI
- Variable offsets are negative relative to frame pointer (fp)
- C tests require RISC-V toolchain: `riscv64-none-elf-gcc`, `spike`, and `pk`

## Running Single Tests

To run a single Rust unit test:
```bash
cargo test parse_respects_precedence
```

To run a single C integration test (manually):
```bash
cargo run -- tests/return_42.c > /tmp/out.S
riscv64-none-elf-gcc -o /tmp/out /tmp/out.S
spike pk /tmp/out
echo $?  # Should match tests/return_42.expect
```
