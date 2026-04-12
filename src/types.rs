use std::collections::HashMap;
use crate::parser::{Tree, Parsed, Typed, TypedTree, Op};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Char,
    Ptr(Box<Type>),
    Array(Box<Type>, usize),
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Self::Int => 4,
            Self::Char => 1,
            Self::Ptr(_) => 8,
            Self::Array(inner, size) => Self::size(inner) * size,
        }
    }
}

pub type Env = HashMap<String, Type>;

pub struct TypeChecker {
    env: Vec<Env>,
    functions: HashMap<String, (Type, Vec<Type>)>,
    strings: HashMap<String, String>,
    globals: Env,
    current_return: Option<Type>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: Vec::new(),
            functions: HashMap::new(),
            strings: HashMap::new(),
            globals: HashMap::new(),
            current_return: None,
        }
    }

    pub fn globals(&self) -> &Env {
        &self.globals
    }

    pub fn strings(&self) -> &HashMap<String, String> {
        &self.strings
    }

    pub fn check(&mut self, tree: &Tree<Parsed>) -> Tree<Typed> {
        self.collect_functions(tree);
        self.check_program(tree)
    }

    fn collect_functions(&mut self, tree: &Tree<Parsed>) {
        let trees = match tree {
            Tree::Program(trees) => trees,
            _ => panic!("top-level tree must be Program"),
        };

        self.functions.clear();
        for tree in trees {
            if let Tree::FuncDef(ty, name, params, body) = tree {
                if self.functions.contains_key(name) {
                    panic!("double declaration of function: {}", name);
                }
                let param_types = params.iter().map(|(ty, _)| ty.clone()).collect();
                self.functions
                    .insert(name.to_string(), (ty.clone(), param_types));
            }
        }
    }

    fn enter_scope(&mut self) {
        self.env.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.env.pop();
    }

    fn declare(&mut self, name: String, ty: Type) {
        let scope = self.env.last_mut().expect("no scope for declaration");
        if scope.contains_key(&name) {
            panic!("double declaration of variable: {}", name);
        }
        scope.insert(name, ty);
    }

    fn lookup(&self, name: &str) -> Option<Type> {
        for scope in self.env.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }

        self.globals.get(name).cloned()
    }

    fn check_lvalue(typed: &Tree<Typed>) {
        match typed {
            Tree::Var(_, _) | Tree::Deref(_, _) => {}
            _ => panic!("not an lvalue"),
        }
    }

    fn check_program(&mut self, tree: &Tree<Parsed>) -> Tree<Typed> {
        if let Tree::Program(trees) = tree {
            Tree::Program(
                trees
                    .iter()
                    .filter_map(|tree| match tree {
                        Tree::FuncDef(ty, name, params, body) => {
                            let prev_return = self.current_return.take();
                            self.current_return = Some(ty.clone());

                            self.enter_scope();
                            for (param_ty, param_name) in params {
                                self.declare(param_name.to_string(), param_ty.clone());
                            }
                            let body = self.check_tree(body);
                            self.exit_scope();

                            self.current_return = prev_return;

                            Some(Tree::FuncDef(
                                ty.clone(),
                                name.to_string(),
                                params.to_vec(),
                                Box::new(body),
                            ))
                        }
                        Tree::VarDeclare(ty, name) => {
                            if self.globals.insert(name.to_string(), ty.clone()).is_some() {
                                panic!("duplicate of global var: {}", name);
                            } else {
                                None
                            }
                        }
                        _ => panic!("invalid top-level tree"),
                    })
                    .collect(),
            )
        } else {
            panic!("top-level must be program");
        }
    }

    fn check_tree(&mut self, tree: &Tree<Parsed>) -> Tree<Typed> {
        match tree {
            Tree::Sizeof(expr) => {
                let size = match &**expr {
                    Tree::Var(name, _) => self
                        .lookup(&name)
                        .unwrap_or_else(|| panic!("not declared variable: {}", name))
                        .size(),
                    _ => {
                        let expr = self.check_tree(expr);
                        expr.ty().size()
                    }
                };
                Tree::Integer(size as i64, Type::Int)
            }
            Tree::BinOp(op, lhs, rhs, _) => {
                let lhs = self.check_tree(lhs);
                let rhs = self.check_tree(rhs);

                self.check_binop(op, lhs, rhs)
            }
            Tree::Assign(lhs, rhs, _) => {
                let lhs = self.check_tree(lhs);
                let rhs = self.check_tree(rhs);
                let ty = lhs.ty();

                Self::check_lvalue(&lhs);

                match (ty, rhs.ty()) {
                    (_, _) if ty == rhs.ty() => {}
                    (Type::Int, Type::Char) => {}
                    (Type::Char, Type::Int) => {}
                    _ => panic!("assign type mismatch: {:?} = {:?}", ty, rhs.ty()),
                }

                Tree::Assign(Box::new(lhs.clone()), Box::new(rhs), ty.clone())
            }
            Tree::Block(stmts) => Tree::Block({
                self.enter_scope();
                let checked = stmts.iter().map(|s| self.check_tree(s)).collect();
                self.exit_scope();
                checked
            }),
            Tree::If(cond, block_a, block_b) => {
                let cond = self.check_tree(cond);
                if cond.ty() != &Type::Int {
                    panic!("if condition must be int");
                }
                let block_a = self.check_tree(block_a);
                let block_b = block_b.as_ref().map(|t| self.check_tree(t));

                Tree::If(Box::new(cond), Box::new(block_a), block_b.map(Box::new))
            }
            Tree::While(cond, body) => {
                let cond = self.check_tree(cond);
                if cond.ty() != &Type::Int {
                    panic!("while condition must be int");
                }
                let body = self.check_tree(body);

                Tree::While(Box::new(cond), Box::new(body))
            }
            Tree::For(init, cond, update, body) => {
                let init = init.as_ref().map(|t| self.check_tree(t));
                let cond = cond.as_ref().map(|t| self.check_tree(t));
                let update = update.as_ref().map(|t| self.check_tree(t));

                if let Some(cond) = &cond
                    && cond.ty() != &Type::Int
                {
                    panic!("for condition must be int");
                }

                let body = self.check_tree(body);

                Tree::For(
                    init.map(Box::new),
                    cond.map(Box::new),
                    update.map(Box::new),
                    Box::new(body),
                )
            }
            Tree::Integer(n, _) => Tree::Integer(*n, Type::Int),
            Tree::String(s, _) => {
                let label = self.add_string_literal(s);
                Tree::String(label, Type::Ptr(Box::new(Type::Char)))
            }
            Tree::Var(name, _) => {
                let ty = self
                    .lookup(&name)
                    .unwrap_or_else(|| panic!("not declared variable: {}", name));
                match ty.clone() {
                    Type::Array(inner, _) => Tree::Addr(
                        Box::new(Tree::Var(name.to_string(), *inner.clone())),
                        Type::Ptr(Box::new(*inner)),
                    ),
                    _ => Tree::Var(name.to_string(), ty),
                }
            }
            Tree::Indexed(lhs, rhs, _) => {
                let lhs = self.check_tree(lhs);
                let rhs = self.check_tree(rhs);

                let binop = self.check_binop(&Op::Add, lhs.clone(), rhs.clone());
                match binop.ty() {
                    Type::Ptr(inner) => Tree::Indexed(Box::new(lhs), Box::new(rhs), binop.ty.clone()),
                    _ => panic!("index access for not pointer"),
                }
            }
            Tree::VarDeclare(ty, name) => {
                self.declare(name.to_string(), ty.clone());
                Tree::VarDeclare(ty.clone(), name.to_string())
            }
            Tree::Addr(expr, _) => {
                let expr = self.check_tree(expr);
                Self::check_lvalue(&expr);
                let expr_ty = expr.ty().clone();
                Tree::Addr(Box::new(expr), Type::Ptr(Box::new(expr_ty)))
            }
            Tree::Deref(expr, _) => {
                let expr = self.check_tree(expr);
                match expr.ty().clone() {
                    Type::Ptr(inner) => Tree::Deref(Box::new(expr), *inner),
                    _ => panic!("{:?} is not ptr", expr.ty()),
                }
            }
            Tree::Call(name, args, _) => {
                let (ret_ty, params) = self
                    .functions
                    .get(name)
                    .unwrap_or_else(|| panic!("{} is not declared", name))
                    .clone();
                if params.len() != args.len() {
                    panic!("invalid number of args for {}", name);
                }
                let checked_args: Vec<_> = args.iter().map(|arg| self.check_tree(arg)).collect();
                for (index, (arg, param_ty)) in checked_args.iter().zip(params.iter()).enumerate() {
                    if arg.ty() != param_ty {
                        panic!("arg {} type mismatch for {}", index, name);
                    }
                }
                Tree::Call(name.to_string(), checked_args, ret_ty)
            }
            Tree::Return(expr, _) => {
                let expr = self.check_tree(expr);
                self.current_return
                    .as_ref()
                    .expect("return outside of function");
                let ret_ty = expr.ty().clone();
                Tree::Return(Box::new(expr), ret_ty)
            }
            _ => unreachable!(),
        }
    }

    fn check_binop(&self, op: &Op, lhs: Tree<Typed>, rhs: Tree<Typed>) -> Tree<Typed> {
        let lhs_type = lhs.ty().clone();
        let rhs_type = rhs.ty().clone();

        match (op, &lhs_type, &rhs_type) {
            (_, Type::Int | Type::Char, Type::Int | Type::Char) => {
                Tree::BinOp(op.clone(), Box::new(lhs), Box::new(rhs), Type::Int)
            }
            (Op::Add | Op::Sub, Type::Ptr(ty), Type::Int) => Tree::BinOp(
                op.clone(),
                Box::new(lhs),
                Box::new(rhs),
                Type::Ptr(ty.clone()),
            ),
            (Op::Add, Type::Int, Type::Ptr(ty)) => Tree::BinOp(
                op.clone(),
                Box::new(lhs),
                Box::new(rhs),
                Type::Ptr(ty.clone()),
            ),
            (Op::Sub, Type::Ptr(lhs_ty), Type::Ptr(rhs_ty)) if lhs_ty == rhs_ty => {
                Tree::BinOp(op.clone(), Box::new(lhs), Box::new(rhs), Type::Int)
            }
            (Op::Eq, lhs_ty, rhs_ty)
            | (Op::NotEq, lhs_ty, rhs_ty)
            | (Op::Gt, lhs_ty, rhs_ty)
            | (Op::Gte, lhs_ty, rhs_ty)
            | (Op::Lt, lhs_ty, rhs_ty)
            | (Op::Lte, lhs_ty, rhs_ty)
                if lhs_ty == rhs_ty =>
            {
                Tree::BinOp(op.clone(), Box::new(lhs), Box::new(rhs), Type::Int)
            }
            _ => panic!(
                "{:?} for {:?} and {:?} is not allowed",
                op, lhs_type, rhs_type
            ),
        }
    }

    fn add_string_literal(&mut self, s: &str) -> String {
        let label = format!("LC{}", self.strings.len());
        self.strings.insert(label.clone(), s.to_string());
        label
    }
}

#[cfg(test)]
mod tests {
    use super::{TypeChecker};
    use crate::parser::{Op, parse};
    use crate::types::Type;

    fn typecheck(source: &str) -> TypedTree {
        let tree = parse(source).unwrap();
        let mut checker = TypeChecker::new(tree);
        checker.check()
    }

    fn typed_program(source: &str) -> Vec<TypedTree> {
        match typecheck(source) {
            TypedTree::Program(trees) => trees,
            _ => panic!("top-level tree must be Program"),
        }
    }

    fn find_func<'a>(trees: &'a [TypedTree], name: &str) -> &'a TypedTree {
        trees
            .iter()
            .find(|tree| matches!(tree, TypedTree::FuncDef(_, n, _, _) if n == name))
            .unwrap_or_else(|| panic!("function {} not found", name))
    }

    #[test]
    fn typecheck_basic_arith() {
        let trees = typed_program("int main(){ return 1+2*3; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[0] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Int);
                        match &**expr {
                            TypedTree::BinOp(Op::Add, _, _, ty) => {
                                assert_eq!(ty, &Type::Int);
                            }
                            _ => panic!("expected add expression"),
                        }
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn typecheck_addr_deref() {
        let trees = typed_program("int main(){ int x; return *&x; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[1] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Int);
                        match &**expr {
                            TypedTree::Deref(inner, ty) => {
                                assert_eq!(ty, &Type::Int);
                                match &**inner {
                                    TypedTree::Addr(target, addr_ty) => {
                                        assert_eq!(addr_ty, &Type::Ptr(Box::new(Type::Int)));
                                        assert!(matches!(**target, TypedTree::Var(_, _)));
                                    }
                                    _ => panic!("expected addr"),
                                }
                            }
                            _ => panic!("expected deref"),
                        }
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn typecheck_ptr_arith() {
        let trees = typed_program("int main(){ int *p; return p+1; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[1] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Ptr(Box::new(Type::Int)));
                        match &**expr {
                            TypedTree::BinOp(Op::Add, _, _, ty) => {
                                assert_eq!(ty, &Type::Ptr(Box::new(Type::Int)));
                            }
                            _ => panic!("expected add expression"),
                        }
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn typecheck_call_args() {
        let trees =
            typed_program("int foo(int *p){ return *p; } int main(){ int x; return foo(&x); }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[1] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Int);
                        match &**expr {
                            TypedTree::Call(name, args, call_ty) => {
                                assert_eq!(name, "foo");
                                assert_eq!(args.len(), 1);
                                assert_eq!(call_ty, &Type::Int);
                            }
                            _ => panic!("expected call"),
                        }
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    #[should_panic(expected = "assign type mismatch")]
    fn typecheck_assign_mismatch_panics() {
        let _ = typecheck("int main(){ int *p; int x; p=x; }");
    }

    #[test]
    fn typecheck_sizeof_expr() {
        let trees = typed_program("int main(){ int *p; return sizeof p; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[1] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Int);
                        assert!(matches!(**expr, TypedTree::Integer(8, Type::Int)));
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn typecheck_array_var_decays_to_ptr() {
        let trees = typed_program("int main(){ int a[4]; return a; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[1] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Ptr(Box::new(Type::Int)));
                        match &**expr {
                            TypedTree::Addr(inner, addr_ty) => {
                                assert_eq!(addr_ty, &Type::Ptr(Box::new(Type::Int)));
                                assert!(
                                    matches!(**inner, TypedTree::Var(ref name, Type::Int) if name == "a")
                                );
                            }
                            _ => panic!("expected addr from array-to-pointer decay"),
                        }
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn typecheck_array_index_yields_element_lvalue() {
        let trees = typed_program("int main(){ int *a; int v; a[2] = v; return a[2]; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => {
                    match &stmts[2] {
                        TypedTree::Assign(lhs, rhs, ty) => {
                            assert_eq!(ty, &Type::Int);
                            assert!(
                                matches!(**rhs, TypedTree::Var(ref name, Type::Int) if name == "v")
                            );
                            match &**lhs {
                                TypedTree::Deref(indexed, lhs_ty) => {
                                    assert_eq!(lhs_ty, &Type::Int);
                                    match &**indexed {
                                        TypedTree::BinOp(Op::Add, base, index, ptr_ty) => {
                                            assert!(matches!(
                                                **index,
                                                TypedTree::Integer(2, Type::Int)
                                            ));
                                            assert_eq!(ptr_ty, &Type::Ptr(Box::new(Type::Int)));
                                            assert!(matches!(
                                                **base,
                                                TypedTree::Var(ref name, Type::Ptr(ref inner))
                                                    if name == "a" && **inner == Type::Int
                                            ));
                                        }
                                        _ => panic!("expected pointer add for index"),
                                    }
                                }
                                _ => panic!("expected deref on lhs"),
                            }
                        }
                        _ => panic!("expected assign"),
                    }

                    match &stmts[3] {
                        TypedTree::Return(expr, ret_ty) => {
                            assert_eq!(ret_ty, &Type::Int);
                            assert!(matches!(**expr, TypedTree::Deref(_, Type::Int)));
                        }
                        _ => panic!("expected return"),
                    }
                }
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn typecheck_sizeof_array_expr() {
        let trees = typed_program("int main(){ int a[4]; return sizeof a; }");
        let func = find_func(&trees, "main");

        match func {
            TypedTree::FuncDef(_, _, _, body) => match &**body {
                TypedTree::Block(stmts) => match &stmts[1] {
                    TypedTree::Return(expr, ret_ty) => {
                        assert_eq!(ret_ty, &Type::Int);
                        assert!(matches!(**expr, TypedTree::Integer(16, Type::Int)));
                    }
                    _ => panic!("expected return"),
                },
                _ => panic!("expected block"),
            },
            _ => panic!("expected func"),
        }
    }

    #[test]
    fn globals_are_collected() {
        let tree = parse("int g; int main(){ return 1; }").unwrap();
        let mut checker = TypeChecker::new(tree);
        let typed = checker.check();
        let globals = checker.globals();
        assert!(globals.contains_key("g"));
        assert_eq!(globals.get("g"), Some(&Type::Int));

        match typed {
            TypedTree::Program(trees) => {
                // only one function definition should be present
                assert!(
                    trees
                        .iter()
                        .any(|t| matches!(t, TypedTree::FuncDef(_, n, _, _) if n == "main"))
                );
            }
            _ => panic!("expected program"),
        }
    }

    #[test]
    #[should_panic(expected = "duplicate of global var")]
    fn duplicate_global_decl_panics() {
        let tree = parse("int g; int g; int main(){ return 0; }").unwrap();
        let mut checker = TypeChecker::new(tree);
        let _ = checker.check();
    }
}
