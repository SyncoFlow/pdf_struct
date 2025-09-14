use dashmap::DashMap;
use pdf_struct_traits::*;
use std::any::{Any, TypeId};
use std::error::Error;
use std::fmt::{Debug, Display};
use std::sync::{Arc, RwLock, Weak};

/// Requires an instance of each associated-concrete type within each variant.
/// See [ConcretePageTypeIdentifiers] for the opposite.
#[derive(Clone)]
pub enum ConcretePageType {
    Key(ConcreteKeyPage),
    Inferred(ConcreteInferredPage),
    Pair(ConcretePair),
}

/// Doesn't require an instance of each associated-concrete type within each variant.
/// See [ConcretePageType] for the opposite.
pub enum ConcretePageTypeIdentifiers {
    Key,
    Inferred,
    Pair,
}

impl ConcretePageType {
    pub fn inner(&self) -> Arc<RwLock<ConcreteObject>> {
        match self {
            Self::Inferred(i) => i.inner(),
            Self::Key(k) => k.inner(),
            Self::Pair(p) => p.inner.clone(),
        }
    }

    pub fn inner_mut(&self) -> Arc<RwLock<ConcreteObject>> {
        // With Arc<RwLock<>>, we don't need a separate inner_mut method
        // as the RwLock provides the mutability through write locks
        self.inner()
    }
}

pub(crate) trait AnyClone: Any {
    /// Deep-clones the underlying data within a Box
    /// Or in other words, clones T within Box<T>.
    fn clone_box(&self) -> Box<dyn AnyClone>;
}

impl<T: Any + Clone> AnyClone for T {
    fn clone_box(&self) -> Box<dyn AnyClone> {
        Box::new(self.clone())
    }
}

/// Where:
///     T is shared data
///     E is error
pub(crate) type ClassificationMethod<T, E> = fn(&[u8]) -> ClassificationResult<T, E>;

/// Where:
///     T is shared data
///     E is error
///     S is Self (the constructed object)
pub(crate) type ExtractionMethod<T, E, S> = fn(&[u8], T) -> Result<S, E>;

/// Cache holding information of each  
pub(crate) type ObjectCache = DashMap<TypeId, Arc<RwLock<ConcretePageType>>>;

/// Crate-level error that can only be called when attempting
/// to cast into a [ClassificationMethod] or [ExtractionMethod]
#[derive(Debug)]
pub(crate) enum CastError {
    TypeMismatch { expected: TypeId, actual: TypeId },
}

/// Represents a type that implements [pdf_struct_traits::Object] at runtime.
pub struct ConcreteObject {
    pub parent: Option<Arc<RwLock<ConcretePageType>>>,
    pub children: Vec<Arc<RwLock<ConcretePageType>>>,
    /// Box<ClassificationMethod<T, E>;
    /// Where T is a type shared between Classify and Extract
    /// And E is an error type.
    /// ! This member should NOT be manually set or casted into.
    /// ! Utilize [ConcreteObject::cast_classification]
    pub(crate) classification_method: Box<dyn AnyClone>,
    /// Box<ExtractionMethod<T, E>;
    /// Where T is a type shared between Classify and Extract
    /// And E is an error type.
    /// ! This member should NOT be manually set or casted into.
    /// ! Utilize [ConcreteObject::cast_extraction]
    pub(crate) extraction_method: Box<dyn AnyClone>,
    /// Reflected information of the type defined as an object
    /// Which Self represents at runtime.
    pub(crate) obj_type: TypeInformation,
    /// The reflected type information for the children of this type.
    pub(crate) expected_children: Vec<TypeInformation>,
}

impl ConcreteObject {
    pub(crate) fn from_obj_with_cache<T, E>(
        cache: &mut ObjectCache,
    ) -> Arc<RwLock<ConcretePageType>>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        if let Some(cached) = cache.get(&T::TYPE.id) {
            return cached.clone();
        }

        let raw = Self::from_obj_internal::<T, E>(cache);
        let page_type = if T::KEY_PAGE {
            Arc::new(RwLock::new(ConcretePageType::Key(ConcreteKeyPage(
                Arc::new(RwLock::new(raw)),
            ))))
        } else if T::INFERRED_PAGE {
            Arc::new(RwLock::new(ConcretePageType::Inferred(
                ConcreteInferredPage(Arc::new(RwLock::new(raw))),
            )))
        } else {
            Arc::new(RwLock::new(ConcretePageType::Inferred(
                ConcreteInferredPage(Arc::new(RwLock::new(raw))),
            )))
        };
        cache.insert(T::TYPE.id, page_type.clone());
        page_type
    }

    /// Casts T and E into fn<T, E>(&\[u8]) -> ClassificationResult<T, E>;
    /// This method is unsafe because [ConcreteObject::classification_method]
    /// may not match the expected TypeId to cast back into a concrete [ClassificationMethod]
    pub(crate) unsafe fn cast_classification<T, E>(
        &self,
    ) -> Result<ClassificationMethod<T, E>, CastError>
    where
        T: Send + Sync + 'static,
        E: Error + Debug + Display + 'static,
    {
        let expected_type_id = TypeId::of::<fn(&[u8]) -> ClassificationResult<T, E>>();
        let actual_type_id = self.classification_method.type_id();

        let func_ptr =
            (self.classification_method.as_ref() as &dyn Any).downcast_ref::<fn(
                &[u8],
            )
                -> ClassificationResult<T, E>>(
            );

        match func_ptr {
            Some(f) => Ok(*f),
            None => Err(CastError::TypeMismatch {
                expected: expected_type_id,
                actual: actual_type_id,
            }),
        }
    }

    /// Casts T, E, S into fn(&\[u8], T) -> Result<S, E>;
    /// This method is unsafe because [ConcreteObject::extraction_method]
    /// may not match the expected TypeId to cast back into a concrete [ExtractionMethod]
    pub(crate) unsafe fn cast_extraction<T, E, S>(
        &self,
    ) -> Result<ExtractionMethod<T, E, S>, CastError>
    where
        T: Send + Sync + 'static,
        E: Error + Debug + Display + 'static,
        S: Sized + 'static,
    {
        let expected_type_id = TypeId::of::<fn(&[u8], T) -> Result<S, E>>();
        let actual_type_id = self.extraction_method.type_id();

        let func_ptr = (self.extraction_method.as_ref() as &dyn Any)
            .downcast_ref::<fn(&[u8], T) -> Result<S, E>>();

        match func_ptr {
            Some(f) => Ok(*f),
            None => Err(CastError::TypeMismatch {
                expected: expected_type_id,
                actual: actual_type_id,
            }),
        }
    }

    /// Internal method that does the actual construction
    fn from_obj_internal<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        let parent = if T::Parent::TYPE.ident == "()" {
            None
        } else {
            Some(Self::from_obj_with_cache::<T::Parent, E>(cache))
        };

        Self {
            parent,
            children: vec![],
            classification_method: Box::new(
                T::classify::<E> as ClassificationMethod<<T as Classify>::SharedData, E>,
            ),
            extraction_method: Box::new(
                T::extract::<E> as ExtractionMethod<<T as Classify>::SharedData, E, T>,
            ),
            obj_type: T::TYPE,
            expected_children: T::CHILDREN.to_vec(),
        }
    }

    /// Add a child, checking if it's allowed
    pub fn add_child(&mut self, child: Arc<RwLock<ConcretePageType>>) -> Result<(), String> {
        let child_type_id = {
            let child_inner = child.read().unwrap();
            let child_obj = child_inner.inner();
            let child_obj = child_obj.read().unwrap();
            child_obj.obj_type.id
        };

        if self.children.iter().any(|x| {
            let x_inner = x.read().unwrap();
            let x_inner = x_inner.inner();
            let x_obj = x_inner.read().unwrap();
            x_obj.obj_type.id == child_type_id
        }) {
            return Err("Child of this type already exists".to_string());
        }

        if !self
            .expected_children
            .iter()
            .any(|child_type| child_type.id == child_type_id)
        {
            let child_inner = child.read().unwrap();
            let child_obj = child_inner.inner();
            let child_obj = child_obj.read().unwrap();
            return Err(format!(
                "Child {} is not allowed for parent {}. Expected children: {:?}",
                child_obj.obj_type.ident,
                self.obj_type.ident,
                self.expected_children
                    .iter()
                    .map(|c| c.ident)
                    .collect::<Vec<_>>()
            ));
        }

        self.children.push(child);
        Ok(())
    }

    /// Add child without validation.
    pub fn add_child_unchecked(&mut self, child: Arc<RwLock<ConcretePageType>>) {
        let child_type_id = {
            let child_inner = child.read().unwrap();
            let child_obj = child_inner.inner();
            let child_obj = child_obj.read().unwrap();
            child_obj.obj_type.id
        };

        self.children.push(child);
    }

    /// Find and add all children from cache that have this object as their parent
    pub fn collect_children_from_cache(&mut self, cache: &ObjectCache) {
        let mut existing_child_types: Vec<TypeId> = Vec::new();
        for child in &self.children {
            if let Ok(child_inner) = child.try_read() {
                if let Ok(child_obj) = child_inner.inner().try_read() {
                    existing_child_types.push(child_obj.obj_type.id);
                }
            }
        }

        let expected_child_types: Vec<TypeId> = self
            .expected_children
            .iter()
            .map(|type_info| type_info.id)
            .collect();

        let mut candidates: Vec<(Arc<RwLock<ConcretePageType>>, TypeId, String)> = Vec::new();

        for item in cache.iter() {
            let obj = item.value();

            if self.children.iter().any(|child| Arc::ptr_eq(child, obj)) {
                continue;
            }

            let type_info = {
                let max_attempts = 3;
                let mut attempt_count = 0;

                loop {
                    if let Ok(obj_guard) = obj.try_read() {
                        let result = match &*obj_guard {
                            ConcretePageType::Key(key_page) => {
                                if let Ok(inner) = key_page.inner().try_read() {
                                    Some((inner.obj_type.id, inner.obj_type.ident.to_string()))
                                } else {
                                    None
                                }
                            }
                            ConcretePageType::Inferred(inferred_page) => {
                                if let Ok(inner) = inferred_page.inner().try_read() {
                                    Some((inner.obj_type.id, inner.obj_type.ident.to_string()))
                                } else {
                                    None
                                }
                            }
                            ConcretePageType::Pair(pair) => {
                                if let Ok(inner) = pair.inner.try_read() {
                                    Some((inner.obj_type.id, inner.obj_type.ident.to_string()))
                                } else {
                                    None
                                }
                            }
                        };

                        drop(obj_guard);

                        if let Some((type_id, type_name)) = result {
                            break Some((type_id, type_name));
                        }
                    }

                    attempt_count += 1;
                    if attempt_count >= max_attempts {
                        break None;
                    }

                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            };

            if let Some((type_id, type_name)) = type_info {
                if !existing_child_types.contains(&type_id) {
                    if expected_child_types.contains(&type_id) {
                        candidates.push((obj.clone(), type_id, type_name));
                    }
                }
            }
        }

        for (obj, type_id, _type_name) in candidates {
            self.children.push(obj);
            existing_child_types.push(type_id);
        }
    }

    /// Check if this object can have a specific child type
    pub fn can_have_child(&self, child_type_id: TypeId) -> bool {
        self.expected_children
            .iter()
            .any(|child_type| child_type.id == child_type_id)
    }

    /// Get all possible child types
    pub fn get_expected_child_types(&self) -> &[TypeInformation] {
        &self.expected_children
    }

    pub fn parent_inner(&self) -> Option<Arc<RwLock<ConcreteObject>>> {
        match &self.parent {
            Some(s) => Some(s.read().unwrap().inner()),
            None => None,
        }
    }
}

impl Clone for ConcreteObject {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            children: self.children.clone(),
            classification_method: self.classification_method.clone_box(),
            extraction_method: self.extraction_method.clone_box(),
            obj_type: self.obj_type.clone(),
            expected_children: self.expected_children.clone(),
        }
    }
}

/// Represents any type that is an [pdf_struct_traits::Object] and also implements [pdf_struct_traits::PairWith]
pub struct ConcretePair {
    pub sequence: PairSequence,
    pub patterns: Arc<Vec<Pattern>>,
    pub inner: Arc<RwLock<ConcreteObject>>,
    /// Should never be None, as creation of this pointer is handled by [ConcretePair::bind_pair]
    pub pair: RwLock<Option<Weak<ConcretePair>>>,
}

impl ConcretePair {
    pub fn bind_pair(
        obj_1: Arc<RwLock<ConcreteObject>>,
        obj_2: Arc<RwLock<ConcreteObject>>,
        patterns: Vec<Pattern>,
    ) -> (Arc<ConcretePair>, Arc<ConcretePair>) {
        let patterns = Arc::new(patterns);

        let p1 = Arc::new(Self::mutate_object_to_pair(
            obj_1,
            PairSequence::First,
            patterns.clone(),
        ));

        let p2 = Arc::new(Self::mutate_object_to_pair(
            obj_2,
            PairSequence::Last,
            patterns,
        ));

        *p1.pair.write().unwrap() = Some(Arc::downgrade(&p2));
        *p2.pair.write().unwrap() = Some(Arc::downgrade(&p1));

        (p1, p2)
    }

    /// Get pair information without creating circular references
    pub fn get_pair_info(&self) -> Option<Arc<ConcretePair>> {
        self.pair
            .read()
            .unwrap()
            .as_ref()
            .and_then(|weak| weak.upgrade())
    }

    /// Get the actual pair object from cache if it exists
    pub fn get_pair_inner(&self, cache: &ObjectCache) -> Option<Arc<RwLock<ConcreteObject>>> {
        self.pair.read().unwrap().as_ref().and_then(|pair| {
            let upgrade = pair.upgrade();
            if upgrade.is_none() {
                None
            } else {
                Some(pair.upgrade().unwrap().inner.clone())
            }
        })
    }

    /// Mutates a [ConcreteObject] into a [ConcretePair]
    /// But sets the pair pointer to point to nothing.  
    fn mutate_object_to_pair(
        obj: Arc<RwLock<ConcreteObject>>,
        sequence: PairSequence,
        patterns: Arc<Vec<Pattern>>,
    ) -> ConcretePair {
        Self {
            inner: obj,
            pair: RwLock::new(None),
            sequence,
            patterns,
        }
    }
}

impl Clone for ConcretePair {
    fn clone(&self) -> Self {
        Self {
            sequence: self.sequence.clone(),
            patterns: self.patterns.clone(),
            inner: self.inner.clone(),
            pair: RwLock::new(self.pair.read().unwrap().clone()),
        }
    }
}

pub struct ConcreteObjectBuilder {
    cache: ObjectCache,
}

impl ConcreteObjectBuilder {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    pub fn build<T, E>(&mut self) -> Arc<RwLock<ConcretePageType>>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        ConcreteObject::from_obj_with_cache::<T, E>(&mut self.cache)
    }

    pub fn build_with_pair<T, U, TE, UE>(&mut self) -> (Arc<ConcretePair>, Arc<ConcretePair>)
    where
        T: PairWith<U> + 'static,
        U: PairWith<T> + 'static,
        TE: Error + Debug + Display + 'static,
        UE: Error + Debug + Display + 'static,
    {
        if T::PATTERNS != U::PATTERNS {
            panic!(
                "Patterns of type {} didn't match patterns of type {}!",
                T::TYPE.ident,
                U::TYPE.ident
            );
        }

        let patterns = T::PATTERNS;
        let o1 = self.build::<T, TE>();
        let o2 = self.build::<U, UE>();
        let p1 = ConcretePair::bind_pair(
            o1.read().unwrap().inner(),
            o2.read().unwrap().inner(),
            patterns.to_vec(),
        );

        p1
    }

    fn unwrap_obj(obj: Arc<RwLock<ConcreteObject>>) -> ConcreteObject {
        match Arc::try_unwrap(obj) {
            Ok(rwlock) => rwlock.into_inner().unwrap(),
            Err(arc) => arc.read().unwrap().clone(),
        }
    }

    /// Build and automatically connect parent-child relationships
    pub fn build_with_relationships<T, E>(&mut self) -> Arc<RwLock<ConcretePageType>>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        let obj = self.build::<T, E>();

        let inner_arc = obj.read().unwrap().inner();
        let mut obj_mut = match Arc::try_unwrap(inner_arc) {
            Ok(rwlock) => rwlock.into_inner().unwrap(),
            Err(arc) => arc.read().unwrap().clone(),
        };

        obj_mut.collect_children_from_cache(&self.cache);

        let updated_obj = Arc::new(RwLock::new(obj_mut));
        let page_type = if T::KEY_PAGE {
            Arc::new(RwLock::new(ConcretePageType::Key(ConcreteKeyPage::from(
                updated_obj,
            ))))
        } else if T::INFERRED_PAGE {
            Arc::new(RwLock::new(ConcretePageType::Inferred(
                ConcreteInferredPage::from(updated_obj),
            )))
        } else {
            Arc::new(RwLock::new(ConcretePageType::Inferred(
                ConcreteInferredPage::from(updated_obj),
            )))
        };

        self.cache.insert(T::TYPE.id, page_type.clone());
        page_type
    }

    pub fn get_cache(&self) -> &ObjectCache {
        &self.cache
    }

    pub fn get_cache_mut(&mut self) -> &mut ObjectCache {
        &mut self.cache
    }
}

/// Represents any type that is an [pdf_struct_traits::Object] and also implements [pdf_struct_traits::Root]
pub struct ConcreteRoot {
    pub children: Vec<Arc<RwLock<ConcretePageType>>>,
    pub cache: ObjectCache,
}

impl ConcreteRoot {
    pub fn new() -> Self {
        Self {
            children: vec![],
            cache: DashMap::new(),
        }
    }

    /// Validate that a root child of type T is not already present.
    fn validate_root_child<T, E>(&mut self) -> Result<(), String>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        if self.children.iter().any(|c| {
            let child = c.read().unwrap();
            let child_inner = child.inner();
            let child_obj = child_inner.read().unwrap();
            child_obj.obj_type.id == T::TYPE.id
        }) {
            Err(format!(
                "Root child of type {} already exists",
                T::TYPE.ident
            ))
        } else {
            Ok(())
        }
    }

    pub fn add_child<T, E>(&mut self) -> Result<(), String>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        self.validate_root_child::<T, E>()?;

        if T::KEY_PAGE {
            let page = ConcretePageType::Key(ConcreteKeyPage::new::<T, E>(&mut self.cache));
            self.children.push(Arc::new(RwLock::new(page)));
        } else {
            let page =
                ConcretePageType::Inferred(ConcreteInferredPage::new::<T, E>(&mut self.cache));
            self.children.push(Arc::new(RwLock::new(page)));
        }

        Ok(())
    }

    /// Add a Pair child (creates both sides of the pair and inserts them).
    /// T and U must be mutually PairWith each other.
    pub fn add_pair_child<T, U, TE, UE>(&mut self) -> Result<(), String>
    where
        T: PairWith<U> + 'static,
        U: PairWith<T> + 'static,
        TE: Error + Debug + Display + 'static,
        UE: Error + Debug + Display + 'static,
    {
        // ensure neither side already present
        if self.children.iter().any(|c| {
            let child = c.read().unwrap();
            let child_inner = child.inner();
            let child_obj = child_inner.read().unwrap();
            let id = child_obj.obj_type.id;
            id == T::TYPE.id || id == U::TYPE.id
        }) {
            return Err(format!(
                "One of the pair types ({}, {}) already exists as a root child",
                T::TYPE.ident,
                U::TYPE.ident
            ));
        }

        let o1 = ConcreteObject::from_obj_with_cache::<T, TE>(&mut self.cache);
        let o2 = ConcreteObject::from_obj_with_cache::<U, UE>(&mut self.cache);

        let patterns = T::PATTERNS.to_vec();

        let (p1_rc, p2_rc) = ConcretePair::bind_pair(
            o1.read().unwrap().inner(),
            o2.read().unwrap().inner(),
            patterns,
        );

        self.children
            .push(Arc::new(RwLock::new(ConcretePageType::Pair(
                (*p1_rc).clone(),
            ))));
        self.children
            .push(Arc::new(RwLock::new(ConcretePageType::Pair(
                (*p2_rc).clone(),
            ))));

        Ok(())
    }

    /// Connect all parent-child relationships based on the cache
    pub fn connect_relationships(&mut self) {
        let mut all_objects: std::collections::HashMap<TypeId, Arc<RwLock<ConcretePageType>>> =
            std::collections::HashMap::new();

        for item in self.cache.iter() {
            let page_type = item.value();
            let page_type_locked = page_type.read().unwrap();
            let inner_obj = page_type_locked.inner();
            let obj_type_id = inner_obj.read().unwrap().obj_type.id;
            all_objects.insert(obj_type_id, page_type.clone());
        }

        let mut children_id_map: std::collections::HashMap<TypeId, Vec<TypeId>> =
            std::collections::HashMap::new();
        for obj in all_objects.values() {
            let obj_locked = obj.read().unwrap();
            let inner_obj = obj_locked.inner();
            let inner_obj_locked = inner_obj.read().unwrap();

            if let Some(parent_arc) = &inner_obj_locked.parent {
                let parent_locked = parent_arc.read().unwrap();
                let parent_inner = parent_locked.inner();
                let parent_obj = parent_inner.read().unwrap();
                let parent_id = parent_obj.obj_type.id;
                let child_id = inner_obj_locked.obj_type.id;

                children_id_map.entry(parent_id).or_default().push(child_id);
            }
        }

        for (parent_id, child_ids) in children_id_map {
            if let Some(parent_page_type) = all_objects.get(&parent_id) {
                let parent_locked = parent_page_type.read().unwrap();
                let parent_inner = parent_locked.inner();
                let mut parent_obj = parent_inner.write().unwrap();

                parent_obj.children.clear();

                for child_id in child_ids {
                    if let Some(child_page_type) = all_objects.get(&child_id) {
                        parent_obj.children.push(child_page_type.clone());
                    }
                }
            }
        }
    }
}

/// Represents any type that is an [pdf_struct_traits::Object] and also implements [pdf_struct_traits::KeyPage]
pub struct ConcreteKeyPage(Arc<RwLock<ConcreteObject>>);

impl Clone for ConcreteKeyPage {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ConcreteKeyPage {
    pub fn new<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        if !T::KEY_PAGE {
            panic!(
                "Attempted to construct a key page, without key being `true` within object information for object {}!",
                T::TYPE.ident
            )
        }

        let page_type = ConcreteObject::from_obj_with_cache::<T, E>(cache);
        Self(page_type.read().unwrap().inner())
    }

    pub fn inner(&self) -> Arc<RwLock<ConcreteObject>> {
        self.0.clone()
    }
}
impl From<Arc<RwLock<ConcreteObject>>> for ConcreteKeyPage {
    fn from(value: Arc<RwLock<ConcreteObject>>) -> Self {
        Self(value)
    }
}

/// Represents any type that is an [pdf_struct_traits::Object] and also implements [pdf_struct_traits::InferredPage]
pub struct ConcreteInferredPage(Arc<RwLock<ConcreteObject>>);

impl Clone for ConcreteInferredPage {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ConcreteInferredPage {
    pub fn new<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        if !T::INFERRED_PAGE {
            panic!(
                "Attempted to construct an inferred page, without inferred being `true` within object information! {}",
                T::TYPE.ident
            )
        }

        let page_type = ConcreteObject::from_obj_with_cache::<T, E>(cache);
        Self(page_type.read().unwrap().inner())
    }

    pub fn inner(&self) -> Arc<RwLock<ConcreteObject>> {
        self.0.clone()
    }
}

impl From<Arc<RwLock<ConcreteObject>>> for ConcreteInferredPage {
    fn from(value: Arc<RwLock<ConcreteObject>>) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_classification_and_extraction_casts() {
        use super::*;
        use pdf_struct_traits::*;
        use std::any::TypeId;
        use std::error::Error;
        use std::fmt::Display;

        #[derive(Debug)]
        struct SharedData;
        #[derive(Debug)]
        struct Constructed;
        #[derive(Debug)]
        struct MyError;
        impl Display for MyError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "my error")
            }
        }
        impl Error for MyError {}

        // fn(&[u8]) -> ClassificationResult<SharedData, MyError>
        fn classify_fn(_: &[u8]) -> ClassificationResult<SharedData, MyError> {
            ClassificationResult::Confident(90.0, SharedData)
        }

        // fn(&[u8], SharedData) -> Result<Constructed, MyError>
        fn extract_fn(_: &[u8], _s: SharedData) -> Result<Constructed, MyError> {
            Ok(Constructed)
        }

        let classification_method =
            Box::new(classify_fn as ClassificationMethod<SharedData, MyError>) as Box<dyn AnyClone>;

        let obj = ConcreteObject {
            parent: None,
            children: vec![],
            classification_method,
            extraction_method: Box::new(
                extract_fn as ExtractionMethod<SharedData, MyError, Constructed>,
            ) as Box<dyn AnyClone>,
            obj_type: TypeInformation {
                id: TypeId::of::<()>(),
                ident: "Test",
            },
            expected_children: vec![],
        };

        unsafe {
            let got_classify = obj
                .cast_classification::<SharedData, MyError>()
                .expect("classification cast failed");
            let got_ptr = got_classify as *const ();
            let want_ptr = classify_fn as *const ();
            assert_eq!(
                got_ptr, want_ptr,
                "classification function pointer mismatch"
            );

            let got_extract = obj
                .cast_extraction::<SharedData, MyError, Constructed>()
                .expect("extraction cast failed");
            let got_e_ptr = got_extract as *const ();
            let want_e_ptr = extract_fn as *const ();
            assert_eq!(
                got_e_ptr, want_e_ptr,
                "extraction function pointer mismatch"
            );
        }
    }
}
