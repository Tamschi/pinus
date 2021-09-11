//! Prickly [`BTreeMap`](`std::collections::BTreeMap`)s.
//!
//! This crate contains a number of b-tree containers that allow pin-projections towards their values.
//! It's also possible to insert new entries through shared references.
//!
//! As the API is largely shared between these implementations (barring minor details),
//! it is exposed through the traits in the [`prelude`] rather than through associated methods.

#![doc(html_root_url = "https://docs.rs/pinus/0.0.1")]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::semicolon_if_nothing_returned)]

use std::convert::Infallible;

#[cfg(doctest)]
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
