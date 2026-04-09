#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Ptr(Box<Type>),
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Self::Int => 4,
            Self::Ptr(_) => 8,
        }
    }
}
