use crate::prelude::{PinnedPineMap, UnpinnedPineMap};
use parking_lot::RwLock;
use std::{
	cell::UnsafeCell,
	collections::BTreeMap,
	mem::{self, ManuallyDrop},
};
use tap::Pipe;
use vec1::Vec1;

pub struct PineMap<K: Ord, V> {
	contents: RwLock<Cambium<K, V>>,
}

struct Cambium<K, V> {
	slot_map: BTreeMap<K, (usize, usize)>,
	values: Vec1<Vec<UnsafeCell<ManuallyDrop<V>>>>,
	holes: Vec<(usize, usize)>,
}

impl<K: Ord, V> PineMap<K, V> {
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
		}
	}
}

impl<K: Ord, V> Default for PineMap<K, V> {
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
				.pipe(|value| &**value)
		}
		.pipe(Some)
	}

	fn try_insert_with<F: FnOnce(&K) -> Result<V, E>, E>(
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
			let value = value_factory(&key)?;
			slot_map.insert(key, hole);
			let slot =
				unsafe { values.get_unchecked_mut(hole.0).get_unchecked_mut(hole.1) }.get_mut();
			*slot = ManuallyDrop::new(value);
			Ok(unsafe { &**(slot as *const ManuallyDrop<V>) })
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
			slab.push(
				value_factory(&key)?
					.pipe(ManuallyDrop::new)
					.pipe(UnsafeCell::new),
			);
			let i = slab.len() - 1;
			slot_map.insert(key, (slab_i, i));
			slab.last()
				.map(|value| unsafe { &**value.get() })
				.ok_or_else(|| unreachable!())
		}
		.pipe(Ok)
	}

	fn clear(&mut self) {
		let contents = self.contents.get_mut(/* poisoned */);
		contents.holes.clear();

		// WAITING ON: <https://github.com/rust-lang/rust/issues/70530> (`BTreeMap::drain_filter`)
		for (_, (slab, i)) in mem::take(&mut contents.slot_map) {
			unsafe {
				contents
					.values
					.get_unchecked_mut(slab)
					.get_unchecked(i)
					.get()
					.pipe(|value| ManuallyDrop::drop(&mut *value))
			}
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
				.pipe(|value| &mut **value)
		}
		.pipe(Some)
	}

	fn try_insert_with_mut<F: FnOnce(&K) -> Result<V, E>, E>(
		&mut self,
		key: K,
		value_factory: F,
	) -> Result<Result<&mut V, (K, F)>, E> {
		let contents = self.contents.get_mut(/* poisoned */);
		let Cambium {
			slot_map,
			values,
			holes,
		} = &mut *contents;
		#[allow(clippy::map_entry)]
		if slot_map.contains_key(&key) {
			Err((key, value_factory))
		} else if let Some(hole) = holes.pop() {
			let value = value_factory(&key)?;
			slot_map.insert(key, hole);
			let slot =
				unsafe { values.get_unchecked_mut(hole.0).get_unchecked_mut(hole.1) }.get_mut();
			*slot = ManuallyDrop::new(value);
			Ok(unsafe { &mut **(slot as *mut ManuallyDrop<V>) })
		} else {
			let mut slab_i = values.len() - 1;
			let slab = values.last_mut();
			if slab.len() >= Vec::capacity(slab) {
			} else {
				slab_i += 1;
				let target_capacity = slab.len() * 2;
				let mut slab = Vec::new();
				slab.reserve(target_capacity);
				values.push(slab);
			};
			let slab = values.last_mut();
			slab.push(
				value_factory(&key)?
					.pipe(ManuallyDrop::new)
					.pipe(UnsafeCell::new),
			);
			let i = slab.len() - 1;
			slot_map.insert(key, (slab_i, i));
			slab.last_mut()
				.map(|value| &mut **value.get_mut())
				.ok_or_else(|| unreachable!())
		}
		.pipe(Ok)
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
				.pipe(|value| ManuallyDrop::take(&mut *value))
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
		unsafe {
			contents
				.values
				.get_unchecked(slab)
				.get_unchecked(i)
				.get()
				.pipe(|value| ManuallyDrop::drop(&mut *value))
		};
		Some(key)
	}
}

unsafe impl<K: Ord, V> PinnedPineMap<K, V> for PineMap<K, V> {}

unsafe impl<K: Ord, V> Sync for PineMap<K, V>
where
	K: Sync + Send,
	V: Sync + Send,
{
}

unsafe impl<K: Ord, V> Send for PineMap<K, V>
where
	K: Send,
	V: Send,
{
}

impl<K: Ord, V> Drop for PineMap<K, V> {
	fn drop(&mut self) {
		// None of the data will be used in the future,
		// so explicit cleanup can be a bit more concise (and hopefully a little faster) than calling `.clean()`.

		let contents = self.contents.get_mut(/* poisoned */);
		for (_, (slab, i)) in mem::take(&mut contents.slot_map) {
			unsafe {
				contents
					.values
					.get_unchecked_mut(slab)
					.get_unchecked(i)
					.get()
					.pipe(|value| ManuallyDrop::drop(&mut *value))
			}
		}
	}
}
