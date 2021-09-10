use std::{borrow::Borrow, pin::Pin};

pub trait RefPineMap<K: Ord, V> {
	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get<Q>(&self, key: &Q) -> Option<&V>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized;

	/// Tries to insert a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_insert_with<F: FnOnce() -> Result<V, E>, E>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E>;
}

pub trait MutPineMap<K: Ord, V> {
	/// Clears the map, removing all elements.
	///
	/// # Panics
	///
	/// Iff the instance was poisoned.
	fn clear(&mut self);

	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get<Q>(&mut self, key: &Q) -> Option<&mut V>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized;

	/// Tries to insert a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_insert_with<F: FnOnce() -> Result<V, E>, E>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E>;
}

/// # Safety
///
/// Any implementors must ensure their [`RefPineMap`] implementation would be valid if all `Value`s were pinned.
///
/// See: [`pin` -> `Drop` guarantee](https://doc.rust-lang.org/std/pin/index.html#drop-guarantee)
pub unsafe trait PinRefPineMap<K: Ord, V>: RefPineMap<K, V> {
	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get<Q>(self: Pin<&Self>, key: &Q) -> Option<Pin<&V>>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe {
			<Self as RefPineMap<_, _>>::get(&*self, key)
				.map(|value| Pin::new_unchecked(&*(value as *const _)))
		}
	}

	/// Tries to insert a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_insert_with<F: FnOnce() -> Result<V, E>, E>(
		self: Pin<&Self>,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&V>, (K, F)>, E> {
		unsafe {
			<Self as RefPineMap<_, _>>::try_insert_with(&*self, key, value_factory)
				.map(|inner| inner.map(|value| Pin::new_unchecked(&*(value as *const _))))
		}
	}
}

/// # Safety
///
/// Any implementors must ensure their [`MutPineMap`] implementation would be valid if all `Value`s were pinned.
///
/// See: [`pin` -> `Drop` guarantee](https://doc.rust-lang.org/std/pin/index.html#drop-guarantee)
pub unsafe trait PinMutPineMap<K: Ord, V>: MutPineMap<K, V> {
	/// Clears the map, removing all elements.
	///
	/// # Panics
	///
	/// Iff the instance was poisoned.
	fn clear(self: Pin<&mut Self>) {
		unsafe { <Self as MutPineMap<_, _>>::clear(self.get_unchecked_mut()) }
	}

	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get<Q>(self: Pin<&mut Self>, key: &Q) -> Option<Pin<&mut V>>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe {
			<Self as MutPineMap<_, _>>::get(self.get_unchecked_mut(), key)
				.map(|value| Pin::new_unchecked(value))
		}
	}

	/// Tries to insert a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_insert_with<F: FnOnce() -> Result<V, E>, E>(
		self: Pin<&mut Self>,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&mut V>, (K, F)>, E> {
		unsafe {
			<Self as MutPineMap<_, _>>::try_insert_with(
				self.get_unchecked_mut(),
				key,
				value_factory,
			)
			.map(|inner| inner.map(|value| Pin::new_unchecked(&mut *(value as *mut _))))
		}
	}
}
