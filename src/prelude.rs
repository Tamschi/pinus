//! The meat of the API.

use crate::UnwrapInfallible;
use std::{
	borrow::{Borrow, BorrowMut},
	cell::Cell,
	mem::MaybeUninit,
	ops::Deref,
	pin::Pin,
};
use tap::Pipe;

pub trait UnpinnedPineMap<K: Ord, V: ?Sized> {
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
	fn try_insert_with<F: FnOnce(&K) -> Result<V, E>, E>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E>
	where
		V: Sized;

	/// Inserts a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_with<F: FnOnce(&K) -> V>(&self, key: K, value_factory: F) -> Result<&V, (K, F)>
	where
		V: Sized, // Just for clarity.
	{
		let value_factory = Cell::new(Some(value_factory));
		self.try_insert_with(key, |key| {
			value_factory.take().expect("unreachable")(key).pipe(Ok)
		})
		.unwrap_infallible()
		.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
	}

	/// Inserts a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert(&self, key: K, value: V) -> Result<&V, (K, V)>
	where
		V: Sized,
	{
		let value = Cell::new(Some(value));
		self.insert_with(key, |_| value.take().expect("unreachable"))
			.map_err(|(key, _)| (key, value.take().expect("unreachable")))
	}

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
	fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
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
	fn try_insert_with_mut<F: FnOnce(&K) -> Result<V, E>, E>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E>
	where
		V: Sized;

	/// Inserts a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_with_mut<F: FnOnce(&K) -> V>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<&mut V, (K, F)>
	where
		V: Sized, // Just for clarity.
	{
		let value_factory = Cell::new(Some(value_factory));
		self.try_insert_with_mut(key, |key| {
			value_factory.take().expect("unreachable")(key).pipe(Ok)
		})
		.unwrap_infallible()
		.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
	}

	/// Inserts a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_mut(&mut self, key: K, value: V) -> Result<&mut V, (K, V)>
	where
		V: Sized,
	{
		let value = Cell::new(Some(value));
		self.insert_with_mut(key, |_| value.take().expect("unreachable"))
			.map_err(|(key, _)| (key, value.take().expect("unreachable")))
	}

	/// Removes and returns a key-value pair if a matching key exists.
	fn remove_pair<Q>(&mut self, key: &Q) -> Option<(K, V)>
	where
		V: Sized,
		K: Borrow<Q>,
		Q: Ord + ?Sized;

	/// Removes and returns a key-value pair if a matching key exists.
	fn remove_value<Q>(&mut self, key: &Q) -> Option<V>
	where
		V: Sized,
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		self.remove_pair(key).map(|(_, v)| v)
	}

	/// Removes and returns a key if a matching key exists.
	///
	/// The value is dropped, and the collection isn't poisoned if this causes a panic.
	fn remove_key<Q>(&mut self, key: &Q) -> Option<K>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized;

	/// If a matching key exists, drops the associated key and value. (In unspecified order!)
	///
	/// The collection isn't poisoned if a panic occurs while dropping either key or value.
	///
	/// # Returns
	///
	/// Whether a matching entry was found.
	fn drop_entry<Q>(&mut self, key: &Q) -> bool
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		self.remove_key(key).is_some()
	}
}

pub trait UnpinnedPineMapEmplace<K: Ord, V: ?Sized, W> {
	/// Tries to emplace a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_emplace_with<F: for<'a> FnOnce(&K, &'a mut MaybeUninit<W>) -> Result<&'a mut V, E>, E>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E>;

	/// Emplaces a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_with<F: for<'a> FnOnce(&K, &'a mut MaybeUninit<W>) -> &'a mut V>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<&V, (K, F)> {
		let value_factory = Cell::new(Some(value_factory));
		self.try_emplace_with(key, |key, slot| {
			value_factory.take().expect("unreachable")(key, slot).pipe(Ok)
		})
		.unwrap_infallible()
		.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
	}

	/// Emplaces a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace(&self, key: K, value: W) -> Result<&V, (K, W)>
	where
		W: BorrowMut<V>,
	{
		let value = Cell::new(Some(value));
		self.emplace_with(key, |_, slot| {
			slot.write(value.take().expect("unreachable")).borrow_mut()
		})
		.map_err(|(key, _)| (key, value.take().expect("unreachable")))
	}

	/// Tries to emplace a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_emplace_with_mut<
		F: for<'a> FnOnce(&K, &'a mut MaybeUninit<W>) -> Result<&'a mut V, E>,
		E,
	>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E>;

	/// Emplaces a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_with_mut<F: for<'a> FnOnce(&K, &'a mut MaybeUninit<W>) -> &'a mut V>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<&mut V, (K, F)> {
		let value_factory = Cell::new(Some(value_factory));
		self.try_emplace_with_mut(key, |key, slot| {
			value_factory.take().expect("unreachable")(key, slot).pipe(Ok)
		})
		.unwrap_infallible()
		.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
	}

	/// Emplaces a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_mut(&mut self, key: K, value: W) -> Result<&mut V, (K, W)>
	where
		W: BorrowMut<V>,
	{
		let value = Cell::new(Some(value));
		self.emplace_with_mut(key, |_, slot| {
			slot.write(value.take().expect("unreachable")).borrow_mut()
		})
		.map_err(|(key, _)| (key, value.take().expect("unreachable")))
	}
}

/// # Safety
///
/// Any implementors must ensure their [`UnpinnedPineMap`] implementation would be valid if all `V`alues were pinned.
///
/// See: [`pin` -> `Drop` guarantee](https://doc.rust-lang.org/std/pin/index.html#drop-guarantee)
pub unsafe trait PinnedPineMap<K: Ord, V: ?Sized>: UnpinnedPineMap<K, V> {
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
			<Self as UnpinnedPineMap<_, _>>::get(&*self, key)
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
	fn try_insert_with<F: FnOnce(&K) -> Result<V, E>, E>(
		self: Pin<&Self>,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&V>, (K, F)>, E>
	where
		V: Sized,
	{
		unsafe {
			<Self as UnpinnedPineMap<_, _>>::try_insert_with(&*self, key, value_factory)
				.map(|inner| inner.map(|value| Pin::new_unchecked(&*(value as *const _))))
		}
	}

	/// Inserts a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_with<F: FnOnce(&K) -> V>(
		self: Pin<&Self>,
		key: K,
		value_factory: F,
	) -> Result<Pin<&V>, (K, F)>
	where
		V: Sized, // Just for clarity.
	{
		let value_factory = Cell::new(Some(value_factory));
		self.try_insert_with(key, |key| {
			value_factory.take().expect("unreachable")(key).pipe(Ok)
		})
		.unwrap_infallible()
		.map_err(|(k, _)| (k, value_factory.take().expect("unreachable")))
	}

	/// Inserts a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert(self: Pin<&Self>, key: K, value: V) -> Result<Pin<&V>, (K, V)>
	where
		V: Sized,
	{
		let value = Cell::new(Some(value));
		self.insert_with(key, |_| value.take().expect("unreachable"))
			.map_err(|(k, _)| (k, value.take().expect("unreachable")))
	}

	/// Clears the map, removing all elements.
	///
	/// # Panics
	///
	/// Iff the instance was poisoned.
	fn clear(self: Pin<&mut Self>) {
		unsafe { <Self as UnpinnedPineMap<_, _>>::clear(self.get_unchecked_mut()) }
	}

	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get_mut_pinned<Q>(self: Pin<&mut Self>, key: &Q) -> Option<Pin<&mut V>>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe {
			<Self as UnpinnedPineMap<_, _>>::get_mut(self.get_unchecked_mut(), key)
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
	fn try_insert_with_mut<F: FnOnce(&K) -> Result<V, E>, E>(
		self: Pin<&mut Self>,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&mut V>, (K, F)>, E>
	where
		V: Sized,
	{
		unsafe {
			<Self as UnpinnedPineMap<_, _>>::try_insert_with_mut(
				self.get_unchecked_mut(),
				key,
				value_factory,
			)
			.map(|inner| inner.map(|value| Pin::new_unchecked(&mut *(value as *mut _))))
		}
	}

	/// Inserts a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_with_mut<F: FnOnce(&K) -> V>(
		self: Pin<&mut Self>,
		key: K,
		value_factory: F,
	) -> Result<Pin<&mut V>, (K, F)>
	where
		V: Sized, // Just for clarity.
	{
		let value_factory = Cell::new(Some(value_factory));
		self.try_insert_with_mut(key, |key| {
			value_factory.take().expect("unreachable")(key).pipe(Ok)
		})
		.unwrap_infallible()
		.map_err(|(k, _)| (k, value_factory.take().expect("unreachable")))
	}

	/// Inserts a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_mut(self: Pin<&mut Self>, key: K, value: V) -> Result<Pin<&mut V>, (K, V)>
	where
		V: Sized,
	{
		let value = Cell::new(Some(value));
		self.insert_with_mut(key, |_| value.take().expect("unreachable"))
			.map_err(|(k, _)| (k, value.take().expect("unreachable")))
	}

	/// Removes and returns a key if a matching key exists.
	///
	/// The collection isn't poisoned if this causes a panic.
	fn remove_key<Q>(self: Pin<&mut Self>, key: &Q) -> Option<K>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe { <Self as UnpinnedPineMap<_, _>>::remove_key(self.get_unchecked_mut(), key) }
	}

	/// If a matching key exists, drops the associated key and value. (In unspecified order!)
	///
	/// The collection isn't poisoned if a panic occurs while dropping either key or value.
	///
	/// **Note that only the value is pinned!** The key is not necessarily dropped in place.
	///
	/// # Returns
	///
	/// Whether a matching entry was found.
	fn drop_entry<Q>(self: Pin<&mut Self>, key: &Q) -> bool
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		self.remove_key(key).is_some()
	}
}

/// # Safety
///
/// Any implementors must ensure their [`UnpinnedPineMapEmplace`] implementation would be valid if all `V`alues were pinned.
///
/// See: [`pin` -> `Drop` guarantee](https://doc.rust-lang.org/std/pin/index.html#drop-guarantee)
pub unsafe trait PinnedPineMapEmplace<K: Ord, V: ?Sized, W>:
	UnpinnedPineMapEmplace<K, V, W>
{
	/// Tries to emplace a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_emplace_with<
		F: for<'a> FnOnce(&K, Pin<&'a mut MaybeUninit<W>>) -> Result<Pin<&'a mut V>, E>,
		E,
	>(
		self: Pin<&Self>,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&V>, (K, F)>, E> {
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			self.deref()
				.try_emplace_with(key, |key, slot| {
					value_factory.take().expect("unreachable")(key, Pin::new_unchecked(slot))
						.map(|value| Pin::into_inner_unchecked(value))
				})?
				.map(|value| Pin::new_unchecked(&*(value as *const _)))
				.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
				.pipe(Ok)
		}
	}

	/// Emplaces a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_with<F: for<'a> FnOnce(&K, Pin<&'a mut MaybeUninit<W>>) -> Pin<&'a mut V>>(
		self: Pin<&Self>,
		key: K,
		value_factory: F,
	) -> Result<Pin<&V>, (K, F)> {
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			self.deref()
				.emplace_with(key, |key, slot| {
					value_factory.take().expect("unreachable")(key, Pin::new_unchecked(slot))
						.pipe(|value| Pin::into_inner_unchecked(value))
				})
				.map(|value| Pin::new_unchecked(&*(value as *const _)))
				.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
		}
	}

	/// Emplaces a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace(self: Pin<&Self>, key: K, value: W) -> Result<Pin<&V>, (K, W)>
	where
		W: BorrowMut<V>,
	{
		let value = Cell::new(Some(value));
		unsafe {
			self.deref()
				.emplace_with(key, |_, slot| {
					slot.write(value.take().expect("unreachable")).borrow_mut()
				})
				.map(|value| Pin::new_unchecked(&*(value as *const _)))
				.map_err(|(key, _)| (key, value.take().expect("unreachable")))
		}
	}
	/// Tries to emplace a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_emplace_with_mut<
		F: for<'a> FnOnce(&K, Pin<&'a mut MaybeUninit<W>>) -> Result<Pin<&'a mut V>, E>,
		E,
	>(
		self: Pin<&mut Self>,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&mut V>, (K, F)>, E> {
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			Pin::into_inner_unchecked(self)
				.try_emplace_with_mut(key, |key, slot| {
					value_factory.take().expect("unreachable")(key, Pin::new_unchecked(slot))
						.map(|value| Pin::into_inner_unchecked(value))
				})?
				.map(|value| Pin::new_unchecked(value))
				.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
				.pipe(Ok)
		}
	}

	/// Emplaces a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_with_mut<F: for<'a> FnOnce(&K, Pin<&'a mut MaybeUninit<W>>) -> Pin<&'a mut V>>(
		self: Pin<&mut Self>,
		key: K,
		value_factory: F,
	) -> Result<Pin<&mut V>, (K, F)> {
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			Pin::into_inner_unchecked(self)
				.emplace_with_mut(key, |key, slot| {
					value_factory.take().expect("unreachable")(key, Pin::new_unchecked(slot))
						.pipe(|value| Pin::into_inner_unchecked(value))
				})
				.map(|value| Pin::new_unchecked(value))
				.map_err(|(key, _)| (key, value_factory.take().expect("unreachable")))
		}
	}

	/// Emplaces a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_mut(self: Pin<&mut Self>, key: K, value: W) -> Result<Pin<&mut V>, (K, W)>
	where
		W: BorrowMut<V>,
	{
		let value = Cell::new(Some(value));
		unsafe {
			Pin::into_inner_unchecked(self)
				.emplace_with_mut(key, |_, slot| {
					slot.write(value.take().expect("unreachable")).borrow_mut()
				})
				.map(|value| Pin::new_unchecked(value))
				.map_err(|(key, _)| (key, value.take().expect("unreachable")))
		}
	}
}
