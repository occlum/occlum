use super::*;
use crate::std::untrusted::path::PathEx;
use crate::util::sgx::allow_debug as sgx_allow_debug;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::CString;
use std::io::Read;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sgxfs::SgxFile;

use crate::util::mem_util::from_user;

use log::{set_max_level, LevelFilter};

lazy_static! {
    pub static ref LIBOS_CONFIG: Config = {
        let config_path =
            unsafe { format!("{}{}", INSTANCE_DIR, "/build/.Occlum_sys.json.protected") };
        let expected_mac = conf_get_hardcoded_file_mac();
        match load_config(&config_path, &expected_mac) {
            Err(e) => {
                error!("failed to load config: {}", e.backtrace());
                panic!();
            }
            Ok(config) => config,
        }
    };
}

// Envs merged from default envs and possible envs passed by syscall do_mount_rootfs
lazy_static! {
    pub static ref TRUSTED_ENVS: RwLock<Vec<CString>> =
        RwLock::new(LIBOS_CONFIG.env.default.clone());
}

pub fn load_config(config_path: &str, expected_mac: &sgx_aes_gcm_128bit_tag_t) -> Result<Config> {
    let mut config_file = {
        let config_file = SgxFile::open_integrity_only(config_path).map_err(|e| errno!(e))?;
        let actual_mac = config_file.get_mac().map_err(|e| errno!(e))?;
        if actual_mac != *expected_mac {
            return_errno!(EINVAL, "unexpected file MAC");
        }
        config_file
    };
    let config_json = {
        let mut config_json = String::new();
        config_file
            .read_to_string(&mut config_json)
            .map_err(|e| errno!(e))?;
        config_json
    };
    let config_input: InputConfig = serde_json::from_str(&config_json).map_err(|e| errno!(e))?;
    let config =
        Config::from_input(&config_input).cause_err(|e| errno!(EINVAL, "invalid config JSON"))?;
    Ok(config)
}

// This value will be modified during occlum build
#[used]
#[link_section = ".builtin_config"]
static OCCLUM_JSON_MAC: [u8; 47] = [0; 47];

fn conf_get_hardcoded_file_mac() -> sgx_aes_gcm_128bit_tag_t {
    // Use black_box to avoid the compiler's optimization for OCCLUM_JSON_MAC
    let json_mac = std::hint::black_box(&OCCLUM_JSON_MAC);
    let mac_str = String::from_utf8(json_mac.to_vec()).expect("MAC contains non UTF-8 characters");
    let mac = parse_mac(&mac_str).expect("MAC string cannot be converted to numbers");
    mac
}

pub fn parse_mac(mac_str: &str) -> Result<sgx_aes_gcm_128bit_tag_t> {
    let bytes_str_vec = {
        let bytes_str_vec: Vec<&str> = mac_str.split("-").collect();
        if bytes_str_vec.len() != 16 {
            return_errno!(EINVAL, "The length or format of MAC string is invalid");
        }
        bytes_str_vec
    };

    let mut mac: sgx_aes_gcm_128bit_tag_t = Default::default();
    for (byte_i, byte_str) in bytes_str_vec.iter().enumerate() {
        mac[byte_i] = u8::from_str_radix(byte_str, 16).map_err(|e| errno!(e))?;
    }
    Ok(mac)
}

pub fn parse_key(key_str: &str) -> Result<sgx_key_128bit_t> {
    let bytes_str_vec = {
        let bytes_str_vec: Vec<&str> = key_str.split("-").collect();
        if bytes_str_vec.len() != 16 {
            return_errno!(EINVAL, "The length or format of KEY string is invalid");
        }
        bytes_str_vec
    };

    let mut key: sgx_key_128bit_t = Default::default();
    for (byte_i, byte_str) in bytes_str_vec.iter().enumerate() {
        key[byte_i] = u8::from_str_radix(byte_str, 16).map_err(|e| errno!(e))?;
    }
    Ok(key)
}

#[derive(Debug)]
pub struct Config {
    pub resource_limits: ConfigResourceLimits,
    pub process: ConfigProcess,
    pub env: ConfigEnv,
    pub app: Vec<ConfigApp>,
    pub feature: ConfigFeature,
}

#[derive(Debug)]
pub struct ConfigResourceLimits {
    pub user_space_init_size: usize,
    pub user_space_max_size: usize,
}

#[derive(Debug)]
pub struct ConfigProcess {
    pub default_stack_size: usize,
    pub default_heap_size: usize,
    pub default_mmap_size: usize,
}

#[derive(Debug)]
pub struct ConfigEnv {
    pub default: Vec<CString>,
    pub untrusted: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct ConfigMount {
    pub type_: ConfigMountFsType,
    pub target: PathBuf,
    pub source: Option<PathBuf>,
    pub options: ConfigMountOptions,
}

#[derive(Clone, Debug)]
pub struct ConfigApp {
    pub entry_points: Vec<PathBuf>,
    pub stage: String,
    pub mount: Vec<ConfigMount>,
}

#[derive(Clone, Debug)]
pub struct ConfigFeature {
    pub amx: u32,
    pub pkru: u32,
    pub io_uring: u32,
    pub enable_edmm: bool,
    pub enable_posix_shm: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ConfigMountFsType {
    TYPE_SEFS,
    TYPE_HOSTFS,
    TYPE_RAMFS,
    TYPE_UNIONFS,
    TYPE_DEVFS,
    TYPE_PROCFS,
    TYPE_EXT2,
}

impl ConfigMountFsType {
    pub fn from_input(input: &str) -> Result<ConfigMountFsType> {
        const ALL_FS_TYPES: [&str; 6] = ["sefs", "hostfs", "ramfs", "unionfs", "devfs", "procfs"];

        let type_ = match input {
            "sefs" => ConfigMountFsType::TYPE_SEFS,
            "hostfs" => ConfigMountFsType::TYPE_HOSTFS,
            "ramfs" => ConfigMountFsType::TYPE_RAMFS,
            "unionfs" => ConfigMountFsType::TYPE_UNIONFS,
            "devfs" => ConfigMountFsType::TYPE_DEVFS,
            "procfs" => ConfigMountFsType::TYPE_PROCFS,
            "ext2" => ConfigMountFsType::TYPE_EXT2,
            _ => {
                return_errno!(EINVAL, "Unsupported file system type");
            }
        };
        Ok(type_)
    }
}

#[derive(Clone, Default, Debug)]
pub struct ConfigMountOptions {
    pub mac: Option<sgx_aes_gcm_128bit_tag_t>,
    pub layers: Option<Vec<ConfigMount>>,
    pub temporary: bool,
    pub cache_size: Option<u64>,
    pub disk_size: Option<u64>,
    pub index: u32,
}

impl Config {
    fn from_input(input: &InputConfig) -> Result<Config> {
        let resource_limits = ConfigResourceLimits::from_input(&input.resource_limits)?;
        let process = ConfigProcess::from_input(&input.process)?;
        let env = ConfigEnv::from_input(&input.env)?;
        let app = {
            let mut app = Vec::new();
            for input_app in &input.app {
                app.push(ConfigApp::from_input(&input_app)?);
            }
            app
        };
        let feature = ConfigFeature::from_input(&input.feature)?;

        if input.disable_log {
            log::set_max_level(LevelFilter::Off);
        } else if !sgx_allow_debug() {
            if log::max_level() != LevelFilter::Off {
                // Release enclave can only set error level log
                log::set_max_level(LevelFilter::Error);
            }
            eprintln!("Warnning: Occlum Log is enabled for release enclave!");
            eprintln!(
                "Uses can disable Occlum Log by setting metadata.disable_log=true \
                in Occlum.json and rebuild Occlum instance.\n"
            );
        }

        Ok(Config {
            resource_limits,
            process,
            env,
            app,
            feature,
        })
    }

    pub fn get_app_config(&self, stage: &str) -> Result<&ConfigApp> {
        let config_app = self
            .app
            .iter()
            .find(|m| m.stage.eq(stage))
            .ok_or_else(|| errno!(Errno::ENOENT, "No expected config app"))?;

        Ok(config_app)
    }
}

impl ConfigResourceLimits {
    fn from_input(input: &InputConfigResourceLimits) -> Result<ConfigResourceLimits> {
        let user_space_init_size = parse_memory_size(&input.user_space_init_size)?;
        let user_space_max_size = parse_memory_size(&input.user_space_max_size)?;
        Ok(ConfigResourceLimits {
            user_space_init_size,
            user_space_max_size,
        })
    }
}

impl ConfigProcess {
    fn from_input(input: &InputConfigProcess) -> Result<ConfigProcess> {
        let default_stack_size = parse_memory_size(&input.default_stack_size)?;
        let default_heap_size = parse_memory_size(&input.default_heap_size)?;
        let default_mmap_size = parse_memory_size(&input.default_mmap_size)?;
        Ok(ConfigProcess {
            default_stack_size,
            default_heap_size,
            default_mmap_size,
        })
    }
}

impl ConfigEnv {
    fn from_input(input: &InputConfigEnv) -> Result<ConfigEnv> {
        Ok(ConfigEnv {
            default: input.default.clone(),
            untrusted: input.untrusted.clone(),
        })
    }
}

impl ConfigApp {
    fn from_input(input: &InputConfigApp) -> Result<ConfigApp> {
        let stage = input.stage.clone();
        let entry_points = {
            let mut entry_points = Vec::new();
            for ep in &input.entry_points {
                let ep_path = Path::new(ep).to_path_buf();
                if !ep_path.is_absolute() {
                    return_errno!(EINVAL, "entry point must be an absolute path")
                }
                entry_points.push(ep_path);
            }
            entry_points
        };
        let mount = {
            let mut mount = Vec::new();
            for input_mount in &input.mount {
                mount.push(ConfigMount::from_input(&input_mount)?);
            }
            mount
        };

        Ok(ConfigApp {
            stage,
            entry_points,
            mount,
        })
    }
}

impl ConfigFeature {
    fn from_input(input: &InputConfigFeature) -> Result<ConfigFeature> {
        Ok(ConfigFeature {
            amx: input.amx,
            pkru: input.pkru,
            io_uring: input.io_uring,
            enable_edmm: input.enable_edmm,
            enable_posix_shm: input.enable_posix_shm,
        })
    }
}

impl ConfigMount {
    fn from_input(input: &InputConfigMount) -> Result<ConfigMount> {
        let type_ = ConfigMountFsType::from_input(input.type_.as_str())?;
        let target = {
            let target = PathBuf::from(&input.target);
            if !target.starts_with("/") {
                return_errno!(EINVAL, "Target must be an absolute path");
            }
            target
        };
        let source = input.source.as_ref().map(|s| PathBuf::from(s));
        let source = if source.is_none() {
            None
        } else {
            let path = unsafe { PathBuf::from(&INSTANCE_DIR) };
            path.join(source.unwrap()).canonicalize().ok()
        };
        let options = ConfigMountOptions::from_input(&input.options)?;
        Ok(ConfigMount {
            type_,
            target,
            source,
            options,
        })
    }
}

impl ConfigMountOptions {
    fn from_input(input: &InputConfigMountOptions) -> Result<ConfigMountOptions> {
        let mac = if input.mac.is_some() {
            Some(parse_mac(&input.mac.as_ref().unwrap())?)
        } else {
            None
        };
        let layers = if let Some(layers) = &input.layers {
            let layers = layers
                .iter()
                .map(|config| ConfigMount::from_input(config).expect("invalid mount config"))
                .collect();
            Some(layers)
        } else {
            None
        };
        let cache_size = if input.cache_size.is_some() {
            Some(parse_memory_size(input.cache_size.as_ref().unwrap())? as _)
        } else {
            None
        };
        let disk_size = if input.disk_size.is_some() {
            Some(parse_memory_size(input.disk_size.as_ref().unwrap())? as _)
        } else {
            None
        };
        Ok(ConfigMountOptions {
            mac,
            layers,
            temporary: input.temporary,
            cache_size,
            disk_size,
            index: input.index,
        })
    }
}

fn parse_memory_size(mem_str: &str) -> Result<usize> {
    const UNIT2FACTOR: [(&str, usize); 5] = [
        ("KB", 1024),
        ("MB", 1024 * 1024),
        ("GB", 1024 * 1024 * 1024),
        ("TB", 1024 * 1024 * 1024 * 1024),
        ("B", 1),
    ];

    let mem_str = mem_str.trim();
    let (unit, factor) = UNIT2FACTOR
        .iter()
        .position(|(unit, _)| mem_str.ends_with(unit))
        .ok_or_else(|| errno!(EINVAL, "No unit"))
        .map(|unit_i| &UNIT2FACTOR[unit_i])?;
    let number = match mem_str[0..mem_str.len() - unit.len()]
        .trim()
        .parse::<usize>()
    {
        Err(_) => return_errno!(EINVAL, "No number"),
        Ok(number) => number,
    };
    Ok(number * factor)
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfig {
    #[serde(default)]
    pub resource_limits: InputConfigResourceLimits,
    #[serde(default)]
    pub process: InputConfigProcess,
    #[serde(default)]
    pub env: InputConfigEnv,
    #[serde(default)]
    pub disable_log: bool,
    #[serde(default)]
    pub app: Vec<InputConfigApp>,
    #[serde(default)]
    pub feature: InputConfigFeature,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigResourceLimits {
    #[serde(default = "InputConfigResourceLimits::get_user_space_init_size")]
    pub user_space_init_size: String,
    #[serde(default = "InputConfigResourceLimits::get_user_space_max_size")]
    pub user_space_max_size: String,
}

impl InputConfigResourceLimits {
    fn get_user_space_init_size() -> String {
        "128MB".to_string()
    }

    // For default, just make it equal with the init size
    fn get_user_space_max_size() -> String {
        "128MB".to_string()
    }
}

impl Default for InputConfigResourceLimits {
    fn default() -> InputConfigResourceLimits {
        InputConfigResourceLimits {
            user_space_init_size: InputConfigResourceLimits::get_user_space_init_size(),
            user_space_max_size: InputConfigResourceLimits::get_user_space_max_size(),
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigProcess {
    #[serde(default = "InputConfigProcess::get_default_stack_size")]
    pub default_stack_size: String,
    #[serde(default = "InputConfigProcess::get_default_heap_size")]
    pub default_heap_size: String,
    #[serde(default = "InputConfigProcess::get_default_mmap_size")]
    pub default_mmap_size: String,
}

impl InputConfigProcess {
    fn get_default_stack_size() -> String {
        "8MB".to_string()
    }

    fn get_default_heap_size() -> String {
        "16MB".to_string()
    }

    fn get_default_mmap_size() -> String {
        "32MB".to_string()
    }
}

impl Default for InputConfigProcess {
    fn default() -> InputConfigProcess {
        InputConfigProcess {
            default_stack_size: InputConfigProcess::get_default_stack_size(),
            default_heap_size: InputConfigProcess::get_default_heap_size(),
            default_mmap_size: InputConfigProcess::get_default_mmap_size(),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigEnv {
    pub default: Vec<CString>,
    pub untrusted: HashSet<String>,
}

impl Default for InputConfigEnv {
    fn default() -> InputConfigEnv {
        InputConfigEnv {
            default: Vec::new(),
            untrusted: HashSet::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigMount {
    #[serde(rename = "type")]
    pub type_: String,
    pub target: String,
    pub source: Option<String>,
    #[serde(default)]
    pub options: InputConfigMountOptions,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct InputConfigMountOptions {
    #[serde(rename = "MAC")]
    #[serde(default)]
    pub mac: Option<String>,
    #[serde(default)]
    pub layers: Option<Vec<InputConfigMount>>,
    #[serde(default)]
    pub temporary: bool,
    #[serde(default)]
    pub cache_size: Option<String>,
    #[serde(default)]
    pub disk_size: Option<String>,
    #[serde(default)]
    pub index: u32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigApp {
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub entry_points: Vec<String>,
    #[serde(default)]
    pub mount: Vec<InputConfigMount>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigFeature {
    #[serde(default)]
    pub amx: u32,
    #[serde(default)]
    pub pkru: u32,
    #[serde(default)]
    pub io_uring: u32,
    #[serde(default)]
    pub enable_edmm: bool,
    #[serde(default)]
    pub enable_posix_shm: bool,
}

impl Default for InputConfigFeature {
    fn default() -> InputConfigFeature {
        InputConfigFeature {
            amx: 0,
            pkru: 0,
            io_uring: 0,
            enable_edmm: false,
            enable_posix_shm: false,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct user_rootfs_config {
    // length of the struct
    len: usize,
    // UnionFS type rootfs upper layer, read-write layer
    upper_layer_path: *const i8,
    // UnionFS type rootfs lower layer, read-only layer
    lower_layer_path: *const i8,
    entry_point: *const i8,
    // HostFS source path
    hostfs_source: *const i8,
    // HostFS target path, default value is "/host"
    hostfs_target: *const i8,
    // An array of pointers to null-terminated strings
    // and must be terminated by a null pointer
    envp: *const *const i8,
}

fn to_option_pathbuf(path: *const i8) -> Result<Option<PathBuf>> {
    let path = if path.is_null() {
        None
    } else {
        Some(PathBuf::from(
            from_user::clone_cstring_safely(path)?
                .to_string_lossy()
                .into_owned(),
        ))
    };

    Ok(path)
}

fn combine_trusted_envs(envp: *const *const i8) -> Result<()> {
    let mut user_envs = from_user::clone_cstrings_safely(envp)?;
    trace!("User envs: {:?}", user_envs);
    let env_key: Vec<&str> = user_envs
        .iter()
        .map(|x| {
            let kv: Vec<&str> = x.to_str().unwrap().splitn(2, '=').collect();
            kv[0]
        })
        .collect();

    let mut merged = config::TRUSTED_ENVS.write().unwrap();
    // First clear the default envs then do the merge again
    merged.clear();
    merged.extend_from_slice(&user_envs);

    for (_idx, val) in config::LIBOS_CONFIG.env.default.iter().enumerate() {
        let kv: Vec<&str> = val.to_str().unwrap().splitn(2, '=').collect(); // only split the first "="
        info!("kv: {:?}", kv);
        if !env_key.contains(&kv[0]) {
            unsafe { merged.push(val.clone()) };
        }
    }

    // trace!("Combined trusted envs: {:?}", merged);
    Ok(())
}

impl ConfigApp {
    pub fn from_user(config: &user_rootfs_config) -> Result<ConfigApp> {
        // Check config struct length for future possible extension
        if config.len != size_of::<user_rootfs_config>() {
            return_errno!(EINVAL, "User Config Struct length not match");
        }

        // Combine the default envs and user envs if necessary
        if !config.envp.is_null() {
            combine_trusted_envs(config.envp)?;
        }

        let upper_layer = to_option_pathbuf(config.upper_layer_path)?;
        let lower_layer = to_option_pathbuf(config.lower_layer_path)?;
        let entry_point = to_option_pathbuf(config.entry_point)?;
        let hostfs_source = to_option_pathbuf(config.hostfs_source)?;

        let hostfs_target = if config.hostfs_target.is_null() {
            PathBuf::from("/host")
        } else {
            PathBuf::from(
                from_user::clone_cstring_safely(config.hostfs_target)?
                    .to_string_lossy()
                    .into_owned(),
            )
        };

        let mut config_app = LIBOS_CONFIG.get_app_config("app").unwrap().clone();
        let root_mount_config = config_app
            .mount
            .iter_mut()
            .find(|m| m.target == Path::new("/") && m.type_ == ConfigMountFsType::TYPE_UNIONFS)
            .ok_or_else(|| errno!(Errno::ENOENT, "the root UnionFS is not valid"))?;

        if lower_layer.is_some() {
            let layer_mount_configs = root_mount_config.options.layers.as_mut().unwrap();
            // image SEFS in layers
            let root_image_sefs_mount_config = layer_mount_configs
                .iter_mut()
                .find(|m| {
                    m.target == Path::new("/")
                        && m.type_ == ConfigMountFsType::TYPE_SEFS
                        && (m.options.mac.is_some() || m.options.index == 1)
                })
                .ok_or_else(|| errno!(Errno::ENOENT, "the image SEFS in layers is not valid"))?;

            root_image_sefs_mount_config.source = lower_layer;
            root_image_sefs_mount_config.options.mac = None;
            root_image_sefs_mount_config.options.index = 1;
        }

        if upper_layer.is_some() {
            let layer_mount_configs = root_mount_config.options.layers.as_mut().unwrap();
            // container SEFS in layers
            let root_container_sefs_mount_config = layer_mount_configs
                .iter_mut()
                .find(|m| {
                    m.target == Path::new("/")
                        && m.type_ == ConfigMountFsType::TYPE_SEFS
                        && m.options.mac.is_none()
                        && m.options.index == 0
                })
                .ok_or_else(|| {
                    errno!(Errno::ENOENT, "the container SEFS in layers is not valid")
                })?;

            root_container_sefs_mount_config.source = upper_layer;
        }

        if entry_point.is_some() {
            config_app.entry_points.clear();
            config_app.entry_points.push(entry_point.unwrap())
        }

        if hostfs_source.is_some() {
            let hostfs_mount_config = config_app
                .mount
                .iter_mut()
                .find(|m| m.type_ == ConfigMountFsType::TYPE_HOSTFS)
                .ok_or_else(|| errno!(Errno::ENOENT, "the HostFS is not valid"))?;
            hostfs_mount_config.source = hostfs_source;
            hostfs_mount_config.target = hostfs_target;
        }

        Ok(config_app)
    }
}
