use crate::config::Config;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

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
pub trait Classify<E>
where
    E: Debug + Display + Error,
{
    fn classify(img: &[u8]) -> ClassificationResult<Self, E>
    where
        Self: Sized;
}

#[derive(thiserror::Error, Debug)]
pub enum ClassiferError {
    #[error("No key objects were provided!")]
    NoKeysProvided,
}

pub struct Classifier<E>
where
    E: std::error::Error + Debug + Display + Send + Sync + 'static,
{
    config: Config<E>,
    path: PathBuf,
    context: HashMap<i32, ClassificationResult<Box<dyn Any>, ClassiferError>>,
    pages: i32,
}

impl<E> Classifier<E>
where
    E: std::error::Error + Debug + Display + Send + Sync + 'static,
{
    pub fn new(config: Config<E>, path: PathBuf) -> Self {
        Self {
            config,
            path,
            context: HashMap::new(),
            pages: 0,
        }
    }

    pub fn begin(&self) -> Result<(), ClassiferError> {
        Ok(())
    }

    fn classify_chunk(&self, start_page: i32) -> Result<(), ClassiferError> {
        type Error = ClassiferError;

        if self.config.keys.is_empty() {
            return Err(Error::NoKeysProvided);
        }

        let first = match self.config.keys.first() {
            None => return Err(Error::NoKeysProvided),
            Some(s) => s,
        };

        if let Some(classifier) = self.config.key_classifiers.get(first) {}

        Ok(())
    }
}
