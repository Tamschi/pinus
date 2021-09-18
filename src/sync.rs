//! Thread-safe b-trees.

use crate::prelude::{
	PinnedPineMap, PinnedPineMapEmplace, UnpinnedPineMap, UnpinnedPineMapEmplace,
};
use bumpalo::Bump;
use parking_lot::RwLock;
use std::{
	cell::Cell,
	collections::BTreeMap,
	mem::{self, MaybeUninit},
	panic::{self, catch_unwind, AssertUnwindSafe},
	pin::Pin,
};
use tap::{Pipe, TapFallible};

/// A homogeneous [`BTreeMap`] that allows pin-projection to its values and additions through shared references, reusing memory.
///
/// See also [`PressedPineMap`] to store trait objects efficiently.
///
/// See [`UnpinnedPineMap`], [`UnpinnedPineMapEmplace`], [`PinnedPineMap`] and [`PinnedPineMapEmplace`] for the full API.
///
/// # Usage / Example
///
/// ```rust
/// use pinus::{prelude::*, sync::PineMap};
/// use std::{convert::Infallible, pin::Pin};
///
/// // `PineMap` is interior-mutable, so either is useful:
/// let map = PineMap::new();
/// let mut mut_map = PineMap::new();
///
///
/// // Get parallel shared references by inserting like this:
/// let a: &String = map.insert("Hello!", "Hello!".to_string())
///   .unwrap(/* Your data back if the entry already existed. */);
/// let b: &String = map.insert_with("Hello again!", |k| k.to_string())
///   .map_err(|(key, _factory)| key).unwrap();
/// let c: &String = map.try_insert_with::<_, Infallible>("Hello once again!", |k| Ok(k.to_string()))
///   .unwrap(/* Error from factory. */)
///   .map_err(|(key, _factory)| key).unwrap();
///
/// let a2: &String = map.get("Hello!").unwrap();
///
/// let _ = (a, a2, b, c);
///
///
/// // Get exclusive references like this (also with or without factory):
/// let mut_a: &mut String = mut_map.insert_with_mut("Hi!", |k| k.to_string())
///   .map_err(|(key, _factory)| key).unwrap();
///
/// let mut_a2: &mut String = mut_map.get_mut("Hi!").unwrap();
///
/// // The `…_mut` methods are actually faster, but their results can't be held onto at once:
/// // let _ = (mut_a, mut_a2); // "error[E0499]: cannot borrow `mut_map` as mutable more than once at a time"
///
///
/// // Remove entries like this:
/// mut_map.clear();
/// let _: Option<(&str, String)> = mut_map.remove_pair("A");
/// let _: Option<String> = mut_map.remove_value("B");
/// let _: Option<&str> = mut_map.remove_key("C");
/// let _: bool = mut_map.drop_entry("D");
///
///
/// /////
///
///
/// // Now on to part 2, pinning:
/// let mut map: Pin<_> = map.pin();
/// let mut mut_map: Pin<_> = mut_map.pin();
///
///
/// // Shared references to values are now pinned:
/// let a: Pin<&String> = map.insert("Hello!!", "Hello!!".to_string())
///   .unwrap();
/// let b: Pin<&String> = map.insert_with("Hello again!!", |k| k.to_string())
///   .ok().unwrap();
/// let c: Pin<&String> = map.try_insert_with::<_, Infallible>("Hello once again!!", |k| Ok(k.to_string()))
///   .unwrap().ok().unwrap();
///
/// let a2: Pin<&String> = map.get("Hello!").unwrap();
///
/// let _ = (a, a2, b, c);
///
///
/// // Exclusive references to values are also pinned:
/// let mut mut_a: Pin<&mut String> = mut_map.insert_with_mut("Hi!", |k| k.to_string())
///   .map_err(|(key, _factory)| key).unwrap();
///
/// let mut mut_a2: Pin<&mut String> = mut_map.get_mut("Hi!").unwrap();
///
/// // The `…_mut` methods are actually faster, but their results can't be held onto at once:
/// // let _ = (mut_a, mut_a2); // "error[E0499]: cannot borrow `mut_map` as mutable more than once at a time"
///
/// // Only keys can be removed now, but values must be dropped in place:
/// mut_map.clear();
/// let _: Option<&str> = mut_map.remove_key("C");
/// let _: bool = mut_map.drop_entry("D");
/// ```
pub struct PineMap<K: Ord, V> {
	contents: RwLock<Cambium<K, V>>,
}

/// A heterogeneous [`BTreeMap`] that allows pin-projection to its values and additions through shared references, rarely reusing memory.
///
/// Unlike [`PineMap`], this one can store trait objects fairly efficiently.
/// As a tradeoff, memory used to store values is not reused until the collection is dropped or cleared.
///
/// See [`UnpinnedPineMap`], [`UnpinnedPineMapEmplace`], [`PinnedPineMap`] and [`PinnedPineMapEmplace`] for the full API.
///
/// # Example
///
/// ```rust
/// use pinus::{prelude::*, sync::PressedPineMap};
/// use std::{
///   any::Any,
///   borrow::{Borrow, BorrowMut},
///   convert::Infallible,
///   pin::Pin,
/// };
///
/// let map = PressedPineMap::<_, dyn Any>::new();
///
/// // `dyn Any` is `!Sized`,
/// // so it's necessary to use the loosely-typed emplacement methods:
/// let _: &dyn Any = map
///   .emplace_with(1, |_key, slot| slot.write(()))
///   .ok(/* or key and factory */).unwrap();
/// let _: &dyn Any = map
///   .try_emplace_with::<_, Infallible>(2, |_key, slot| Ok(slot.write(())))
///   .unwrap(/* or factory error */)
///   .ok(/* or key and factory */).unwrap();
///
/// // There's also a by-value method,
/// // but it has slightly steeper requirements:
/// #[derive(Debug)]
/// struct MyAny;
/// impl std::borrow::Borrow<dyn Any> for MyAny { //…
/// #   fn borrow(&self) -> &dyn Any { self }
/// # }
/// impl std::borrow::BorrowMut<dyn Any> for MyAny { //…
/// #   fn borrow_mut(&mut self) -> &mut dyn Any { self }
/// # }
///
/// let _: &dyn Any = map
///   .emplace(3, MyAny)
///   .unwrap(/* or key and value */);
///
/// // As usual the map's values can be pinned:
/// let map: Pin<PressedPineMap<_, _>> = map.pin();
///
/// // And then further value references are pinned:
/// let _: Pin<&dyn Any> = map.emplace(4, MyAny).unwrap();
///
/// // To immediately get an unpinned reference, just use `.as_unpinned()`:
/// let _: &dyn Any = map.as_unpinned().emplace(5, MyAny).unwrap();
/// ```
pub struct PressedPineMap<K: Ord, V: ?Sized> {
	contents: RwLock<PressedCambium<K, V>>,
}

struct Cambium<K, V> {
	addresses: BTreeMap<K, *mut V>,
	memory: Bump,
	holes: Vec<*mut MaybeUninit<V>>,
}

struct PressedCambium<K, V: ?Sized> {
	addresses: BTreeMap<K, *mut V>,
	memory: Bump,
	// We can't determine (cross-architecture) if we actually own the value pointers,
	// because pointer comparisons not from within the same allocation aren't meaningful,
	// so we can't derive holes on value removal.
	//
	// We could keep track of all the allocations in addition to the value address,
	// but the intended use-case of this particular collection won't see many removals in the first place,
	// short of clearing or dropping the instance entirely.
}

impl<K: Ord, V> PineMap<K, V> {
	/// Creates a new empty [`PineMap`].
	#[must_use]
	pub fn new() -> Self {
		Self {
			contents: RwLock::new(Cambium {
				addresses: BTreeMap::new(),
				memory: Bump::new(),
				holes: Vec::new(),
			}),
		}
	}

	/// Creates a new empty [`PineMap`] that will store values contiguously
	/// until `capacity` (in concurrently live entries) is exceeded.
	#[must_use]
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			contents: RwLock::new(Cambium {
				addresses: BTreeMap::new(),
				memory: Bump::with_capacity(mem::size_of::<V>() * capacity),
				holes: Vec::new(),
			}),
		}
	}
}

impl<K: Ord, V: ?Sized> PressedPineMap<K, V> {
	/// Creates a new empty [`PressedPineMap`].
	#[must_use]
	pub fn new() -> Self {
		Self {
			contents: RwLock::new(PressedCambium {
				addresses: BTreeMap::new(),
				memory: Bump::new(),
			}),
		}
	}

	/// Creates a new empty [`PressedPineMap`] that will store values (almost) contiguously
	/// until `capacity` (in bytes that are the size of a maximally aligned buffer!) are exceeded.
	#[must_use]
	pub fn with_capacity(capacity_bytes: usize) -> Self {
		Self {
			contents: RwLock::new(PressedCambium {
				addresses: BTreeMap::new(),
				memory: Bump::with_capacity(capacity_bytes),
			}),
		}
	}
}

impl<K: Ord, V> Default for PineMap<K, V> {
	fn default() -> Self {
		Self::new()
	}
}

impl<K: Ord, V: ?Sized> Default for PressedPineMap<K, V> {
	fn default() -> Self {
		Self::new()
	}
}

impl<K: Ord, V> UnpinnedPineMap<K, V> for PineMap<K, V> {
	fn get<Q>(&self, key: &Q) -> Option<&V>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.read(/* poisoned */);
		contents.addresses.get(key).map(|value| unsafe { &**value })
	}

	fn try_insert_with<F: FnOnce(&K) -> Result<V, E>, E>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E> {
		let value_factory = Cell::new(Some(value_factory));
		self.try_emplace_with(key, |key, slot| {
			slot.write(value_factory.take().expect("unreachable")(key)?)
				.pipe(Ok)
		})
		.map(|inner| inner.map_err(|(key, _)| (key, value_factory.take().expect("unreachable"))))
	}

	/// Drops all keys and all values in this collection, even if some of them panic while being done so.
	///
	/// The drop order is unspecified and may change at any point (even between compilations or runs).
	///
	/// # Panics
	///
	/// Iff more than one panic happens,
	/// they are resumed collected inside a [`Vec<Box<dyn Any + Send>>`],
	/// unless that vector (re)allocation itself fails, in which case that's not caught at all.
	///
	/// > That's probably not the ideal way to handle this. I'm taking suggestions.
	fn clear(&mut self) {
		let contents = self.contents.get_mut(/* poisoned */);

		contents.holes.clear();

		let success = if mem::needs_drop::<V>() {
			catch_unwind(AssertUnwindSafe(|| {
				drop_all_pinned(mem::take(&mut contents.addresses))
			}))
		} else {
			contents.addresses.clear();
			Ok(())
		};

		contents.memory.reset();

		success.unwrap_or_else(|panic| panic::resume_unwind(panic));
	}

	fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		contents
			.addresses
			.get(key)
			.map(|value| unsafe { &mut **value })
	}

	fn try_insert_with_mut<F: FnOnce(&K) -> Result<V, E>, E>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E> {
		let value_factory = Cell::new(Some(value_factory));
		self.try_emplace_with_mut(key, |key, slot| {
			slot.write(value_factory.take().expect("unreachable")(key)?)
				.pipe(Ok)
		})
		.map(|inner| inner.map_err(|(key, _)| (key, value_factory.take().expect("unreachable"))))
	}

	fn remove_pair<Q>(&mut self, key: &Q) -> Option<(K, V)>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		let (key, value) = contents.addresses.remove_entry(key)?;
		contents.holes.push(value.cast());
		Some((key, unsafe { value.read() }))
	}

	fn remove_key<Q>(&mut self, key: &Q) -> Option<K>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		let (key, value) = contents.addresses.remove_entry(key)?;
		contents.holes.push(value.cast());
		unsafe { value.drop_in_place() };
		Some(key)
	}
}

impl<K: Ord, V: ?Sized> UnpinnedPineMap<K, V> for PressedPineMap<K, V> {
	fn get<Q>(&self, key: &Q) -> Option<&V>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.read(/* poisoned */);
		contents.addresses.get(key).map(|value| unsafe { &**value })
	}

	fn try_insert_with<F: FnOnce(&K) -> Result<V, E>, E>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E>
	where
		V: Sized,
	{
		let value_factory = Cell::new(Some(value_factory));
		self.try_emplace_with(key, |key, slot| {
			slot.write(value_factory.take().expect("unreachable")(key)?)
				.pipe(Ok)
		})
		.map(|inner| inner.map_err(|(key, _)| (key, value_factory.take().expect("unreachable"))))
	}

	/// Drops all keys and all values in this collection, even if some of them panic while being done so.
	///
	/// The drop order is unspecified and may change at any point (even between compilations or runs).
	///
	/// # Panics
	///
	/// Iff more than one panic happens,
	/// they are resumed collected inside a [`Vec<Box<dyn Any + Send>>`],
	/// unless that vector (re)allocation itself fails, in which case that's not caught at all.
	///
	/// > That's probably not the ideal way to handle this. I'm taking suggestions.
	fn clear(&mut self) {
		let contents = self.contents.get_mut(/* poisoned */);

		let success = catch_unwind(AssertUnwindSafe(|| {
			drop_all_pinned(mem::take(&mut contents.addresses))
		}));

		contents.memory.reset();

		success.unwrap_or_else(|panic| panic::resume_unwind(panic));
	}

	fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		contents
			.addresses
			.get(key)
			.map(|value| unsafe { &mut **value })
	}

	fn try_insert_with_mut<F: FnOnce(&K) -> Result<V, E>, E>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E>
	where
		V: Sized,
	{
		let value_factory = Cell::new(Some(value_factory));
		self.try_emplace_with_mut(key, |key, slot| {
			slot.write(value_factory.take().expect("unreachable")(key)?)
				.pipe(Ok)
		})
		.map(|inner| inner.map_err(|(key, _)| (key, value_factory.take().expect("unreachable"))))
	}

	fn remove_pair<Q>(&mut self, key: &Q) -> Option<(K, V)>
	where
		V: Sized,
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		let (key, value) = contents.addresses.remove_entry(key)?;
		Some((key, unsafe { value.read() }))
	}

	fn remove_key<Q>(&mut self, key: &Q) -> Option<K>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		let (key, value) = contents.addresses.remove_entry(key)?;
		unsafe { value.drop_in_place() };
		Some(key)
	}
}

/// > An implementation detail, but perhaps interesting:
/// >
/// > The site that becomes managed by this instance (until [`.clear`](`UnpinnedPineMap::clear`) is called)
/// > is the `&'a mut V` returned from a given `value_factory` and not (necessarily) the initial [`&'a mut MaybeUninit<V>`](`MaybeUninit`).
/// >
/// > The latter is effectively leaked until the collection is cleared or dropped
/// > (but please don't rely on this, I don't guarantee this will stay the case in any way).
impl<K: Ord, V> UnpinnedPineMapEmplace<K, V, V> for PineMap<K, V> {
	fn try_emplace_with<
		F: for<'a> FnOnce(&K, &'a mut MaybeUninit<V>) -> Result<&'a mut V, E>,
		E,
	>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E> {
		let mut contents = self.contents.write(/* poisoned */);
		let Cambium {
			addresses,
			memory,
			holes,
		} = &mut *contents;
		#[allow(clippy::map_entry)]
		if addresses.contains_key(&key) {
			Err((key, value_factory))
		} else if let Some(hole) = holes.pop() {
			let slot = unsafe { &mut *hole };
			let value = value_factory(&key, slot).tap_err(|_| holes.push(hole))?;
			addresses.insert(key, value as *mut _);
			Ok(value)
		} else {
			let value = value_factory(&key, memory.alloc(MaybeUninit::uninit()))?;
			addresses.insert(key, value as *mut _);
			Ok(value)
		}
		.map(|value| unsafe { &*(value as *const _) })
		.pipe(Ok)
	}

	fn try_emplace_with_mut<
		F: for<'a> FnOnce(&K, &'a mut MaybeUninit<V>) -> Result<&'a mut V, E>,
		E,
	>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E> {
		let Cambium {
			addresses,
			memory,
			holes,
		} = self.contents.get_mut();
		#[allow(clippy::map_entry)]
		if addresses.contains_key(&key) {
			Err((key, value_factory))
		} else if let Some(hole) = holes.pop() {
			let slot = unsafe { &mut *hole };
			let value = value_factory(&key, slot).tap_err(|_| holes.push(hole))?;
			addresses.insert(key, value as *mut _);
			Ok(value)
		} else {
			let value = value_factory(&key, memory.alloc(MaybeUninit::uninit()))?;
			addresses.insert(key, value as *mut _);
			Ok(value)
		}
		.map(|value| unsafe { &mut *(value as *mut _) })
		.pipe(Ok)
	}
}

impl<K: Ord, V: ?Sized, W> UnpinnedPineMapEmplace<K, V, W> for PressedPineMap<K, V> {
	fn try_emplace_with<
		F: for<'a> FnOnce(&K, &'a mut MaybeUninit<W>) -> Result<&'a mut V, E>,
		E,
	>(
		&self,
		key: K,
		value_factory: F,
	) -> Result<Result<&V, (K, F)>, E> {
		let mut contents = self.contents.write(/* poisoned */);
		let PressedCambium { addresses, memory } = &mut *contents;
		#[allow(clippy::map_entry)]
		if addresses.contains_key(&key) {
			Err((key, value_factory))
		} else {
			let value = value_factory(&key, memory.alloc(MaybeUninit::uninit()))?;
			addresses.insert(key, value as *mut _);
			Ok(unsafe { &*(value as *const _) })
		}
		.pipe(Ok)
	}

	fn try_emplace_with_mut<
		F: for<'a> FnOnce(&K, &'a mut MaybeUninit<W>) -> Result<&'a mut V, E>,
		E,
	>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E> {
		let PressedCambium { addresses, memory } = self.contents.get_mut(/* poisoned */);
		#[allow(clippy::map_entry)]
		if addresses.contains_key(&key) {
			Err((key, value_factory))
		} else {
			let value = value_factory(&key, memory.alloc(MaybeUninit::uninit()))?;
			addresses.insert(key, value as *mut _);
			Ok(unsafe { &mut *(value as *mut _) })
		}
		.pipe(Ok)
	}
}

unsafe impl<K: Ord, V> PinnedPineMap<K, V> for Pin<PineMap<K, V>> {
	type Unpinned = PineMap<K, V>;
}
unsafe impl<K: Ord, V: ?Sized> PinnedPineMap<K, V> for Pin<PressedPineMap<K, V>> {
	type Unpinned = PressedPineMap<K, V>;
}

unsafe impl<K: Ord, V> PinnedPineMapEmplace<K, V, V> for Pin<PineMap<K, V>> {}
unsafe impl<K: Ord, V: ?Sized, W> PinnedPineMapEmplace<K, V, W> for Pin<PressedPineMap<K, V>> {}

unsafe impl<K: Ord, V> Send for PineMap<K, V>
where
	K: Send,
	V: Send,
{
}
unsafe impl<K: Ord, V: ?Sized> Send for PressedPineMap<K, V>
where
	K: Send,
	V: Send,
{
}

unsafe impl<K: Ord, V> Sync for PineMap<K, V>
where
	K: Sync + Send,
	V: Sync + Send,
{
}
unsafe impl<K: Ord, V: ?Sized> Sync for PressedPineMap<K, V>
where
	K: Sync + Send,
	V: Sync + Send,
{
}

/// Drops all keys and all values in this collection, even if some of them panic while being done so.
///
/// The drop order is unspecified and may change at any point (even between compilations or runs).
///
/// # Panics
///
/// Iff more than one panic happens,
/// they are resumed collected inside a [`Vec<Box<dyn Any + Send>>`],
/// unless that vector (re)allocation itself fails, in which case that's not caught at all.
///
/// > That's probably not the ideal way to handle this. I'm taking suggestions.
impl<K: Ord, V> Drop for PineMap<K, V> {
	fn drop(&mut self) {
		// None of the data will be used in the future,
		// so explicit cleanup can be a bit more concise (and hopefully a little faster) than calling `.clean()`.

		if !mem::needs_drop::<V>() {
			return;
		}

		let contents = self.contents.get_mut(/* poisoned */);

		drop_all_pinned(mem::take(&mut contents.addresses));
	}
}

/// Drops all keys and all values in this collection, even if some of them panic while being done so.
///
/// The drop order is unspecified and may change at any point (even between compilations or runs).
///
/// # Panics
///
/// Iff more than one panic happens,
/// they are resumed collected inside a [`Vec<Box<dyn Any + Send>>`],
/// unless that vector (re)allocation itself fails, in which case that's not caught at all.
///
/// > That's probably not the ideal way to handle this. I'm taking suggestions.
impl<K: Ord, V: ?Sized> Drop for PressedPineMap<K, V> {
	fn drop(&mut self) {
		// None of the data will be used in the future,
		// so explicit cleanup can be a bit more concise (and hopefully a little faster) than calling `.clean()`.

		let contents = self.contents.get_mut(/* poisoned */);

		drop_all_pinned(mem::take(&mut contents.addresses));
	}
}

fn drop_all_pinned<K, V: ?Sized>(addresses: BTreeMap<K, *mut V>) {
	let mut panics = vec![];

	// WAITING ON: <https://github.com/rust-lang/rust/issues/70530> (`BTreeMap::drain_filter`)
	for (key, value) in addresses {
		catch_unwind(AssertUnwindSafe(|| drop(key))).unwrap_or_else(|panic| panics.push(panic));
		catch_unwind(AssertUnwindSafe(|| unsafe { value.drop_in_place() }))
			.unwrap_or_else(|panic| panics.push(panic));
	}
	match panics.len() {
		0 => (),
		1 => panic::resume_unwind(panics.into_iter().next().expect("unreachable")),
		_ => panic::resume_unwind(Box::new(panics)),
	}
}
