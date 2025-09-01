#![allow(unused)] // TODO: remove after finishing Classifier

pub mod config;
pub mod instances;

#[cfg(test)]
mod tests;

use pdf_struct_traits::ClassificationResult;

use crate::config::Config;
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ClassiferError {
    #[error("No key objects were provided!")]
    NoKeysProvided,
}

/// Classifier is meant to bridge the context we are provided from the user
/// within the Config with the actual physical extraction of a document.
///
/// It will single-threadedly iterate over each page within the document, and as we classify
/// each page into the type they are expected to be, we call T::extract on a seperate thread.
///
/// For more information regarding this see [issue #2](https://github.com/SyncoFlow/pdf_struct/issues/2)
pub struct Classifier {
    config: Config,
    path: PathBuf,
    context: HashMap<i32, ClassificationResult<Box<dyn Any>, ClassiferError>>,
    pages: i32,
}

impl Classifier {
    pub fn new(config: Config, path: PathBuf) -> Self {
        Self {
            config,
            path,
            context: HashMap::new(),
            pages: 0,
        }
    }

    pub fn begin(&self) -> Result<(), ClassiferError> {
        todo!()
    }

    /// We specify chunks as each unique key object that is a child of root.
    /// (child as in first-generation child, nothing that is a child of a child of root is counted.)
    /// I.e:
    ///     Root
    ///       |- Chapter
    ///           |- SubChapter
    ///       |- SomeOtherKey
    ///           |- SomeOtherKeysChild
    fn classify_chunk(&self, start_page: i32) -> Result<(), ClassiferError> {
        todo!()
    }
}
