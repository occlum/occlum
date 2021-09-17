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

