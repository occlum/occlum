use super::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sgxfs::SgxFile;

const LIBOS_CONFIG_PATH: &str = "./.occlum/build/Occlum.json.protected";

lazy_static! {
    pub static ref LIBOS_CONFIG: Config = {
        let mut config_file = {
            let config_file = match SgxFile::open_integrity_only(LIBOS_CONFIG_PATH) {
                Err(_) => panic!(
                    "Failed to find or open Occlum's config file: {}",
                    LIBOS_CONFIG_PATH
                ),
                Ok(file) => file,
            };

            let actual_mac = match config_file.get_mac() {
                Err(_) => panic!(
                    "Failed to get the MAC of Occlum's config file: {}",
                    LIBOS_CONFIG_PATH
                ),
                Ok(mac) => mac,
            };
            let expected_mac = conf_get_hardcoded_file_mac();
            if actual_mac != expected_mac {
                panic!(
                    "The MAC of Occlum's config file is not as expected: {}",
                    LIBOS_CONFIG_PATH
                );
            }

            config_file
        };
        let config_json = {
            let mut config_json = String::new();
            config_file.read_to_string(&mut config_json).map_err(|_| {
                panic!(
                    "Failed to read from Occlum's config file: {}",
                    LIBOS_CONFIG_PATH
                );
            });
            config_json
        };
        let config_input: InputConfig = match serde_json::from_str(&config_json) {
            Err(_) => panic!(
                "Failed to parse JSON from Occlum's config file: {}",
                LIBOS_CONFIG_PATH
            ),
            Ok(config_input) => config_input,
        };
        let config = match Config::from_input(&config_input) {
            Err(_) => panic!(
                "Found invalid config in Occlum's config file: {}",
                LIBOS_CONFIG_PATH
            ),
            Ok(config) => config,
        };
        config
    };
}

fn conf_get_hardcoded_file_mac() -> sgx_aes_gcm_128bit_tag_t {
    // Wrap the unsafe C version to get the safe Rust version
    extern "C" {
        fn conf_get_hardcoded_file_mac() -> *const c_char;
    }

    let mac_str = unsafe {
        CStr::from_ptr(conf_get_hardcoded_file_mac())
            .to_str()
            .expect("Invalid MAC")
    };
    let mac = parse_mac(mac_str).expect("Invalid MAC");
    mac
}

fn parse_mac(mac_str: &str) -> Result<sgx_aes_gcm_128bit_tag_t, Error> {
    let bytes_str_vec = {
        let bytes_str_vec: Vec<&str> = mac_str.split("-").collect();
        if bytes_str_vec.len() != 16 {
            return errno!(EINVAL, "The length or format of MAC string is invalid");
        }
        bytes_str_vec
    };
    let mut mac: sgx_aes_gcm_128bit_tag_t = Default::default();
    for (byte_i, byte_str) in bytes_str_vec.iter().enumerate() {
        mac[byte_i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| Error::new(Errno::EINVAL, "The format of MAC string is invalid"))?;
    }
    Ok(mac)
}

#[derive(Debug)]
pub struct Config {
    pub vm: ConfigVM,
    pub process: ConfigProcess,
    pub mount: Vec<ConfigMount>,
}

#[derive(Debug)]
pub struct ConfigVM {
    pub user_space_size: usize,
}

#[derive(Debug)]
pub struct ConfigProcess {
    pub default_stack_size: usize,
    pub default_heap_size: usize,
    pub default_mmap_size: usize,
}

#[derive(Debug)]
pub struct ConfigMount {
    pub type_: ConfigMountFsType,
    pub target: PathBuf,
    pub source: Option<PathBuf>,
    pub options: ConfigMountOptions,
}

#[derive(Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ConfigMountFsType {
    TYPE_SEFS,
    TYPE_HOSTFS,
    TYPE_RAMFS,
}

#[derive(Debug)]
pub struct ConfigMountOptions {
    pub integrity_only: bool,
    pub mac: Option<sgx_aes_gcm_128bit_tag_t>,
}

impl Config {
    fn from_input(input: &InputConfig) -> Result<Config, Error> {
        let vm = ConfigVM::from_input(&input.vm)?;
        let process = ConfigProcess::from_input(&input.process)?;
        let mount = {
            let mut mount = Vec::new();
            for input_mount in &input.mount {
                mount.push(ConfigMount::from_input(&input_mount)?);
            }
            mount
        };
        Ok(Config { vm, process, mount })
    }
}

impl ConfigVM {
    fn from_input(input: &InputConfigVM) -> Result<ConfigVM, Error> {
        let user_space_size = parse_memory_size(&input.user_space_size)?;
        Ok(ConfigVM { user_space_size })
    }
}

impl ConfigProcess {
    fn from_input(input: &InputConfigProcess) -> Result<ConfigProcess, Error> {
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

impl ConfigMount {
    fn from_input(input: &InputConfigMount) -> Result<ConfigMount, Error> {
        const ALL_FS_TYPES: [&str; 3] = ["sefs", "hostfs", "ramfs"];

        let type_ = match input.type_.as_str() {
            "sefs" => ConfigMountFsType::TYPE_SEFS,
            "hostfs" => ConfigMountFsType::TYPE_HOSTFS,
            "ramfs" => ConfigMountFsType::TYPE_RAMFS,
            _ => {
                return errno!(EINVAL, "Unsupported file system type");
            }
        };
        let target = {
            let target = PathBuf::from(&input.target);
            if !target.starts_with("/") {
                return errno!(EINVAL, "Target must be an absolute path");
            }
            target
        };
        let source = input.source.as_ref().map(|s| PathBuf::from(s));
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
    fn from_input(input: &InputConfigMountOptions) -> Result<ConfigMountOptions, Error> {
        let (integrity_only, mac) = if !input.integrity_only {
            (false, None)
        } else {
            if input.mac.is_none() {
                return errno!(EINVAL, "MAC is expected");
            }
            (true, Some(parse_mac(&input.mac.as_ref().unwrap())?))
        };
        Ok(ConfigMountOptions {
            integrity_only,
            mac,
        })
    }
}

fn parse_memory_size(mem_str: &str) -> Result<usize, Error> {
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
        .ok_or_else(|| Error::new(Errno::EINVAL, "No unit"))
        .map(|unit_i| &UNIT2FACTOR[unit_i])?;
    let number = match mem_str[0..mem_str.len() - unit.len()]
        .trim()
        .parse::<usize>()
    {
        Err(_) => return errno!(EINVAL, "No number"),
        Ok(number) => number,
    };
    Ok(number * factor)
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfig {
    #[serde(default)]
    pub vm: InputConfigVM,
    #[serde(default)]
    pub process: InputConfigProcess,
    #[serde(default)]
    pub mount: Vec<InputConfigMount>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InputConfigVM {
    #[serde(default = "InputConfigVM::get_user_space_size")]
    pub user_space_size: String,
}

impl InputConfigVM {
    fn get_user_space_size() -> String {
        "128MB".to_string()
    }
}

impl Default for InputConfigVM {
    fn default() -> InputConfigVM {
        InputConfigVM {
            user_space_size: InputConfigVM::get_user_space_size(),
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
    #[serde(default)]
    pub integrity_only: bool,
    #[serde(rename = "MAC")]
    #[serde(default)]
    pub mac: Option<String>,
}
