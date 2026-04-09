use crate::parser::{Op, Tree};
use crate::types::Type;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Codegen<W: Write> {
    trees: Vec<Tree>,
    env: Vec<HashMap<String, (Type, i64)>>,
    functions: HashMap<String, (Type, usize)>, // name, type, param
    current_frame_size: usize,
    stack_offset: i64,
    label: usize,
    writer: W,
}

impl<W: Write> Codegen<W> {
    pub fn new(tree: Tree, writer: W) -> Self {
        let trees = match tree {
            Tree::Program(trees) => trees,
            _ => panic!("top-level tree must be Program"),
        };
        Self {
            trees,
            env: Vec::new(),
            functions: HashMap::new(),
            current_frame_size: 0,
            stack_offset: 0,
            label: 0,
            writer,
        }
    }

    fn declare(&mut self, name: String, ty: Type) -> i64 {
        self.stack_offset -= 8;
        let offset = self.stack_offset;
        self.env.last_mut().unwrap().insert(name, (ty, offset));
        offset
    }

    fn lookup(&self, name: &str) -> Option<(Type, i64)> {
        for scope in self.env.iter().rev() {
            if let Some((ty, offset)) = scope.get(name) {
                return Some((ty.clone(), *offset));
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
            Tree::VarDeclare(_ty, name) => {
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
            if let Tree::FuncDef(ty, name, params, _) = tree {
                if params.len() > 8 {
                    panic!("Too much params: {}", name);
                }
                if self.functions.contains_key(name) {
                    panic!("Double Declation of function: {}", name);
                }
                if name == "main" {
                    has_main = true;
                }
                self.functions
                    .insert(name.to_string(), (ty.clone(), params.len()));
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
            Tree::FuncDef(_ty, name, params, body) => {
                self.env = vec![HashMap::new()];
                self.stack_offset = 0;

                writeln!(self.writer, "{}:", name)?;

                let mut locals: HashSet<String> = HashSet::new();
                Self::collect_locals(body, &mut locals);

                let frame_size = ((locals.len() + params.len()) * 8).div_ceil(16) * 16;
                self.current_frame_size = frame_size;

                self.prologue(frame_size)?;

                for (index, param) in params.iter().enumerate() {
                    let (ty, name) = param;
                    let offset = self.declare(name.to_string(), ty.clone());
                    self.emit_store(&format!("a{}", index), "fp", offset, ty)?;
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
                if let Some((ty, offset)) = self.lookup(name) {
                    self.emit_load("t0", "fp", offset, &ty)?;
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
                self.emit_load("t0", "t0", 0, self.expr_type(expr).deref().unwrap())?;
                self.push("t0")?;
            }
            Tree::Call(name, args) => {
                if args.len() > 8 {
                    panic!("Too much args: {}", name);
                }

                match self.functions.get(name) {
                    Some((_, params)) if *params == args.len() => {
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
                let ty = self.gen_lvalue(lhs)?;
                self.gen_expr(rhs)?;
                self.pop("t1")?;
                self.pop("t0")?;
                self.emit_store("t1", "t0", 0, &ty)?;

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
                    Op::Gt => writeln!(self.writer, "    slt t0, t1, t0")?,
                    Op::Gte => {
                        writeln!(self.writer, "    slt t0, t0, t1")?;
                        writeln!(self.writer, "    xori t0, t0, 1")?
                    }
                    Op::Lt => writeln!(self.writer, "    slt t0, t0, t1")?,
                    Op::Lte => {
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
            Tree::VarDeclare(ty, name) => {
                self.declare(name.to_string(), ty.clone());
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

    fn gen_lvalue(&mut self, tree: &Tree) -> io::Result<Type> {
        match tree {
            Tree::Var(name) => {
                let (ty, offset) = self
                    .lookup(name)
                    .unwrap_or_else(|| panic!("Not declared variable: {}", name));
                writeln!(self.writer, "    addi t0, fp, {}", offset)?;
                self.push("t0")?;

                Ok(ty)
            }
            Tree::Deref(expr) => {
                let inner_type = self.expr_type(expr);
                self.gen_expr(expr)?;
                Ok(inner_type
                    .deref()
                    .unwrap_or_else(|| panic!("{:?} is not ptr", expr))
                    .clone())
            }
            _ => panic!("not an lvalue"),
        }
    }

    fn expr_type(&self, tree: &Tree) -> Type {
        match tree {
            Tree::Var(name) => {
                let (ty, _) = self.lookup(name).expect("Not in env");
                ty
            }
            Tree::Addr(expr) => {
                let inner_type = self.expr_type(expr);
                Type::Ptr(Box::new(inner_type))
            }
            Tree::Deref(expr) => {
                let inner_type = self.expr_type(expr);
                match inner_type {
                    Type::Ptr(_) => inner_type.deref().unwrap().clone(),
                    _ => panic!("{:?} is not ptr", expr),
                }
            }
            Tree::Call(name, _) => {
                let (ty, _) = self.functions.get(name).expect("Not declared function");
                ty.clone()
            }
            Tree::BinOp(_, _, _) => Type::Int,
            Tree::Assign(lhs, _) => self.expr_type(lhs),
            Tree::Integer(_) => Type::Int,
            _ => unimplemented!(),
        }
    }

    fn emit_load(&mut self, reg: &str, base: &str, offset: i64, ty: &Type) -> io::Result<()> {
        match ty.size() {
            4 => writeln!(self.writer, "    lw {}, {}({})", reg, offset, base)?,
            8 => writeln!(self.writer, "    ld {}, {}({})", reg, offset, base)?,
            _ => unreachable!(),
        }

        Ok(())
    }

    fn emit_store(&mut self, reg: &str, base: &str, offset: i64, ty: &Type) -> io::Result<()> {
        match ty.size() {
            4 => writeln!(self.writer, "    sw {}, {}({})", reg, offset, base)?,
            8 => writeln!(self.writer, "    sd {}, {}({})", reg, offset, base)?,
            _ => unreachable!(),
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
