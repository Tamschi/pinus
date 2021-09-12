//! Thread-safe b-trees.

use crate::prelude::{
	PinnedPineMap, PinnedPineMapEmplace, UnpinnedPineMap, UnpinnedPineMapEmplace,
};
use bumpalo::Bump;
use parking_lot::RwLock;
use std::{
	cell::{Cell, UnsafeCell},
	collections::BTreeMap,
	marker::PhantomPinned,
	mem::{self, MaybeUninit},
	num::NonZeroUsize,
};
use tap::{Pipe, TapFallible};
use vec1::Vec1;

/// A [`BTreeMap`] that allows pin-projection to its values and additions through shared references.
///
/// See also [`PressedPineMap`] to store trait objects efficiently.
///
/// See [`UnpinnedPineMap`], [`UnpinnedPineMapEmplace`], [`PinnedPineMap`] and [`PinnedPineMapEmplace`] for the full API.
pub struct PineMap<K: Ord, V> {
	contents: RwLock<Cambium<K, V>>,
	_pin: PhantomPinned,
}

/// A [`BTreeMap`] that allows pin-projection to its values and additions through shared references.
///
/// Unlike [`PineMap`], this one can store heterogeneous trait objects fairly efficiently.
/// As a tradeoff, memory used to store values is not reused until the collection is dropped or cleared.
///
/// See [`UnpinnedPineMap`], [`UnpinnedPineMapEmplace`], [`PinnedPineMap`] and [`PinnedPineMapEmplace`] for the full API.
pub struct PressedPineMap<K: Ord, V: ?Sized> {
	contents: RwLock<PressedCambium<K, V>>,
	_pin: PhantomPinned,
}

struct Cambium<K, V> {
	slot_map: BTreeMap<K, (usize, usize)>,
	// In practice this is just `ManuallyDrop` at rest, but I can't reborrow the `Vec` appropriately.
	values: Vec1<Vec<UnsafeCell<MaybeUninit<V>>>>,
	holes: Vec<(usize, usize)>,
}

struct PressedCambium<K, V: ?Sized> {
	addresses: BTreeMap<K, *mut V>,
	values: Bump,
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
				slot_map: BTreeMap::new(),
				values: Vec1::new({
					let mut vec = Vec::new();
					vec.reserve(1);
					vec
				}),
				holes: Vec::new(),
			}),
			_pin: PhantomPinned,
		}
	}

	/// Creates a new empty [`PineMap`] that will store values contiguously
	/// until `capacity` (in concurrently live entries) is exceeded.
	#[must_use]
	pub fn with_capacity(capacity: NonZeroUsize) -> Self {
		Self {
			contents: RwLock::new(Cambium {
				slot_map: BTreeMap::new(),
				values: Vec1::new(Vec::with_capacity(capacity.get())),
				holes: Vec::new(),
			}),
			_pin: PhantomPinned,
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
				values: Bump::new(),
			}),
			_pin: PhantomPinned,
		}
	}

	/// Creates a new empty [`PressedPineMap`] that will store values (almost) contiguously
	/// until `capacity` (in bytes that are the size of a maximally aligned buffer!) are exceeded.
	#[must_use]
	pub fn with_capacity(capacity_bytes: usize) -> Self {
		Self {
			contents: RwLock::new(PressedCambium {
				addresses: BTreeMap::new(),
				values: Bump::with_capacity(capacity_bytes),
			}),
			_pin: PhantomPinned,
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
		let &(slab, i) = contents.slot_map.get(key)?;

		unsafe {
			contents
				.values
				.get_unchecked(slab)
				.get_unchecked(i)
				.get()
				.pipe(|value| (*value).assume_init_ref())
		}
		.pipe(Some)
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

	fn clear(&mut self) {
		let contents = self.contents.get_mut(/* poisoned */);
		contents.holes.clear();

		// WAITING ON: <https://github.com/rust-lang/rust/issues/70530> (`BTreeMap::drain_filter`)
		if mem::needs_drop::<V>() {
			for (_, (slab, i)) in mem::take(&mut contents.slot_map) {
				unsafe {
					contents
						.values
						.get_unchecked_mut(slab)
						.get_unchecked(i)
						.get()
						.pipe(|value| (*value).as_mut_ptr().drop_in_place())
				}
			}
		} else {
			contents.slot_map.clear()
		}

		if contents.values.len() > 1 {
			contents.values.swap_remove(0).expect("unreachable");
			contents.values.truncate(1).expect("unreachable");
		}
	}

	fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		let &(slab, i) = contents.slot_map.get(key)?;

		unsafe {
			contents
				.values
				.get_unchecked(slab)
				.get_unchecked(i)
				.get()
				.pipe(|value| (*value).assume_init_mut())
		}
		.pipe(Some)
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
		let (key, (slab, i)) = contents.slot_map.remove_entry(key)?;
		contents.holes.push((slab, i));
		let value = unsafe {
			contents
				.values
				.get_unchecked(slab)
				.get_unchecked(i)
				.get()
				.pipe(|value| (*value).as_ptr().read())
		};
		Some((key, value))
	}

	fn remove_key<Q>(&mut self, key: &Q) -> Option<K>
	where
		K: std::borrow::Borrow<Q>,
		Q: Ord + ?Sized,
	{
		let contents = self.contents.get_mut(/* poisoned */);
		let (key, (slab, i)) = contents.slot_map.remove_entry(key)?;
		contents.holes.push((slab, i));
		if mem::needs_drop::<V>() {
			unsafe {
				contents
					.values
					.get_unchecked(slab)
					.get_unchecked(i)
					.get()
					.pipe(|value| (*value).as_mut_ptr().drop_in_place())
			}
		}
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

	fn clear(&mut self) {
		let contents = self.contents.get_mut(/* poisoned */);

		// WAITING ON: <https://github.com/rust-lang/rust/issues/70530> (`BTreeMap::drain_filter`)
		for (_, value) in mem::take(&mut contents.addresses) {
			unsafe { value.drop_in_place() }
		}

		contents.values.reset()
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
		contents.addresses.remove_entry(key)?.0.pipe(Some)
	}
}

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
			slot_map,
			values,
			holes,
		} = &mut *contents;
		#[allow(clippy::map_entry)]
		if slot_map.contains_key(&key) {
			Err((key, value_factory))
		} else if let Some(hole) = holes.pop() {
			let slot =
				unsafe { values.get_unchecked_mut(hole.0).get_unchecked_mut(hole.1) }.get_mut();
			let value = value_factory(&key, slot)?;
			slot_map.insert(key, hole);
			Ok(value)
		} else {
			let mut slab_i = values.len() - 1;
			let slab = values.last_mut();
			let slab = if slab.len() < Vec::capacity(slab) {
				slab
			} else {
				slab_i += 1;
				let target_capacity = slab.len() * 2;
				let mut slab = Vec::new();
				slab.reserve(target_capacity);
				values.push(slab);
				values.last_mut()
			};
			slab.push(UnsafeCell::new(MaybeUninit::uninit()));
			let value = value_factory(&key, unsafe {
				&mut *slab.last().expect("unreachable").get()
			})
			.tap_err(|_| drop(slab.pop()))?;
			let i = slab.len() - 1;
			slot_map.insert(key, (slab_i, i));
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
			slot_map,
			values,
			holes,
		} = self.contents.get_mut(/* poisoned */);
		#[allow(clippy::map_entry)]
		if slot_map.contains_key(&key) {
			Err((key, value_factory))
		} else if let Some(hole) = holes.pop() {
			let slot =
				unsafe { values.get_unchecked_mut(hole.0).get_unchecked_mut(hole.1) }.get_mut();
			let value = value_factory(&key, slot)?;
			slot_map.insert(key, hole);
			Ok(value)
		} else {
			let mut slab_i = values.len() - 1;
			let slab = values.last_mut();
			let slab = if slab.len() < Vec::capacity(slab) {
				slab
			} else {
				slab_i += 1;
				let target_capacity = slab.len() * 2;
				let mut slab = Vec::new();
				slab.reserve(target_capacity);
				values.push(slab);
				values.last_mut()
			};
			slab.push(UnsafeCell::new(MaybeUninit::uninit()));
			let value = value_factory(&key, unsafe {
				&mut *slab.last().expect("unreachable").get()
			})
			.tap_err(|_| drop(slab.pop()))?;
			let i = slab.len() - 1;
			slot_map.insert(key, (slab_i, i));
			Ok(value)
		}
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
		let PressedCambium { addresses, values } = &mut *contents;
		if addresses.contains_key(&key) {
			Err((key, value_factory))
		} else {
			let value = value_factory(&key, values.alloc(MaybeUninit::uninit()))?;
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
		let PressedCambium { addresses, values } = self.contents.get_mut(/* poisoned */);
		if addresses.contains_key(&key) {
			Err((key, value_factory))
		} else {
			let value = value_factory(&key, values.alloc(MaybeUninit::uninit()))?;
			Ok(unsafe { &mut *(value as *mut _) })
		}
		.pipe(Ok)
	}
}

unsafe impl<K: Ord, V> PinnedPineMap<K, V> for PineMap<K, V> {}
unsafe impl<K: Ord, V: ?Sized> PinnedPineMap<K, V> for PressedPineMap<K, V> {}

unsafe impl<K: Ord, V> PinnedPineMapEmplace<K, V, V> for PineMap<K, V> {}
unsafe impl<K: Ord, V: ?Sized, W> PinnedPineMapEmplace<K, V, W> for PressedPineMap<K, V> {}

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

impl<K: Ord, V> Unpin for PineMap<K, V> where V: Unpin {}
impl<K: Ord, V: ?Sized> Unpin for PressedPineMap<K, V> where V: Unpin {}

impl<K: Ord, V> Drop for PineMap<K, V> {
	fn drop(&mut self) {
		// None of the data will be used in the future,
		// so explicit cleanup can be a bit more concise (and hopefully a little faster) than calling `.clean()`.

		if !mem::needs_drop::<V>() {
			return;
		}

		let contents = self.contents.get_mut(/* poisoned */);
		for (_, (slab, i)) in mem::take(&mut contents.slot_map) {
			unsafe {
				contents
					.values
					.get_unchecked_mut(slab)
					.get_unchecked(i)
					.get()
					.pipe(|value| (*value).as_mut_ptr().drop_in_place())
			}
		}
	}
}

impl<K: Ord, V: ?Sized> Drop for PressedPineMap<K, V> {
	fn drop(&mut self) {
		// None of the data will be used in the future,
		// so explicit cleanup can be a bit more concise (and hopefully a little faster) than calling `.clean()`.

		let contents = self.contents.get_mut(/* poisoned */);
		for (_, value) in mem::take(&mut contents.addresses) {
			unsafe { value.drop_in_place() }
		}
	}
}
