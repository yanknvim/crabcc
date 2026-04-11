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
