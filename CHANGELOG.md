# pinus Changelog

<!-- markdownlint-disable no-trailing-punctuation -->

## 0.0.4

2021-10-10

- **Breaking changes**:
  - renamed `.as_unpinned_mut()` to `.as_unpinned_mut_unchecked()`.
    > This is to make room for the safe `.as_unpinned_mut()` below.
- Features:
  - `.as_unpinned_mut()` is now a safe method that requires `V: Unpin`.
  - added safe `.try_emplace_with_mut_unpinned(…)` and `.emplace_with_mut_unpinned(…)` methods.
- Revisions:
  - Documentation improvements.

## 0.0.3

2021-09-19

- Revisions:
  - Fixed wrong minimum Rust version in README.

## 0.0.2 (yanked)

2021-09-19

- **Breaking changes**:
  - The required minimum Rust version is now `1.55`
    > as this is required for `MaybeUninit::write`.

- Features:
  - Added an emplacement API and an alternative implementation for heterogeneous trait objects.

- Revisions:
  - Collections will now aggressively drop further values during [`PinnedPineMap.clear`](https://docs.rs/pinus/0.0.2/pinus/prelude/trait.PinnedPineMap.html#method.clear) and [`Drop::drop`](https://doc.rust-lang.org/stable/std/ops/trait.Drop.html#tymethod.drop) even if a panic occurs along the way.

    **This was most likely previously unsound, so version 0.0.1 will be yanked.**

    > See also [Issue #5: Please check for pinning and unwind safety issues!](https://github.com/Tamschi/pinus/issues/5).

## 0.0.1 (yanked)

2021-09-10

Initial unstable release
