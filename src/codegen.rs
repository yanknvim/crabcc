use crate::parser::{Lowered, Op, Tree, TypedTree};
use crate::types::{Env, Type};
use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Debug)]
pub struct Codegen<W: Write> {
    env: Vec<HashMap<String, (Type, i64)>>,
    globals: Env,
    strings: HashMap<String, String>,
    functions: HashMap<String, (Type, usize)>, // name, type, param
    current_frame_size: usize,
    stack_offset: i64,
    label: usize,
    writer: W,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarLocation {
    Local(i64),
    Global(String),
}

impl<W: Write> Codegen<W> {
    pub fn new(globals: Env, strings: HashMap<String, String>, writer: W) -> Self {
        Self {
            env: Vec::new(),
            globals,
            strings,
            functions: HashMap::new(),
            current_frame_size: 0,
            stack_offset: 0,
            label: 0,
            writer,
        }
    }

    fn declare(&mut self, name: String, ty: Type) -> i64 {
        self.stack_offset -= match ty.clone() {
            Type::Array(inner, size) => inner.size() * size,
            _ => ty.size(),
        } as i64;
        let offset = self.stack_offset;
        self.env.last_mut().unwrap().insert(name, (ty, offset));
        offset
    }

    fn lookup(&self, name: &str) -> Option<(Type, VarLocation)> {
        for scope in self.env.iter().rev() {
            if let Some((ty, offset)) = scope.get(name) {
                return Some((ty.clone(), VarLocation::Local(*offset)));
            }
        }

        self.globals
            .get(name)
            .map(|ty| (ty.clone(), VarLocation::Global(name.to_string())))
    }

    fn collect_locals(tree: &Tree<Lowered>, locals: &mut HashMap<String, Type>) {
        match tree {
            Tree::Assign(lhs, _, _) => {
                if let Tree::Var(name, ty) = &**lhs {
                    locals.insert(name.clone(), ty.clone());
                }
            }
            Tree::VarDeclare(ty, name) => {
                locals.insert(name.clone(), ty.clone());
            }
            _ => {}
        }

        match tree {
            Tree::Program(trees) | Tree::Block(trees) => {
                for child in trees {
                    Self::collect_locals(child, locals);
                }
            }
            Tree::BinOp(_, lhs, rhs, _) | Tree::Assign(lhs, rhs, _) => {
                Self::collect_locals(lhs, locals);
                Self::collect_locals(rhs, locals);
            }
            Tree::If(cond, then_block, else_block) => {
                Self::collect_locals(cond, locals);
                Self::collect_locals(then_block, locals);
                if let Some(else_block) = else_block {
                    Self::collect_locals(else_block, locals);
                }
            }
            Tree::While(cond, body) => {
                Self::collect_locals(cond, locals);
                Self::collect_locals(body, locals);
            }
            Tree::For(init, cond, update, body) => {
                if let Some(init) = init {
                    Self::collect_locals(init, locals);
                }
                if let Some(cond) = cond {
                    Self::collect_locals(cond, locals);
                }
                if let Some(update) = update {
                    Self::collect_locals(update, locals);
                }
                Self::collect_locals(body, locals);
            }
            Tree::FuncDef(_, _, _, body) => {
                Self::collect_locals(body, locals);
            }
            Tree::Addr(expr, _) | Tree::Deref(expr, _) | Tree::Return(expr, _) => {
                Self::collect_locals(expr, locals);
            }
            Tree::Call(_, args, _) => {
                for arg in args {
                    Self::collect_locals(arg, locals);
                }
            }
            _ => {}
        }
    }

    pub fn generate(&mut self, tree: &Tree<Lowered>) -> io::Result<()> {
        let trees = match tree {
            Tree::Program(trees) => trees,
            _ => panic!("top-level tree must be Program"),
        };

        writeln!(self.writer, ".data")?;
        writeln!(self.writer, ".align 4")?;
        for (name, ty) in &self.globals {
            writeln!(self.writer, "{}:", name)?;
            writeln!(self.writer, "    .space {}", ty.size())?;
        }

        for (label, s) in &self.strings {
            writeln!(self.writer, "{}:", label)?;
            writeln!(self.writer, "    .ascii \"{}\"", s)?;
        }

        writeln!(self.writer, ".text")?;
        writeln!(self.writer, ".globl main")?;

        self.functions.clear();
        let mut has_main = false;
        for tree in trees {
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

        for tree in trees {
            self.gen_func(tree)?;
        }

        Ok(())
    }

    fn prologue(&mut self, frame_size: usize) -> io::Result<()> {
        writeln!(self.writer, "    addi sp, sp, -16")?;
        writeln!(self.writer, "    sd ra, 8(sp)")?;
        writeln!(self.writer, "    sd fp, 0(sp)")?;
        writeln!(self.writer, "    addi fp, sp, 0")?;

        writeln!(self.writer, "    li t0, -{}", frame_size)?;
        writeln!(self.writer, "    add sp, sp, t0")?;

        Ok(())
    }

    fn epilogue(&mut self, frame_size: usize) -> io::Result<()> {
        writeln!(self.writer, "    li t0, {}", frame_size)?;
        writeln!(self.writer, "    add sp, sp, t0")?;

        writeln!(self.writer, "    ld ra, 8(sp)")?;
        writeln!(self.writer, "    ld fp, 0(sp)")?;
        writeln!(self.writer, "    addi sp, sp, 16")?;
        writeln!(self.writer, "    ret")?;

        Ok(())
    }

    fn get_frame_size(types: Vec<&Type>) -> usize {
        types.iter().map(|ty| ty.size()).sum()
    }

    fn gen_func(&mut self, tree: &Tree<Lowered>) -> io::Result<()> {
        match tree {
            Tree::FuncDef(_ty, name, params, body) => {
                self.env = vec![HashMap::new()];
                self.stack_offset = 0;

                writeln!(self.writer, "{}:", name)?;

                let mut locals: HashMap<String, Type> = HashMap::new();
                Self::collect_locals(body, &mut locals);

                let locals_size = Self::get_frame_size(locals.values().collect::<Vec<_>>());
                let params_size = Self::get_frame_size(params.iter().map(|(ty, _)| ty).collect());
                let frame_size = (locals_size + params_size + 15) & !15;
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

    fn gen_expr(&mut self, tree: &Tree<Lowered>) -> io::Result<()> {
        match tree {
            Tree::Integer(n, _) => {
                writeln!(self.writer, "    li t0, {}", n)?;
                self.push("t0")?;
            }
            Tree::Var(name, _ty) => match self.lookup(name) {
                Some((ty, VarLocation::Local(offset))) => {
                    self.emit_load("t0", "fp", offset, &ty)?;
                    self.push("t0")?;
                }
                Some((ty, VarLocation::Global(name))) => {
                    writeln!(self.writer, "    la t0, {}", name)?;
                    self.emit_load("t0", "t0", 0, &ty)?;
                    self.push("t0")?;
                }
                None => panic!("Not declared variable: {}", name),
            },
            Tree::String(label, _ty) => {
                writeln!(self.writer, "    la t0, {}", label)?;
                self.push("t0")?;
            }
            Tree::Addr(expr, _) => {
                self.gen_lvalue(expr)?;
            }
            Tree::Deref(expr, ty) => {
                self.gen_expr(expr)?;
                self.pop("t0")?;
                self.emit_load("t0", "t0", 0, ty)?;
                self.push("t0")?;
            }
            Tree::Call(name, args, _) => {
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
            Tree::Assign(lhs, rhs, _) => {
                let ty = self.gen_lvalue(lhs)?;
                self.gen_expr(rhs)?;
                self.pop("t1")?;
                self.pop("t0")?;
                self.emit_store("t1", "t0", 0, &ty)?;

                self.push("t1")?;
            }
            Tree::BinOp(op, lhs, rhs, _ty) => {
                self.gen_expr(lhs)?;
                self.gen_expr(rhs)?;

                self.pop("t1")?; // rhs
                self.pop("t0")?; // lhs

                // Handle pointer arithmetic scaling: when one side is a pointer and the
                // other is an integer we must scale the integer by the element size.
                match op {
                    Op::Add => {
                        match (lhs.ty().clone(), rhs.ty().clone()) {
                            (Type::Ptr(inner), Type::Int) => {
                                // t0: pointer, t1: int -> scale t1 then add
                                writeln!(self.writer, "    li t2, {}", inner.size())?;
                                writeln!(self.writer, "    mul t1, t1, t2")?;
                                writeln!(self.writer, "    add t0, t0, t1")?;
                            }
                            (Type::Int, Type::Ptr(inner)) => {
                                // t0: int, t1: pointer -> scale t0 then add to pointer
                                writeln!(self.writer, "    li t2, {}", inner.size())?;
                                writeln!(self.writer, "    mul t0, t0, t2")?;
                                writeln!(self.writer, "    add t0, t1, t0")?;
                            }
                            _ => {
                                writeln!(self.writer, "    add t0, t1, t0")?;
                            }
                        }
                    }
                    Op::Sub => {
                        match (lhs.ty().clone(), rhs.ty().clone()) {
                            (Type::Ptr(inner), Type::Int) => {
                                // pointer - int: scale int then subtract
                                writeln!(self.writer, "    li t2, {}", inner.size())?;
                                writeln!(self.writer, "    mul t1, t1, t2")?;
                                writeln!(self.writer, "    sub t0, t0, t1")?;
                            }
                            (Type::Ptr(inner_l), Type::Ptr(_)) => {
                                // pointer - pointer => (addr_l - addr_r) / elem_size
                                writeln!(self.writer, "    sub t0, t0, t1")?;
                                writeln!(self.writer, "    li t2, {}", inner_l.size())?;
                                writeln!(self.writer, "    div t0, t0, t2")?;
                            }
                            _ => {
                                writeln!(self.writer, "    sub t0, t0, t1")?;
                            }
                        }
                    }
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

    fn gen_stmt(&mut self, tree: &Tree<Lowered>) -> io::Result<()> {
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
                let current_label = self.label;
                self.label += 1;

                self.gen_expr(cond)?;
                self.pop("t0")?;

                writeln!(self.writer, "    beq t0, x0, Else{}", current_label)?;
                self.gen_stmt(a)?;

                if let Some(b) = b {
                    writeln!(self.writer, "    jal x0, End{}", current_label)?;
                    writeln!(self.writer, "Else{}:", current_label)?;
                    self.gen_stmt(b)?;
                    writeln!(self.writer, "End{}:", current_label)?;
                } else {
                    writeln!(self.writer, "Else{}:", current_label)?;
                }
            }
            Tree::While(cond, stmt) => {
                let current_label = self.label;
                self.label += 1;

                writeln!(self.writer, "Begin{}:", current_label)?;

                self.gen_expr(cond)?;
                self.pop("t0")?;

                writeln!(self.writer, "    beq t0, x0, End{}", current_label)?;
                self.gen_stmt(stmt)?;

                writeln!(self.writer, "    jal x0, Begin{}", current_label)?;

                writeln!(self.writer, "End{}:", current_label)?;
            }
            Tree::For(init, cond, update, stmt) => {
                let current_label = self.label;
                self.label += 1;

                if let Some(init) = init {
                    self.gen_expr(init)?;
                    self.pop("t0")?;
                }

                writeln!(self.writer, "Begin{}:", current_label)?;

                if let Some(cond) = cond {
                    self.gen_expr(cond)?;
                    self.pop("t0")?;
                    writeln!(self.writer, "    beq t0, x0, End{}", current_label)?;
                }

                self.gen_stmt(stmt)?;

                if let Some(update) = update {
                    self.gen_expr(update)?;
                    self.pop("t0")?;
                }

                writeln!(self.writer, "    jal x0, Begin{}", current_label)?;

                writeln!(self.writer, "End{}:", current_label)?;
            }
            Tree::Return(expr, _) => {
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

    fn gen_lvalue(&mut self, tree: &Tree<Lowered>) -> io::Result<Type> {
        match tree {
            Tree::Var(name, ty) => {
                let location = self
                    .lookup(name)
                    .unwrap_or_else(|| panic!("Not declared variable: {}", name));

                match location {
                    (_ty, VarLocation::Local(offset)) => {
                        writeln!(self.writer, "    li t1, {}", offset)?;
                        writeln!(self.writer, "    add t0, fp, t1")?;
                        self.push("t0")?;
                    }
                    (_ty, VarLocation::Global(name)) => {
                        writeln!(self.writer, "    la t0, {}", name)?;
                        self.push("t0")?;
                    }
                }

                Ok(ty.clone())
            }
            Tree::Deref(expr, ty) => {
                self.gen_expr(expr)?;
                Ok(ty.clone())
            }
            _ => panic!("not an lvalue"),
        }
    }

    fn emit_load(&mut self, reg: &str, base: &str, offset: i64, ty: &Type) -> io::Result<()> {
        writeln!(self.writer, "    li t2, {offset}")?;
        writeln!(self.writer, "    add t2, t2, {base}")?;

        match ty.size() {
            1 => writeln!(self.writer, "    lb {reg}, 0(t2)")?,
            4 => writeln!(self.writer, "    lw {reg}, 0(t2)")?,
            8 => writeln!(self.writer, "    ld {reg}, 0(t2)")?,
            _ => unreachable!(),
        }

        Ok(())
    }

    fn emit_store(&mut self, reg: &str, base: &str, offset: i64, ty: &Type) -> io::Result<()> {
        writeln!(self.writer, "    li t2, {offset}")?;
        writeln!(self.writer, "    add t2, t2, {base}")?;

        match ty.size() {
            1 => writeln!(self.writer, "    sb {reg}, 0(t2)")?,
            4 => writeln!(self.writer, "    sw {reg}, 0(t2)")?,
            8 => writeln!(self.writer, "    sd {reg}, 0(t2)")?,
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
