use crate::{Registry, SymbolNoRc, SymbolRegistry};
use std::alloc;
use std::cell::UnsafeCell;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{size_of, align_of, ManuallyDrop};
use std::ptr::NonNull;
use std::sync::{Mutex, Weak};
use std::sync::atomic::{fence, AtomicUsize, Ordering};

fn make_layout(len: usize) -> alloc::Layout {
	alloc::Layout::from_size_align(
		size_of::<Inner>().checked_add(len as usize).expect("size overflow"),
		align_of::<Inner>(),
	).expect("size overflow")
}

const MAX_REFCOUNT: usize = isize::max_value() as usize;

struct Inner {
	strong: AtomicUsize,
	registry: UnsafeCell<Option<Weak<Mutex<Registry>>>>,
	len: usize,
}

const DATA_OFFSET: usize = size_of::<Inner>();

/// Stores a shared string
///
/// Sharing established by either cloning the `Symbol` or by looking it
/// up in the registry.
pub struct Symbol {
	ptr: NonNull<Inner>,
	_phantom: PhantomData<Inner>,
}

unsafe impl Send for Symbol {}
unsafe impl Sync for Symbol {}

impl Symbol {
	/// Create new standalone symbol
	pub fn new(data: &str) -> Self {
		let len = data.len();
		let inner = Inner {
			strong: AtomicUsize::new(1),
			registry: UnsafeCell::new(None),
			len,
		};
		unsafe {
			let ptr = alloc::alloc(make_layout(len));
			assert_ne!(ptr, std::ptr::null_mut(), "allocation failed");
			(ptr as *mut Inner).write(inner);

			let buf = {
				let data: *mut u8 = ptr.add(DATA_OFFSET);
				std::slice::from_raw_parts_mut(data, len)
			};
			buf.copy_from_slice(data.as_bytes());
			Symbol {
				ptr: NonNull::new_unchecked(ptr as *mut Inner), // checked above
				_phantom: PhantomData,
			}
		}
	}

	/// String value of symbol
	pub fn value(&self) -> &str {
		let len = self.inner().len as usize;
		unsafe {
			let data: *const u8 = (self.ptr.as_ptr() as *const u8).add(DATA_OFFSET);
			std::str::from_utf8_unchecked(
				std::slice::from_raw_parts(data, len)
			)
		}
	}

	/// Compare by data pointer: only equal if in same registry.
	///
	/// Standalone symbols are only equal to each other if they were
	/// cloned from the same "base" symbol.
	pub fn ptr_eq(&self, other: &Self) -> bool {
		self.ptr == other.ptr
	}

	pub(crate) unsafe fn set_registry(&self, registry: Weak<Mutex<Registry>>) {
		*self.inner().registry.get() = Some(registry);
	}

	pub(crate) fn clone_no_rc(&self) -> SymbolNoRc {
		SymbolNoRc(ManuallyDrop::new(Symbol {
			ptr: self.ptr,
			_phantom: self._phantom,
		}))
	}

	fn inner(&self) -> &Inner {
		unsafe { self.ptr.as_ref() }
	}

	pub(crate) fn registry(&self) -> Option<SymbolRegistry> {
		let reg = unsafe { &*self.inner().registry.get() };
		reg.as_ref().and_then(|r| {
			Some(SymbolRegistry {
				registry: Weak::upgrade(r)?,
			})
		})
	}

	#[inline(never)]
	unsafe fn drop_slow(&mut self) {
		if let Some(reg) = self.registry() {
			let mut reg = reg.registry.lock().expect("registry lock");

			if self.inner().strong.load(Ordering::Relaxed) > 0 {
				// although rc dropped to 0 in between, registry lookup
				// increased it again - cancel drop
				return;
			}

			// now we got the registry lock *and* rc is 0. remove from registry.

			#[cfg(debug_assertions)]
			{
				let have = reg.content.get(&**self).expect("must be registered");
				assert!(have.0.ptr_eq(self), "must match expected entry");
			}
			// (could to debug lookup to make sure entry is actually)

			reg.content.remove(&**self);
		}

		let layout = make_layout(self.inner().len);
		std::ptr::drop_in_place(self.ptr.as_ptr());
		alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
	}
}

impl std::ops::Deref for Symbol {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.value()
	}
}

impl std::borrow::Borrow<str> for Symbol {
	fn borrow(&self) -> &str {
		self.value()
	}
}

impl Clone for Symbol {
	fn clone(&self) -> Self {
		let old_size = self.inner().strong.fetch_add(1, Ordering::Relaxed);

		if old_size > MAX_REFCOUNT {
			std::process::abort();
		}

		Symbol {
			ptr: self.ptr,
			_phantom: self._phantom,
		}
	}
}

impl Drop for Symbol {
	#[inline]
	fn drop(&mut self) {
		if self.inner().strong.fetch_sub(1, Ordering::Release) != 1 {
			return;
		}
		fence(Ordering::Acquire);
		unsafe {
			self.drop_slow();
		}
	}
}

impl PartialEq for Symbol {
	fn eq(&self, other: &Symbol) -> bool {
		self.value() == other.value()
	}
}

impl Eq for Symbol {
}

impl PartialOrd for Symbol {
	fn partial_cmp(&self, other: &Symbol) -> Option<std::cmp::Ordering> {
		Some(self.value().cmp(other.value()))
	}
}

impl Ord for Symbol {
	fn cmp(&self, other: &Symbol) -> std::cmp::Ordering {
		self.value().cmp(other.value())
	}
}

impl std::hash::Hash for Symbol {
	fn hash<H>(&self, state: &mut H)
	where
		H: std::hash::Hasher,
	{
		self.value().hash(state)
	}
}

impl fmt::Debug for Symbol {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		(**self).fmt(f)
	}
}

impl From<&str> for Symbol {
	fn from(v: &str) -> Self {
		Symbol::new(v)
	}
}
