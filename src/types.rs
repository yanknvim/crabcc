#[derive(Debug, Clone, PartialEq)]
pub enum Type<'a> {
    Int,
    Char,
    Ptr(&'a Type<'a>),
    Array(&'a Type<'a>, usize),
}

impl<'a> Type<'a> {
    pub fn size(&self) -> usize {
        match self {
            Self::Int => 4,
            Self::Char => 1,
            Self::Ptr(_) => 8,
            Self::Array(inner, size) => Self::size(inner) * size,
        }
    }
}
