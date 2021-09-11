#![doc(html_root_url = "https://docs.rs/pinus/0.0.1")]
#![warn(clippy::pedantic)]
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
