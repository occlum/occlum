//! This file defines data structures to store contents in bom file
//! as well as all management on these strcutures.
//! Structures `Bom`, `Target`, `SymLink`, `Source`, `NormalFile`, `FileWithOption`
//! are used to parse the bom file.
//! Structures ending with `management` are used to define managements on different levels.
//! We will construct a BomManagement for each bom file (the top bom file and all included bom files).
//! Then do real file operations on each BomManagement
use crate::error::{FILE_NOT_EXISTS_ERROR, INVALID_BOM_FILE_ERROR};
use crate::util::{
    check_file_hash, copy_dir, copy_file, copy_shared_object, create_link, dest_in_root,
    find_dependent_shared_objects, find_included_bom_file, mkdir, resolve_envs,
};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::path::PathBuf;
use std::slice::Iter;

// The whole bom file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bom {
    pub includes: Option<Vec<String>>,
    pub excludes: Option<Vec<String>>,
    targets: Option<Vec<Target>>,
}

// target in a bom file.
// Each target represents the same destination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    target: String,
    mkdirs: Option<Vec<String>>,
    createlinks: Option<Vec<SymLink>>,
    copy: Option<Vec<Source>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymLink {
    src: String,
    linkname: String,
}

// source in a target.
// each Source has the same `from` directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    from: Option<String>,
    dirs: Option<Vec<String>>,
    files: Option<Vec<NormalFile>>,
}

// A file need to be copied
// It can be only a filename; or file with multiple options to enable checking hash, renaming file, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum NormalFile {
    FileName(String),
    FileWithOption(FileWithOption),
}

// A file with multiple optional options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileWithOption {
    name: String,
    hash: Option<String>,
    autodep: Option<bool>,
    rename: Option<String>,
}

/// all operations defined for one bom file
#[derive(Default)]
pub struct BomManagement {
    dirs_to_make: Vec<String>,
    links_to_create: Vec<(String, String)>,
    dirs_to_copy: Vec<(String, String)>,
    files_to_copy: Vec<(String, String)>,
    shared_objects_to_copy: Vec<(String, String)>,
}

/// all operations defined for one target
#[derive(Default)]
pub struct TargetManagement {
    dirs_to_make: Vec<String>,
    links_to_create: Vec<(String, String)>,
    dirs_to_copy: Vec<(String, String)>,
    files_to_copy: Vec<(String, String)>,
    files_autodep: Vec<String>,
}

/// all operations defined for one source
#[derive(Default)]
pub struct SourceManagement {
    dirs_to_copy: Vec<(String, String)>,
    files_to_copy: Vec<(String, String)>,
    files_autodep: Vec<String>,
}

