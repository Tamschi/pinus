use std::error::Error;

use pinus::{prelude::RefPineMap, sync::PineMap};

#[test]
fn new() {
	let _ = PineMap::<usize, usize>::new();
}

#[test]
fn insert() {
	let map = PineMap::<usize, usize>::new();
	assert_eq!(
		map.try_insert_with::<_, ()>(1, || Ok(2))
			.unwrap()
			.ok()
			.unwrap(),
		&2
	);
}

#[test]
fn complicated() {
	let map = PineMap::<usize, usize>::new();
	assert_eq!(
		map.try_insert_with::<_, ()>(1, || Ok(2))
			.unwrap()
			.ok()
			.unwrap(),
		&2
	);

	assert_eq!(
		map.try_insert_with::<_, ()>(2, || Ok(3))
			.unwrap()
			.ok()
			.unwrap(),
		&3
	);

	assert_eq!(
		map.try_insert_with::<_, ()>(3, || Ok(4))
			.unwrap()
			.ok()
			.unwrap(),
		&4
	);

	assert_eq!(
		map.try_insert_with::<_, ()>(3, || Ok(4))
			.unwrap()
			.unwrap_err()
			.0,
		3
	);

	let a = map.get(&1);
	let b = map.get(&2);
	let c = map.get(&3);

	let result = map.try_insert_with::<_, Box<dyn Error>>(5, || Ok(7));

	println!("{:?}", a);
	println!("{:?}", b);
	println!("{:?}", c);

	println!("{:?}", result.unwrap().ok().unwrap());
}
