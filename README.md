# `stack-sizes`

> Library to parse the stack usage (`.stack_sizes`) information emitted by LLVM

## Background information

Since ` nightly-2018-09-27` `rustc` has a (nightly only) [`-Z emit-stack-sizes`]
flag to (make LLVM) emit stack usage information for each Rust function.

[`-Z emit-stack-sizes`]: https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/emit-stack-sizes.html

> **NOTE**: This feature only works when the output artifact has the ELF object
> format.

The `stack-sizes` library provides an API to parse the metadata emitted by that flag.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
