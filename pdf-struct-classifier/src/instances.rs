use pdf_struct_traits::*;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::rc::{Rc, Weak};

pub trait AnyClone: Any {
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
type ClassificationMethod<T, E> = fn(&[u8]) -> ClassificationResult<T, E>;

/// Where:
///     T is shared data
///     E is error
///     S is Self (the constructed object)
type ExtractionMethod<T, E, S> = fn(&[u8], T) -> Result<S, E>;

/// Cache holding information of each  
type ObjectCache = HashMap<TypeId, Rc<ConcreteObject>>;

/// Crate-level error that can only be called when attempting
/// to cast into a [ClassificationMethod] or [ExtractionMethod]
#[derive(Debug)]
pub(crate) enum CastError {
    TypeMismatch { expected: TypeId, actual: TypeId },
}

/// Represents a type that implements [pdf_struct_traits::Object] at runtime.
pub struct ConcreteObject {
    pub parent: Option<Rc<ConcreteObject>>,
    pub children: Vec<Rc<ConcreteObject>>,
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
    pub fn from_obj_with_cache<T, E>(cache: &mut ObjectCache) -> Rc<Self>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        if let Some(cached) = cache.get(&T::TYPE.id) {
            return cached.clone();
        }

        let obj = Rc::new(Self::from_obj_internal::<T, E>(cache));

        cache.insert(T::TYPE.id, obj.clone());

        obj
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
    pub fn add_child(&mut self, child: Rc<ConcreteObject>) -> Result<(), String> {
        if self
            .children
            .iter()
            .any(|x| x.obj_type.id == child.obj_type.id)
        {
            return Err("Child of this type already exists".to_string());
        }

        if !self
            .expected_children
            .iter()
            .any(|child_type| child_type.id == child.obj_type.id)
        {
            return Err(format!(
                "Child {} is not allowed for parent {}. Expected children: {:?}",
                child.obj_type.ident,
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

    /// Add child without validation (for internal use)
    pub fn add_child_unchecked(&mut self, child: Rc<ConcreteObject>) {
        if !self
            .children
            .iter()
            .any(|x| x.obj_type.id == child.obj_type.id)
        {
            self.children.push(child);
        }
    }

    /// Find and add all children from cache that have this object as their parent
    pub fn collect_children_from_cache(&mut self, cache: &ObjectCache) {
        let matching_children: Vec<Rc<ConcreteObject>> = cache
            .values()
            .filter(|obj| {
                // Check if this object is the parent of the cached object
                obj.parent
                    .as_ref()
                    .map(|parent| parent.obj_type.id == self.obj_type.id)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        for child in matching_children {
            self.add_child_unchecked(child);
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
    pub patterns: Rc<Vec<Pattern>>,
    pub inner: Rc<ConcreteObject>,
    /// Should never be None, as creation of this pointer is handled by [ConcretePair::bind_pair]
    pub pair: RefCell<Option<Weak<ConcretePair>>>,
}

impl ConcretePair {
    pub fn bind_pair(
        obj_1: Rc<ConcreteObject>,
        obj_2: Rc<ConcreteObject>,
        patterns: Vec<Pattern>,
    ) -> (Rc<ConcretePair>, Rc<ConcretePair>) {
        let patterns = Rc::new(patterns);

        let p1 = Rc::new(Self::mutate_object(
            obj_1,
            PairSequence::First,
            patterns.clone(),
        ));

        // cargo fmt doesn't expand this because we don't clone patterns so it isn't long enough
        // and it's is pissing me off
        let p2 = Rc::new(Self::mutate_object(obj_2, PairSequence::Last, patterns));

        *p1.pair.borrow_mut() = Some(Rc::downgrade(&p2));
        *p2.pair.borrow_mut() = Some(Rc::downgrade(&p1));

        (p1, p2)
    }

    /// Get pair information without creating circular references
    pub fn get_pair_info(&self) -> Option<Rc<ConcretePair>> {
        self.pair.borrow().as_ref().and_then(|weak| weak.upgrade())
    }

    /// Get the actual pair object from cache if it exists
    pub fn get_pair_inner(&self, cache: &ObjectCache) -> Option<Rc<ConcreteObject>> {
        self.pair.borrow().as_ref().and_then(|pair| {
            let upgrade = pair.upgrade();
            if upgrade.is_none() {
                None
            } else {
                Some(pair.upgrade().unwrap().inner.clone())
            }
        })
    }

    /// Mutates a ConcreteObject into a ConcretePair
    /// But sets the pair pointer to point to nothing.  
    fn mutate_object(
        obj: Rc<ConcreteObject>,
        sequence: PairSequence,
        patterns: Rc<Vec<Pattern>>,
    ) -> ConcretePair {
        Self {
            inner: obj,
            pair: RefCell::new(None),
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
            pair: RefCell::new(self.pair.borrow().clone()),
        }
    }
}

pub struct ConcreteObjectBuilder {
    cache: ObjectCache,
}

impl ConcreteObjectBuilder {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn build<T, E>(&mut self) -> Rc<ConcreteObject>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        ConcreteObject::from_obj_with_cache::<T, E>(&mut self.cache)
    }

    pub fn build_with_pair<T, U, TE, UE>(&mut self) -> (Rc<ConcretePair>, Rc<ConcretePair>)
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

        // Since we check if the patterns differ above, we can just use either patterns variable as they will be identical.
        let patterns = T::PATTERNS;
        let o1 = self.build::<T, TE>();
        let o2 = self.build::<U, UE>();
        let p1 = ConcretePair::bind_pair(o1, o2, patterns.to_vec());

        p1
    }

    fn unwrap_obj(obj: Rc<ConcreteObject>) -> ConcreteObject {
        match Rc::try_unwrap(obj) {
            Ok(t) => t,
            Err(rc) => (*rc).clone(),
        }
    }

    /// Build and automatically connect parent-child relationships
    pub fn build_with_relationships<T, E>(&mut self) -> Rc<ConcreteObject>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        let obj = self.build::<T, E>();

        let mut obj_mut = Rc::try_unwrap(obj).unwrap_or_else(|rc| (*rc).clone());
        obj_mut.collect_children_from_cache(&self.cache);

        let obj_rc = Rc::new(obj_mut);
        self.cache.insert(T::TYPE.id, obj_rc.clone());

        obj_rc
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
    pub children: Vec<Rc<ConcreteObject>>,
    pub cache: ObjectCache,
}

impl ConcreteRoot {
    pub fn new() -> Self {
        Self {
            children: vec![],
            cache: HashMap::new(),
        }
    }

    pub fn add_root_child<T, E>(&mut self) -> Result<(), String>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        let child = ConcreteObject::from_obj_with_cache::<T, E>(&mut self.cache);

        if self.children.iter().any(|c| c.obj_type.id == T::TYPE.id) {
            return Err(format!(
                "Root child of type {} already exists",
                T::TYPE.ident
            ));
        }

        self.children.push(child);
        Ok(())
    }

    /// Connect all parent-child relationships based on the cache
    pub fn connect_relationships(&mut self) {
        let snapshot = self.cache.clone();

        let mut children_id_map: HashMap<TypeId, Vec<TypeId>> = HashMap::new();
        for child in snapshot.values() {
            if let Some(parent_rc) = child.parent.as_ref() {
                children_id_map
                    .entry(parent_rc.obj_type.id)
                    .or_default()
                    .push(child.obj_type.id);
            }
        }

        let mut new_cache: ObjectCache = HashMap::new();
        for (id, rc) in snapshot.into_iter() {
            let mut obj = (*rc).clone();
            obj.children = Vec::new();
            new_cache.insert(id, Rc::new(obj));
        }

        // Populate children by iterating over a collected list of keys to avoid
        // borrowing/move conflicts while we replace entries in the map.
        let keys: Vec<TypeId> = new_cache.keys().cloned().collect();
        for id in keys {
            if let Some(inter_rc) = new_cache.get(&id).cloned() {
                let mut obj = (*inter_rc).clone();
                if let Some(child_ids) = children_id_map.get(&id) {
                    obj.children = child_ids
                        .iter()
                        .filter_map(|cid| new_cache.get(cid).cloned())
                        .collect();
                }
                new_cache.insert(id, Rc::new(obj));
            }
        }

        self.cache = new_cache;
    }
}

/// Represents any type that is an [pdf_struct_traits::Object] and also implements [pdf_struct_traits::KeyPage]
pub struct ConcreteKeyPage(Rc<ConcreteObject>);

impl ConcreteKeyPage {
    pub fn new<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: KeyPage + 'static,
        E: Error + Debug + Display + 'static,
    {
        Self(ConcreteObject::from_obj_with_cache::<T, E>(cache))
    }

    pub fn inner(&self) -> &ConcreteObject {
        &self.0
    }
}
impl From<Rc<ConcreteObject>> for ConcreteKeyPage {
    fn from(value: Rc<ConcreteObject>) -> Self {
        Self { 0: value }
    }
}

/// Represents any type that is an [pdf_struct_traits::Object] and also implements [pdf_struct_traits::InferredPage]
pub struct ConcreteInferredPage(Rc<ConcreteObject>);

impl ConcreteInferredPage {
    pub fn new<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: InferredPage + 'static,
        E: Error + Debug + Display + 'static,
    {
        Self(ConcreteObject::from_obj_with_cache::<T, E>(cache))
    }

    pub fn inner(&self) -> &ConcreteObject {
        &self.0
    }
}

impl From<Rc<ConcreteObject>> for ConcreteInferredPage {
    fn from(value: Rc<ConcreteObject>) -> Self {
        Self { 0: value }
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
