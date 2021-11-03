use crate::error::{
    COPY_DIR_ERROR, COPY_FILE_ERROR, CREATE_DIR_ERROR, CREATE_SYMLINK_ERROR, FILE_NOT_EXISTS_ERROR,
    INCORRECT_HASH_ERROR, 
};
use data_encoding::HEXUPPER;
use elf::types::{ET_DYN, ET_EXEC, Type};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::vec;

/// This structure represents loader information in config file.
/// `loader_paths` stores the actual path of each loader. key: the loader name, value: the loader path in host
/// `ld_library_path_envs` stores the LD_LIBRARY_PATH environmental variable.
/// We combine the loader dir and the user provided path to get the environmental variable.
/// `default_lib_dirs` stores all directories we parse from the LD_LIBRARY_PATH variable.
/// We use `default_lib_dirs` because we may not have the same LD_LIBRARY_PATH in occlum image.
/// When we use `occlum run`, the loader in occlum image won‘t try to find libraries in all libs in LD_LIBRARY_PATH.
/// So, we will copy libraries in these `default_lib_dirs` to the directory of loader.
#[derive(Debug)]
struct OcclumLoaders {
    loader_paths: HashMap<String, String>,
    ld_library_path_envs: HashMap<String, String>,
    default_lib_dirs: HashMap<String, Vec<String>>,
}

lazy_static! {
    /// This map stores the path of occlum-modified loaders.
    /// The `key` is the name of the loader. The `value` is the loader path.
    /// We read the loaders from the `LOADER_CONFIG_FILE`
    static ref OCCLUM_LOADERS: OcclumLoaders = {
        const LOADER_CONFIG_FILE: &'static str = "/opt/occlum/etc/template/occlum_elf_loader.config";
        let mut loader_paths = HashMap::new();
        let mut ld_library_path_envs = HashMap::new();
        let mut default_lib_dirs = HashMap::new();
        let config_path = PathBuf::from(LOADER_CONFIG_FILE);
        if !config_path.is_file() {
            // if no given config file is found, we will use the default loader in elf headers
            warn!("fail to find loader config file {}. No loader is set!", LOADER_CONFIG_FILE);
        } else {
            let file_content = std::fs::read_to_string(config_path).unwrap();
            for line in file_content.lines() {
                let trim_line = line.trim();
                if trim_line.len() <= 0 {
                    continue;
                }
                let line_split: Vec<_> = trim_line.split(' ').collect();
                // The first string is loader path
                let loader_path = line_split[0].to_string();
                let loader_path_buf = PathBuf::from(&loader_path);
                let loader_file_name = loader_path_buf.file_name().unwrap().to_string_lossy().to_string();
                // The second string plus the loader directory is LD_LIBRARY_PATH
                let loader_dir = loader_path_buf.parent().unwrap().to_string_lossy().to_string();
                let ld_library_path = format!("{}:{}", loader_dir, line_split[1]);
                // parse all libraries in LD_LIBRARY_PATH
                let lib_paths = ld_library_path.split(':').filter(|s| s.len()>0).map(|s| s.to_string()).collect();
                loader_paths.insert(loader_file_name, loader_path.clone());
                ld_library_path_envs.insert(loader_path.clone(), ld_library_path);
                default_lib_dirs.insert(loader_path, lib_paths);
            }
        }
        debug!("occlum elf loaders: {:?}", loader_paths);
        debug!("occlum ld_library_path envs: {:?}", ld_library_path_envs);
        debug!("default lib dirs: {:?}", default_lib_dirs);
        OcclumLoaders {loader_paths, ld_library_path_envs, default_lib_dirs}
    };
}

// pattern used to extract dependencies from ldd result
lazy_static! {
    /// pattern: name => path
    /// example: libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6
    static ref DEPENDENCY_REGEX: Regex = Regex::new(r"^(?P<name>\S+) => (?P<path>\S+) ").unwrap();
}

pub fn copy_file(src: &str, dest: &str, dry_run: bool) {
    info!("rsync -aL {} {}", src, dest);
    if !dry_run {
        let output = Command::new("rsync").arg("-aL").arg(src).arg(dest).output();
        match output {
            Ok(output) => deal_with_output(output, COPY_FILE_ERROR),
            Err(e) => {
                error!("copy file {} to {} failed. {}", src, dest, e);
                std::process::exit(COPY_FILE_ERROR);
            }
        }
    }
}

fn format_command_args(args: &Vec<String>) -> String {
    let mut res = String::new();
    for arg in args {
        res = format!("{} {}", res, arg);
    }
    res.trim().to_string()
}

pub fn mkdir(dest: &str, dry_run: bool) {
    info!("mkdir -p {}", dest);
    if !dry_run {
        if let Err(e) = std::fs::create_dir_all(dest) {
            error!("mkdir {} fails. {}", dest, e);
            std::process::exit(CREATE_DIR_ERROR);
        }
    }
}

pub fn create_link(src: &str, linkname: &str, dry_run: bool) {
    info!("ln -s {} {}", src, linkname);
    if !dry_run {
        // When we try to create a link, if there is already a file, the create will fail
        // So we delete the link at first if an old file exists.
        let _ = std::fs::remove_file(linkname);
        if let Err(e) = std::os::unix::fs::symlink(src, linkname) {
            error!("ln -s {} {} failed. {}", src, linkname, e);
            std::process::exit(CREATE_SYMLINK_ERROR);
        }
    }
}

pub fn copy_dir(src: &str, dest: &str, dry_run: bool, excludes: &Vec<String>) {
    // we should not pass --delete args. Otherwise it will overwrite files in the same place
    // We pass --copy-unsafe-links instead of -L arg. So links point to current directory will be kept.
    let mut args: Vec<_> = vec!["-ar", "--copy-unsafe-links"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let excludes: Vec<_> = excludes
        .iter()
        .map(|arg| format!("--exclude={}", arg))
        .collect();
    args.extend(excludes.into_iter());
    info!("rsync {} {} {}", format_command_args(&args), src, dest);
    if !dry_run {
        let output = Command::new("rsync").args(args).arg(src).arg(dest).output();
        match output {
            Ok(output) => deal_with_output(output, CREATE_DIR_ERROR),
            Err(e) => {
                error!("copy dir {} to {} failed. {}", src, dest, e);
                std::process::exit(COPY_DIR_ERROR);
            }
        }
    }
}

pub fn copy_shared_object(src: &str, dest: &str, dry_run: bool) {
    debug!("copy shared object {} to {}.", src, dest);
    copy_file(src, dest, dry_run);
}

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

/// This is the main function of finding dependent shared objects for an elf file.
/// Currently, we only support dependent shared objects with absolute path.
/// This function works in such a process.
/// It will first analyze the dynamic loader of the file if it has a dynamic loader,
/// which means the file is an elf file. Then, we will use the loader defined in *OCCLUM_LOADERS*
/// to replace the original loader. The modified loader will find dependencies for occlum.
/// We will use the dynamic loader to analyze the dependencies. We run the dynamic loader in command line
/// and analyze the stdout. We use regex to match the pattern of the loader output.
/// The loader will automatically find all dependencies recursively, i.e., it will also find dependencies
/// for each shared object, so we only need to analyze the top elf file.
/// The flag `exit_when_encountering_errors` is used to indicate the behavior if we can't find dependencies for an elf file.
/// If this flag is set true, the default behavior when encountering autodep errors is to print error message and exit program.
/// Otherwise, we will only print error message.
pub fn find_dependent_shared_objects(
    file_path: &str,
    default_loader: &Option<(String, String)>,
) -> HashSet<(String, String)> {
    let mut shared_objects = HashSet::new();
    // find dependencies for the input file
    // first, we find the dynamic loader for the elf file, if we can't find the loader, return empty shared objects
    let dynamic_loader = auto_dynamic_loader(file_path, default_loader);
    if dynamic_loader.is_none() {
        return shared_objects;
    }
    let (occlum_elf_loader, inlined_elf_loader) = dynamic_loader.unwrap();
    shared_objects.insert((occlum_elf_loader.clone(), inlined_elf_loader));
    let output = command_output_of_executing_dynamic_loader(&file_path, &occlum_elf_loader);
    if let Ok(output) = output {
        let default_lib_dirs = OCCLUM_LOADERS
            .default_lib_dirs
            .get(&occlum_elf_loader)
            .cloned();
        let mut objects = extract_dependencies_from_output(
            &file_path,
            output,
            default_lib_dirs,
        );
        for item in objects.drain() {
            shared_objects.insert(item);
        }
    }
    shared_objects
}

/// get the output of the given dynamic loader.
/// This function will use the dynamic loader to analyze the dependencies of an elf file
/// and return the command line output of the dynamic loader.
fn command_output_of_executing_dynamic_loader(
    file_path: &str,
    dynamic_loader: &str,
) -> Result<Output, std::io::Error> {
    // if the file path has only filename, we need to add a "." directory
    let file_path_buf = PathBuf::from(file_path);
    let file_path = if file_path_buf.parent() == None {
        PathBuf::from(".")
            .join(&file_path_buf)
            .to_string_lossy()
            .to_string()
    } else {
        file_path_buf.to_string_lossy().to_string()
    };
    // return the output of the command to analyze dependencies
    match OCCLUM_LOADERS.ld_library_path_envs.get(dynamic_loader) {
        None => {
            debug!("{} --list {}", dynamic_loader, file_path);
            Command::new(dynamic_loader)
                .arg("--list")
                .arg(file_path)
                .output()
        }
        Some(ld_library_path) => {
            debug!(
                "LD_LIBRARY_PATH='{}' {} --list {}",
                ld_library_path, dynamic_loader, file_path
            );
            Command::new(dynamic_loader)
                .arg("--list")
                .arg(file_path)
                .env("LD_LIBRARY_PATH", ld_library_path)
                .output()
        }
    }
}

/// This function will try to find a dynamic loader for a elf file automatically.
/// If will first try to read the interp section of elf file. If the file does not have interp section,
/// and the default loader is *NOT* None, it will return default loader.
/// It there is no interp section and default loader is None, it will return None.
/// If we find the loader, we will return Some((occlum_elf_loader, inlined_elf_loader)).
/// This is because the occlum_elf_loader and inlined_elf_loader may not be the same directory.
fn auto_dynamic_loader(
    filename: &str,
    default_loader: &Option<(String, String)>,
) -> Option<(String, String)> {
    let elf_file = match elf::File::open_path(filename) {
        Err(_) => return None,
        Ok(elf_file) => elf_file,
    };
    // We should only try to find dependencies for dynamic libraries or executables
    // relocatable files and core files are not included
    match elf_file.ehdr.elftype {
        ET_DYN|ET_EXEC => {},
        Type(_) => return None,
    }
    match elf_file.get_section(".interp") {
        None => {
            // When the elf file does not has interp section
            // 1. if we have default loader, we will return the default loader
            // 2. Otherwise we will return None and give warning.
            if let Some(default_loader) = default_loader {
                return Some(default_loader.clone());
            } else {
                warn!("cannot autodep for file {}. No dynamic loader can be found or inferred.", filename);
                return None;
            }
        }
        Some(_) => read_loader_from_interp_section(filename),
    }
}

fn read_loader_from_interp_section(filename: &str) -> Option<(String, String)> {
    let elf_file = match elf::File::open_path(filename) {
        Err(_) => return None,
        Ok(elf_file) => elf_file,
    };
    let interp_scan = match elf_file.get_section(".interp") {
        None => return None,
        Some(section) => section,
    };
    let interp_data = String::from_utf8_lossy(&interp_scan.data).to_string();
    let inlined_elf_loader = interp_data.trim_end_matches("\u{0}"); // this interp_data always with a \u{0} at end
    debug!("the loader of {} is {}.", filename, inlined_elf_loader);
    let inlined_elf_loader_path = PathBuf::from(inlined_elf_loader);
    let loader_file_name = inlined_elf_loader_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap();
    // If the loader file name is glibc loader or musl loader, we will use occlum-modified loader
    let occlum_elf_loader = OCCLUM_LOADERS
        .loader_paths
        .get(loader_file_name)
        .cloned()
        .unwrap_or(inlined_elf_loader.to_string());
    Some((
        occlum_elf_loader.to_string(),
        inlined_elf_loader.to_string(),
    ))
}

// try to infer default loader for all files to autodep
// If all files with .interp section points to the same loader,
// this loader will be viewed as the default loader
// Otherwise, no default loader can be found.
pub fn infer_default_loader(files_autodep: &Vec<String>) -> Option<(String, String)> {
    let mut loaders = HashSet::new();
    for filename in files_autodep.iter() {
        if let Some(loader) = read_loader_from_interp_section(filename) {
            loaders.insert(loader);
        }
    }
    if loaders.len() == 1 {
        return loaders.into_iter().next();
    } else {
        return None;
    }
}

/// resolve the results of dynamic loader to extract dependencies
pub fn extract_dependencies_from_output(
    file_path: &str,
    output: Output,
    default_lib_dirs: Option<Vec<String>>,
) -> HashSet<(String, String)> {
    let mut shared_objects = HashSet::new();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    debug!("The loader output of {}:\n {}", file_path, stdout);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    // audodep may output error message. We should return this message to user for further checking.
    if stderr.trim().len() > 0 {
        warn!("cannot autodep for {}. stderr: {}", file_path, stderr);
    }
    for line in stdout.lines() {
        let line = line.trim();
        let captures = DEPENDENCY_REGEX.captures(line);
        if let Some(captures) = captures {
            let raw_path = (&captures["path"]).to_string();
            if let Some(absolute_path) = convert_to_absolute(file_path, &raw_path) {
                match default_lib_dirs {
                    None => {
                        shared_objects.insert((absolute_path.clone(), absolute_path));
                    }
                    Some(ref default_lib_dirs) => {
                        let file_name = (&captures["name"]).to_string();
                        let lib_dir_in_host = PathBuf::from(&absolute_path)
                            .parent()
                            .unwrap()
                            .to_string_lossy()
                            .to_string();
                        // if the shared object is from one of the default dirs,
                        // we will copy to the first default dir(the loader dir)
                        // Otherwise it will be copied to the same dir as its dir in host.
                        if default_lib_dirs.contains(&lib_dir_in_host) {
                            let target_dir = default_lib_dirs.first().unwrap();
                            let target_path = PathBuf::from(target_dir)
                                .join(file_name)
                                .to_string_lossy()
                                .to_string();
                            shared_objects.insert((absolute_path, target_path));
                        } else {
                            shared_objects.insert((absolute_path.clone(), absolute_path));
                        }
                    }
                }
            }
        }
    }
    debug!("find objects: {:?}", shared_objects);
    shared_objects
}

/// convert the raw path to an absolute path.
/// The raw_path may be an absolute path itself, or a relative path relative to some file
/// If the conversion succeeds, return Some(converted_absolute_path)
/// otherwise, return None
pub fn convert_to_absolute(file_path: &str, raw_path: &str) -> Option<String> {
    let raw_path = PathBuf::from(raw_path);
    // if raw path is absolute, return
    if raw_path.is_absolute() {
        return Some(raw_path.to_string_lossy().to_string());
    }
    // if the given relative path can be converted to an absolute path , return
    let converted_path = resolve_relative_path(file_path, &raw_path.to_string_lossy());
    let converted_path = PathBuf::from(converted_path);
    if converted_path.is_absolute() {
        return Some(converted_path.to_string_lossy().to_string());
    }
    // return None
    return None;
}

/// convert `a path relative to file` to the real path in file system
pub fn resolve_relative_path(filename: &str, relative_path: &str) -> String {
    let file_path = PathBuf::from(filename);
    let file_dir_path = file_path
        .parent()
        .map_or(PathBuf::from("."), |p| PathBuf::from(p));
    let resolved_path = file_dir_path.join(relative_path);
    resolved_path.to_string_lossy().to_string()
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

/// Try to resolve a path may contain environmental variables to a path without environmental variables
/// This function relies on a third-party crate shellexpand.
/// Known limitations: If the environmental variable points to an empty value, the conversion may fail.
pub fn resolve_envs(path: &str) -> String {
    shellexpand::env(path).map_or_else(
        |_| {
            warn!("{} resolve fails.", path);
            path.to_string()
        },
        |res| res.to_string(),
    )
}

fn deal_with_output(output: Output, error_number: i32) {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if stdout.trim().len() > 0 {
        debug!("{}", stdout);
    }
    // if stderr is not None, the operation fails. We should abort the process and output error log.
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if stderr.trim().len() > 0 {
        error!("{}", stderr);
        std::process::exit(error_number);
    }
}
