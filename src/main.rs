extern crate byteorder;
extern crate clap;
#[macro_use]
extern crate failure;
extern crate leb128;
extern crate rustc_demangle;
extern crate xmas_elf;

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::process;

use byteorder::{ReadBytesExt, LE};
use clap::{App, Arg};
use failure::Error;
use xmas_elf::sections::{SectionData, SectionHeader};
use xmas_elf::symbol_table::{Entry, Type};
use xmas_elf::ElfFile;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let matches = App::new("stack-sizes")
        .arg(
            Arg::with_name("ELF")
                .help("ELF file to analyze")
                .required(true)
                .index(1),
        )
        .get_matches();

    let path = matches.value_of("ELF").unwrap();
    let mut bytes = vec![];
    File::open(path)?.read_to_end(&mut bytes)?;

    let elf = ElfFile::new(&bytes).map_err(failure::err_msg)?;

    let mut func_names = HashMap::new();

    if let Some(section) = elf.find_section_by_name(".symtab") {
        match section.get_data(&elf).map_err(failure::err_msg)? {
            SectionData::SymbolTable32(entries) => {
                for entry in entries {
                    if entry.get_type() == Ok(Type::Func) {
                        func_names.insert(
                            entry.value() as u64,
                            entry.get_name(&elf).map_err(failure::err_msg)?,
                        );
                    }
                }
            }
            SectionData::SymbolTable64(entries) => {
                for entry in entries {
                    if entry.get_type() == Ok(Type::Func) {
                        func_names.insert(
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
            if !data.is_empty() {
                println!("address\t\tsize\tname");
            }

            while cursor.position() < end {
                let addr = cursor.read_u32::<LE>()?;
                let name = func_names[&(addr as u64)];
                let sz = leb128::read::unsigned(&mut cursor)?;

                println!("{:#08x}\t{}\t{}", addr, sz, rustc_demangle::demangle(name));
            }
        }
        SectionHeader::Sh64(..) => {
            if !data.is_empty() {
                println!("address\t\t\tsize\tname");
            }

            while cursor.position() < end {
                let addr = cursor.read_u64::<LE>()?;
                let name = func_names[&addr];
                let sz = leb128::read::unsigned(&mut cursor)?;

                println!("{:#016x}\t{}\t{}", addr, sz, rustc_demangle::demangle(name));
            }
        }
    }

    Ok(())
}
