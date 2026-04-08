use crate::parser::{Op, Tree};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Codegen<W: Write> {
    trees: Vec<Tree>,
    env: Vec<HashMap<String, i64>>,
    stack_offset: i64,
    label: usize,
    writer: W,
}

impl<W: Write> Codegen<W> {
    pub fn new(tree: Vec<Tree>, writer: W) -> Self {
        let hm = HashMap::new();
        Self {
            trees: tree,
            env: vec![hm],
            stack_offset: 0,
            label: 0,
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

    fn collect_locals(tree: &Tree, locals: &mut HashSet<String>) {
        if let Tree::Assign(lhs, _) = tree
        && let Tree::Var(name) = &**lhs {
            locals.insert(name.clone());
        }

        for child in tree.children() {
            Self::collect_locals(child, locals);
        }
    }

    fn count_locals(&self) -> usize {
        let mut locals: HashSet<String> = HashSet::new();
        for tree in &self.trees {
            Self::collect_locals(tree, &mut locals);
        }

        locals.len()
    }

    pub fn generate(&mut self) -> io::Result<()> {
        writeln!(self.writer, ".text")?;
        writeln!(self.writer, ".globl main")?;
        writeln!(self.writer, "main:")?;

        self.prologue()?;

        for tree in self.trees.clone() {
            self.gen_stmt(&tree)?;
        }

        self.epilogue()?;

        Ok(())
    }

    fn prologue(&mut self) -> io::Result<()> {
        let var_frame_size = (self.count_locals() * 8).div_ceil(16) * 16;

        // Prologue
        writeln!(self.writer, "    addi sp, sp, -16")?;
        writeln!(self.writer, "    sd ra, 8(sp)")?;
        writeln!(self.writer, "    sd fp, 0(sp)")?;
        writeln!(self.writer, "    addi fp, sp, 0")?;

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
                    Op::GreaterThan => writeln!(self.writer, "    slt t0, t1, t0")?,
                    Op::GreaterThanOrEq => {
                        writeln!(self.writer, "    slt t0, t0, t1")?;
                        writeln!(self.writer, "    xori t0, t0, 1")?
                    }
                    Op::LessThan => writeln!(self.writer, "    slt t0, t0, t1")?,
                    Op::LessThanOrEq => {
                        writeln!(self.writer, "    slt t0, t1, t0")?;
                        writeln!(self.writer, "    xori t0, t0, 1")?
                    }
                }

                self.push("t0")?;
            }
            _ => unimplemented!("expr only"),
        }

        Ok(())
    }

    fn gen_stmt(&mut self, tree: &Tree) -> io::Result<()> {
        match tree {
            Tree::If(cond, a, b) => {
                self.gen_expr(cond)?;
                self.pop("t0")?;

                writeln!(self.writer, "    beq t0, x0, Else{}", self.label)?;
                self.gen_stmt(a)?;

                if let Some(b) = b {
                    writeln!(self.writer, "    jal x0, End{}", self.label)?;
                    writeln!(self.writer, "Else{}:", self.label)?;
                    self.gen_stmt(b)?;
                    writeln!(self.writer, "End{}:", self.label)?;
                } else {
                    writeln!(self.writer, "Else{}:", self.label)?;
                }

                self.label += 1;
            }
            Tree::While(cond, stmt) => {
                writeln!(self.writer, "Begin{}:", self.label)?;

                self.gen_expr(cond)?;
                self.pop("t0")?;

                writeln!(self.writer, "    beq t0, x0, End{}", self.label)?;
                self.gen_stmt(stmt)?;

                writeln!(self.writer, "    jal x0, Begin{}", self.label)?;

                writeln!(self.writer, "End{}:", self.label)?;

                self.label += 1;
            }
            Tree::For(init, cond, update, stmt) => {
                if let Some(init) = init {
                    self.gen_expr(init)?;
                    self.pop("t0")?;
                }

                writeln!(self.writer, "Begin{}:", self.label)?;

                if let Some(cond) = cond {
                    self.gen_expr(cond)?;
                    self.pop("t0")?;
                    writeln!(self.writer, "    beq t0, x0, End{}", self.label)?;
                }

                self.gen_stmt(stmt)?;

                if let Some(update) = update {
                    self.gen_expr(update)?;
                    self.pop("t0")?;
                }

                writeln!(self.writer, "    jal x0, Begin{}", self.label)?;

                writeln!(self.writer, "End{}:", self.label)?;

                self.label += 1;
            }
            Tree::Return(expr) => {
                self.gen_expr(expr)?;
                self.pop("a0")?;
                self.epilogue()?;
            }
            _ => {
                self.gen_expr(tree)?;
                self.pop("t0")?;
            }
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
