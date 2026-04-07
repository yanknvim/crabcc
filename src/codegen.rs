use crate::parser::{Op, Tree};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Codegen<W: Write> {
    trees: Vec<Tree>,
    writer: W,
}

impl<W: Write> Codegen<W> {
    pub fn new(tree: Vec<Tree>, writer: W) -> Self {
        Self {
            trees: tree,
            writer,
        }
    }

    pub fn generate(&mut self) -> io::Result<()> {
        writeln!(self.writer, ".text")?;
        writeln!(self.writer, ".globl main")?;
        writeln!(self.writer, "main:")?;

        for tree in self.trees.clone() {
            self.gen_expr(&tree)?;
            self.push("t0")?;
        }

        self.pop("a0")?;
        writeln!(self.writer, "    ret")?;

        Ok(())
    }

    fn gen_expr(&mut self, tree: &Tree) -> io::Result<()> {
        match tree {
            Tree::Integer(n) => {
                writeln!(self.writer, "    li t0, {}", n)?;
                self.push("t0")?;
            }
            Tree::BinOp(op, lhs, rhs) => {
                self.gen_expr(lhs)?;
                writeln!(self.writer, "    mv t1, t0")?;

                self.gen_expr(rhs)?;

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
