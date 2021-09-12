# pinus Changelog

<!-- markdownlint-disable no-trailing-punctuation -->

## next (0.0.2)

TODO: Date

- **Breaking changes**:
  - The required minimum Rust version is now `1.55`
    > as this is required for `MaybeUninit::write`.

- Features:
  - Added an emplacement API and an alternative implementation for heterogeneous trait objects.

- Revisions:
  - Collections will now aggressively drop further values during [`PinnedPineMap.clear`](https://docs.rs/pinus/0.0.2/pinus/prelude/trait.PinnedPineMap.html#method.clear) and [`Drop::drop`](https://doc.rust-lang.org/stable/std/ops/trait.Drop.html#tymethod.drop) even if a panic occurs along the way.

    **This was most likely previously unsound, so version 0.0.1 will be yanked.**

    > See also [Issue #5: Please check for pinning and unwind safety issues!](https://github.com/Tamschi/pinus/issues/5).

## 0.0.1

2021-09-10

Initial unstable release
