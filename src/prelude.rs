//! The shared bulk of the API.

use crate::UnwrapInfallible;
use std::{
	borrow::{Borrow, BorrowMut},
	cell::Cell,
	mem::{ManuallyDrop, MaybeUninit},
	pin::Pin,
};
use tap::Pipe;

/// Defines the API for trees that haven't been pinned (yet).
pub trait UnpinnedPineMap<K: Ord, V: ?Sized> {
	/// Pins this tree.
	///
	/// # Safety Notes
	///
	/// Well. Technically this is fully safe on stable Rust `1.55.0`.
	///
	/// However, this is *slightly* misusing the standard [`Pin`] type here,
	/// and the `#[repr(transparent)]` on there might not be entirely stabilised.
	///
	/// > For now this is behind the `"unstable"` feature.
	/// >
	/// > I'll talk to more people and try to stabilise it.
	///
	/// ```rust
	/// use pinus::sync::{PineMap, PressedPineMap};
	/// use static_assertions::{assert_eq_align, assert_eq_size};
	/// use std::pin::Pin;
	///
	/// assert_eq_align!(PineMap<(), ()>, Pin<PineMap<(), ()>>);
	/// assert_eq_size!(PineMap<(), ()>, Pin<PineMap<(), ()>>);
	///
	/// assert_eq_align!(PressedPineMap<(), ()>, Pin<PressedPineMap<(), ()>>);
	/// assert_eq_size!(PressedPineMap<(), ()>, Pin<PressedPineMap<(), ()>>);
	/// ```
	#[cfg(feature = "unstable")]
	fn pin(self) -> Pin<Self>
	where
		Self: Sized,
	{
		unsafe {
			//SAFETY:
			//
			// Well. This is *slightly* misusing the `Pin` type.
			// It's marked `#repr[transparent]` in the standard library docs, but whether that's really truly stable…
			//
			// I'll try to break `tests/layout.rs` if it's ever not the case.
			(ManuallyDrop::new(self).borrow_mut() as *mut ManuallyDrop<Self>)
				.cast::<Pin<Self>>()
				.read()
		}
	}

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

/// Defines the emplacement API for trees that haven't been pinned (yet).
pub trait UnpinnedPineMapEmplace<K: Ord, V: ?Sized, W>: UnpinnedPineMap<K, V> {
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

/// Defines the pin-projecting API.
///
/// # Safety
///
/// Any implementors must ensure their [`<Self::Unpinned as UnpinnedPineMap>`](`UnpinnedPineMap`) implementation would be valid if all `V`alues were pinned.
///
/// > If you MUST implement this yourself, pin this package to a specific minor version!
/// > New methods with default implementations may be added in any feature update.
///
/// It must be possible reinterpret `Self` as `Self::Unpinned`.
///
/// See: [`pin` -> `Drop` guarantee](https://doc.rust-lang.org/std/pin/index.html#drop-guarantee)
pub unsafe trait PinnedPineMap<K: Ord, V: ?Sized> {
	/// The unpinned identity of this tree.
	///
	/// As `Self` should be `Pin<Self::Unpinned>`, this should be the matching plain collection type.
	type Unpinned: UnpinnedPineMap<K, V>;

	/// Unpins this collections.
	///
	/// This is safe as [`V: Unpin`](`Unpin`) is required.
	fn unpin(self) -> Self::Unpinned
	where
		Self: Sized,
		V: Unpin,
	{
		unsafe { self.unpin_unchecked() }
	}

	/// Unpins this collection.
	///
	/// # Safety
	///
	/// Pinning invariants for any remaining values `V` must still be upheld.
	///
	/// If [`V: Unpin`](`Unpin`), use [`.unpin()`](`PinnedPineMap::unpin`) instead.
	unsafe fn unpin_unchecked(self) -> Self::Unpinned
	where
		Self: Sized,
	{
		(ManuallyDrop::new(self).borrow_mut() as *mut ManuallyDrop<Self>)
			.cast::<Self::Unpinned>()
			.read()
	}

	/// Access the unpinned API.
	fn as_unpinned(&self) -> &Self::Unpinned {
		unsafe { &*(self as *const Self).cast() }
	}

	/// Access the unpinned mutable API.
	///
	/// # Safety
	///
	/// Pinning invariants for any remaining values `V` must still be upheld.
	unsafe fn as_unpinned_mut(&mut self) -> &mut Self::Unpinned {
		&mut *(self as *mut Self).cast()
	}

	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get<Q>(&self, key: &Q) -> Option<Pin<&V>>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		self.as_unpinned()
			.get(key)
			.map(|value| unsafe { Pin::new_unchecked(&*(value as *const _)) })
	}

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
	) -> Result<Result<Pin<&V>, (K, F)>, E>
	where
		V: Sized,
	{
		self.as_unpinned()
			.try_insert_with(key, value_factory)?
			.map(|value| unsafe { Pin::new_unchecked(&*(value as *const _)) })
			.pipe(Ok)
	}

	/// Inserts a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_with<F: FnOnce(&K) -> V>(&self, key: K, value_factory: F) -> Result<Pin<&V>, (K, F)>
	where
		V: Sized, // Just for clarity.
	{
		self.as_unpinned()
			.insert_with(key, value_factory)
			.map(|value| unsafe { Pin::new_unchecked(&*(value as *const _)) })
	}

	/// Inserts a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert(&self, key: K, value: V) -> Result<Pin<&V>, (K, V)>
	where
		V: Sized,
	{
		self.as_unpinned()
			.insert(key, value)
			.map(|value| unsafe { Pin::new_unchecked(&*(value as *const _)) })
	}

	/// Clears the map, removing all elements.
	///
	/// # Panics
	///
	/// Iff the instance was poisoned.
	fn clear(&mut self) {
		unsafe { self.as_unpinned_mut() }.clear()
	}

	/// Returns a reference to the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but the ordering on the borrowed form *must* match the ordering on the key type.
	fn get_mut_pinned<'a, Q>(&'a mut self, key: &Q) -> Option<Pin<&'a mut V>>
	where
		Self::Unpinned: 'a,
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe {
			self.as_unpinned_mut()
				.get_mut(key)
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
	fn try_insert_with_mut<'a, F: FnOnce(&K) -> Result<V, E>, E>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&'a mut V>, (K, F)>, E>
	where
		V: Sized,
	{
		unsafe {
			self.as_unpinned_mut()
				.try_insert_with_mut(key, value_factory)?
				.map(|value| Pin::new_unchecked(&mut *(value as *mut _)))
		}
		.pipe(Ok)
	}

	/// Inserts a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_with_mut<'a, F: FnOnce(&K) -> V>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Pin<&'a mut V>, (K, F)>
	where
		V: Sized, // Just for clarity.
	{
		unsafe {
			self.as_unpinned_mut()
				.insert_with_mut(key, value_factory)
				.map(|value| Pin::new_unchecked(&mut *(value as *mut _)))
		}
	}

	/// Inserts a new value, but only if no such key exists yet.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn insert_mut<'a>(&mut self, key: K, value: V) -> Result<Pin<&'a mut V>, (K, V)>
	where
		V: Sized,
	{
		unsafe {
			self.as_unpinned_mut()
				.insert_mut(key, value)
				.map(|value| Pin::new_unchecked(&mut *(value as *mut _)))
		}
	}

	/// Removes and returns a key if a matching key exists.
	///
	/// The collection isn't poisoned if this causes a panic.
	fn remove_key<Q>(&mut self, key: &Q) -> Option<K>
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe { self.as_unpinned_mut() }.remove_key(key)
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
	fn drop_entry<Q>(&mut self, key: &Q) -> bool
	where
		K: Borrow<Q>,
		Q: Ord + ?Sized,
	{
		unsafe { self.as_unpinned_mut() }.drop_entry(key)
	}
}

/// Defines the pin-projecting emplacement API.
///
/// # Safety
///
/// Any implementors must ensure their [`UnpinnedPineMapEmplace`] implementation would be valid if all `V`alues were pinned.
///
/// > If you MUST implement this yourself, pin this package to a specific minor version!
/// > New methods with default implementations may be added in any feature update.
///
/// See: [`pin` -> `Drop` guarantee](https://doc.rust-lang.org/std/pin/index.html#drop-guarantee)
pub unsafe trait PinnedPineMapEmplace<K: Ord, V: ?Sized, W>: PinnedPineMap<K, V>
where
	Self::Unpinned: UnpinnedPineMapEmplace<K, V, W>,
{
	/// Tries to emplace a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Safety
	///
	#[allow(clippy::doc_markdown)] // No backticks needed in `<code>`.
	/// Note that the <code>[Pin]<&'a mut [MaybeUninit]<W>></code> doesn't imply a drop guarantee for `W`,
	/// but only that that entire allocation will remain untouched for `'a`.
	///
	/// Any resulting [`Pin<&'a V>`](`Pin`) or [`Pin<&'a mut V>`](`Pin`) will have
	/// its [`Deref::Target`](`std::ops::Deref::Target`) dropped in place (or leaked), however, as implied.
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
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&V>, (K, F)>, E> {
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			self.as_unpinned()
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
	/// # Safety
	///
	#[allow(clippy::doc_markdown)] // No backticks needed in `<code>`.
	/// Note that the <code>[Pin]<&'a mut [MaybeUninit]<W>></code> doesn't imply a drop guarantee for `W`,
	/// but only that that entire allocation will remain untouched for `'a`.
	///
	/// Any resulting [`Pin<&'a V>`](`Pin`) or [`Pin<&'a mut V>`](`Pin`) will have
	/// its [`Deref::Target`](`std::ops::Deref::Target`) dropped in place (or leaked), however, as implied.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_with<F: for<'a> FnOnce(&K, Pin<&'a mut MaybeUninit<W>>) -> Pin<&'a mut V>>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Pin<&V>, (K, F)> {
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			self.as_unpinned()
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
	/// # Safety
	///
	/// Note that there is no drop guarantee for `W`
	/// (and that type will in fact not have its [`Drop::drop`] called directly)!
	///
	/// Any resulting [`Pin<&'a V>`](`Pin`) or [`Pin<&'a mut V>`](`Pin`) will have
	/// its [`Deref::Target`](`std::ops::Deref::Target`) dropped in place (or leaked), however, as implied.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace(&self, key: K, value: W) -> Result<Pin<&V>, (K, W)>
	where
		W: BorrowMut<V>,
	{
		unsafe {
			self.as_unpinned()
				.emplace(key, value)
				.map(|value| Pin::new_unchecked(&*(value as *const _)))
		}
	}
	/// Tries to emplace a new value produced by the given factory, but only if no such key exists yet.
	///
	/// # Safety
	///
	#[allow(clippy::doc_markdown)] // No backticks needed in `<code>`.
	/// Note that the <code>[Pin]<&'a mut [MaybeUninit]<W>></code> doesn't imply a drop guarantee for `W`,
	/// but only that that entire allocation will remain untouched for `'a`.
	///
	/// Any resulting [`Pin<&'a V>`](`Pin`) or [`Pin<&'a mut V>`](`Pin`) will have
	/// its [`Deref::Target`](`std::ops::Deref::Target`) dropped in place (or leaked), however, as implied.
	///
	/// # Errors
	///
	/// Outer error: Iff `value_factory` fails.
	///
	/// Inner error: Iff an entry matching `key` already exists.
	fn try_emplace_with_mut<
		'a,
		F: for<'b> FnOnce(&K, Pin<&'b mut MaybeUninit<W>>) -> Result<Pin<&'b mut V>, E>,
		E,
	>(
		&'a mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<Pin<&'a mut V>, (K, F)>, E>
	where
		Self::Unpinned: 'a,
	{
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			self.as_unpinned_mut()
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
	/// # Safety
	///
	#[allow(clippy::doc_markdown)] // No backticks needed in `<code>`.
	/// Note that the <code>[Pin]<&'a mut [MaybeUninit]<W>></code> doesn't imply a drop guarantee for `W`,
	/// but only that that entire allocation will remain untouched for `'a`.
	///
	/// Any resulting [`Pin<&'a V>`](`Pin`) or [`Pin<&'a mut V>`](`Pin`) will have
	/// its [`Deref::Target`](`std::ops::Deref::Target`) dropped in place (or leaked), however, as implied.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_with_mut<'a, F: for<'b> FnOnce(&K, Pin<&'b mut MaybeUninit<W>>) -> Pin<&'b mut V>>(
		&'a mut self,
		key: K,
		value_factory: F,
	) -> Result<Pin<&'a mut V>, (K, F)>
	where
		Self::Unpinned: 'a,
	{
		let value_factory = Cell::new(Some(value_factory));
		unsafe {
			self.as_unpinned_mut()
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
	/// # Safety
	///
	/// Note that there is no drop guarantee for `W`
	/// (and that type will in fact not have its [`Drop::drop`] called directly)!
	///
	/// Any resulting [`Pin<&'a V>`](`Pin`) or [`Pin<&'a mut V>`](`Pin`) will have
	/// its [`Deref::Target`](`std::ops::Deref::Target`) dropped in place (or leaked), however, as implied.
	///
	/// # Errors
	///
	/// Iff an entry matching `key` already exists.
	fn emplace_mut<'a>(&'a mut self, key: K, value: W) -> Result<Pin<&'a mut V>, (K, W)>
	where
		Self::Unpinned: 'a,
		W: BorrowMut<V>,
	{
		let value = Cell::new(Some(value));
		unsafe {
			self.as_unpinned_mut()
				.emplace_with_mut(key, |_, slot| {
					slot.write(value.take().expect("unreachable")).borrow_mut()
				})
				.map(|value| Pin::new_unchecked(value))
				.map_err(|(key, _)| (key, value.take().expect("unreachable")))
		}
	}
}
