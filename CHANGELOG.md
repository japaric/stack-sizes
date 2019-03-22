# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

## [v0.4.0] - 2019-03-22

### Added

- A `stack_sizes::analyze_object` function has been added. This function is
  geared towards analyzing relocatable object (`.o`) files.

### Changed

- [breaking-change] `stack_sizes::analyze` has been renamed to
  `stack_sizes::analyze_executable` and its returns type has changed to
  `Functions`.

- `analyze_executable` no longer includes tags like `$a.1`, `$d.2` and `$t.3` in
  the list of aliases of a function.

- [breaking-change] `Function` no longer provides an `address` method and it's
  no longer a generic struct .

- the `stack-sizes-rustc` binary, which was an implementation detail of
  `cargo-stack-sizes`, has been removed.

- The `stack-sizes` tool can now analyze relocatable object (`.o`) files.

- `cargo-stack-sizes` now always forces a rebuild -- the Cargo caching behavior
  makes it hard to locate the object file that contains the stack usage
  information.

## [v0.3.1] - 2019-03-10

### Added

- `Function` now has a `size` method to get the size of the subroutine in bytes.

## [v0.3.0] - 2019-03-03

### Changed

- [breaking-change] `Function.address` now returns an `Option`.

- `stack_sizes::analyze` now detects even more function aliases, specially ones
  that has been created using linker scripts (see `PROVIDE`).

- `stack_sizes::analyze` does *not* error if the `.stack_sizes` section is
  missing.

- `stack_sizes::analyze` now also reports undefined symbols (symbols that will
  be loaded at runtime from a dynamic library); these have an address of `None`.

- `Function` now implements the `Debug` trait

## v0.2.0 - 2018-12-02

### Added

- `analyze` now handles symbol aliases. `Function` gained a `names` method that
  returns a list of names (or aliases) associated to an address.

### Changed

- [breaking-change] `Function.stack` now returns `Option<u64>`.

- `analyze` now returns *all* the symbols in the ELF; even those for which there
  is no information about their stack usage.

### Removed

- [breaking-change] `Function.name` method has been removed.

## v0.1.1 - 2018-11-30

### Changed

- Extended the lifetime of the string returned by `Function.name`

## v0.1.0 - 2018-09-28

- Initial release

[Unreleased]: https://github.com/japaric/stack-sizes/compare/v0.4.0...HEAD
[v0.4.0]: https://github.com/japaric/stack-sizes/compare/v0.3.1...v0.4.0
[v0.3.1]: https://github.com/japaric/stack-sizes/compare/v0.3.0...v0.3.1
[v0.3.0]: https://github.com/japaric/stack-sizes/compare/v0.2.0...v0.3.0
[v0.2.0]: https://github.com/japaric/stack-sizes/compare/v0.1.1...v0.2.0
[v0.1.1]: https://github.com/japaric/stack-sizes/compare/v0.1.0...v0.1.1
