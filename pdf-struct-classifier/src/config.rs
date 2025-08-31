use crate::instances::*;
use std::fmt::{Debug, Display};
use pdf_struct_traits::{Classify, InferredPage, KeyPage, Object, Root};

/// Represents the configuration for document structure.
pub struct Config {
    pub(crate) keys: Vec<InstanstiatedKeyPage>,
    pub(crate) inferred: Vec<InstanstiatedInferredPage>,
    pub(crate) root: InstanstiatedRoot,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder {
            keys: vec![],
            inferred: vec![],
            root: None,
        }
    }
}

/// Constructs a new Config
pub struct ConfigBuilder {
    keys: Vec<InstanstiatedKeyPage>,
    inferred: Vec<InstanstiatedInferredPage>,
    root: Option<InstanstiatedRoot>,
}

impl ConfigBuilder {
    /// Adds a key type to the config.
    pub fn with_key<T, E>(mut self) -> Self
    where
        T: KeyPage + Classify + Object + 'static,
        E: std::error::Error + Debug + Display + Send + Sync + 'static,
    {
        let mut builder = InstanstiatedObjectBuilder::new();
        let instanstiated = builder.build::<T, E>();

        self.keys.push(instanstiated.into());
        self
    }

    /// Adds an inferred type to the config.
    pub fn with_inferred<T, E>(mut self) -> Self
    where
        T: InferredPage + Classify + Object + 'static,
        E: std::error::Error + Debug + Display + Send + Sync + 'static,
    {
        let mut builder = InstanstiatedObjectBuilder::new();
        let instanstiated = builder.build::<T, E>();

        self.inferred.push(instanstiated.into());
        self
    }

    /// Defines the root document.
    pub fn with_root<T>(mut self) -> Self
    where
        T: Root + 'static,
    {
        let root = InstanstiatedRoot::new();
        self.root = Some(root);

        self
    }

    /// Consumes the builder into a Config.
    pub fn build(self) -> Config {
        Config {
            keys: self.keys,
            inferred: self.inferred,
            root: self.root.expect("A root struct is required!"),
        }
    }
}
