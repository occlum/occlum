use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::io::Read;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sgxfs::SgxFile;
use std::untrusted::path::PathEx;

use sgx_tse::rsgx_self_report;

use serde::{Deserialize, Serialize};

use crate::entry::enclave::INSTANCE_DIR;
use crate::prelude::*;
use crate::util::mem_util::from_user;

lazy_static! {
    pub static ref LIBOS_CONFIG: Config = {
        let config_path =
            unsafe { format!("{}{}", INSTANCE_DIR, "/build/.Occlum_sys.json.protected") };
        let expected_mac = conf_get_file_mac();
        trace!("expected_mac: {:?}", expected_mac);
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
        trace!("actual_mac: {:?}", actual_mac);
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
    info!("config_json = {:?}", config_json);
    let config_input: InputConfig = serde_json::from_str(&config_json).map_err(|e| errno!(e))?;
    let config =
        Config::from_input(&config_input).cause_err(|e| errno!(EINVAL, "invalid config JSON"))?;
    Ok(config)
}

fn conf_get_file_mac() -> sgx_aes_gcm_128bit_tag_t {
    let report = rsgx_self_report();
    let mut mac = report.body.isv_family_id;
    mac.reverse();
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
    pub untrusted_unix_socks: Option<Vec<ConfigUntrustedUnixSock>>,
    pub app: Vec<ConfigApp>,
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
}

#[derive(Debug)]
pub struct ConfigEnv {
    pub default: Vec<CString>,
    pub untrusted: HashSet<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ConfigUntrustedUnixSock {
    pub host: PathBuf,
    pub libos: PathBuf,
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
    pub encrypted: bool,
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
    TYPE_ASYNC_SFS,
}

impl ConfigMountFsType {
    pub fn from_input(input: &str) -> Result<ConfigMountFsType> {
        const ALL_FS_TYPES: [&str; 7] = [
            "sefs",
            "hostfs",
            "ramfs",
            "unionfs",
            "devfs",
            "procfs",
            "async_sfs",
        ];

        let type_ = match input {
            "sefs" => ConfigMountFsType::TYPE_SEFS,
            "hostfs" => ConfigMountFsType::TYPE_HOSTFS,
            "ramfs" => ConfigMountFsType::TYPE_RAMFS,
            "unionfs" => ConfigMountFsType::TYPE_UNIONFS,
            "devfs" => ConfigMountFsType::TYPE_DEVFS,
            "procfs" => ConfigMountFsType::TYPE_PROCFS,
            "async_sfs" => ConfigMountFsType::TYPE_ASYNC_SFS,
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
    pub async_sfs_total_size: Option<usize>,
    pub page_cache_size: Option<usize>,
    pub index: u32,
    pub autokey_policy: Option<u32>,
    pub sefs_cache_size: Option<u64>,
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
        let untrusted_unix_socks = {
            if let Some(input_socks) = &input.untrusted_unix_socks {
                let mut untrusted_socks = Vec::new();
                for sock in input_socks {
                    let untrusted_sock = ConfigUntrustedUnixSock {
                        host: Path::new(&sock.host).to_path_buf(),
                        libos: Path::new(&sock.libos).to_path_buf(),
                    };
                    untrusted_socks.push(untrusted_sock);
                }
                Some(untrusted_socks)
            } else {
                None
            }
        };

        Ok(Config {
            resource_limits,
            process,
            env,
            untrusted_unix_socks,
            app,
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
        Ok(ConfigProcess {
            default_stack_size,
            default_heap_size,
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
        let encrypted = input.encrypted;
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
            encrypted,
        })
    }

    pub fn is_image_encrypted(&self) -> bool {
        self.encrypted
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
            Some(path.join(source.unwrap()))
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
        let async_sfs_total_size = if input.async_sfs_total_size.is_some() {
            Some(parse_memory_size(
                input.async_sfs_total_size.as_ref().unwrap(),
            )?)
        } else {
            None
        };
        let page_cache_size = if input.page_cache_size.is_some() {
            Some(parse_memory_size(input.page_cache_size.as_ref().unwrap())?)
        } else {
            None
        };
        let sefs_cache_size = if input.sefs_cache_size.is_some() {
            Some(parse_memory_size(input.sefs_cache_size.as_ref().unwrap())? as _)
        } else {
            None
        };
        Ok(ConfigMountOptions {
            mac,
            layers,
            temporary: input.temporary,
            async_sfs_total_size,
            page_cache_size,
            index: input.index,
            autokey_policy: input.autokey_policy,
            sefs_cache_size,
        })
    }

    pub fn gen_async_sfs_default() -> Self {
        let (async_sfs_total_size, page_cache_size) = {
            const MB: usize = 1024 * 1024;
            const GB: usize = 1024 * MB;
            (10 * GB, 256 * MB)
        };
        Self {
            mac: None,
            layers: None,
            temporary: false,
            async_sfs_total_size: Some(async_sfs_total_size),
            page_cache_size: Some(page_cache_size),
            index: 0,
            autokey_policy: None,
            sefs_cache_size: None,
        }
    }
}

pub fn parse_memory_size(mem_str: &str) -> Result<usize> {
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
    pub untrusted_unix_socks: Option<Vec<InputConfigUntrustedUnixSock>>,
    #[serde(default)]
    pub app: Vec<InputConfigApp>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigResourceLimits {
    #[serde(default = "InputConfigResourceLimits::get_user_space_size")]
    pub user_space_init_size: String,
    #[serde(default = "InputConfigResourceLimits::get_user_space_size")]
    pub user_space_max_size: String,
}

impl InputConfigResourceLimits {
    fn get_user_space_size() -> String {
        "128MB".to_string()
    }
}

impl Default for InputConfigResourceLimits {
    fn default() -> InputConfigResourceLimits {
        InputConfigResourceLimits {
            user_space_init_size: InputConfigResourceLimits::get_user_space_size(),
            user_space_max_size: InputConfigResourceLimits::get_user_space_size(),
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
}

impl InputConfigProcess {
    fn get_default_stack_size() -> String {
        "8MB".to_string()
    }

    fn get_default_heap_size() -> String {
        "16MB".to_string()
    }
}

impl Default for InputConfigProcess {
    fn default() -> InputConfigProcess {
        InputConfigProcess {
            default_stack_size: InputConfigProcess::get_default_stack_size(),
            default_heap_size: InputConfigProcess::get_default_heap_size(),
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
struct InputConfigUntrustedUnixSock {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub libos: String,
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
    pub async_sfs_total_size: Option<String>,
    #[serde(default)]
    pub page_cache_size: Option<String>,
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub autokey_policy: Option<u32>,
    #[serde(default)]
    pub sefs_cache_size: Option<String>,
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
    #[serde(default)]
    pub encrypted: bool,
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

    let mut merged = TRUSTED_ENVS.write().unwrap();
    // First clear the default envs then do the merge again
    merged.clear();
    merged.extend_from_slice(&user_envs);

    for (_idx, val) in LIBOS_CONFIG.env.default.iter().enumerate() {
        let kv: Vec<&str> = val.to_str().unwrap().splitn(2, '=').collect(); // only split the first "="
        info!("kv: {:?}", kv);
        if !env_key.contains(&kv[0]) {
            unsafe { merged.push(val.clone()) };
        }
    }

    trace!("Combined trusted envs: {:?}", merged);
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
            // container AsyncSFS/SEFS in layers
            let root_container_fs_mount_config = layer_mount_configs
                .iter_mut()
                .find(|m| {
                    m.target == Path::new("/")
                        && (m.type_ == ConfigMountFsType::TYPE_ASYNC_SFS
                            || m.type_ == ConfigMountFsType::TYPE_SEFS)
                        && m.options.mac.is_none()
                        && m.options.index == 0
                })
                .ok_or_else(|| errno!(Errno::ENOENT, "the container FS in layers is not valid"))?;

            root_container_fs_mount_config.source = upper_layer;
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
