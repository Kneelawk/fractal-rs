use itertools::Itertools;
use kstring::KString;
use liquid_core::{parser::FilterChain, Template, Value};
use std::{collections::HashMap, sync::Arc};

/// Stores global macro state.
///
/// All macros defined are global. This behavior may not be desirable in most
/// circumstances, but given that the present use of liquid is to emulate a
/// C-like pre-processor, this behavior is intended.
#[derive(Debug, Default, Clone)]
pub struct MacrosRegister {
    macros: HashMap<KString, MacroObject>,
}

/// Represents a declared macro.
///
/// Declared macros can be of three types, representing various levels of
/// complexity.
#[derive(Debug, Clone)]
pub enum MacroObject {
    /// Represents a flag macro that contains no actual state.
    ///
    /// This is useful for `{% ifdef %}` or `{% ifndef %}` blocks, allowing
    /// someone to make sure something included multiple times is only rendered
    /// once.
    Flag,

    /// Represents a filter-chain that can be evaluated in the context of the
    /// macro call-site.
    ///
    /// This is useful for simple reusable logic or for composing macros using
    /// macro filters.
    FilterChain {
        arguments: Vec<MacroArgument>,
        chain: Arc<FilterChain>,
    },

    /// Represents a full partial to be evaluated in the context of the
    /// call-site.
    ///
    /// This is useful for complex logic.
    Partial {
        arguments: Vec<MacroArgument>,
        template: Arc<Template>,
    },
}

/// Represents an argument in a macro declaration.
///
/// Macro arguments can either be required arguments or defaulted arguments.
#[derive(Debug, Clone)]
pub enum MacroArgument {
    /// Represents a macro argument that must be specified at the call-site.
    Required { name: KString },

    /// Represents a macro argument with a default value.
    ///
    /// Note: This is currently the only way for a macro to capture
    /// declaration-site variables.
    Defaulted { name: KString, default: Value },
}

impl MacrosRegister {
    /// Inserts a macro object into this register with the associated name.
    ///
    /// If a macro already existed with the given name, then it is returned.
    /// Otherwise `None` is returned.
    pub fn define(&mut self, name: KString, obj: MacroObject) -> Option<MacroObject> {
        self.macros.insert(name, obj)
    }

    /// Checks whether a macro with the given name has been defined already.
    pub fn is_defined(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// Removes the macro object with the given name from this register.
    ///
    /// If a macro with the given name did indeed exist in this register, then
    /// it is returned. Otherwise `None` is returned.
    pub fn undefine(&mut self, name: &str) -> Option<MacroObject> {
        self.macros.remove(name)
    }

    /// Gets a macro object for the given name.
    pub fn get(&self, name: &str) -> Option<&MacroObject> {
        self.macros.get(name)
    }

    /// Gets a list of all currently defined macro names.
    pub fn available(&self) -> Vec<KString> {
        self.macros.keys().cloned().sorted().collect()
    }
}

impl MacroObject {
    /// Gets whether this macro object is a "Flag" type macro.
    pub fn is_flag(&self) -> bool {
        match self {
            MacroObject::Flag => true,
            MacroObject::FilterChain { .. } => false,
            MacroObject::Partial { .. } => false,
        }
    }
}
