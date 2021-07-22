use std::collections::HashSet;

use bom_core::BomFile;
use clap::{App, Arg};

#[derive(Debug, Clone)]
struct UpdateBomOption {
    bom_file_name: String,
    include_bom_files: HashSet<String>,
}

fn parse_option() -> UpdateBomOption {
    let arg_matches = App::new("copy-bom")
        .version("v0.1")
        .about("copy files described in a bom file")
        .arg(
            Arg::with_name("bom-file-name")
                .short("f")
                .long("file")
                .required(true)
                .takes_value(true)
                .help("Set the input bom file name"),
        )
        .arg(
            Arg::with_name("include-bom-files")
                .short("i")
                .long("include")
                .takes_value(true)
                .multiple(true)
                .help("Include other bom files"),
        )
        .get_matches();
    let bom_file_name = match arg_matches.value_of("bom-file-name") {
        None => unreachable!(),
        Some(bom_file_name) => bom_file_name.to_string(),
    };

    let mut include_bom_files = HashSet::new();
    if let Some(bom_files) = arg_matches.values_of("include-bom-files") {
        for bom_file in bom_files.into_iter() {
            include_bom_files.insert(bom_file.to_string());
        }
    }
    UpdateBomOption {
        bom_file_name,
        include_bom_files,
    }
}

fn update_bom_file(update_bom_option: &UpdateBomOption) {
    let mut bom_file = BomFile::from_toml_file(&update_bom_option.bom_file_name);
    bom_file.update();
    for include_bom_file in update_bom_option.include_bom_files.iter() {
        bom_file.include_other_bom_file(include_bom_file);
    }

    bom_file.write_toml_file(&update_bom_option.bom_file_name);
    println!("Update {:?} successfully", update_bom_option.bom_file_name);
}

fn main() {
    let update_bom_option = parse_option();
    update_bom_file(&update_bom_option);
}
