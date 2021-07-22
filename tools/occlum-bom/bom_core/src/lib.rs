use data_encoding::HEXUPPER;
use regex::{Captures, Regex};
use serde_derive::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::hash::Hash;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
extern crate toml;
#[macro_use]
extern crate lazy_static;

/// depenedency pattern presented by ldd
static DEPENDENCY_PATTERN: &'static str =
    r"(?P<name>.+) => (?P<path>.+) \(0x(?P<address>[0-9a-z]{16})\)";
/// where to put shared objects
static SHARED_OBJECT_OUTPUT_DIRECTORY: &'static str = "image/lib";

lazy_static! {
    static ref DEPENDENCY_REGEX: Regex = Regex::new(DEPENDENCY_PATTERN).unwrap();
}

/// Internal representation for a bom file
/// include: Included bom files
/// files: All files included in the file (include executables and non-executables)
/// directories: user-provided directoires(not recursive)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Hash)]
pub struct BomFile {
    include: Option<Vec<String>>,
    files: Option<Vec<NormalFile>>,
    directories: Option<Vec<Directory>>,
}

/// Normal file (can be executable but not directory).
/// Hash is None if we don't want to check the consistency of the file content
/// Target_executable id Some(true) if we want to find dependencies for this file
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Hash)]
struct NormalFile {
    path: String,
    hash: Option<String>,
    output_path: String,
    target_executable: Option<bool>,
}

/// The directory node in a bom file
/// path: the directory full path
/// output path: where to put the directory
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Hash)]
struct Directory {
    path: Option<String>,
    output_path: String,
}

/// SharedObject represents the dependencies of target executable
/// Name: name of the shared library
/// Path: the actual path of the shared_library
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SharedObject {
    name: String,
    path: String,
    output_directory: String,
}

impl NormalFile {
    fn new(
        path: String,
        hash: Option<String>,
        output_directory: String,
        target_executable: Option<bool>,
    ) -> Self {
        NormalFile {
            path,
            hash,
            target_executable,
            output_path: output_directory,
        }
    }

    fn find_dependencies(&self) -> HashSet<SharedObject> {
        if self.is_target_executable() {
            find_dependencies_for_executable(&self.path)
                .into_iter()
                .collect()
        } else {
            HashSet::new()
        }
    }

    fn check_validity(&self, reference_file_path: &str) {
        let file_name = self.path.clone();
        let file_path = convert_to_relative_path_from_current_dir(reference_file_path, &file_name);
        if !check_file_exist(&file_path) {
            println!(
                "File {} does not exist. Please update the bom file.",
                file_path
            );
            std::process::exit(-3);
        }
        if let Some(hash) = self.hash.as_ref() {
            let new_hash = calculate_file_hash(&file_path);
            if new_hash.as_str() != hash {
                println!("The content of File {} changes. The new hash value is {}. Please update the bom file.", file_path, new_hash);
                std::process::exit(-4);
            }
        }
    }

    fn is_target_executable(&self) -> bool {
        if let Some(executable) = self.target_executable {
            executable
        } else {
            false
        }
    }
}

impl Directory {
    fn new(path: Option<String>, output_path: String) -> Self {
        Directory { path, output_path }
    }

    fn check_validity(&self, reference_file_path: &str) {
        if let Some(directory_name) = self.path.as_ref() {
            let directory_path =
                convert_to_relative_path_from_current_dir(reference_file_path, directory_name);
            if !check_directory_exist(&directory_path) {
                println!(
                    "Directory {} does not exist. Please update the bom file.",
                    directory_path
                );
                std::process::exit(-2);
            }
        }
    }
}

impl SharedObject {
    fn new(name: String, path: String) -> Self {
        SharedObject {
            name,
            path,
            output_directory: SHARED_OBJECT_OUTPUT_DIRECTORY.to_string(),
        }
    }
}

impl BomFile {
    /// init an empty BomFile
    pub fn new() -> Self {
        BomFile {
            include: None,
            files: None,
            directories: None,
        }
    }

    /// Add normal files(not directories) to bom file,
    pub fn add_file(
        &mut self,
        filename: &str,
        executable: Option<bool>,
        output_directory: &str,
        with_hash: bool,
    ) {
        let output_directory = get_output_path(filename, output_directory);
        let path = filename.to_string();
        let hash = if with_hash {
            Some(calculate_file_hash(filename))
        } else {
            None
        };
        let normal_file =
            NormalFile::new(path.clone(), hash, output_directory.to_string(), executable);

        if self.files == None {
            let mut files = Vec::new();
            files.push(normal_file);
            self.files = Some(files);
        } else {
            for files in self.files.iter_mut() {
                files.push(normal_file.clone());
            }
        }
    }

    /// Add directory (not recursive)
    pub fn add_directory(&mut self, directory_name: &str, output_directory: &str) {
        let output_directory = get_output_path(directory_name, output_directory);
        let path = directory_name.to_string();
        let directory = Directory::new(Some(path.clone()), output_directory);

        if self.directories == None {
            let mut directories = Vec::new();
            directories.push(directory);
            self.directories = Some(directories);
        } else {
            for directories in self.directories.iter_mut() {
                directories.push(directory.clone());
            }
        }
    }

    /// check whether a BomFile is valid. This check does three jobs
    /// (1) check if each included bom file, file and directory exists
    /// (2) check if the hash of each file changes(if hash is not None).
    /// if some file does not exist or the hash value changes, the bom file is invalid.
    fn check_validity(&self, bom_file_path: &str) {
        // check included filenames
        if let Some(include) = self.include.as_ref() {
            for include_filename in include {
                let include_filename =
                    convert_to_relative_path_from_current_dir(bom_file_path, include_filename);
                if !check_file_exist(&include_filename) {
                    println!(
                        "Include file {} does not exist. Please update the bom file.",
                        include_filename
                    );
                    std::process::exit(-1);
                }
            }
        }
        //check directories
        if let Some(directories) = self.directories.as_ref() {
            for directory in directories {
                directory.check_validity(bom_file_path);
            }
        }
        //check file
        if let Some(files) = self.files.as_ref() {
            for file in files {
                file.check_validity(bom_file_path);
            }
        }
    }

    /// check if a bomfile is valid resursively(check all included bom files)
    pub fn check_validity_recursive(&self, bom_file_path: &str) {
        self.check_validity(bom_file_path);
        let include_files = self.get_included_bom_files(bom_file_path);
        for include_file in include_files {
            let bom_file = BomFile::from_toml_file(&include_file);
            bom_file.check_validity(&include_file);
        }
    }

    /// convert bom file to a toml string
    fn to_toml_string(&self) -> String {
        toml::to_string(&self).unwrap()
    }

    /// write to a bom file
    pub fn write_toml_file(&self, output_filename: &str) {
        let output_filename = PathBuf::from(&output_filename);
        let mut output_file = ensure_empty_file(&output_filename);
        output_file
            .write_all(self.to_toml_string().as_bytes())
            .expect(format!("Error: Write file {:?}", output_filename).as_str());
    }

    /// read a bom file from a toml string
    fn from_toml_string(input: &str) -> Self {
        toml::from_str(input).unwrap()
    }

    /// read a bom file from a toml file
    pub fn from_toml_file(file_name: &str) -> Self {
        let mut file =
            File::open(file_name).expect(format!("Can't open file {}", file_name).as_str());
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .expect(format!("Can't read file {}", file_name).as_str());
        BomFile::from_toml_string(&buf)
    }

    /// Determine which files need to be copied. This file won't find files in included bom files
    /// bom_file_path: path of the bom file in file system
    /// Return value: HashSet<String>: The path of directories to create;
    /// HashSet<(String, String)>: The from path and to path of directories to copy
    /// HashSet<(String, String)>: The from path and to directory of files to copy
    pub fn get_copied_files(
        &self,
        bom_file_path: &str,
    ) -> (
        HashSet<String>,
        HashSet<(String, String)>,
        HashSet<(String, String)>,
    ) {
        let mut copy_files = HashSet::new();
        let mut create_dirs = HashSet::new();
        let mut copy_dirs = HashSet::new();
        let mut shared_objects = HashSet::new();

        if let Some(files) = &self.files {
            for normal_file in files.iter() {
                let absolute_from_path = convert_to_relative_path_from_current_dir(
                    bom_file_path,
                    normal_file.path.as_str(),
                );
                //println!("absolute_from_path: {}", absolute_from_path);
                let absolute_to_path = convert_to_relative_path_from_current_dir(
                    bom_file_path,
                    normal_file.output_path.as_str(),
                );
                //println!("absolute_to_path: {}", absolute_to_path);
                copy_files.insert((absolute_from_path, absolute_to_path));
                if normal_file.is_target_executable() {
                    let dependencies = normal_file.find_dependencies();
                    for shared_object in dependencies.into_iter() {
                        shared_objects.insert(shared_object);
                    }
                }
            }
        }
        if let Some(directories) = &self.directories {
            for directory in directories.iter() {
                let absolute_to_path = convert_to_relative_path_from_current_dir(
                    bom_file_path,
                    directory.output_path.as_str(),
                );
                if let Some(path) = directory.path.as_ref() {
                    let absolute_from_path =
                        convert_to_relative_path_from_current_dir(bom_file_path, path);
                    copy_dirs.insert((absolute_from_path, absolute_to_path));
                } else {
                    create_dirs.insert(absolute_to_path);
                }
            }
        }
        for shared_object in shared_objects.into_iter() {
            let output_directory =
                get_output_path(&shared_object.path, &shared_object.output_directory);
            copy_files.insert((shared_object.path, output_directory));
        }

        (create_dirs, copy_dirs, copy_files)
    }

    /// Update bom file due to file content changes
    /// Only update files with hash value
    pub fn update(&mut self) {
        if let Some(files) = self.files.as_mut() {
            for normal_file in files.iter_mut() {
                if normal_file.hash != None {
                    let new_hash = calculate_file_hash(&normal_file.path);
                    normal_file.hash = Some(new_hash);
                }
            }
        }
    }

    /// include other bom file in this file
    pub fn include_other_bom_file(&mut self, other_bom_file_name: &str) {
        //println!("to include: {}", other_bom_file.to_string());
        if self.include == None {
            let mut include_files = Vec::new();
            include_files.push(other_bom_file_name.to_string());
            self.include = Some(include_files);
        } else {
            for include_files in self.include.iter_mut() {
                include_files.push(other_bom_file_name.to_string());
            }
        }
    }

    /// get included bom files recursively
    pub fn get_included_bom_files(&self, bom_file_path: &str) -> HashSet<String> {
        let mut bom_filenames = HashSet::new();
        if let Some(include_files) = self.include.as_ref() {
            for include_file in include_files {
                let relative_include_file_path =
                    convert_to_relative_path_from_current_dir(bom_file_path, include_file);
                bom_filenames.insert(relative_include_file_path);
            }
        }

        let mut include_filenames = HashSet::new();
        loop {
            let init_size = bom_filenames.len();
            include_filenames.clear();
            for bom_filename in bom_filenames.iter() {
                let bom_file = BomFile::from_toml_file(bom_filename);
                if let Some(includes) = bom_file.include {
                    for include_filename in includes.iter() {
                        let relative_include_filepath = convert_to_relative_path_from_current_dir(
                            bom_filename,
                            include_filename,
                        );
                        include_filenames.insert(relative_include_filepath);
                    }
                }
            }
            for include_filename in include_filenames.iter() {
                bom_filenames.insert(include_filename.clone());
            }
            let last_size = bom_filenames.len();
            if init_size == last_size {
                break;
            }
        }
        bom_filenames
    }
}

/// Use sha256 to calculate hash for file content. The returned hash is a hex-encoded string.
fn calculate_file_hash(filename: &str) -> String {
    let mut file = File::open(filename).unwrap();
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).unwrap();
    let hash = hasher.finalize();
    let hash = HEXUPPER.encode(&hash);
    hash
}

/// Find matched str in text from start_index
fn find_matched_str<'a>(line: &'a str, regex_: &Regex, start_index: &mut usize) -> Option<&'a str> {
    let match_ = regex_.find_at(line, *start_index);
    let res = {
        if let Some(match_str) = match_ {
            *start_index = match_str.start();
            match_str.as_str()
        } else {
            return None;
        }
    };
    *start_index = *start_index + res.len();
    Some(res)
}

/// read matched pattern from captures
fn read_content_from_text<T>(caps: &Captures, content_name: &str) -> T
where
    T: FromStr,
{
    (&caps[content_name]).to_string().parse::<T>().ok().unwrap()
}

/// Create an empty file. If there already exists some file or directory, it will be deleted.
fn ensure_empty_file(path: &PathBuf) -> File {
    if path.is_dir() {
        fs::remove_dir_all(&path).unwrap();
    }
    if path.is_file() {
        fs::remove_file(&path).unwrap();
    }
    fs::File::create(path).unwrap()
}

/// find dependent shared objects for executables. This is done by analyze the result of `ldd`.
fn find_dependencies_for_executable(filename: &str) -> Vec<SharedObject> {
    let output = Command::new("ldd").arg(filename).output().unwrap().stdout;
    let output_string = String::from_utf8(output).unwrap();
    let lines = output_string.split("\n").collect::<Vec<_>>();
    let mut shared_objects = Vec::new();
    for line in lines {
        let line = line.trim();
        let matched_content = find_matched_str(line, &DEPENDENCY_REGEX, &mut 0);
        if let Some(matched_content) = matched_content {
            let caps = DEPENDENCY_REGEX.captures(matched_content).unwrap();
            let name = read_content_from_text::<String>(&caps, "name");
            let path = read_content_from_text::<String>(&caps, "path");
            let (name, path) = shared_object_prefer_musl(name, path);
            let shared_object = SharedObject::new(name, path);
            shared_objects.push(shared_object);
        }
    }
    shared_objects
}

/// given file to copy and output directory, get the output path
fn get_output_path(file_full_path: &str, output_directory: &str) -> String {
    let file_path = PathBuf::from(file_full_path);
    let file_name = file_path.file_name().unwrap().to_str().unwrap();
    let output_path = PathBuf::from(output_directory).join(file_name);
    output_path.to_str().unwrap().to_string()
}

/// if we find libraries in the musl lib path, we will copy so file from musl lib path instead of the ldd result
fn shared_object_prefer_musl(name: String, path: String) -> (String, String) {
    lazy_static! {
        static ref MUSL_LIB: HashSet<&'static str> = {
            let mut m = HashSet::new();
            m.insert("libatomic.so");
            m.insert("libc.so");
            m.insert("libgomp.so");
            m.insert("libitm.so");
            m.insert("libquadmath.so");
            m.insert("libssp.so");
            m.insert("libstdc++.so");
            m.insert("libz.so");
            m
        };
    }
    static MUSL_LIB_ROOT: &'static str = "/opt/occlum/toolchains/gcc/x86_64-linux-musl";
    if MUSL_LIB.contains(name.as_str()) {
        let lib_path = PathBuf::from(MUSL_LIB_ROOT).join(name.as_str());
        (name.clone(), lib_path.to_str().unwrap().to_string())
    } else {
        (name, path)
    }
}

/// check whether a file exists in give path
fn check_file_exist(file_name: &str) -> bool {
    let path = PathBuf::from(file_name);
    if path.is_file() {
        true
    } else {
        false
    }
}

/// check whether a directory exists in give path
fn check_directory_exist(directory_name: &str) -> bool {
    let directory = PathBuf::from(directory_name);
    if directory.is_dir() {
        true
    } else {
        false
    }
}

/// convert a relative path from referencen file path to an relative path from current directory.
/// if the relative path is an absolute path ,this function will return the absolute path.
fn convert_to_relative_path_from_current_dir(
    reference_file_path: &str,
    relative_path: &str,
) -> String {
    let reference_file_path = PathBuf::from(reference_file_path);
    let reference_directory = reference_file_path.parent().unwrap();
    let relative_from_current = reference_directory.join(relative_path);
    relative_from_current.to_str().unwrap().to_string()
}
