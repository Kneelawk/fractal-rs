use itertools::Itertools;
use kstring::KString;
use liquid_core::{runtime::PartialStore, Renderable};
use std::{collections::HashMap, sync::Arc};

/// Combines a bunch of `PartialStore`s into one.
///
/// This will attempt to get partials from each of the stores it contains in
/// order. The returned partial is from the first store to successfully supply
/// the requested partial.
///
/// Determining which store provides which partial is done via the `names()`
/// method. The first store to list a partial name in its `names()` vec will be
/// the store to provide that partial. If the store errors while providing a
/// partial, an error is returned without checking any of the other stores.
#[derive(Debug, Clone)]
pub struct CompositePartialStore {
    stores: Vec<Arc<dyn PartialStore + Send + Sync>>,
    keys: HashMap<KString, usize>,
    names: Vec<KString>,
}

impl CompositePartialStore {
    /// Creates a new composite partial store containing the given partial
    /// stores.
    ///
    /// If multiple stores provide the same partial, the first store
    /// listed will be the one to provide the partial.
    pub fn new(stores: Vec<Arc<dyn PartialStore + Send + Sync>>) -> CompositePartialStore {
        // Maybe all this indexing is a little over-kill, but I don't really like the
        // idea of recalculating this stuff *every time* `names()` is called.
        let mut keys = HashMap::new();

        // `rev()` so that earlier stores override later stores.
        for (index, store) in stores.iter().enumerate().rev() {
            for name in store.names() {
                keys.insert(KString::from_ref(name), index);
            }
        }

        let names = keys.keys().cloned().sorted().collect();

        Self {
            stores,
            keys,
            names,
        }
    }
}

impl PartialStore for CompositePartialStore {
    fn contains(&self, name: &str) -> bool {
        self.keys.contains_key(name)
    }

    fn names(&self) -> Vec<&str> {
        self.names.iter().map(|s| s.as_str()).collect()
    }

    fn try_get(&self, name: &str) -> Option<Arc<dyn Renderable>> {
        self.keys
            .get(name)
            .and_then(|&index| self.stores.get(index))
            .and_then(|store| store.try_get(name))
    }

    fn get(&self, name: &str) -> liquid_core::Result<Arc<dyn Renderable>> {
        self.keys
            .get(name)
            .and_then(|&index| self.stores.get(index))
            .ok_or_else(|| {
                let available = itertools::join(self.names.iter(), ", ");
                liquid_core::Error::with_msg("Unknown partial-template")
                    .context("requested partial", name.to_owned())
                    .context("available partials", available)
            })
            .and_then(|store| store.get(name))
    }
}
