use crate::parser::{Lowered, Op, Tree, Typed};

pub fn lower(tree: Tree<Typed>) -> Tree<Lowered> {
    match tree {
        Tree::Program(trees) => Tree::Program(trees.into_iter().map(lower).collect()),
        Tree::BinOp(op, lhs, rhs, ty) => {
            Tree::BinOp(op, Box::new(lower(*lhs)), Box::new(lower(*rhs)), ty)
        }
        Tree::Assign(lhs, rhs, ty) => {
            Tree::Assign(Box::new(lower(*lhs)), Box::new(lower(*rhs)), ty)
        }
        Tree::Block(stmts) => Tree::Block(stmts.into_iter().map(lower).collect()),
        Tree::FuncDef(tyt, name, params, body) => {
            Tree::FuncDef(tyt, name, params, Box::new(lower(*body)))
        }
        Tree::If(cond, then, other) => Tree::If(
            Box::new(lower(*cond)),
            Box::new(lower(*then)),
            other.map(|b| Box::new(lower(*b))),
        ),
        Tree::While(cond, body) => Tree::While(Box::new(lower(*cond)), Box::new(lower(*body))),
        Tree::For(init, cond, update, body) => Tree::For(
            init.map(|b| Box::new(lower(*b))),
            cond.map(|b| Box::new(lower(*b))),
            update.map(|b| Box::new(lower(*b))),
            Box::new(lower(*body)),
        ),
        Tree::Integer(val, ty) => Tree::Integer(val, ty),
        Tree::String(val, ty) => Tree::String(val, ty),
        Tree::Var(name, ty) => Tree::Var(name, ty),
        Tree::Indexed(left, right, ty) => {
            let left = lower(*left);
            let right = lower(*right);

            Tree::Deref(
                Box::new(Tree::BinOp(
                    Op::Add,
                    Box::new(left),
                    Box::new(right),
                    ty.clone(),
                )),
                ty,
            )
        }
        Tree::VarDeclare(ty, name) => Tree::VarDeclare(ty, name),
        Tree::Addr(expr, ty) => Tree::Addr(Box::new(lower(*expr)), ty),
        Tree::Deref(expr, ty) => Tree::Deref(Box::new(lower(*expr)), ty),
        Tree::Sizeof(expr) => Tree::Sizeof(Box::new(lower(*expr))),
        Tree::Call(name, args, ty) => Tree::Call(name, args.into_iter().map(lower).collect(), ty),
        Tree::Return(expr, ty) => Tree::Return(Box::new(lower(*expr)), ty),
        _ => panic!("unexpected tree in lowering phase"),
    }
}
