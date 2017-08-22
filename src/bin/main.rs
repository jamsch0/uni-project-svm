#![feature(stmt_expr_attributes)]

extern crate clap;
extern crate svm;

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

use clap::{App, Arg};

use svm::VirtualMachine;

macro_rules! exit {
    ($($arg: tt)*) => {
        #[allow(unused_must_use)]
        {
            writeln!(io::stderr(), $($arg)*);
            process::exit(1);
        }
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, io::Error> {
    let mut file = File::open(path)?;

    if file.metadata()?.is_dir() {
        return Err(io::Error::new(io::ErrorKind::Other, "Is a directory"));
    }

    let mut vec = Vec::new();
    file.read_to_end(&mut vec)?;

    Ok(vec)
}

fn write_to_file(path: &Path, vm: &VirtualMachine) -> Result<(), io::Error> {
    let mut file = File::create(path)?;

    for page in (0..vm.memory.pages.len()).flat_map(|i| vm.memory.pages.get(i)) {
        file.write_all(page)?;
    }

    Ok(())
}

fn main() {
    let matches = App::new("Simple Virtual Machine")
                          .version("0.1.0")
                          .author("James Chapman <james.chapman2@mail.bcu.ac.uk>")
                          .about("A fast, low-level virtual machine")
                          .arg(Arg::with_name("page-size")
                              .short("p")
                              .long("page-size")
                              .value_name("SIZE")
                              .help("Set a custom page size")
                              .takes_value(true))
                          .arg(Arg::with_name("memory-dump")
                              .short("m")
                              .long("memory-dump")
                              .value_name("FILE")
                              .help("Dump memory to <FILE> on exit"))
                          .arg(Arg::with_name("FILE")
                              .help("The program to execute")
                              .required(true))
                          .arg(Arg::with_name("verbose")
                              .short("v")
                              .long("verbose")
                              .help("Use verbose output"))
                          .arg(Arg::with_name("breakpoints")
                              .short("b")
                              .long("enable-breakpoints")
                              .help("Enable triggering of breakpoints during execution"))
                          .get_matches();

    let path = Path::new(matches.value_of("FILE").unwrap());
    let program = read_file(&path).unwrap_or_else(|error| exit!("svm: {}: {}", path.display(), error));

    let vm = match matches.value_of("page-size") {
        Some(page_size) => {
            let page_size = page_size.parse().unwrap_or_else(|_| exit!("svm: invalid integer: {}", page_size));
            VirtualMachine::with_page_size(page_size, program)
        },
        None => VirtualMachine::new(program)
    };

    let verbose = matches.is_present("verbose");

    vm.and_then(|mut vm| {
        vm.verbose_output = verbose;
        vm.breakpoints_enabled = matches.is_present("breakpoints");

        vm.run().and_then(|exit_code| {
            if verbose {
                println!("svm: exiting with code {}", exit_code);
            }

            if let Some(path) = matches.value_of("memory-dump") {
                write_to_file(Path::new(path), &vm).unwrap_or_else(|error| exit!("svm: {}", error));
            }

            process::exit(exit_code);
        })
    }).unwrap_or_else(|error| exit!("svm: {}", error));
}
