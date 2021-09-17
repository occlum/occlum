use crate::error::{
    COPY_DIR_ERROR, COPY_FILE_ERROR, CREATE_DIR_ERROR, CREATE_SYMLINK_ERROR, FILE_NOT_EXISTS_ERROR,
    INCORRECT_HASH_ERROR,
};
use data_encoding::HEXUPPER;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::vec;

/// convert a dest path(usually absolute) to a dest path in root directory
pub fn dest_in_root(root_dir: &str, dest: &str) -> PathBuf {
    let root_path = PathBuf::from(root_dir);
    let dest_path = PathBuf::from(dest);
    let dest_relative = if dest_path.is_absolute() {
        PathBuf::from(dest_path.strip_prefix("/").unwrap())
    } else {
        dest_path
    };
    return root_path.join(dest_relative);
}

/// check if hash of the file is equal to the passed hash value.
pub fn check_file_hash(filename: &str, hash: &str) {
    let file_hash = calculate_file_hash(filename);
    if file_hash != hash.to_string() {
        error!(
            "The hash value of {} should be {:?}. Please correct it.",
            filename, file_hash
        );
        std::process::exit(INCORRECT_HASH_ERROR);
    }
}

/// Use sha256 to calculate hash for file content. The returned hash is a hex-encoded string.
pub fn calculate_file_hash(filename: &str) -> String {
    let mut file = std::fs::File::open(filename).unwrap_or_else(|e| {
        println!("can not open file {}. {}", filename, e);
        std::process::exit(FILE_NOT_EXISTS_ERROR);
    });
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).unwrap();
    let hash = hasher.finalize();
    let hash = HEXUPPER.encode(&hash);
    hash
}

/// find an included file in the file system. If we can find the bom file, return the path
/// otherwise, the process exit with error
/// if included dir is relative path, if will be viewed as path relative to the `current` path (where we execute command)
pub fn find_included_bom_file(
    included_file: &str,
    bom_file: &str,
    included_dirs: &Vec<String>,
) -> String {
    let bom_file_path = PathBuf::from(bom_file);
    let bom_file_dir_path = bom_file_path
        .parent()
        .map_or(PathBuf::from("."), |p| p.to_path_buf());
    // first, we find the included bom file in the current dir of the bom file
    let included_file_path = bom_file_dir_path.join(included_file);
    if included_file_path.is_file() {
        return included_file_path.to_string_lossy().to_string();
    }
    // Then, we find the bom file in each included dir.
    for included_dir in included_dirs {
        let included_dir_path = std::env::current_dir().unwrap().join(included_dir);
        let included_file_path = included_dir_path.join(included_file);
        if included_file_path.is_file() {
            return included_file_path.to_string_lossy().to_string();
        }
    }
    // fail to find the bom file
    error!(
        "cannot find included bom file {} in {}.",
        included_file, bom_file
    );
    std::process::exit(FILE_NOT_EXISTS_ERROR);
}

