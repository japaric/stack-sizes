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
    names: Vec<&'a str>,
    stack: Option<u64>,
}

impl<'a, A> Function<'a, A> {
    /// Returns the address of the function
    pub fn address(&self) -> A
    where
        A: Copy,
    {
        self.address
    }

    /// Returns the (mangled) name of the function and its aliases
    pub fn names(&self) -> &[&'a str] {
        &self.names
    }

    /// Returns the stack usage of the function in bytes
    pub fn stack(&self) -> Option<u64> {
        self.stack
    }
}

/// Parses an ELF file and returns a list of functions and their stack usage
pub fn analyze(
    elf: &[u8],
) -> Result<Either<Vec<Function<u32>>, Vec<Function<u64>>>, failure::Error> {
    let elf = ElfFile::new(elf).map_err(failure::err_msg)?;

    // address -> [name]
    let mut all_names = HashMap::new();

    let mut maybe_aliases = HashMap::new();
    let mut is_64_bit = false;
    if let Some(section) = elf.find_section_by_name(".symtab") {
        match section.get_data(&elf).map_err(failure::err_msg)? {
            SectionData::SymbolTable32(entries) => {
                for entry in entries {
                    let ty = entry.get_type();
                    let value = entry.value();
                    let name = entry.get_name(&elf).map_err(failure::err_msg)?;
                    if ty == Ok(Type::Func) {
                        all_names.entry(value).or_insert(vec![]).push(name);
                    } else if ty == Ok(Type::NoType) {
                        maybe_aliases.entry(value).or_insert(vec![]).push(name);
                    }
                }
            }
            SectionData::SymbolTable64(entries) => {
                is_64_bit = true;

                for entry in entries {
                    let ty = entry.get_type();
                    let value = entry.value();
                    let name = entry.get_name(&elf).map_err(failure::err_msg)?;
                    if ty == Ok(Type::Func) {
                        all_names.entry(value).or_insert(vec![]).push(name);
                    } else if ty == Ok(Type::NoType) {
                        maybe_aliases.entry(value).or_insert(vec![]).push(name);
                    }
                }
            }
            _ => bail!("malformed .symtab section"),
        }
    }

    for (value, alias) in maybe_aliases {
        if let Some(names) = all_names.get_mut(&value) {
            names.extend(alias);
        }
    }

    if let Some(stack_sizes) = elf.find_section_by_name(".stack_sizes") {
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
                    let names = all_names
                        .remove(&(u64::from(address)))
                        .or_else(|| all_names.remove(&(u64::from(address) + 1)))
                        .unwrap_or(vec![]);
                    let stack = Some(leb128::read::unsigned(&mut cursor)?);

                    funs.push(Function {
                        address,
                        stack,
                        names,
                    });
                }

                funs.sort_by(|a, b| b.stack().cmp(&a.stack()));

                // add functions for which we don't have stack size information
                for (address, names) in all_names {
                    funs.push(Function {
                        address: address as u32,
                        stack: None,
                        names,
                    });
                }

                Ok(Either::Left(funs))
            }
            SectionHeader::Sh64(..) => {
                let mut funs = vec![];

                while cursor.position() < end {
                    let address = cursor.read_u64::<LE>()?;
                    // NOTE we also try the address plus one because this could be a function in Thumb
                    // mode
                    let names = all_names
                        .remove(&address)
                        .or_else(|| all_names.remove(&(address + 1)))
                        .unwrap_or(vec![]);
                    let stack = Some(leb128::read::unsigned(&mut cursor)?);

                    funs.push(Function {
                        address,
                        stack,
                        names,
                    });
                }

                funs.sort_by(|a, b| b.stack().cmp(&a.stack()));

                // add functions for which we don't have stack size information
                for (address, names) in all_names {
                    funs.push(Function {
                        address,
                        stack: None,
                        names,
                    });
                }

                Ok(Either::Right(funs))
            }
        }
    } else if is_64_bit {
        Ok(Either::Right(
            all_names
                .into_iter()
                .map(|(address, names)| Function {
                    address,
                    stack: None,
                    names,
                })
                .collect(),
        ))
    } else {
        Ok(Either::Left(
            all_names
                .into_iter()
                .map(|(address, names)| Function {
                    address: address as u32,
                    stack: None,
                    names,
                })
                .collect(),
        ))
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
                if let (Some(name), Some(stack)) = (fun.names().first(), fun.stack()) {
                    println!(
                        "{:#010x}\t{}\t{}",
                        fun.address(),
                        stack,
                        rustc_demangle::demangle(name)
                    );
                }
            }
        }
        Either::Right(funs) => {
            // 64-bit address space
            println!("address\t\t\tstack\tname");

            for fun in funs {
                if let (Some(name), Some(stack)) = (fun.names().first(), fun.stack()) {
                    println!(
                        "{:#018x}\t{}\t{}",
                        fun.address(),
                        stack,
                        rustc_demangle::demangle(name)
                    );
                }
            }
        }
    }

    Ok(())
}
