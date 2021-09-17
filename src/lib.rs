//! Prickly [`BTreeMap`](`std::collections::BTreeMap`)s.
//!
//! This crate contains a number of b-tree containers that allow pin-projections towards their values.
//! It's also possible to insert new entries through shared references.
//!
//! As the API is largely shared between these implementations (barring minor details),
//! it is exposed through the traits in the [`prelude`] rather than through associated methods.
//!
//! # Features
//!
//! ## `"unstable"`
//!
//! Enables parts of the API that haven't been fully stabilised yet.
//!
//! Currently, this means:
//!
//! - pinning using the standard [`Pin`](`std::pin::Pin`) wrapper.
//!
//! # General usage notes
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
//! Same as before, this will be much nicer to add once GATs land.

#![doc(html_root_url = "https://docs.rs/pinus/0.0.1")]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::semicolon_if_nothing_returned)]

use std::convert::Infallible;

#[cfg(all(doctest, feature = "unstable"))]
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
