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

impl Bom {
    /// This func will manage the root bom file. Find all included bom files, and manage files defined in each bom file
    pub fn manage_top_bom(
        &self,
        bom_file: &str,
        root_dir: &str,
        dry_run: bool,
        included_dirs: &Vec<String>,
    ) {
        // We need to keep the order of boms and bom managements
        let mut sorted_boms = Vec::new();
        let mut bom_managements = Vec::new();
        // find all included boms
        let all_included_boms = find_all_included_bom_files(bom_file, included_dirs);
        for included_bom in all_included_boms.iter() {
            let bom = Bom::from_yaml_file(included_bom);
            sorted_boms.push(bom.clone());
            let bom_management = bom.get_bom_management(root_dir);
            bom_managements.push(bom_management);
        }
        // remove redundant operations in each bom management
        remove_redundant(&mut bom_managements);
        // Since we have different copy options for each bom, we cannot copy all targets together.
        let mut bom_managements_iter = bom_managements.into_iter();
        for bom in sorted_boms.into_iter() {
            // each bom corresponds to a bom management, so the unwrap will never fail
            bom.manage_self(bom_managements_iter.next().unwrap(), dry_run);
        }
    }

    /// This func will only manage the current bom file without finding included bom files
    pub fn manage_self(self, bom_management: BomManagement, dry_run: bool) {
        let excludes = self.excludes.unwrap_or(Vec::new());
        bom_management.manage(dry_run, excludes);
    }

    /// This func will return all operations in one bom
    fn get_bom_management(self, root_dir: &str) -> BomManagement {
        let mut bom_management = BomManagement::default();
        bom_management.dirs_to_make.push(root_dir.to_string()); // init root dir
        if let Some(ref targets) = self.targets {
            for target in targets {
                let target_management = target.get_target_management(root_dir);
                bom_management.add_target_management(target_management, root_dir);
            }
        }
        bom_management
    }

    /// init a bom from a yaml string
    fn from_yaml_string(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// init a bom from a yaml file
    pub fn from_yaml_file(filename: &str) -> Self {
        let file_content = std::fs::read_to_string(filename).unwrap_or_else(|e| {
            error!("cannot read bom file {}. {}.", filename, e);
            std::process::exit(FILE_NOT_EXISTS_ERROR);
        });
        Bom::from_yaml_string(&file_content).unwrap_or_else(|e| {
            error!("{} is not a valid bom file. {}.", filename, e);
            std::process::exit(INVALID_BOM_FILE_ERROR);
        })
    }
}

impl BomManagement {
    fn add_target_management(&mut self, mut target_management: TargetManagement, root_dir: &str) {
        // First, we need to resolve environmental variables
        target_management.resolve_environmental_variables();
        let TargetManagement {
            dirs_to_make,
            links_to_create,
            dirs_to_copy,
            files_to_copy,
            files_autodep,
        } = target_management;
        self.dirs_to_make.extend(dirs_to_make.into_iter());
        self.links_to_create.extend(links_to_create.into_iter());
        self.dirs_to_copy.extend(dirs_to_copy.into_iter());
        self.files_to_copy.extend(files_to_copy.into_iter());
        self.autodep(files_autodep, root_dir);
    }

    // do real jobs
    // mkdirs, create links, copy dirs, copy files(including shared objects)
    fn manage(&self, dry_run: bool, excludes: Vec<String>) {
        let BomManagement {
            dirs_to_make,
            links_to_create,
            dirs_to_copy,
            files_to_copy,
            shared_objects_to_copy,
        } = self;
        dirs_to_make.iter().for_each(|dir| mkdir(dir, dry_run));
        links_to_create
            .iter()
            .for_each(|(src, linkname)| create_link(src, linkname, dry_run));
        dirs_to_copy
            .iter()
            .for_each(|(src, dest)| copy_dir(src, dest, dry_run, &excludes));
        files_to_copy
            .iter()
            .for_each(|(src, dest)| copy_file(src, dest, dry_run));
        shared_objects_to_copy
            .iter()
            .for_each(|(src, dest)| copy_shared_object(src, dest, dry_run));
    }
}
