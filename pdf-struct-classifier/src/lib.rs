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

        // if let Some(classifier) = self.config.key_classifiers.get(first) {}

        Ok(())
    }
}
