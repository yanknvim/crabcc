use crate::parser::{Op, Tree};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Codegen<W: Write> {
    trees: Vec<Tree>,
    env: Vec<HashMap<String, i64>>,
    functions: HashMap<String, usize>, // name, param
    current_frame_size: usize,
    stack_offset: i64,
    label: usize,
    writer: W,
}

impl<W: Write> Codegen<W> {
    pub fn new(tree: Vec<Tree>, writer: W) -> Self {
        Self {
            trees: tree,
            env: Vec::new(),
            functions: HashMap::new(),
            current_frame_size: 0,
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
        match tree {
            Tree::Assign(lhs, _) => {
                if let Tree::Var(name) = &**lhs {
                    locals.insert(name.clone());
                }
            }
            Tree::VarDeclare(name) => {
                locals.insert(name.clone());
            }
            _ => {}
        }

        for child in tree.children() {
            Self::collect_locals(child, locals);
        }
    }

    pub fn generate(&mut self) -> io::Result<()> {
        writeln!(self.writer, ".text")?;
        writeln!(self.writer, ".globl main")?;

        self.functions.clear();
        let mut has_main = false;
        for tree in &self.trees {
            if let Tree::FuncDef(name, params, _) = tree {
                if params.len() > 8 {
                    panic!("Too much params: {}", name);
                }
                if self.functions.contains_key(name) {
                    panic!("Double Declation of function: {}", name);
                }
                if name == "main" {
                    has_main = true;
                }
                self.functions.insert(name.to_string(), params.len());
            }
        }
        if !has_main {
            panic!("main function is missing");
        }

        for tree in self.trees.clone() {
            self.gen_func(&tree)?;
        }

        Ok(())
    }

    fn prologue(&mut self, frame_size: usize) -> io::Result<()> {
        writeln!(self.writer, "    addi sp, sp, -16")?;
        writeln!(self.writer, "    sd ra, 8(sp)")?;
        writeln!(self.writer, "    sd fp, 0(sp)")?;
        writeln!(self.writer, "    addi fp, sp, 0")?;

        writeln!(self.writer, "    addi sp, sp, -{}", frame_size)?;

        Ok(())
    }

    fn epilogue(&mut self, frame_size: usize) -> io::Result<()> {
        writeln!(self.writer, "    addi sp, sp, {}", frame_size)?;

        writeln!(self.writer, "    ld ra, 8(sp)")?;
        writeln!(self.writer, "    ld fp, 0(sp)")?;
        writeln!(self.writer, "    addi sp, sp, 16")?;
        writeln!(self.writer, "    ret")?;

        Ok(())
    }

    fn gen_func(&mut self, tree: &Tree) -> io::Result<()> {
        match tree {
            Tree::FuncDef(name, params, body) => {
                self.env = vec![HashMap::new()];
                self.stack_offset = 0;

                writeln!(self.writer, "{}:", name)?;

                let mut locals: HashSet<String> = HashSet::new();
                Self::collect_locals(body, &mut locals);

                let frame_size = ((locals.len() + params.len()) * 8).div_ceil(16) * 16;
                self.current_frame_size = frame_size;

                self.prologue(frame_size)?;

                for (index, param) in params.iter().enumerate() {
                    let offset = self.declare(param.to_string());
                    writeln!(self.writer, "    sd a{}, {}(fp)", index, offset)?;
                }

                self.gen_stmt(body)?;

                self.epilogue(frame_size)?;
                self.current_frame_size = 0;
            }
            _ => unreachable!(),
        }

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
            Tree::Addr(expr) => {
                self.gen_lvalue(expr)?;
            }
            Tree::Deref(expr) => {
                self.gen_expr(expr)?;
                self.pop("t0")?;
                writeln!(self.writer, "    ld t0, 0(t0)")?;
                self.push("t0")?;
            }
            Tree::Call(name, args) => {
                if args.len() > 8 {
                    panic!("Too much args: {}", name);
                }

                match self.functions.get(name) {
                    Some(params) if *params == args.len() => {
                        for arg in args {
                            self.gen_expr(arg)?;
                        }

                        for (i, _) in args.iter().enumerate().rev() {
                            self.pop(&format!("a{}", i))?;
                        }

                        writeln!(self.writer, "    call {}", name)?;
                        self.push("a0")?;
                    }
                    Some(_) => panic!("Invalid number of args"),
                    None => panic!("{} is not declared", name),
                }
            }
            Tree::Assign(lhs, rhs) => {
                self.gen_lvalue(lhs)?;
                self.gen_expr(rhs)?;
                self.pop("t1")?;
                self.pop("t0")?;
                writeln!(self.writer, "    sd t1, 0(t0)")?;
                self.push("t1")?;
            }
            Tree::BinOp(op, lhs, rhs) => {
                self.gen_expr(lhs)?;
                self.gen_expr(rhs)?;

                self.pop("t1")?;
                self.pop("t0")?;

                match op {
                    Op::Add => writeln!(self.writer, "    add t0, t1, t0")?,
                    Op::Sub => writeln!(self.writer, "    sub t0, t0, t1")?,
                    Op::Mul => writeln!(self.writer, "    mul t0, t1, t0")?,
                    Op::Div => writeln!(self.writer, "    div t0, t0, t1")?,
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
            Tree::Block(trees) => {
                self.env.push(HashMap::new());

                for tree in trees {
                    self.gen_stmt(tree)?;
                }

                self.env.pop();
            }
            Tree::VarDeclare(name) => {
                self.declare(name.to_string());
            }
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
                self.epilogue(self.current_frame_size)?;
            }
            _ => {
                self.gen_expr(tree)?;
                self.pop("t0")?;
            }
        }

        Ok(())
    }

    fn gen_lvalue(&mut self, tree: &Tree) -> io::Result<()> {
        match tree {
            Tree::Var(name) => {
                let offset = self
                    .lookup(name)
                    .unwrap_or_else(|| panic!("Not declared variable: {}", name));
                writeln!(self.writer, "    addi t0, fp, {}", offset)?;
                self.push("t0")?;
            }
            Tree::Deref(expr) => {
                self.gen_expr(expr)?;
            }
            _ => panic!("not an lvalue"),
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
