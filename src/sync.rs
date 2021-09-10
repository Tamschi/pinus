use crate::prelude::{MutPineMap, PinMutPineMap, PinRefPineMap, RefPineMap};
use parking_lot::RwLock;
use std::{
	cell::UnsafeCell,
	collections::BTreeMap,
	mem::{self, ManuallyDrop},
};
use tap::Pipe;
use vec1::Vec1;

pub struct PineMap<K, V> {
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

impl<K: Ord, V> RefPineMap<K, V> for PineMap<K, V> {
	#[track_caller]
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

	#[track_caller]
	fn try_insert_with<F: FnOnce() -> Result<V, E>, E>(
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
			let value = value_factory()?;
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
				value_factory()?
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
}

impl<K: Ord, V> MutPineMap<K, V> for PineMap<K, V> {
	#[track_caller]
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

	#[track_caller]
	fn get<Q>(&mut self, key: &Q) -> Option<&mut V>
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
}

unsafe impl<K: Ord, V> PinRefPineMap<K, V> for PineMap<K, V> {}
unsafe impl<K: Ord, V> PinMutPineMap<K, V> for PineMap<K, V> {}
