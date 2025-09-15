use crate::instances::*;
use pdf_struct_traits::{Classify, Object, Root};
use std::fmt::{Debug, Display};
use std::sync::{Arc, RwLock};

/// Represents the configuration for document structure.
pub struct Config {
    pub(crate) types: Vec<Arc<RwLock<ConcretePageType>>>,
    pub(crate) root: ConcreteRoot,
    pub(crate) offset: usize,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder {
            types: vec![],
            root: None,
            offset: 0,
        }
    }
}

/// Constructs a new Config
pub struct ConfigBuilder {
    types: Vec<Arc<RwLock<ConcretePageType>>>,
    root: Option<ConcreteRoot>,
    offset: usize,
}

impl ConfigBuilder {
    pub fn with_obj<T, E>(mut self) -> Self
    where
        T: Object + Classify + 'static,
        E: std::error::Error + Debug + Display + Send + Sync + 'static,
    {
        let mut builder = ConcreteObjectBuilder::new();
        let instanstiated = builder.build::<T, E>();

        self.types.push(instanstiated.clone());
        self
    }

    /// Defines the root document.
    pub fn with_root<T>(mut self) -> Self
    where
        T: Root + 'static,
    {
        let root = ConcreteRoot::new();
        self.root = Some(root);

        self
    }

    /// Sets the page the classifier will start from, instead of 0.
    /// ! (pages are zero-indexed)
    pub fn set_start(mut self, offset: usize) -> Self {
        self.offset = offset;

        self
    }

    /// Consumes the builder into a Config.
    pub fn build(self) -> Config {
        Config {
            types: self.types,
            root: self.root.expect("A root struct is required!"),
            offset: self.offset,
        }
    }
}
