# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

### Changed

- `stack_sizes::analyze` now detects even more function aliases, specially ones
  that has been created using linker scripts (see `PROVIDE`).

- `stack_sizes::analyze` does *not* error if the `.stack_sizes` section is
  missing.

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

[v0.2.0]: https://github.com/japaric/stack-sizes/compare/v0.1.1...v0.2.0
[v0.1.1]: https://github.com/japaric/stack-sizes/compare/v0.1.0...v0.1.1
[Unreleased]: https://github.com/japaric/stack-sizes/compare/v0.1.0...HEAD
