use std::collections::HashSet;
// use std::path::PathBuf;
// use std::fs;

use clap::{App, Arg, ArgMatches};
// use is_executable::IsExecutable;
use bom_core::BomFile;

#[derive(Debug, Clone)]
struct GenerateBomOption {
    output_directory: String,
    output_filename: String,
    input_filenames: HashSet<String>,
    input_directories: HashSet<String>,
    executable_filenames: HashSet<String>,
    include_bomfiles: HashSet<String>,
    with_hash: bool,
}

impl GenerateBomOption {
    /// convert GenerateBomOption to BomFile
    fn to_bom_file(&self) -> BomFile {
        let mut bom_file = BomFile::new();
        for filename in self.input_filenames.iter() {
            bom_file.add_file(
                filename,
                None,
                self.output_directory.as_str(),
                self.with_hash,
            );
        }
        for directory in self.input_directories.iter() {
            bom_file.add_directory(directory, self.output_directory.as_str());
        }
        for executable in self.executable_filenames.iter() {
            bom_file.add_file(
                executable,
                Some(true),
                self.output_directory.as_str(),
                self.with_hash,
            );
        }
        for include_bom_file in self.include_bomfiles.iter() {
            bom_file.include_other_bom_file(include_bom_file);
        }
        bom_file
    }
}

/// Command line options with clap
fn parse_option() -> GenerateBomOption {
    let arg_matches = App::new("generate-bom")
        .version("v0.1")
        .about("Generate bom file from command line arguments")
        .arg(
            Arg::with_name("output_directory")
                .long("directory")
                .short("d")
                .required(true)
                .takes_value(true)
                .help("The relative directory in occlum image"),
        )
        .arg(
            Arg::with_name("output_filename")
                .short("o")
                .long("output")
                .required(true)
                .takes_value(true)
                .help("Output bom filename"),
        )
        .arg(
            Arg::with_name("input_filenames")
                .short("f")
                .long("filename")
                .multiple(true)
                .takes_value(true)
                .help("Input filenames"),
        )
        .arg(
            Arg::with_name("input_directories")
                .short("r")
                .long("recursive")
                .multiple(true)
                .takes_value(true)
                .help("Input directories"),
        )
        .arg(
            Arg::with_name("executable_filenames")
                .short("e")
                .long("executable")
                .multiple(true)
                .takes_value(true)
                .help("Target executable filenames"),
        )
        .arg(
            Arg::with_name("include_bomfiles")
                .short("i")
                .long("include")
                .multiple(true)
                .takes_value(true)
                .help("Include other bom files"),
        )
        .arg(
            Arg::with_name("with_hash")
                .long("hash")
                .takes_value(false)
                .help("Set this flag will calculate hash value for each input file"),
        )
        .get_matches();
    parse_arg_matches(&arg_matches)
}

fn parse_arg_matches(arg_matches: &ArgMatches) -> GenerateBomOption {
    let output_directory = parse_single_value(arg_matches, "output_directory");
    let output_filename = parse_single_value(arg_matches, "output_filename");
    let input_filenames = parse_multiple_values(arg_matches, "input_filenames");
    let input_directories = parse_multiple_values(arg_matches, "input_directories");
    let executable_filenames = parse_multiple_values(arg_matches, "executable_filenames");
    let include_bomfiles = parse_multiple_values(arg_matches, "include_bomfiles");
    let with_hash = arg_matches.is_present("with_hash");

    GenerateBomOption {
        output_directory,
        output_filename,
        input_filenames,
        input_directories,
        executable_filenames,
        include_bomfiles,
        with_hash,
    }
}

// parse flags which present only once
fn parse_single_value(arg_matches: &ArgMatches, value_name: &str) -> String {
    match arg_matches.value_of(value_name) {
        None => unreachable!(),
        Some(output_directory) => output_directory.to_string(),
    }
}

// parse flags which present multiple times
fn parse_multiple_values(arg_matches: &ArgMatches, value_name: &str) -> HashSet<String> {
    match arg_matches.values_of(value_name) {
        None => HashSet::new(),
        Some(multiple_values) => {
            let mut res = HashSet::new();
            for value in multiple_values.into_iter() {
                res.insert(value.to_string());
            }
            res
        }
    }
}

fn write_bom_file(generate_bom_option: &GenerateBomOption, bom_file: &BomFile) {
    bom_file.write_toml_file(&generate_bom_option.output_filename);
}

fn main() {
    let generate_bom_option = parse_option();
    //let generate_bom_option = check_user_option(&generate_bom_option);
    let bom_file = generate_bom_option.to_bom_file();
    write_bom_file(&generate_bom_option, &bom_file);
}
