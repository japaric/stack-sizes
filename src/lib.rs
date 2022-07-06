//! Library to parse stack usage information ([`.stack_sizes`]) produced by LLVM
//!
//! [`.stack_sizes`]: https://llvm.org/docs/CodeGenerator.html#emitting-function-stack-size-information

#![deny(rust_2018_idioms)]
#![deny(missing_docs)]
#![deny(warnings)]

use core::u16;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Cursor,
};
#[cfg(feature = "tools")]
use std::{fs, path::Path};

use anyhow::{anyhow, bail};
use byteorder::{ReadBytesExt, LE};
use xmas_elf::{
    header,
    sections::SectionData,
    symbol_table::{Entry, Type},
    ElfFile,
};

/// Functions found after analyzing an executable
#[derive(Clone, Debug)]
pub struct Functions<'a> {
    /// Whether the addresses of these functions are 32-bit or 64-bit
    pub have_32_bit_addresses: bool,

    /// "undefined" symbols, symbols that need to be dynamically loaded
    pub undefined: HashSet<&'a str>,

    /// "defined" symbols, symbols with known locations (addresses)
    pub defined: BTreeMap<u64, Function<'a>>,
}

/// A symbol that represents a function (subroutine)
#[derive(Clone, Debug)]
pub struct Function<'a> {
    names: Vec<&'a str>,
    size: u64,
    stack: Option<u64>,
}

impl<'a> Function<'a> {
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

// is this symbol a tag used to delimit code / data sections within a subroutine?
fn is_tag(name: &str) -> bool {
    name == "$a" || name == "$t" || name == "$d" || {
        (name.starts_with("$a.") || name.starts_with("$d.") || name.starts_with("$t."))
            && name.splitn(2, '.').nth(1).unwrap().parse::<u64>().is_ok()
    }
}

fn process_symtab_obj<'a, E>(
    entries: &'a [E],
    elf: &ElfFile<'a>,
) -> anyhow::Result<
    (
        BTreeMap<u16, BTreeMap<u64, HashSet<&'a str>>>,
        BTreeMap<u32, u16>,
    )
>
where
    E: Entry,
{
    let mut names: BTreeMap<_, BTreeMap<_, HashSet<_>>> = BTreeMap::new();
    let mut shndxs = BTreeMap::new();

    for (entry, i) in entries.iter().zip(0..) {
        let name = entry.get_name(elf);
        let shndx = entry.shndx();
        let addr = entry.value() & !1; // clear the thumb bit
        let ty = entry.get_type();

        if shndx != 0 {
            shndxs.insert(i, shndx);
        }

        if ty == Ok(Type::Func)
            || (ty == Ok(Type::NoType)
                && name
                    .map(|name| !name.is_empty() && !is_tag(name))
                    .unwrap_or(false))
        {
            let name = name.map_err(anyhow::Error::msg)?;

            names
                .entry(shndx)
                .or_default()
                .entry(addr)
                .or_default()
                .insert(name);
        }
    }

    Ok((names, shndxs))
}

/// Parses an *input* (AKA relocatable) object file (`.o`) and returns a list of symbols and their
/// stack usage
pub fn analyze_object(obj: &[u8]) -> anyhow::Result<HashMap<&str, u64>> {
    let elf = &ElfFile::new(obj).map_err(anyhow::Error::msg)?;

    if elf.header.pt2.type_().as_type() != header::Type::Relocatable {
        bail!("object file is not relocatable")
    }

    // shndx -> (address -> [symbol-name])
    let mut is_64_bit = false;
    let (shndx2names, symtab2shndx) = match elf
        .find_section_by_name(".symtab")
        .ok_or_else(|| anyhow!("`.symtab` section not found"))?
        .get_data(elf)
    {
        Ok(SectionData::SymbolTable32(entries)) => process_symtab_obj(entries, elf)?,

        Ok(SectionData::SymbolTable64(entries)) => {
            is_64_bit = true;
            process_symtab_obj(entries, elf)?
        }

        _ => bail!("malformed .symtab section"),
    };

    let mut sizes = HashMap::new();
    let mut sections = elf.section_iter();
    while let Some(section) = sections.next() {
        if section.get_name(elf) == Ok(".stack_sizes") {
            let mut stack_sizes = Cursor::new(section.raw_data(elf));

            // next section should be `.rel.stack_sizes` or `.rela.stack_sizes`
            // XXX should we check the section name?
            let relocs: Vec<_> = match sections
                .next()
                .and_then(|section| section.get_data(elf).ok())
            {
                Some(SectionData::Rel32(rels)) if !is_64_bit => rels
                    .iter()
                    .map(|rel| rel.get_symbol_table_index())
                    .collect(),

                Some(SectionData::Rela32(relas)) if !is_64_bit => relas
                    .iter()
                    .map(|rel| rel.get_symbol_table_index())
                    .collect(),

                Some(SectionData::Rel64(rels)) if is_64_bit => rels
                    .iter()
                    .map(|rel| rel.get_symbol_table_index())
                    .collect(),

                Some(SectionData::Rela64(relas)) if is_64_bit => relas
                    .iter()
                    .map(|rel| rel.get_symbol_table_index())
                    .collect(),

                _ => bail!("expected a section with relocation information after `.stack_sizes`"),
            };

            for index in relocs {
                let addr = if is_64_bit {
                    stack_sizes.read_u64::<LE>()?
                } else {
                    u64::from(stack_sizes.read_u32::<LE>()?)
                };
                let stack = leb128::read::unsigned(&mut stack_sizes).unwrap();

                let shndx = symtab2shndx[&index];
                let entries = shndx2names
                    .get(&(shndx as u16))
                    .unwrap_or_else(|| panic!("section header with index {} not found", shndx));

                assert!(sizes
                    .insert(
                        *entries
                            .get(&addr)
                            .unwrap_or_else(|| panic!(
                                "symbol with address {} not found at section {} ({:?})",
                                addr, shndx, entries
                            ))
                            .iter()
                            .next()
                            .unwrap(),
                        stack
                    )
                    .is_none());
            }

            if stack_sizes.position() != stack_sizes.get_ref().len() as u64 {
                bail!(
                    "the number of relocations doesn't match the number of `.stack_sizes` entries"
                );
            }
        }
    }

    Ok(sizes)
}

fn process_symtab_exec<'a, E>(
    entries: &'a [E],
    elf: &ElfFile<'a>,
) -> anyhow::Result<(HashSet<&'a str>, BTreeMap<u64, Function<'a>>)>
where
    E: Entry + core::fmt::Debug,
{
    let mut defined = BTreeMap::new();
    let mut maybe_aliases = BTreeMap::new();
    let mut undefined = HashSet::new();

    for entry in entries {
        let ty = entry.get_type();
        let value = entry.value();
        let size = entry.size();
        let name = entry.get_name(&elf);

        if ty == Ok(Type::Func) {
            let name = name.map_err(anyhow::Error::msg)?;

            if value == 0 && size == 0 {
                undefined.insert(name);
            } else {
                defined
                    .entry(value)
                    .or_insert(Function {
                        names: vec![],
                        size,
                        stack: None,
                    })
                    .names
                    .push(name);
            }
        } else if ty == Ok(Type::NoType) {
            if let Ok(name) = name {
                if !is_tag(name) {
                    maybe_aliases.entry(value).or_insert(vec![]).push(name);
                }
            }
        }
    }

    for (value, alias) in maybe_aliases {
        // try with the thumb bit both set and clear
        if let Some(sym) = defined.get_mut(&(value | 1)) {
            sym.names.extend(alias);
        } else if let Some(sym) = defined.get_mut(&(value & !1)) {
            sym.names.extend(alias);
        }
    }

    Ok((undefined, defined))
}

/// Parses an executable ELF file and returns a list of functions and their stack usage
pub fn analyze_executable(elf: &[u8]) -> anyhow::Result<Functions<'_>> {
    let elf = &ElfFile::new(elf).map_err(anyhow::Error::msg)?;

    let mut have_32_bit_addresses = false;
    let (undefined, mut defined) = if let Some(section) = elf.find_section_by_name(".symtab") {
        match section.get_data(elf).map_err(anyhow::Error::msg)? {
            SectionData::SymbolTable32(entries) => {
                have_32_bit_addresses = true;

                process_symtab_exec(entries, elf)?
            }

            SectionData::SymbolTable64(entries) => process_symtab_exec(entries, elf)?,
            _ => bail!("malformed .symtab section"),
        }
    } else {
        (HashSet::new(), BTreeMap::new())
    };

    if let Some(stack_sizes) = elf.find_section_by_name(".stack_sizes") {
        let data = stack_sizes.raw_data(elf);
        let end = data.len() as u64;
        let mut cursor = Cursor::new(data);

        while cursor.position() < end {
            let address = if have_32_bit_addresses {
                u64::from(cursor.read_u32::<LE>()?)
            } else {
                cursor.read_u64::<LE>()?
            };
            let stack = leb128::read::unsigned(&mut cursor)?;

            // NOTE try with the thumb bit both set and clear
            if let Some(sym) = defined.get_mut(&(address | 1)) {
                sym.stack = Some(stack);
            } else if let Some(sym) = defined.get_mut(&(address & !1)) {
                sym.stack = Some(stack);
            } else {
                unreachable!()
            }
        }
    }

    Ok(Functions {
        have_32_bit_addresses,
        defined,
        undefined,
    })
}

#[cfg(feature = "tools")]
#[doc(hidden)]
pub fn run_exec(exec: &Path, obj: &Path) -> anyhow::Result<()> {
    let exec = &fs::read(exec)?;
    let obj = &fs::read(obj)?;

    let stack_sizes = analyze_object(obj)?;
    let symbols = analyze_executable(exec)?;

    if symbols.have_32_bit_addresses {
        // 32-bit address space
        println!("address\t\tstack\tname");

        for (addr, sym) in symbols.defined {
            let stack = sym
                .names()
                .iter()
                .filter_map(|name| stack_sizes.get(name))
                .next();

            if let (Some(name), Some(stack)) = (sym.names().first(), stack) {
                println!(
                    "{:#010x}\t{}\t{}",
                    addr,
                    stack,
                    rustc_demangle::demangle(name)
                );
            }
        }
    } else {
        // 64-bit address space
        println!("address\t\t\tstack\tname");

        for (addr, sym) in symbols.defined {
            let stack = sym
                .names()
                .iter()
                .filter_map(|name| stack_sizes.get(name))
                .next();

            if let (Some(name), Some(stack)) = (sym.names().first(), stack) {
                println!(
                    "{:#018x}\t{}\t{}",
                    addr,
                    stack,
                    rustc_demangle::demangle(name)
                );
            }
        }
    }

    Ok(())
}

#[cfg(feature = "tools")]
#[doc(hidden)]
pub fn run(path: &Path) -> anyhow::Result<()> {
    let bytes = &fs::read(path)?;
    let elf = &ElfFile::new(bytes).map_err(anyhow::Error::msg)?;

    if elf.header.pt2.type_().as_type() == header::Type::Relocatable {
        let symbols = analyze_object(bytes)?;

        if symbols.is_empty() {
            bail!("this object file contains no stack usage information");
        }

        println!("stack\tname");
        for (name, stack) in symbols {
            println!("{}\t{}", stack, rustc_demangle::demangle(name));
        }

        Ok(())
    } else {
        let symbols = analyze_executable(bytes)?;

        if symbols
            .defined
            .values()
            .all(|symbol| symbol.stack().is_none())
        {
            bail!("this executable contains no stack usage information");
        }

        if symbols.have_32_bit_addresses {
            // 32-bit address space
            println!("address\t\tstack\tname");

            for (addr, sym) in symbols.defined {
                if let (Some(name), Some(stack)) = (sym.names().first(), sym.stack()) {
                    println!(
                        "{:#010x}\t{}\t{}",
                        addr,
                        stack,
                        rustc_demangle::demangle(name)
                    );
                }
            }
        } else {
            // 64-bit address space
            println!("address\t\t\tstack\tname");

            for (addr, sym) in symbols.defined {
                if let (Some(name), Some(stack)) = (sym.names().first(), sym.stack()) {
                    println!(
                        "{:#018x}\t{}\t{}",
                        addr,
                        stack,
                        rustc_demangle::demangle(name)
                    );
                }
            }
        }

        Ok(())
    }
}
