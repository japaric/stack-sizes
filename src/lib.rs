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
#[derive(Debug)]
pub struct Function<'a, A> {
    address: Option<A>,
    names: Vec<&'a str>,
    size: u64,
    stack: Option<u64>,
}

impl<'a, A> Function<'a, A> {
    /// Returns the address of the function
    ///
    /// A value of `None` indicates that this symbol is undefined (dynamically loaded)
    pub fn address(&self) -> Option<A>
    where
        A: Copy,
    {
        self.address
    }

    /// Returns the (mangled) name of the function and its aliases
    pub fn names(&self) -> &[&'a str] {
        &self.names
    }

    /// Returns the size of this subroutine in bytes
    pub fn size(&self) -> u64 {
        self.size
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

    // address -> ([name], size)
    let mut all_names = HashMap::new();
    let mut undefs = vec![];

    let mut maybe_aliases = HashMap::new();
    let mut is_64_bit = false;
    if let Some(section) = elf.find_section_by_name(".symtab") {
        match section.get_data(&elf).map_err(failure::err_msg)? {
            SectionData::SymbolTable32(entries) => {
                for entry in entries {
                    let ty = entry.get_type();
                    let value = entry.value();
                    let size = entry.size();
                    let name = entry.get_name(&elf).map_err(failure::err_msg)?;
                    if ty == Ok(Type::Func) {
                        if value == 0 && size == 0 {
                            undefs.push(name);
                        } else {
                            all_names
                                .entry(value)
                                .or_insert((vec![], size))
                                .0
                                .push(name);
                        }
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
                    let size = entry.size();
                    let name = entry.get_name(&elf).map_err(failure::err_msg)?;
                    if ty == Ok(Type::Func) {
                        if value == 0 && size == 0 {
                            undefs.push(name);
                        } else {
                            all_names
                                .entry(value)
                                .or_insert((vec![], size))
                                .0
                                .push(name);
                        }
                    } else if ty == Ok(Type::NoType) {
                        maybe_aliases.entry(value).or_insert(vec![]).push(name);
                    }
                }
            }
            _ => bail!("malformed .symtab section"),
        }
    }

    for (value, alias) in maybe_aliases {
        if let Some((names, _)) = all_names.get_mut(&value) {
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
                    let (mut names, size) = all_names
                        .remove(&(u64::from(address)))
                        .or_else(|| all_names.remove(&(u64::from(address) + 1)))
                        .expect("UNREACHABLE");
                    let stack = Some(leb128::read::unsigned(&mut cursor)?);

                    names.sort();
                    funs.push(Function {
                        address: Some(address),
                        stack,
                        names,
                        size,
                    });
                }

                funs.sort_by(|a, b| b.stack().cmp(&a.stack()));

                // add functions for which we don't have stack size information
                for (address, (mut names, size)) in all_names {
                    names.sort();

                    funs.push(Function {
                        address: Some(address as u32),
                        stack: None,
                        names,
                        size,
                    });
                }

                if !undefs.is_empty() {
                    funs.push(Function {
                        address: None,
                        stack: None,
                        names: undefs,
                        size: 0,
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
                    let (mut names, size) = all_names
                        .remove(&address)
                        .or_else(|| all_names.remove(&(address + 1)))
                        .expect("UNREACHABLE");
                    let stack = Some(leb128::read::unsigned(&mut cursor)?);

                    names.sort();
                    funs.push(Function {
                        address: Some(address),
                        stack,
                        names,
                        size,
                    });
                }

                funs.sort_by(|a, b| b.stack().cmp(&a.stack()));

                // add functions for which we don't have stack size information
                for (address, (mut names, size)) in all_names {
                    names.sort();

                    funs.push(Function {
                        address: Some(address),
                        stack: None,
                        names,
                        size,
                    });
                }

                if !undefs.is_empty() {
                    funs.push(Function {
                        address: None,
                        stack: None,
                        names: undefs,
                        size: 0,
                    });
                }

                Ok(Either::Right(funs))
            }
        }
    } else if is_64_bit {
        let mut funs = all_names
            .into_iter()
            .map(|(address, (mut names, size))| {
                names.sort();

                Function {
                    address: Some(address),
                    stack: None,
                    names,
                    size,
                }
            })
            .collect::<Vec<_>>();

        if !undefs.is_empty() {
            funs.push(Function {
                address: None,
                stack: None,
                names: undefs,
                size: 0,
            });
        }

        Ok(Either::Right(funs))
    } else {
        let mut funs = all_names
            .into_iter()
            .map(|(address, (mut names, size))| {
                names.sort();
                Function {
                    address: Some(address as u32),
                    stack: None,
                    names,
                    size,
                }
            })
            .collect::<Vec<_>>();

        if !undefs.is_empty() {
            funs.push(Function {
                address: None,
                stack: None,
                names: undefs,
                size: 0,
            });
        }

        Ok(Either::Left(funs))
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
                if let (Some(name), Some(stack), Some(addr)) =
                    (fun.names().first(), fun.stack(), fun.address())
                {
                    println!(
                        "{:#010x}\t{}\t{}",
                        addr,
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
                if let (Some(name), Some(stack), Some(addr)) =
                    (fun.names().first(), fun.stack(), fun.address())
                {
                    println!(
                        "{:#018x}\t{}\t{}",
                        addr,
                        stack,
                        rustc_demangle::demangle(name)
                    );
                }
            }
        }
    }

    Ok(())
}
