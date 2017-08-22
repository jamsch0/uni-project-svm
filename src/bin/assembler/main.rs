#![feature(stmt_expr_attributes)]

extern crate clap;
#[macro_use]
extern crate nom;
extern crate svm;

mod parser;

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

use clap::{App, Arg};

macro_rules! exit {
    ($($arg: tt)*) => {
        #[allow(unused_must_use)]
        {
            writeln!(io::stderr(), $($arg)*);
            process::exit(1);
        }
    }
}

fn process_file(path: &Path) -> Result<Vec<u8>, io::Error> {
    let mut buf = String::new();
    File::open(path)?.read_to_string(&mut buf)?;
    parser::parse(buf).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

fn save_bytes(path: &Path, bytes: Vec<u8>) -> Result<(), io::Error> {
    File::create(path)?.write_all(&bytes)
}

fn main() {
    let matches = App::new("Simple Virtual Machine Assembler")
                          .version("0.1.0")
                          .author("James Chapman <james.chapman2@mail.bcu.ac.uk>")
                          .about("Assembler for Simple Virtual Machine")
                          .arg(Arg::with_name("output")
                              .short("o")
                              .long("output")
                              .value_name("FILE")
                              .help("Set an output file name")
                              .takes_value(true))
                          .arg(Arg::with_name("FILE")
                              .help("The assembly file to process")
                              .required(true))
                          .get_matches();

    let input_filename = matches.value_of("FILE").unwrap();
    let input = Path::new(input_filename);

    process_file(&input).and_then(|bytes| {
        let output_filename = matches.value_of("output")
                                     .unwrap_or_else(|| match &input_filename[input_filename.len() - 5..] {
                                         ".sasm" => &input_filename[..input_filename.len() - 5],
                                         _ => &input_filename[..]
                                     });
        let output = Path::new(output_filename);

        save_bytes(output, bytes)
    }).unwrap_or_else(|error| exit!("sasm: {}", error));
}
