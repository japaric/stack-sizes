# `stack-sizes`

> Tool to print stack usage information emitted by LLVM in human readable format

This tool depends on PR [rust-lang/rust#XXX].

[rust-lang/rust#XXX]: TODO

## Format

The tool expects the object file to contain a `.stack_sizes` section with stack usage information
emitted by LLVM ([`-stack-size-section`]). For convenience, the documentation about
`-stack-size-section` is copied below:

[`-stack-size-section`]: https://llvm.org/docs/CodeGenerator.html#emitting-function-stack-size-information

> A section containing metadata on function stack sizes will be emitted when
> TargetLoweringObjectFile::StackSizesSection is not null, and TargetOptions::EmitStackSizeSection
> is set (-stack-size-section). The section will contain an array of pairs of function symbol values
> (pointer size) and stack sizes (unsigned LEB128). The stack size values only include the space
> allocated in the function prologue. Functions with dynamic stack allocations are not included.

## Installation

```
$ cargo install --git https://github.com/japaric/stack-sizes
```

## Example usage

``` console
$ cargo new --bin hello && cd $_

$ cat >src/main.rs <<'EOF'
use std::{mem, ptr};

fn main() {
    registers();
    stack();
}

#[inline(never)]
fn registers() {
    unsafe {
        // values loaded into registers
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
EOF

$ # we need a custom linking step to preserve the .stack_sizes section
$ cat > keep-stack-sizes.x <<'EOF'
SECTIONS
{
  .stack_sizes :
  {
    KEEP(*(.stack_sizes));
  }
}
EOF

$ cargo rustc --release -- \
    -Z emit-stack-sizes \
    -C link-arg=-Wl,-Tkeep-stack-sizes.x \
    -C link-arg=-N

$ size -A target/release/hello | grep stack_sizes
.stack_sizes    117   185136

$ stack-sizes target/release/hello
address                 size    name
0x000000000004b0        0       core::array::<impl core::iter::traits::IntoIterator for &'a [T; _]>::into_iter::ha50e6661c0ec84aa
0x000000000004c0        8       std::rt::lang_start::ha02aea783e0e1b3e
0x000000000004f0        8       std::rt::lang_start::{{closure}}::h5115b527d5244952
0x00000000000500        8       core::ops::function::FnOnce::call_once::h6bfa1076da82b0fb
0x00000000000510        0       core::ptr::drop_in_place::hb4de82e57787bc70
0x00000000000520        8       hello::main::h08bb6cec0556bd66
0x00000000000530        0       hello::registers::h9d058a5d765ec1d2
0x00000000000540        24      hello::stack::h88c8cb66adfdc6f3
0x00000000000580        8       main
0x000000000005b0        0       __rust_alloc
0x000000000005c0        0       __rust_dealloc
0x000000000005d0        0       __rust_realloc
0x000000000005e0        0       __rust_alloc_zeroed
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
