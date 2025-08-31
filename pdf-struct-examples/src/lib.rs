#![allow(unused)]

use pdf_struct_macros::{object, root};
use pdf_struct_traits::Classify;
use pdf_struct_traits::Pattern;
use pdf_struct_traits::*;

#[object(page_type = Inferred, parent = Chapter)]
#[derive(Debug, Clone)]
struct ChapterMetadata;

// children expects a vector of T instead of a singular
//                     tuple defines it as a pair
// and requires both types to implement Pair of eachother
// metadata are also just objects, it just states they are specifically
// for explaining MORE about this type
// The metadata comes after the page that declares this type
// But before the first child of said type.
// When a child is defined, it needs to have a member that holds a Vec<T> (T being the children type)
// And same goes for metadata.
#[object(children = SubChapter, page_type = Key, metadata = ChapterMetadata)]
struct Chapter;

// self-explanatory from the comment above
// requi res a member that points back to the parent
#[object(children = (Diagram, DataTable), parent = Chapter, page_type = Key)]

struct SubChapter;

// It has a pair of DataTable, and is expected to always come before
// A DataTable
// Doesn't have a parent because it can be used in Chapter, SubChapter, etc
#[object(pair = DataTable, sequence = First, patterns = [Pattern::from_pair::<Diagram, DataTable>()] )]
struct Diagram;

// Has a pair of Diagram and is after a Diagram in the document structure
#[object(pair = Diagram, sequence = Last, patterns = [Pattern::from_pair::<Diagram, DataTable>()])]
struct DataTable;

#[root]

struct Document;

#[derive(Debug, thiserror::Error)]
enum Error {}

// impl Classify

macro_rules! impl_classify {
    ($impl_for:ty) => {
        impl Classify for $impl_for {
            #[allow(unused_variables)]
            fn classify<E>(img: &[u8]) -> pdf_struct_traits::ClassificationResult<Self, E>
            where
                E: std::fmt::Debug + std::fmt::Display + std::error::Error,
                Self: Sized,
            {
                // do some OCR things
                let ocr_confidence = 100.0;

                pdf_struct_traits::ClassificationResult::Confident(Self {}, ocr_confidence)
            }
        }
    };
}
impl_classify!(Chapter);
impl_classify!(SubChapter);
impl_classify!(Diagram);
impl_classify!(DataTable);
impl_classify!(ChapterMetadata);

#[test]
fn test() {
    use pdf_struct_classifier::config::*;

    #[derive(thiserror::Error, Debug)]
    enum Error {}

    let config = Config::builder()
        .with_root::<Document>()
        .with_key::<Chapter, Error>()
        .with_key::<SubChapter, Error>()
        .with_inferred::<Diagram, Error>()
        .with_inferred::<DataTable, Error>()
        .build();
}
