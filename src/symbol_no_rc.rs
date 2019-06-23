use crate::Symbol;
use std::fmt;
use std::mem::ManuallyDrop;

pub(crate) struct SymbolNoRc(pub(crate) ManuallyDrop<Symbol>);

impl SymbolNoRc {
	pub(crate) fn symbol(&self) -> Symbol {
		(*self.0).clone()
	}
}

impl std::borrow::Borrow<str> for SymbolNoRc {
	fn borrow(&self) -> &str {
		self.0.value()
	}
}

impl PartialEq for SymbolNoRc {
	fn eq(&self, other: &SymbolNoRc) -> bool {
		self.0.value() == other.0.value()
	}
}

impl Eq for SymbolNoRc {
}

impl std::hash::Hash for SymbolNoRc {
	fn hash<H>(&self, state: &mut H)
	where
		H: std::hash::Hasher,
	{
		self.0.value().hash(state)
	}
}

impl fmt::Debug for SymbolNoRc {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		(**self.0).fmt(f)
	}
}
