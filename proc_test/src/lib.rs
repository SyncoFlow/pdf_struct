// #![feature(const_type_id)]

use doc_structure::object;
use traits::*;

// children expects a vector of T instead of a singular
//                     tuple defines it as a pair
// and requires both types to implement Pair of eachother
// metadata are also just objects, it just states they are specifically
// for explaining MORE about this type
// The metadata comes after the page that declares this type
// But before the first child of said type.
// When a child is defined, it needs to have a member that holds a Vec<T> (T being the children type)
// And same goes for metadata.
#[object(children = SubChapter, metadata = (Diagram, DataTable), object_type = Key)]
struct Chapter;

// self-explanatory from the comment above
// requi res a member that points back to the parent
#[object(children = (Diagram, DataTable), parent = Chapter, object_type = Key)]
struct SubChapter;

// It has a pair of DataTable, and is expected to always come before
// A DataTable
// Doesn't have a parent because it can be used in Chapter, SubChapter, etc
#[object(pair = DataTable)]
struct Diagram;

// Has a pair of Diagram and is after a Diagram in the document structure
#[object]
struct DataTable;

#[object(root)]
struct Document;

fn config() {
    use pdf_parser_v3::{config::*, pattern::Pattern};

    let builder = Config::builder()
        .with_root::<Document>()
        .with_key::<Chapter>()
        .with_key::<SubChapter>()
        .with_inferred::<Diagram>()
        .with_inferred::<DataTable>()
        .with_pattern(Pattern::from_pair::<Diagram, DataTable>())
        .build();
    
}
