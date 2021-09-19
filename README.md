# pinus

[![Lib.rs](https://img.shields.io/badge/Lib.rs-*-84f)](https://lib.rs/crates/pinus)
[![Crates.io](https://img.shields.io/crates/v/pinus)](https://crates.io/crates/pinus)
[![Docs.rs](https://docs.rs/pinus/badge.svg)](https://docs.rs/pinus)

![Rust 1.55](https://img.shields.io/static/v1?logo=Rust&label=&message=1.55&color=grey)
[![CI](https://github.com/Tamschi/pinus/workflows/CI/badge.svg?branch=unstable)](https://github.com/Tamschi/pinus/actions?query=workflow%3ACI+branch%3Aunstable)
![Crates.io - License](https://img.shields.io/crates/l/pinus/0.0.3)

[![GitHub](https://img.shields.io/static/v1?logo=GitHub&label=&message=%20&color=grey)](https://github.com/Tamschi/pinus)
[![open issues](https://img.shields.io/github/issues-raw/Tamschi/pinus)](https://github.com/Tamschi/pinus/issues)
[![open pull requests](https://img.shields.io/github/issues-pr-raw/Tamschi/pinus)](https://github.com/Tamschi/pinus/pulls)
[![good first issues](https://img.shields.io/github/issues-raw/Tamschi/pinus/good%20first%20issue?label=good+first+issues)](https://github.com/Tamschi/pinus/contribute)

[![crev reviews](https://web.crev.dev/rust-reviews/badge/crev_count/pinus.svg)](https://web.crev.dev/rust-reviews/crate/pinus/)

A prickly BTreeMap.

- You can insert through shared references and values are pin-projected.
- You can remove keys and drop entries through exclusive references.
- You can remove values through exclusive references until the `PineMap` is pinned.

<!-- markdownlint-disable heading-increment no-trailing-punctuation -->

### Help wanted!

<!-- markdownlint-enable heading-increment no-trailing-punctuation -->

I need only a fairly barebones implementation and not necessarily optimized version of this data structure for my own project(s),
but am committed to maintaining it into shape if there's interest.

Follow the "good first issues" badge above for starting points!

Note that the crate uses unsafe code very frequently, so you should be at least somewhat comfortable with ensuring soundness manually. Code reviews are also highly appreciated, both within and outside of this regard.

## Installation

Please use [cargo-edit](https://crates.io/crates/cargo-edit) to always add the latest version of this library:

```cmd
cargo add pinus
```

## Examples

### Homogeneous Map

```rust
use pinus::{prelude::*, sync::PineMap};
use std::{convert::Infallible, pin::Pin};

// `PineMap` is interior-mutable, so either is useful:
let map = PineMap::new();
let mut mut_map = PineMap::new();


// Get parallel shared references by inserting like this:
let a: &String = map.insert("Hello!", "Hello!".to_string())
  .unwrap(/* Your data back if the entry already existed. */);
let b: &String = map.insert_with("Hello again!", |k| k.to_string())
  .map_err(|(key, _factory)| key).unwrap();
let c: &String = map.try_insert_with::<_, Infallible>("Hello once again!", |k| Ok(k.to_string()))
  .unwrap(/* Error from factory. */)
  .map_err(|(key, _factory)| key).unwrap();

let a2: &String = map.get("Hello!").unwrap();

let _ = (a, a2, b, c);


// Get exclusive references like this (also with or without factory):
let mut_a: &mut String = mut_map.insert_with_mut("Hi!", |k| k.to_string())
  .map_err(|(key, _factory)| key).unwrap();

let mut_a2: &mut String = mut_map.get_mut("Hi!").unwrap();

// The `…_mut` methods are actually faster, but their results can't be held onto at once:
// let _ = (mut_a, mut_a2); // "error[E0499]: cannot borrow `mut_map` as mutable more than once at a time"


// Remove entries like this:
mut_map.clear();
let _: Option<(&str, String)> = mut_map.remove_pair("A");
let _: Option<String> = mut_map.remove_value("B");
let _: Option<&str> = mut_map.remove_key("C");
let _: bool = mut_map.drop_entry("D");


/////


// Now on to part 2, value pinning:
let mut map: Pin<_> = map.pin();
let mut mut_map: Pin<_> = mut_map.pin();


// Shared references to values are now pinned:
let a: Pin<&String> = map.insert("Hello!!", "Hello!!".to_string())
  .unwrap();
let b: Pin<&String> = map.insert_with("Hello again!!", |k| k.to_string())
  .ok().unwrap();
let c: Pin<&String> = map.try_insert_with::<_, Infallible>("Hello once again!!", |k| Ok(k.to_string()))
  .unwrap().ok().unwrap();

let a2: Pin<&String> = map.get("Hello!").unwrap();

let _ = (a, a2, b, c);


// Exclusive references to values are also pinned:
let mut mut_a: Pin<&mut String> = mut_map.insert_with_mut("Hi!", |k| k.to_string())
  .map_err(|(key, _factory)| key).unwrap();

let mut mut_a2: Pin<&mut String> = mut_map.get_mut("Hi!").unwrap();

// The `…_mut` methods are actually faster, but their results can't be held onto at once:
// let _ = (mut_a, mut_a2); // "error[E0499]: cannot borrow `mut_map` as mutable more than once at a time"

// Only keys can be removed now, but values must be dropped in place:
mut_map.clear();
let _: Option<&str> = mut_map.remove_key("C");
let _: bool = mut_map.drop_entry("D");
```

### Heterogeneous Map

```rust
use pinus::{prelude::*, sync::PressedPineMap};
use std::{
  any::Any,
  borrow::{Borrow, BorrowMut},
  convert::Infallible,
  pin::Pin,
};

let map = PressedPineMap::<_, dyn Any>::new();

// `dyn Any` is `!Sized`,
// so it's necessary to use the loosely-typed emplacement methods:
let _: &dyn Any = map
  .emplace_with(1, |_key, slot| slot.write(()))
  .ok(/* or key and factory */).unwrap();
let _: &dyn Any = map
  .try_emplace_with::<_, Infallible>(2, |_key, slot| Ok(slot.write(())))
  .unwrap(/* or factory error */)
  .ok(/* or key and factory */).unwrap();

// There's also a by-value method,
// but it has slightly steeper requirements:
#[derive(Debug)]
struct MyAny;
impl std::borrow::Borrow<dyn Any> for MyAny { //…
#   fn borrow(&self) -> &dyn Any { self }
# }
impl std::borrow::BorrowMut<dyn Any> for MyAny { //…
#   fn borrow_mut(&mut self) -> &mut dyn Any { self }
# }

let _: &dyn Any = map
  .emplace(3, MyAny)
  .unwrap(/* or key and value */);

// As usual the map's values can be pinned:
let map: Pin<PressedPineMap<_, _>> = map.pin();

// And then further value references are pinned:
let _: Pin<&dyn Any> = map.emplace(4, MyAny).unwrap();

// To immediately get an unpinned reference, just use `.as_unpinned()`:
let _: &dyn Any = map.as_unpinned().emplace(5, MyAny).unwrap();
```

## License

Licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## [Code of Conduct](CODE_OF_CONDUCT.md)

## [Changelog](CHANGELOG.md)

## Versioning

`pinus` strictly follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) with the following exceptions:

- The minor version will not reset to 0 on major version changes (except for v1).  
Consider it the global feature level.
- The patch version will not reset to 0 on major or minor version changes (except for v0.1 and v1).  
Consider it the global patch level.

This includes the Rust version requirement specified above.  
Earlier Rust versions may be compatible, but this can change with minor or patch releases.

Which versions are affected by features and patches can be determined from the respective headings in [CHANGELOG.md](CHANGELOG.md).
