//! anymap at home

use downcast::{Any, AnySync, Downcast, downcast_sync};
use dyn_clone::{DynClone, clone_trait_object};
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

// IntoBox code taken from anymap source code
#[macro_export]
macro_rules! into_box {
    ($ty:ident $(+ $bounds:ident)*) => {
        impl<T: $ty $(+ $bounds)*> IntoBox<dyn $ty $(+ $bounds)*> for T {
            fn into_box(self) -> Box<dyn $ty $(+ $bounds)*> {
                Box::new(self)
            }
        }
    };
}

/// A trait that can be implemented on anything that can be converted into a box as something else.
pub trait IntoBox<T: ?Sized> {
    fn into_box(self) -> Box<T>;
}

/// Default type for anything held in an [AnyMap]
pub trait CloneAnySync: DynClone + AnySync {}
clone_trait_object!(CloneAnySync);
downcast_sync!(dyn CloneAnySync);
into_box!(CloneAnySync);
impl<T: Clone + AnySync> CloneAnySync for T {}
impl Debug for dyn CloneAnySync {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CloneAny")
            .field(&self.type_name())
            .finish_non_exhaustive()
    }
}

/// A map of different types of things
#[derive(Debug)]
pub struct AnyMap<T: ?Sized + DynClone = dyn CloneAnySync> {
    map: HashMap<TypeId, Box<T>>,
}

impl<T: ?Sized + DynClone + Any> Default for AnyMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized + DynClone> Clone for AnyMap<T> {
    fn clone(&self) -> Self {
        Self {
            map: self
                .map
                .iter()
                .map(|(id, val)| (*id, dyn_clone::clone_box(&**val)))
                .collect(),
        }
    }
}

impl<T: ?Sized + DynClone + Any> AnyMap<T> {
    /// Create a new empty `AnyMap`
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    /// Inserts a value into this `AnyMap`.
    ///
    /// There can only ever be one value of a given type in an `AnyMap`.
    pub fn insert<S: IntoBox<T> + 'static>(&mut self, value: S) -> Option<S>
    where
        T: Downcast<S>,
    {
        let id = TypeId::of::<S>();
        self.map
            .insert(id, value.into_box())
            .map(|res| {
                res.downcast().unwrap_or_else(|err| {
                    // should never happen
                    panic!(
                        "AnyMap.insert value {:?} type mismatch: {}",
                        id,
                        err.type_mismatch()
                    )
                })
            })
            .map(|res| *res)
    }

    /// Gets the value from this `AnyMap` with the given type.
    pub fn get<S: IntoBox<T> + 'static>(&self) -> Option<&S>
    where
        T: Downcast<S>,
    {
        let id = TypeId::of::<S>();
        self.map.get(&id).map(|res| {
            res.downcast_ref().unwrap_or_else(|err| {
                // should never happen
                panic!("AnyMap.get value {:?} type mismatch: {}", id, err)
            })
        })
    }
}

#[macro_export]
macro_rules! any_map {
    [$($val:expr),*] => {
        {
            let mut map = AnyMap::new();
            $(map.insert($val);)*
            map
        }
    };
}
