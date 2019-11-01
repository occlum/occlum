use super::*;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sgxfs::SgxFile;

lazy_static! {
    pub static ref LIBOS_CONFIG: Config = {
        fn load_config(config_path: &str) -> Result<Config> {
            let mut config_file = {
                let config_file =
                    SgxFile::open_integrity_only(config_path).map_err(|e| errno!(e))?;

                let actual_mac = config_file.get_mac().map_err(|e| errno!(e))?;
                let expected_mac = conf_get_hardcoded_file_mac();
                if actual_mac != expected_mac {
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
            let config_input: InputConfig =
                serde_json::from_str(&config_json).map_err(|e| errno!(e))?;
            let config = Config::from_input(&config_input)
                .cause_err(|e| errno!(EINVAL, "invalid config JSON"))?;
            Ok(config)
        }

        let config_path = "./.occlum/build/Occlum.json.protected";
        match load_config(config_path) {
            Err(e) => {
                error!("failed to load config: {}", e.backtrace());
                panic!();
            }
            Ok(config) => config,
        }
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

fn parse_mac(mac_str: &str) -> Result<sgx_aes_gcm_128bit_tag_t> {
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

#[derive(Debug)]
pub struct Config {
    pub vm: ConfigVM,
    pub process: ConfigProcess,
    pub env: Vec<CString>,
    pub entry_points: Vec<PathBuf>,
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
    fn from_input(input: &InputConfig) -> Result<Config> {
        let vm = ConfigVM::from_input(&input.vm)?;
        let process = ConfigProcess::from_input(&input.process)?;
        let env = {
            let mut env = Vec::new();
            for input_env in &input.env {
                env.push(CString::new(input_env.clone())?);
            }
            env
        };
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
        Ok(Config {
            vm,
            process,
            env,
            entry_points,
            mount,
        })
    }
}

impl ConfigVM {
    fn from_input(input: &InputConfigVM) -> Result<ConfigVM> {
        let user_space_size = parse_memory_size(&input.user_space_size)?;
        Ok(ConfigVM { user_space_size })
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

impl ConfigMount {
    fn from_input(input: &InputConfigMount) -> Result<ConfigMount> {
        const ALL_FS_TYPES: [&str; 3] = ["sefs", "hostfs", "ramfs"];

        let type_ = match input.type_.as_str() {
            "sefs" => ConfigMountFsType::TYPE_SEFS,
            "hostfs" => ConfigMountFsType::TYPE_HOSTFS,
            "ramfs" => ConfigMountFsType::TYPE_RAMFS,
            _ => {
                return_errno!(EINVAL, "Unsupported file system type");
            }
        };
        let target = {
            let target = PathBuf::from(&input.target);
            if !target.starts_with("/") {
                return_errno!(EINVAL, "Target must be an absolute path");
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
    fn from_input(input: &InputConfigMountOptions) -> Result<ConfigMountOptions> {
        let (integrity_only, mac) = if !input.integrity_only {
            (false, None)
        } else {
            if input.mac.is_none() {
                return_errno!(EINVAL, "MAC is expected");
            }
            (true, Some(parse_mac(&input.mac.as_ref().unwrap())?))
        };
        Ok(ConfigMountOptions {
            integrity_only,
            mac,
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
    pub vm: InputConfigVM,
    #[serde(default)]
    pub process: InputConfigProcess,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub entry_points: Vec<String>,
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
