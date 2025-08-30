use std::any::TypeId;
use pdf_struct_traits::PairWith;

/// Defines a Pattern to inference pages.
///
/// For example, if we know after finding a key Subchapter
/// a Diagram and Table are a pattern, we can call Pattern::from_pair()
///
/// This tells the classifier that said pattern exists and to apply it
/// when finding a Subchapter.
pub enum Pattern {
    Pair { first: TypeId, second: TypeId },
}

impl Pattern {
    /// T being the first type in the pair (as it is represented in the document)
    /// U being the second pair in the pair (as it is represented in the document)
    pub fn from_pair<T, U>() -> Self
    where
        T: PairWith<U> + 'static,
        U: PairWith<T> + 'static,
    {
        Self::Pair {
            first: TypeId::of::<T>(),
            second: TypeId::of::<U>(),
        }
    }

    pub fn matches_types<T, U>(&self) -> bool
    where
        T: 'static,
        U: 'static,
    {
        match self {
            Self::Pair { first, second } => {
                *first == TypeId::of::<T>() && *second == TypeId::of::<U>()
            }
        }
    }
}
