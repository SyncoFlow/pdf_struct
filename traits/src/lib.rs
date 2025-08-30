extern crate type_info;

use type_info::TypeInfo;

/// Indicates the position of an object relative to the order of pages
/// and comparing against the object paired.
pub enum PairSequence {
    First,
    Last,
}

/// Indicates that Self is a pair object with [T]
/// In other words, in a document Self will be either the page before or after [T]
/// which is determined by the [SEQUENCE] constant.
pub trait PairWith<T> {
    const SEQUENCE: PairSequence;
}

/// Defines Self to be a page that CANNOT be inferred
/// Self has to be classified to be constructed
///
/// But, Context can still be applied
/// i.e if we know Diagram-Table pairs come after a SubChapter
/// but SubChapter is a KeyPage, if after the inferred pairs the next page
/// can be contextually inferred to be a SubChapter or a Chapter
///
/// The main difference being until the next KeyPage is found
/// we cannot parallelize past that point.
///
/// So, we can keep inferring where Diagram-Table pairs are,
/// but we cannot infer where the sub-chapter 2 sub-chapters ahead is.  
pub trait KeyPage {}

/// Marks Self to be a page that CAN be inferred
/// Self does NOT have be explicitly classified to be constructed
pub trait InferredPage {}

/// Signifies that a struct represents the root document
pub trait Root {}

/// Indicates that Self parents type T
/// within the document structure itself
pub trait Parent<T> {}

/// Indicates that Self is the child object
/// To type T within the document structure itself.
pub trait Child<T> {}

pub enum ObjectType {
    Inferred,
    Key,
    Root,
}

/// Indicates that Self is an in-code representation of a page
/// within a PDF document.
pub trait Object: TypeInfo {
    const OBJECT_TYPE: ObjectType;
}
