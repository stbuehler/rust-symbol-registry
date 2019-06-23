#![warn(missing_docs)]
//! A crate providing sharing of non-mutable strings ("symbols") through
//! registries; each registry will store a certain value only once.
//!
//! A registry will also cleanup (i.e. free) a string if it isn't used
//! by any `Symbol` anymore.
//!
//! Symbols can also be created standalone.
//!
//! ## Implementation
//!
//! Each `Symbol` has a (strong) reference count; when it reaches `0`
//! the symbol will be removed from the registry, unless the symbol got
//! cloned from the registry in the meantime again.
//!
//! The registry itself keeps no reference on the symbol (one might view
//! it as a "weak" reference), and the symbols keep weak reference of
//! the registry.
//!
//! The string data will be stored directly after the metadata of it
//! (i.e. the reference count, the registry reference and the length of
//! the string).

mod symbol;
mod symbol_no_rc;

pub use self::symbol::Symbol;
use self::symbol_no_rc::SymbolNoRc;

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct Registry {
	content: HashSet<SymbolNoRc>,
}

impl Registry {
	fn new() -> Self {
		Registry {
			content: HashSet::new(),
		}
	}

	fn find(&self, name: &str) -> Option<Symbol> {
		self.content.get(name).map(SymbolNoRc::symbol)
	}
}

/// Set of shared strings ("symbols")
///
/// Unused symbols are removed from the set automatically.
#[derive(Clone)]
pub struct SymbolRegistry {
	registry: Arc<Mutex<Registry>>,
}

impl SymbolRegistry {
	/// Create new registry.
	pub fn new() -> Self {
		SymbolRegistry {
			registry: Arc::new(Mutex::new(Registry::new())),
		}
	}

	/// Insert a string into the registry if not already present
	///
	/// Returns the symbol representing the value.
	pub fn insert(&self, value: &str) -> Symbol {
		let mut inner = self.registry.lock().expect("registry lock");

		if let Some(entry) = inner.content.get(value) {
			return entry.symbol();
		}

		let symbol = Symbol::new(value);
		inner.content.insert(symbol.clone_no_rc());
		debug_assert!(inner.content.get(value).expect("just inserted").0.ptr_eq(&symbol));
		// now set registry: we shouldn't drop any symbol within the
		// registry lock anymore (i.e. avoid deadlocks via
		// Symbol::drop); also no one else can have cloned it yet as we
		// still have the lock
		unsafe { symbol.set_registry(Arc::downgrade(&self.registry)); }
		symbol
	}

	/// Find symbol with value if stored in registry
	pub fn find(&self, value: &str) -> Option<Symbol> {
		self.registry.lock().expect("registry lock").find(value)
	}

	/// Check whether symbol is in registry
	///
	/// The actual symbol (not its value) is checked.
	pub fn is_local_symbol(&self, symbol: &Symbol) -> bool {
		if let Some(symreg) = symbol.registry() {
			symreg == *self
		} else {
			false
		}
	}

	/// Find symbol in registry
	///
	/// If symbol is direclty in registry `find_symbol` will return a
	/// clone of it.
	///
	/// Otherwise it will search for the symbol by value.
	pub fn find_symbol(&self, symbol: &Symbol) -> Option<Symbol> {
		if self.is_local_symbol(symbol) {
			return Some(symbol.clone());
		}

		self.find(&**symbol)
	}
}

impl Default for SymbolRegistry {
	fn default() -> Self {
		SymbolRegistry::new()
	}
}

impl PartialEq for SymbolRegistry {
	fn eq(&self, other: &SymbolRegistry) -> bool {
		Arc::ptr_eq(&self.registry, &other.registry)
	}
}

impl Eq for SymbolRegistry {
}

impl fmt::Debug for SymbolRegistry {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_tuple("SymbolRegistry").field(&self.registry.lock().expect("registry lock").content).finish()
	}
}

#[cfg(test)]
mod tests {
	use crate::{Symbol, SymbolRegistry};

	#[test]
	fn standalone() {
		let s = Symbol::from("standalone");
		let s1 = Symbol::from("standalone2");
		let s2 = s1.clone();
		let s3 = s1.clone();
		let s4 = Symbol::from("standalone2");
		assert_ne!(s, s1);
		assert_ne!(s, s2);
		assert_ne!(s, s3);
		assert_eq!(s1, s2);
		assert_eq!(s1, s2);
		assert_eq!(s2, s3);
		assert!(s2.ptr_eq(&s3));
		assert!(!s2.ptr_eq(&s4));
	}

	#[test]
	fn normal() {
		let r = SymbolRegistry::new();
		let s1 = r.insert("foo");
		r.insert("drop immediately");
		let s2 = r.insert("bar");
		assert_eq!(s1, r.find("foo").unwrap());
		assert_eq!(s1, r.find_symbol(&s1).unwrap());
		assert!(r.is_local_symbol(&s1));
		assert!(r.is_local_symbol(&r.find_symbol(&s1).unwrap()));
		assert_ne!(s1, s2);
	}
}
