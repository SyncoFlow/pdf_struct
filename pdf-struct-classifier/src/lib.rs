pub mod config;
pub mod instances;


use crate::config::Config;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

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
