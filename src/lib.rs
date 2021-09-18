//! Prickly [`BTreeMap`](`std::collections::BTreeMap`)s.
//!
//! This crate contains a number of b-tree containers that allow safe value-pinning.
//! It's also possible to insert new entries through shared references, even after the pinning operation.
//!
//! # Examples
//!
//! See:
//!
//! - [`sync::PineMap`] (for a plain map, basic usage) and
//! - [`sync::PressedPineMap`] (for a trait object map).
//!
//! # General usage notes
//!
//! ## Most of the API is in traits
//!
//! As the API is largely shared between these implementations (bar minor details like constructors),
//! it is exposed through the traits in the [`prelude`] rather than through associated methods.
//!
//! > This is also necessary since `self: &Pin<Self>` and similar receiver arguments aren't valid yet.
//!
//! ## The collections are [`Unpin`]
//!
//! As these are heap-based, it doesn't matter where the host instance is located.
//!
//! ## Keys *cannot* be pinned
//!
//! As the collections in this crate are [`Unpin`], and keys can move about even through the shared reference API,
//! there are no guarantees made regarding their memory location.
//!
//! ## Instances are not reentrant
//!
//! Unless otherwise noted.
//!
//! > Currently, that means none are, but this may change in a minor update.  
//! > I am, however, unlikely to add this feature myself before GATs land.
//!
//! This should only affect value factories and [`Drop`] implementations of keys and values.
//!
//! ## Thread **un**safe versions of the collections don't exist yet
//!
//! Same as above, this will be much nicer to add once GATs land.

#![doc(html_root_url = "https://docs.rs/pinus/0.0.2")]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::semicolon_if_nothing_returned)]

use std::convert::Infallible;

#[cfg(all(doctest))]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

pub mod prelude;
pub mod sync;

trait UnwrapInfallible {
	type T;
	fn unwrap_infallible(self) -> Self::T;
}
impl<T> UnwrapInfallible for Result<T, Infallible> {
	type T = T;

	fn unwrap_infallible(self) -> Self::T {
		self.expect("unreachable")
	}
}
