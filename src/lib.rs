//! Library to parse stack usage information ([`.stack_sizes`]) produced by LLVM
//!
//! [`.stack_sizes`]: https://llvm.org/docs/CodeGenerator.html#emitting-function-stack-size-information

#![deny(missing_docs)]
#![deny(warnings)]

extern crate byteorder;
extern crate either;
#[macro_use]
extern crate failure;
extern crate leb128;
#[cfg(feature = "tools")]
extern crate rustc_demangle;
extern crate xmas_elf;

use std::{collections::HashMap, io::Cursor, u32};
#[cfg(feature = "tools")]
use std::{fs::File, io::Read, path::Path};

use byteorder::{ReadBytesExt, LE};
use either::Either;
use xmas_elf::{
    sections::{SectionData, SectionHeader},
    symbol_table::{Entry, Type},
    ElfFile,
};

/// Information about a function
pub struct Function<'a, A> {
    address: A,
    name: &'a str,
    stack: u64,
}

impl<'a, A> Function<'a, A> {
    /// Returns the address of the function
    pub fn address(&self) -> A
    where
        A: Copy,
    {
        self.address
    }

    /// Returns the (mangled) name of the function
    pub fn name(&self) -> &str {
        self.name
    }

    /// Returns the stack usage of the function in bytes
    pub fn stack(&self) -> u64 {
        self.stack
    }
}

/// Parses an ELF file and returns a list of functions and their stack usage
pub fn analyze(
    elf: &[u8],
) -> Result<Either<Vec<Function<u32>>, Vec<Function<u64>>>, failure::Error> {
    let elf = ElfFile::new(elf).map_err(failure::err_msg)?;

    let mut names = HashMap::new();

    if let Some(section) = elf.find_section_by_name(".symtab") {
        match section.get_data(&elf).map_err(failure::err_msg)? {
            SectionData::SymbolTable32(entries) => {
                for entry in entries {
                    if entry.get_type() == Ok(Type::Func) {
                        names.insert(
                            entry.value(),
                            entry.get_name(&elf).map_err(failure::err_msg)?,
                        );
                    }
                }
            }
            SectionData::SymbolTable64(entries) => {
                for entry in entries {
                    if entry.get_type() == Ok(Type::Func) {
                        names.insert(
                            entry.value(),
                            entry.get_name(&elf).map_err(failure::err_msg)?,
                        );
                    }
                }
            }
            _ => bail!("malformed .symtab section"),
        }
    }

    let stack_sizes = elf
        .find_section_by_name(".stack_sizes")
        .ok_or_else(|| failure::err_msg(".stack_sizes section not found"))?;

    let data = stack_sizes.raw_data(&elf);
    let end = data.len() as u64;
    let mut cursor = Cursor::new(data);

    match stack_sizes {
        SectionHeader::Sh32(..) => {
            let mut funs = vec![];

            while cursor.position() < end {
                let address = cursor.read_u32::<LE>()?;
                // NOTE we also try the address plus one because this could be a function in Thumb
                // mode
                let name = names
                    .get(&(address as u64))
                    .or(names.get(&(address as u64 + 1)))
                    .map(|s| *s)
                    .unwrap_or("?");
                let stack = leb128::read::unsigned(&mut cursor)?;

                funs.push(Function {
                    address,
                    stack,
                    name,
                });
            }

            funs.sort_by(|a, b| b.stack().cmp(&a.stack()));

            Ok(Either::Left(funs))
        }
        SectionHeader::Sh64(..) => {
            let mut funs = vec![];

            while cursor.position() < end {
                let address = cursor.read_u64::<LE>()?;
                // NOTE we also try the address plus one because this could be a function in Thumb
                // mode
                let name = names
                    .get(&address)
                    .or(names.get(&(address + 1)))
                    .map(|s| *s)
                    .unwrap_or("?");
                let stack = leb128::read::unsigned(&mut cursor)?;

                funs.push(Function {
                    address,
                    stack,
                    name,
                });
            }

            funs.sort_by(|a, b| b.stack().cmp(&a.stack()));

            Ok(Either::Right(funs))
        }
    }
}

#[cfg(feature = "tools")]
#[doc(hidden)]
pub fn run<P>(path: P) -> Result<(), failure::Error>
where
    P: AsRef<Path>,
{
    let mut bytes = vec![];
    File::open(path)?.read_to_end(&mut bytes)?;

    let funs = analyze(&bytes)?;

    match funs {
        Either::Left(funs) => {
            // 32-bit address space
            println!("address\t\tstack\tname");

            for fun in funs {
                println!(
                    "{:#010x}\t{}\t{}",
                    fun.address(),
                    fun.stack(),
                    rustc_demangle::demangle(fun.name())
                );
            }
        }
        Either::Right(funs) => {
            // 64-bit address space
            println!("address\t\t\tstack\tname");

            for fun in funs {
                println!(
                    "{:#018x}\t{}\t{}",
                    fun.address(),
                    fun.stack(),
                    rustc_demangle::demangle(fun.name())
                );
            }
        }
    }

    Ok(())
}
