**READ THIS**

This proof of concept project is the precursor of [`cargo-call-stack`](https://github.com/japaric/cargo-call-stack) and it's no longer maintained.
If you would like to implement your own tool that analyzes the stack information of a program, including all its dependencies, you can use `cargo-call-stack` as a reference.

`stack-sizes` depends on unstable features of the compiler / Cargo and it's known to work with nightly-2019-03-29. No effort will be made to make it work with newer versions of the Rust toolchain.

---

# `stack-sizes`

> Tools to print stack usage information emitted by LLVM in human readable format

## Background information

Since ` nightly-2018-09-27` `rustc` has a (nightly only) [`-Z emit-stack-sizes`]
flag to (make LLVM) emit stack usage information about functions.

[`-Z emit-stack-sizes`]: https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/emit-stack-sizes.html

> **NOTE**: This feature only works when the output artifact has the ELF object
> format.

The `stack-sizes` tool parses the metadata emitted by that flag and prints it in
human readable format.

The `cargo stack-sizes` subcommand does something similar but first it builds
the whole dependency graph with `-Z emit-stack-sizes`.

## Metadata format

The tools expect the object file to contain a `.stack_sizes` section with stack
usage information emitted by LLVM ([`-stack-size-section`]). For convenience,
the documentation about `-stack-size-section` is copied below:

[`-stack-size-section`]: https://llvm.org/docs/CodeGenerator.html#emitting-function-stack-size-information

> A section containing metadata on function stack sizes will be emitted when
> TargetLoweringObjectFile::StackSizesSection is not null, and
> TargetOptions::EmitStackSizeSection is set (-stack-size-section). The section
> will contain an array of pairs of function symbol values (pointer size) and
> stack sizes (unsigned LEB128). The stack size values only include the space
> allocated in the function prologue. Functions with dynamic stack allocations
> are not included.

## Installation

``` console
$ cargo +stable install stack-sizes
```

## Example usage

The recommended way to analyze your program is to use the `cargo stack-sizes`
subcommand.

Most targets will discard `.stack_sizes` information at link time so the linking
process needs to be tweaked to keep the information in the final binary.

Consider a Cargo project named `hello` with the following `src/main.rs` file:

``` rust
use std::{mem, ptr};

fn main() {
    registers();
    stack();
}

#[inline(never)]
fn registers() {
    unsafe {
        // values loaded into registers; doesn't use the stack
        ptr::read_volatile(&(0u64, 1u64));
    }
}

#[inline(never)]
fn stack() {
    unsafe {
        // array allocated on the stack
        let array: [i32; 4] = mem::uninitialized();
        for elem in &array {
            ptr::read_volatile(&elem);
        }
    }
}
```

We'll use [this linker script](keep-stack-sizes.x) to preserve the
`.stack_sizes` information. Place that linker script in the root of the Cargo
project.

``` console
$ cat keep-stack-sizes.x
```

``` text
SECTIONS
{
  .stack_sizes (INFO) :
  {
    KEEP(*(.stack_sizes));
  }
}
```

Now we can build the project. `cargo stack-sizes` has a similar CLI to `cargo
rustc`. Flags after `--` will be passed to the *top* `rustc` invocation. We'll
use those flags to pass the linker script to the linker.

``` console
$ cargo +nightly stack-sizes \
      --bin hello \
      --release \
      -v \
      -- -C link-arg=-Wl,-Tkeep-stack-sizes.x -C link-arg=-N
RUSTC=stack-sizes-rustc "cargo" "rustc" "--bin" "hello" "--release" "--" "-C" "link-arg=-Wl,-Tkeep-stack-sizes.x" "-C" "link-arg=-N"
(..)
    Finished release [optimized] target(s) in 0.63s
"stack-sizes" "target/release/hello"
address                 stack   name
0x0000000000000550      24      hello::stack::hebd29682aa7dd994
0x00000000000004d0      8       std::rt::lang_start::h272a86063047800b
0x0000000000000500      8       std::rt::lang_start::{{closure}}::h22007b5ddf658a64
0x0000000000000510      8       core::ops::function::FnOnce::call_once::hb7bafcf111f236ed
0x0000000000000530      8       hello::main::hb36271094cf69f90
0x0000000000000590      8       main
0x00000000000004c0      0       core::array::<impl core::iter::traits::IntoIterator for &'a [T; _]>::into_iter::h63e320078b8d6e5b
0x0000000000000520      0       core::ptr::drop_in_place::h6545f3e9027cd0b3
0x0000000000000540      0       hello::registers::h16b8b9c5d4e45cf5
0x00000000000005c0      0       __rust_alloc
0x00000000000005d0      0       __rust_dealloc
0x00000000000005e0      0       __rust_realloc
0x00000000000005f0      0       __rust_alloc_zeroed
```

## Library

This crate can also be used as a library to parse `.stack_sizes` information.
The API documentation can be found [here](https://docs.rs/stack-sizes). It's
recommended to disable the default `tool` feature of this crate, which builds
the tools, when using it as a library.

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
