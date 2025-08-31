#![feature(associated_type_defaults)]

extern crate type_info;

use std::any::TypeId;
use std::error::Error;
use std::fmt::{Debug, Display};

/// Defines a Pattern to inference pages.
///
/// For example, if we know after finding a key Subchapter
/// a Diagram and Table are a pattern, we can call Pattern::from_pair()
///
/// This tells the classifier that said pattern exists and to apply it
/// when finding a Subchapter.
#[derive(Clone)]
pub enum Pattern {
    Pair {
        first: TypeInformation,
        second: TypeInformation,
    },
}

impl Pattern {
    /// T being the first type in the pair (as it is represented in the document)
    /// U being the second pair in the pair (as it is represented in the document)
    pub fn from_pair<T, U>() -> Self
    where
        T: PairWith<U>,
        U: PairWith<T>,
        // extra impls
        T: Object + 'static,
        U: Object + 'static,
    {
        Self::Pair {
            first: T::TYPE,
            second: U::TYPE,
        }
    }
}

/// Indicates the position of an object relative to the order of pages
/// and comparing against the object paired.
#[derive(Clone)]
pub enum PairSequence {
    First,
    Last,
    None,
}

/// Indicates that Self is a pair object with [T]
/// In other words, in a document Self will be either the page before or after [T]
/// which is determined by the [SEQUENCE] constant.
pub trait PairWith<T: Object>: Object {
    const SEQUENCE: PairSequence;
    const PATTERNS: &'static [Pattern];
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
pub trait KeyPage: Object {}

/// Marks Self to be a page that CAN be inferred
/// Self does NOT have be explicitly classified to be constructed
pub trait InferredPage: Object {}

/// Signifies that a struct represents the root document
pub trait Root {}

/// Indicates that Self parents type T
/// within the document structure itself
pub trait Parent: Object {}

/// Indicates that Self is the child object
/// To type T within the document structure itself.
pub trait Child: Object {}

/// A percentage of how confident classification of an image
/// to a type of T is.
pub type ConfidenceScore = f32;

/// The result of an attempt to classify an extracted image of a page.
pub enum ClassificationResult<T, E>
where
    E: Error + Debug + Display,
{
    /// Highly sure the provided image is of type T/Self >90% confidence
    Confident(T, ConfidenceScore),
    /// Probable the provided image is of type T/Self 50-90% confidence
    Probable(T, ConfidenceScore),
    /// Uncertain the provided image is of type T/Self <50% confidence
    Uncertain(ConfidenceScore),
    /// Failed to classify image.
    Err(E),
}

/// Trait implemented onto any document object
/// that defines a classify method, which will state if a page
/// is the type of Self
///
/// I.e if page 3 is type of Chapter, you would implement this trait onto Chapter
/// Then implement logic that runs OCR on the image provided for any page.
/// And then assign a confidence value onto how confident your classification is
///
/// This is what the classifier will call constructing a PDF page into a type.  
pub trait Classify {
    fn classify<E>(img: &[u8]) -> ClassificationResult<Self, E>
    where
        Self: Sized,
        E: Debug + Display + Error;
}

impl Parent for () {}
impl<T: Object> PairWith<T> for () {
    const SEQUENCE: PairSequence = PairSequence::None;
    const PATTERNS: &'static [Pattern] = &[];
}
impl Object for () {
    const TYPE: TypeInformation = TypeInformation {
        id: TypeId::of::<Self>(),
        ident: "()",
    };

    type Pair = ();
    type Parent = ();
}
impl Classify for () {
    #[allow(unused)]
    fn classify<E>(img: &[u8]) -> ClassificationResult<Self, E>
    where
        Self: Sized,
        E: Debug + Display + Error,
    {
        panic!("Attempted to classify on an object that implements Classify as ()!")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TypeInformation {
    pub id: TypeId,
    pub ident: &'static str,
}

/// Indicates that Self is an in-code representation of a page
/// within a PDF document.
pub trait Object
where
    Self: Sized + Classify,
{
    const CHILDREN: &'static [TypeInformation] = &[];
    const TYPE: TypeInformation;

    type Parent: Parent = ();
    type Pair: PairWith<Self> = ();
}
