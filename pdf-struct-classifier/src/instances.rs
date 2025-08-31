use pdf_struct_traits::InferredPage;
use pdf_struct_traits::KeyPage;
use pdf_struct_traits::Object;
use pdf_struct_traits::Pattern;
use pdf_struct_traits::TypeInformation;
use pdf_struct_traits::{PairSequence, PairWith};
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::rc::Rc;

/// Concretely defines an object as a Pair  
pub struct InstanstiatedPair {
    pub pair_type_info: TypeInformation,
    pub sequence: PairSequence,
    pub patterns: Vec<Pattern>,
}

pub trait AnyClone: Any {
    fn clone_box(&self) -> Box<dyn AnyClone>;
}

impl<T: Any + Clone> AnyClone for T {
    fn clone_box(&self) -> Box<dyn AnyClone> {
        Box::new(self.clone())
    }
}

/// Represents any struct that implements [Object]
pub struct InstanstiatedObject {
    pub parent: Option<Rc<InstanstiatedObject>>,
    pub children: Vec<Rc<InstanstiatedObject>>,
    pub pair: Option<InstanstiatedPair>,
    pub classification_method: Box<dyn AnyClone>,
    pub obj_type: TypeInformation,
    pub expected_children: Vec<TypeInformation>,
}

impl Clone for InstanstiatedObject {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            children: self.children.clone(),
            pair: self.pair.clone(),
            classification_method: self.classification_method.clone_box(),
            obj_type: self.obj_type.clone(),
            expected_children: self.expected_children.clone(),
        }
    }
}

// Make InstanstiatedPair cloneable too
impl Clone for InstanstiatedPair {
    fn clone(&self) -> Self {
        Self {
            pair_type_info: self.pair_type_info.clone(),
            sequence: self.sequence.clone(),
            patterns: self.patterns.clone(),
        }
    }
}

type ObjectCache = HashMap<TypeId, Rc<InstanstiatedObject>>;

impl InstanstiatedObject {
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
            pair: if T::Pair::TYPE.ident == "()" {
                None
            } else {
                Some(InstanstiatedPair {
                    pair_type_info: T::Pair::TYPE,
                    sequence: match T::Pair::SEQUENCE {
                        PairSequence::First => PairSequence::Last,
                        PairSequence::Last => PairSequence::First,
                        PairSequence::None => PairSequence::None,
                    },
                    patterns: T::Pair::PATTERNS.to_vec(),
                })
            },
            classification_method: Box::new(T::classify::<E> as fn(&[u8]) -> _),
            obj_type: T::TYPE,
            expected_children: T::CHILDREN.to_vec(),
        }
    }

    /// Add a child, checking if it's allowed
    pub fn add_child(&mut self, child: Rc<InstanstiatedObject>) -> Result<(), String> {
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
    pub fn add_child_unchecked(&mut self, child: Rc<InstanstiatedObject>) {
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
        let matching_children: Vec<Rc<InstanstiatedObject>> = cache
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

    /// Get pair information without creating circular references
    pub fn get_pair_info(&self) -> Option<&InstanstiatedPair> {
        self.pair.as_ref()
    }

    /// Get the actual pair object from cache if it exists
    pub fn get_pair_object(&self, cache: &ObjectCache) -> Option<Rc<InstanstiatedObject>> {
        self.pair
            .as_ref()
            .and_then(|pair| cache.get(&pair.pair_type_info.id))
            .cloned()
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

pub struct InstanstiatedObjectBuilder {
    cache: ObjectCache,
}

impl InstanstiatedObjectBuilder {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn build<T, E>(&mut self) -> Rc<InstanstiatedObject>
    where
        T: Object + 'static,
        E: Error + Debug + Display + 'static,
    {
        InstanstiatedObject::from_obj_with_cache::<T, E>(&mut self.cache)
    }

    /// Build and automatically connect parent-child relationships
    pub fn build_with_relationships<T, E>(&mut self) -> Rc<InstanstiatedObject>
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

/// Represents any struct that implements [Root]
pub struct InstanstiatedRoot {
    pub children: Vec<Rc<InstanstiatedObject>>,
    pub cache: ObjectCache,
}

impl InstanstiatedRoot {
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
        let child = InstanstiatedObject::from_obj_with_cache::<T, E>(&mut self.cache);

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

/// Concretely defines an Object as a KeyPage
pub struct InstanstiatedKeyPage(Rc<InstanstiatedObject>);

impl InstanstiatedKeyPage {
    pub fn new<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: KeyPage + 'static,
        E: Error + Debug + Display + 'static,
    {
        Self(InstanstiatedObject::from_obj_with_cache::<T, E>(cache))
    }

    pub fn inner(&self) -> &InstanstiatedObject {
        &self.0
    }
}

impl From<Rc<InstanstiatedObject>> for InstanstiatedKeyPage {
    fn from(value: Rc<InstanstiatedObject>) -> Self {
        Self { 0: value }
    }
}

/// Concretely defines an Object as an InferredPage
pub struct InstanstiatedInferredPage(Rc<InstanstiatedObject>);

impl InstanstiatedInferredPage {
    pub fn new<T, E>(cache: &mut ObjectCache) -> Self
    where
        T: InferredPage + 'static,
        E: Error + Debug + Display + 'static,
    {
        Self(InstanstiatedObject::from_obj_with_cache::<T, E>(cache))
    }

    pub fn inner(&self) -> &InstanstiatedObject {
        &self.0
    }
}

impl From<Rc<InstanstiatedObject>> for InstanstiatedInferredPage {
    fn from(value: Rc<InstanstiatedObject>) -> Self {
        Self { 0: value }
    }
}
