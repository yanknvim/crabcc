use std::collections::HashMap;

use crate::parser::{Op, Tree};
use crate::types::Type;

#[derive(Debug, Clone)]
pub enum TypedTree {
    Program(Vec<TypedTree>),
    BinOp(Op, Box<TypedTree>, Box<TypedTree>, Type),
    Assign(Box<TypedTree>, Box<TypedTree>, Type),
    Block(Vec<TypedTree>),
    FuncDef(Type, String, Vec<(Type, String)>, Box<TypedTree>),
    If(Box<TypedTree>, Box<TypedTree>, Option<Box<TypedTree>>),
    While(Box<TypedTree>, Box<TypedTree>),
    For(
        Option<Box<TypedTree>>,
        Option<Box<TypedTree>>,
        Option<Box<TypedTree>>,
        Box<TypedTree>,
    ),

    Integer(i64, Type),
    Var(String, Type),
    VarDeclare(Type, String),
    Addr(Box<TypedTree>, Type),
    Deref(Box<TypedTree>, Type),

    Call(String, Vec<TypedTree>, Type),
    Return(Box<TypedTree>, Type),
}

impl TypedTree {
    pub fn ty(&self) -> &Type {
        match self {
            TypedTree::BinOp(_, _, _, ty) => ty,
            TypedTree::Assign(_, _, ty) => ty,
            TypedTree::Integer(_, ty) => ty,
            TypedTree::Var(_, ty) => ty,
            TypedTree::Addr(_, ty) => ty,
            TypedTree::Deref(_, ty) => ty,
            TypedTree::Call(_, _, ty) => ty,
            TypedTree::Return(_, ty) => ty,
            _ => panic!("{:?} is not typed", self),
        }
    }
}

pub struct TypeChecker {
    tree: Tree,
    env: Vec<HashMap<String, Type>>,
    functions: HashMap<String, (Type, Vec<Type>)>,
    current_return: Option<Type>,
}

impl TypeChecker {
    pub fn new(tree: Tree) -> Self {
        Self {
            tree,
            env: Vec::new(),
            functions: HashMap::new(),
            current_return: None,
        }
    }

    pub fn check(&mut self) -> TypedTree {
        self.collect_functions();
        let tree = self.tree.clone();
        self.check_tree(&tree)
    }

    fn collect_functions(&mut self) {
        let trees = match &self.tree {
            Tree::Program(trees) => trees,
            _ => panic!("top-level tree must be Program"),
        };

        self.functions.clear();
        for tree in trees {
            if let Tree::FuncDef(ty, name, params, _) = tree {
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
        None
    }

    fn check_lvalue(typed: &TypedTree) {
        match typed {
            TypedTree::Var(_, _) | TypedTree::Deref(_, _) => {}
            _ => panic!("not an lvalue"),
        }
    }

    fn check_tree(&mut self, tree: &Tree) -> TypedTree {
        match tree {
            Tree::Program(trees) => {
                TypedTree::Program(trees.iter().map(|t| self.check_tree(t)).collect())
            }
            Tree::Sizeof(expr) => {
                let expr = self.check_tree(expr);
                TypedTree::Integer(expr.ty().size() as i64, Type::Int)
            }
            Tree::BinOp(op, lhs, rhs) => {
                let lhs = self.check_tree(lhs);
                let rhs = self.check_tree(rhs);

                let lhs_type = lhs.ty().clone();
                let rhs_type = rhs.ty().clone();

                match (op, &lhs_type, &rhs_type) {
                    (_, Type::Int, Type::Int) => {
                        TypedTree::BinOp(op.clone(), Box::new(lhs), Box::new(rhs), Type::Int)
                    }
                    (Op::Add, Type::Ptr(ty), Type::Int) | (Op::Sub, Type::Ptr(ty), Type::Int) => {
                        TypedTree::BinOp(
                            op.clone(),
                            Box::new(lhs),
                            Box::new(rhs),
                            Type::Ptr(ty.clone()),
                        )
                    }
                    (Op::Add, Type::Int, Type::Ptr(ty)) => TypedTree::BinOp(
                        op.clone(),
                        Box::new(lhs),
                        Box::new(rhs),
                        Type::Ptr(ty.clone()),
                    ),
                    (Op::Sub, Type::Ptr(lhs_ty), Type::Ptr(rhs_ty)) if lhs_ty == rhs_ty => {
                        TypedTree::BinOp(op.clone(), Box::new(lhs), Box::new(rhs), Type::Int)
                    }
                    (Op::Eq, lhs_ty, rhs_ty)
                    | (Op::NotEq, lhs_ty, rhs_ty)
                    | (Op::Gt, lhs_ty, rhs_ty)
                    | (Op::Gte, lhs_ty, rhs_ty)
                    | (Op::Lt, lhs_ty, rhs_ty)
                    | (Op::Lte, lhs_ty, rhs_ty)
                        if lhs_ty == rhs_ty =>
                    {
                        TypedTree::BinOp(op.clone(), Box::new(lhs), Box::new(rhs), Type::Int)
                    }
                    _ => panic!(
                        "{:?} for {:?} and {:?} is not allowed",
                        op, lhs_type, rhs_type
                    ),
                }
            }
            Tree::Assign(lhs, rhs) => {
                let lhs = self.check_tree(lhs);
                let rhs = self.check_tree(rhs);
                let ty = lhs.ty();

                Self::check_lvalue(&lhs);
                if ty != rhs.ty() {
                    panic!("assign type mismatch: {:?} = {:?}", ty, rhs.ty());
                }

                TypedTree::Assign(Box::new(lhs.clone()), Box::new(rhs), ty.clone())
            }
            Tree::Block(stmts) => TypedTree::Block({
                self.enter_scope();
                let checked = stmts.iter().map(|s| self.check_tree(s)).collect();
                self.exit_scope();
                checked
            }),
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

                TypedTree::FuncDef(
                    ty.clone(),
                    name.to_string(),
                    params.to_vec(),
                    Box::new(body),
                )
            }
            Tree::If(cond, block_a, block_b) => {
                let cond = self.check_tree(cond);
                if cond.ty() != &Type::Int {
                    panic!("if condition must be int");
                }
                let block_a = self.check_tree(block_a);
                let block_b = block_b.as_ref().map(|t| self.check_tree(t));

                TypedTree::If(Box::new(cond), Box::new(block_a), block_b.map(Box::new))
            }
            Tree::While(cond, body) => {
                let cond = self.check_tree(cond);
                if cond.ty() != &Type::Int {
                    panic!("while condition must be int");
                }
                let body = self.check_tree(body);

                TypedTree::While(Box::new(cond), Box::new(body))
            }
            Tree::For(init, cond, update, body) => {
                let init = init.as_ref().map(|t| self.check_tree(t));
                let cond = cond.as_ref().map(|t| self.check_tree(t));
                let update = update.as_ref().map(|t| self.check_tree(t));

                if let Some(cond) = &cond
                    && cond.ty() != &Type::Int {
                        panic!("for condition must be int");
                    }

                let body = self.check_tree(body);

                TypedTree::For(
                    init.map(Box::new),
                    cond.map(Box::new),
                    update.map(Box::new),
                    Box::new(body),
                )
            }
            Tree::Integer(n) => TypedTree::Integer(*n, Type::Int),
            Tree::Var(name) => {
                let ty = self
                    .lookup(name)
                    .unwrap_or_else(|| panic!("not declared variable: {}", name));
                TypedTree::Var(name.to_string(), ty)
            }
            Tree::VarDeclare(ty, name) => {
                self.declare(name.to_string(), ty.clone());
                TypedTree::VarDeclare(ty.clone(), name.to_string())
            }
            Tree::Addr(expr) => {
                let expr = self.check_tree(expr);
                Self::check_lvalue(&expr);
                let expr_ty = expr.ty().clone();
                TypedTree::Addr(Box::new(expr), Type::Ptr(Box::new(expr_ty)))
            }
            Tree::Deref(expr) => {
                let expr = self.check_tree(expr);
                match expr.ty().clone() {
                    Type::Ptr(inner) => TypedTree::Deref(Box::new(expr), *inner),
                    _ => panic!("{:?} is not ptr", expr.ty()),
                }
            }
            Tree::Call(name, args) => {
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
                TypedTree::Call(name.to_string(), checked_args, ret_ty)
            }
            Tree::Return(expr) => {
                let expr = self.check_tree(expr);
                self.current_return
                    .as_ref()
                    .expect("return outside of function");
                let ret_ty = expr.ty().clone();
                TypedTree::Return(Box::new(expr), ret_ty)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TypeChecker, TypedTree};
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
}
