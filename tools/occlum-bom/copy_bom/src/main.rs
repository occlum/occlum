use std::{path::PathBuf, process::Command};

use bom_core::BomFile;
use clap::{App, Arg};

#[derive(Debug, Clone)]
struct CopyBomOption {
    bom_file_path: String,
}

/// parse command line options
fn parse_option() -> CopyBomOption {
    let arg_matches = App::new("copy-bom")
        .version("v0.1")
        .about("copy files described in a bom file")
        .arg(
            Arg::with_name("bom-file-path")
                .short("f")
                .long("file")
                .required(true)
                .takes_value(true)
                .help("Set the bom file path"),
        )
        .get_matches();
    let bom_file_name = match arg_matches.value_of("bom-file-path") {
        None => unreachable!(),
        Some(bom_file_name) => bom_file_name.to_string(),
    };

    CopyBomOption {
        bom_file_path: bom_file_name,
    }
}

/// Copy files discribed in bom files to the output directory
/// This function don't take include files into account
fn copy_one_bom_file(bom_file_path: &str) {
    let bom_file = BomFile::from_toml_file(bom_file_path);
    bom_file.check_validity_recursive(bom_file_path);
    let (create_dirs, copy_dirs, copy_files) = bom_file.get_copied_files(bom_file_path);

    // create dirs
    for create_dir in create_dirs.iter() {
        let dir_path = PathBuf::from(create_dir);
        if dir_path.is_dir() {
            continue;
        }
        println!("create dir: {}", create_dir);
        std::fs::create_dir_all(&dir_path)
            .expect(format!("Create directory {:?} failed.", dir_path).as_str());
    }

    //copy dirs
    for (from, destination) in copy_dirs.iter() {
        let destination = PathBuf::from(destination);
        std::fs::create_dir_all(&destination)
            .expect(format!("Create directory {:?} failed.", destination).as_str());
        let destination = destination.parent().unwrap();
        println!("Copy directory {:?} to {:?}", from, destination);
        Command::new("rsync")
            .arg("-aL")
            .arg(from)
            .arg(destination.as_os_str())
            .output()
            .expect(format!("Copy {:?} to {:?} failed.", from, destination).as_str());
    }

    //copy files
    for (from, destination) in copy_files.iter() {
        let destination = PathBuf::from(destination);
        println!("Copy {:?} to: {:?}", from, destination);
        Command::new("rsync")
            .arg("-aL")
            .arg(from)
            .arg(destination.as_os_str())
            .output()
            .expect(format!("Copy {:?} to {:?} failed.", from, destination).as_str());
    }
}

/// copy bom file and included bom files
fn copy_files(copy_bom_option: &CopyBomOption) {
    let bom_file_path = copy_bom_option.bom_file_path.clone();
    let bom_file = BomFile::from_toml_file(&bom_file_path);
    let mut all_bom_paths = bom_file.get_included_bom_files(&bom_file_path);
    all_bom_paths.insert(bom_file_path);
    for bom_file_path in all_bom_paths.iter() {
        copy_one_bom_file(&bom_file_path);
    }
}

fn main() {
    let copy_bom_option = parse_option();
    copy_files(&copy_bom_option);
}
