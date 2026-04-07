use crate::parser::{Op, Tree};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Codegen<W: Write> {
    trees: Vec<Tree>,
    env: Vec<HashMap<String, i64>>,
    stack_offset: i64,
    writer: W,
}

impl<W: Write> Codegen<W> {
    pub fn new(tree: Vec<Tree>, writer: W) -> Self {
        let hm = HashMap::new();
        Self {
            trees: tree,
            env: vec![hm],
            stack_offset: 16,
            writer,
        }
    }

    fn declare(&mut self, name: String) -> i64 {
        self.stack_offset -= 8;
        let offset = self.stack_offset;
        self.env.last_mut().unwrap().insert(name, offset);
        offset
    }

    fn lookup(&self, name: &str) -> Option<i64> {
        for scope in self.env.iter().rev() {
            if let Some(&offset) = scope.get(name) {
                return Some(offset);
            }
        }

        None
    }

    fn count_locals(&self) -> usize {
        let mut locals: HashSet<String> = HashSet::new();
        for tree in self.trees.clone() {
            if let Tree::Assign(lhs, _) = tree
                && let Tree::Var(name) = *lhs
            {
                locals.insert(name);
            }
        }

        locals.len()
    }

    pub fn generate(&mut self) -> io::Result<()> {

        writeln!(self.writer, ".text")?;
        writeln!(self.writer, ".globl main")?;
        writeln!(self.writer, "main:")?;

        self.prologue()?;

        for tree in self.trees.clone() {
            self.gen_expr(&tree)?;
            self.pop("t0")?;
        }

        writeln!(self.writer, "    mv a0, t0")?;

        self.epilogue()?;

        Ok(())
    }

    fn prologue(&mut self) -> io::Result<()> {
        let var_frame_size = (self.count_locals() * 8).div_ceil(16) * 16;

        // Prologue
        writeln!(self.writer, "    addi sp, sp, -16")?;
        writeln!(self.writer, "    sd ra, 8(sp)")?;
        writeln!(self.writer, "    sd fp, 0(sp)")?;
        writeln!(self.writer, "    addi fp, sp, 16")?;

        writeln!(self.writer, "    addi sp, sp, -{}", var_frame_size)?;

        Ok(())
    }

    fn epilogue(&mut self) -> io::Result<()> {
        let var_frame_size = (self.count_locals() * 8).div_ceil(16) * 16;

        // Epilogue
        writeln!(self.writer, "    addi sp, sp, {}", var_frame_size)?;

        writeln!(self.writer, "    ld ra, 8(sp)")?;
        writeln!(self.writer, "    ld fp, 0(sp)")?;
        writeln!(self.writer, "    addi sp, sp, 16")?;
        writeln!(self.writer, "    ret")?;

        Ok(())
    }

    fn gen_expr(&mut self, tree: &Tree) -> io::Result<()> {
        match tree {
            Tree::Integer(n) => {
                writeln!(self.writer, "    li t0, {}", n)?;
                self.push("t0")?;
            }
            Tree::Var(name) => {
                if let Some(offset) = self.lookup(name) {
                    writeln!(self.writer, "    ld t0, {}(fp)", offset)?;
                    self.push("t0")?;
                } else {
                    panic!("Not declared variable: {}", name);
                }
            }
            Tree::Return(inner) => {
                self.gen_expr(inner)?;
                self.pop("a0")?;
                self.epilogue()?;
            }
            Tree::Assign(lhs, rhs) => match **lhs {
                Tree::Var(ref name) => {
                    self.gen_expr(rhs)?;
                    self.pop("t0")?;

                    let offset = if let Some(offset) = self.lookup(name) {
                        offset
                    } else {
                        self.declare(name.to_string())
                    };

                    writeln!(self.writer, "    sd t0, {}(fp)", offset)?;
                    self.push("t0")?;
                }
                _ => panic!("{:?} is not a variable", lhs),
            },
            Tree::BinOp(op, lhs, rhs) => {
                self.gen_expr(lhs)?;
                self.gen_expr(rhs)?;

                self.pop("t1")?;
                self.pop("t0")?;

                match op {
                    Op::Add => writeln!(self.writer, "    add t0, t1, t0")?,
                    Op::Sub => writeln!(self.writer, "    sub t0, t1, t0")?,
                    Op::Mul => writeln!(self.writer, "    mul t0, t1, t0")?,
                    Op::Div => writeln!(self.writer, "    div t0, t1, t0")?,
                    Op::Eq => {
                        writeln!(self.writer, "    sub t0, t1, t0")?;
                        writeln!(self.writer, "    seqz t0, t0")?
                    }
                    Op::NotEq => {
                        writeln!(self.writer, "    sub t0, t1, t0")?;
                        writeln!(self.writer, "    snez t0, t0")?
                    }
                    Op::GreaterThan => writeln!(self.writer, "    slt t0, t0, t1")?,
                    Op::GreaterThanOrEq => {
                        writeln!(self.writer, "    slt t0, t1, t0")?;
                        writeln!(self.writer, "    xori t0, t0, 1")?
                    }
                    Op::LessThan => writeln!(self.writer, "    slt t0, t1, t0")?,
                    Op::LessThanOrEq => {
                        writeln!(self.writer, "    slt t0, t0, t1")?;
                        writeln!(self.writer, "    xori t0, t0, 1")?
                    }
                }

                self.push("t0")?;
            }
            _ => unimplemented!(),
        }

        Ok(())
    }

    fn push(&mut self, reg: &str) -> io::Result<()> {
        writeln!(self.writer, "    addi sp, sp, -8")?;
        writeln!(self.writer, "    sd {}, 0(sp)", reg)?;
        Ok(())
    }

    fn pop(&mut self, reg: &str) -> io::Result<()> {
        writeln!(self.writer, "    ld {}, 0(sp)", reg)?;
        writeln!(self.writer, "    addi sp, sp, 8")?;
        Ok(())
    }
}
