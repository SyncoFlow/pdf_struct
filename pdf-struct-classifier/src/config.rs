use crate::pattern::Pattern;
use crate::{ClassiferError, ClassificationResult, Classify};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::{Debug, Display};

// PairWith states that a type of page is always next to another type of page.
// And defines the direction of both Types
// I.e Diagram and Table are a pair, where Diagram comes before Table.
use pdf_struct_traits::{InferredPage, KeyPage, Root};

/// fn (img: &\[u8\]) -> Result<Any, ClassificationError>; (backslashes are for escaping)
type ClassificationMethod<E> =
    Box<dyn Fn(&[u8]) -> ClassificationResult<Box<dyn Any>, E> + Send + Sync>;

/// Represents the configuration for document structure.
pub struct Config<E>
where
    E: std::error::Error + Debug + Display + Send + Sync + 'static,
{
    pub(crate) keys: Vec<TypeId>,
    pub(crate) inferred: Vec<TypeId>,
    pub(crate) patterns: Vec<Pattern>,
    pub(crate) root: TypeId,
    pub(crate) key_classifiers: HashMap<TypeId, ClassificationMethod<E>>,
    pub(crate) inferred_classifiers: HashMap<TypeId, ClassificationMethod<E>>,
}

impl<E> Config<E>
where
    E: std::error::Error + Debug + Display + Send + Sync + 'static,
{
    pub fn builder() -> ConfigBuilder<E> {
        ConfigBuilder {
            keys: vec![],
            patterns: vec![],
            inferred: vec![],
            root: None,
            key_classifiers: HashMap::new(),
            inferred_classifiers: HashMap::new(),
        }
    }
}

/// Constructs a new Config
pub struct ConfigBuilder<E>
where
    E: std::error::Error + Debug + Display + Send + Sync + 'static,
{
    keys: Vec<TypeId>,
    patterns: Vec<Pattern>,
    inferred: Vec<TypeId>,
    root: Option<TypeId>,
    key_classifiers: HashMap<TypeId, ClassificationMethod<E>>,
    inferred_classifiers: HashMap<TypeId, ClassificationMethod<E>>,
}

impl<E> ConfigBuilder<E>
where
    E: std::error::Error + Debug + Display + Send + Sync + 'static,
{
    /// Adds a key type to the config.
    pub fn with_key<T>(mut self) -> Self
    where
        T: KeyPage + Classify<E> + 'static,
    {
        let type_id = TypeId::of::<T>();
        self.keys.push(type_id);

        // wrapper of T::classify
        let classifier = |img: &[u8]| -> ClassificationResult<Box<dyn Any>, E> {
            match T::classify(img) {
                ClassificationResult::Confident(t, score) => {
                    ClassificationResult::Confident(Box::new(t), score)
                }
                ClassificationResult::Probable(t, score) => {
                    ClassificationResult::Probable(Box::new(t), score)
                }
                ClassificationResult::Uncertain(score) => ClassificationResult::Uncertain(score),
                ClassificationResult::Err(e) => ClassificationResult::Err(e),
            }
        };

        self.key_classifiers.insert(type_id, Box::new(classifier));
        self
    }

    /// Adds a pattern to the config.
    pub fn with_pattern(mut self, pattern: Pattern) -> Self {
        self.patterns.push(pattern);
        self
    }

    /// Adds an inferred type to the config.
    pub fn with_inferred<T>(mut self) -> Self
    where
        T: InferredPage + Classify<E> + 'static,
    {
        let type_id = TypeId::of::<T>();
        self.inferred.push(type_id);

        // wrapper of T::classify
        let classifier = |img: &[u8]| -> ClassificationResult<Box<dyn Any>, E> {
            match T::classify(img) {
                ClassificationResult::Confident(t, score) => {
                    ClassificationResult::Confident(Box::new(t), score)
                }
                ClassificationResult::Probable(t, score) => {
                    ClassificationResult::Probable(Box::new(t), score)
                }
                ClassificationResult::Uncertain(score) => ClassificationResult::Uncertain(score),
                ClassificationResult::Err(e) => ClassificationResult::Err(e),
            }
        };

        self.inferred_classifiers
            .insert(type_id, Box::new(classifier));
        self
    }

    /// Defines the root document.
    pub fn with_root<T>(mut self) -> Self
    where
        T: Root + 'static,
    {
        self.root = Some(TypeId::of::<T>());
        self
    }

    /// Consumes the builder into a Config.
    pub fn build(self) -> Config<E> {
        Config {
            keys: self.keys,
            patterns: self.patterns,
            inferred: self.inferred,
            root: self.root.expect("A root struct is required!"),
            key_classifiers: self.key_classifiers,
            inferred_classifiers: self.inferred_classifiers,
        }
    }
}
