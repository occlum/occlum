extern crate clap;
extern crate env_logger;
extern crate log;
extern crate regex;
extern crate serde;
extern crate serde_derive;
extern crate serde_xml_rs;

use clap::{App, Arg, SubCommand};
use lazy_static::lazy_static;
use log::debug;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::path::Path;

// Some hardcode implicit config value. Please update the values defined in "init()" when necessary.
lazy_static! {
    static ref DEFAULT_CONFIG: DefaultConfig = DefaultConfig::init();
}

struct DefaultConfig {
    // TCS number used by Occlum kernel
    num_of_tcs_used_by_occlum_kernel: u32,
    // Corresponds to TCSMaxNum in Enclave.xml
    num_of_cpus_max: u32,
    // Corresponds to MiscSelect in Enclave.xml
    misc_select: &'static str,
    // Corresponds to MiscMask in Enclave.xml
    misc_mask: &'static str,
}

impl DefaultConfig {
    fn init() -> Self {
        Self {
            // from OCCLUM_KERNEL_TCS_NUM defined in src/libos/src/entry/enclave.rs
            num_of_tcs_used_by_occlum_kernel: 5,
            num_of_cpus_max: 1024,
            // In order to operate on the User Region using EDMM API,
            // both MiscSelect[0] and MiscMask[0] need to be set to 1
            misc_select: "1",
            misc_mask: "0xFFFFFFFF",
        }
    }
}

fn main() {
    env_logger::init();

    let matches = App::new("gen_internal_conf")
        .version("0.2.0")
        // Input: JSON file which users may change
        .arg(
            Arg::with_name("user_json")
                .long("user_json")
                .value_name("input user json")
                .required(true)
                .validator(|f| match Path::new(&f).exists() {
                    true => Ok(()),
                    false => {
                        let err_message = f.to_owned() + " is not exist";
                        Err(err_message)
                    }
                })
                .takes_value(true),
        )
        .subcommand(
            SubCommand::with_name("gen_conf")
                .about("Generate image config")
                // Input: User's Secure Occlum FS image MAC
                .arg(
                    Arg::with_name("user_fs_mac")
                        .long("user_fs_mac")
                        .value_name("input MAC of user image fs")
                        .required(true)
                        .takes_value(true),
                )
                // Input: InitFS image MAC
                .arg(
                    Arg::with_name("init_fs_mac")
                        .long("init_fs_mac")
                        .value_name("input MAC of init image fs")
                        .required(true)
                        .takes_value(true),
                )
                // Output: JSON file used by libOS and users shouldn't touch
                .arg(
                    Arg::with_name("output_json")
                        .long("output_json")
                        .value_name("output json")
                        .required(true)
                        .validator(|f| match File::create(f) {
                            Ok(_) => Ok(()),
                            Err(e) => Err(e.to_string()),
                        })
                        .takes_value(true),
                )
                // Output: XML file used by Intel SGX SDK
                .arg(
                    Arg::with_name("sdk_xml")
                        .long("sdk_xml")
                        .value_name("output sdk's xml")
                        .required(true)
                        .validator(|f| match File::create(f) {
                            Ok(_e) => Ok(()),
                            Err(e) => Err(e.to_string()),
                        })
                        .takes_value(true),
                )
                // Input: InitFS image encrypted or not
                .arg(
                    Arg::with_name("encrypted")
                        .long("encrypted")
                        .value_name("InitFS image encrypted or not")
                        .required(true)
                        .takes_value(true),
                ),
        )
        .get_matches();

    let occlum_config_file_path = matches.value_of("user_json").unwrap();
    debug!(
        "Occlum config (json) file name {:?}",
        occlum_config_file_path
    );
    // Read the occlum configuration file
    let occlum_config_file =
        File::open(occlum_config_file_path).expect("The Occlum configuration file does not exist");
    let occlum_config: OcclumConfiguration = serde_json::from_reader(occlum_config_file)
        .expect("It is not a valid Occlum configuration file.");
    debug!("The occlum config is:{:?}", occlum_config);

    // Match subcommand
    if let Some(sub_matches) = matches.subcommand_matches("gen_conf") {
        let occlum_conf_user_fs_mac = sub_matches.value_of("user_fs_mac").unwrap();
        debug!("Occlum config user FS MAC {:?}", occlum_conf_user_fs_mac);

        let occlum_conf_init_fs_mac = sub_matches.value_of("init_fs_mac").unwrap();
        debug!("Occlum config init FS MAC {:?}", occlum_conf_init_fs_mac);

        let occlum_json_file_path = sub_matches.value_of("output_json").unwrap();
        debug!(
            "Genereated Occlum user config (json) file name {:?}",
            occlum_json_file_path
        );

        let enclave_config_file_path = sub_matches.value_of("sdk_xml").unwrap();
        debug!(
            "Enclave config (xml) file name {:?}",
            enclave_config_file_path
        );

        let image_encrypted = sub_matches
            .value_of("encrypted")
            .unwrap()
            .parse::<bool>()
            .unwrap();
        debug!("Occlum image is encrypted: {}", image_encrypted);

        // get the TCS number
        let tcs_num = occlum_config.resource_limits.num_of_cpus
            + DEFAULT_CONFIG.num_of_tcs_used_by_occlum_kernel;
        let tcs_min_pool = tcs_num;
        let tcs_max_num = std::cmp::max(DEFAULT_CONFIG.num_of_cpus_max, tcs_num);

        // get the kernel stack size
        let stack_max_size =
            parse_memory_size(&occlum_config.resource_limits.kernel_space_stack_size);
        if stack_max_size.is_err() {
            println!(
                "The kernel_space_stack_size \"{}\" is not correct.",
                occlum_config.resource_limits.kernel_space_stack_size
            );
            return;
        }
        let stack_min_size = stack_max_size;

        // get the kernel heap size
        let heap_init_size =
            parse_memory_size(&occlum_config.resource_limits.kernel_space_heap_size.init);
        if heap_init_size.is_err() {
            println!(
                "The kernel_space_heap_size.init \"{}\" is not correct.",
                occlum_config.resource_limits.kernel_space_heap_size.init
            );
            return;
        }
        let heap_min_size = heap_init_size;
        let heap_max_size =
            parse_memory_size(&occlum_config.resource_limits.kernel_space_heap_size.max);
        if heap_max_size.is_err() {
            println!(
                "The kernel_space_heap_size.max \"{}\" is not correct.",
                occlum_config.resource_limits.kernel_space_heap_size.max
            );
            return;
        }
        // get the user space size
        let user_space_init_size =
            parse_memory_size(&occlum_config.resource_limits.user_space_size.init);
        if user_space_init_size.is_err() {
            println!(
                "The user_space_size.init \"{}\" is not correct.",
                occlum_config.resource_limits.user_space_size.init
            );
            return;
        }
        let user_space_max_size =
            parse_memory_size(&occlum_config.resource_limits.user_space_size.max);
        if user_space_max_size.is_err() {
            println!(
                "The user_space_size.max \"{}\" is not correct.",
                occlum_config.resource_limits.user_space_size.max
            );
            return;
        }

        let kss_tuple = parse_kss_conf(&occlum_config);

        // Generate the enclave configuration
        let sgx_enclave_configuration = EnclaveConfiguration {
            ProdID: occlum_config.metadata.product_id,
            ISVSVN: occlum_config.metadata.version_number,
            StackMaxSize: stack_max_size.unwrap() as u64,
            StackMinSize: stack_min_size.unwrap() as u64,
            HeapInitSize: heap_init_size.unwrap() as u64,
            HeapMaxSize: heap_max_size.unwrap() as u64,
            HeapMinSize: heap_min_size.unwrap() as u64,
            TCSNum: tcs_num,
            TCSMinPool: tcs_min_pool,
            TCSMaxNum: tcs_max_num,
            TCSPolicy: 0, // TCS is bound to the untrusted thread
            DisableDebug: match occlum_config.metadata.debuggable {
                true => 0,
                false => 1,
            },
            MiscSelect: DEFAULT_CONFIG.misc_select.to_string(),
            MiscMask: DEFAULT_CONFIG.misc_mask.to_string(),
            // Use init size as the reserved memory region
            ReservedMemMaxSize: user_space_init_size.unwrap() as u64,
            ReservedMemMinSize: user_space_init_size.unwrap() as u64,
            ReservedMemInitSize: user_space_init_size.unwrap() as u64,
            ReservedMemExecutable: 1,
            // TODO: Enable this field when EDMM support is ready
            // UserRegionSize: user_space_max_size.unwrap() as u64,
            EnableKSS: kss_tuple.0,
            ISVEXTPRODID_H: kss_tuple.1,
            ISVEXTPRODID_L: kss_tuple.2,
            // ISV Family ID is reserved for saving conf MAC in Occlum build stage
            ISVFAMILYID_H: 0,
            ISVFAMILYID_L: 0,
            PKRU: occlum_config.metadata.pkru,
        };
        let enclave_config = serde_xml_rs::to_string(&sgx_enclave_configuration).unwrap();
        debug!("The enclave config:{:?}", enclave_config);

        // Generate app config, including "init" and user app
        let app_config = {
            let app_config = gen_app_config(
                occlum_config.entry_points,
                occlum_config.mount,
                occlum_conf_user_fs_mac.to_string(),
                occlum_conf_init_fs_mac.to_string(),
                image_encrypted,
            );
            if app_config.is_err() {
                println!("Mount configuration invalid: {:?}", app_config);
                return;
            }
            app_config.unwrap()
        };
        let untrusted_unix_socks = {
            if let Some(socks) = occlum_config.untrusted_unix_socks {
                Some(serde_json::to_value(socks).unwrap())
            } else {
                None
            }
        };
        let occlum_json_config = InternalOcclumJson {
            resource_limits: InternalResourceLimits {
                user_space_init_size: occlum_config
                    .resource_limits
                    .user_space_size
                    .init
                    .to_string(),
                user_space_max_size: occlum_config
                    .resource_limits
                    .user_space_size
                    .max
                    .to_string(),
            },
            process: OcclumProcess {
                default_stack_size: occlum_config.process.default_stack_size,
                default_heap_size: occlum_config.process.default_heap_size,
            },
            untrusted_unix_socks,
            env: occlum_config.env,
            app: app_config,
        };
        let occlum_json_str = serde_json::to_string_pretty(&occlum_json_config).unwrap();
        debug!("The Occlum.json config:\n{:?}", occlum_json_str);

        // Update the output file
        let mut enclave_config_file = File::create(enclave_config_file_path)
            .expect("Could not open the target Enclave configuration file.");
        enclave_config_file
            .write_all(enclave_config.as_bytes())
            .expect("Failed to update the Enclave configuration file.");

        let mut occlum_json = File::create(occlum_json_file_path)
            .expect("Could not open the output Occlum.json file.");
        occlum_json
            .write_all(occlum_json_str.as_bytes())
            .expect("Failed to update the output Occlum.json file.");
    } else {
        unreachable!();
    }
}

fn parse_memory_size(mem_str: &str) -> Result<usize, &str> {
    const UNIT2FACTOR: [(&str, usize); 5] = [
        ("KB", 1024),
        ("MB", 1024 * 1024),
        ("GB", 1024 * 1024 * 1024),
        ("TB", 1024 * 1024 * 1024 * 1024),
        ("B", 1),
    ];

    // Extract the unit part of the memory size
    let mem_str = mem_str.trim();
    let (mem_unit, unit_factor) = UNIT2FACTOR
        .iter()
        .position(|(mem_unit, _)| mem_str.ends_with(mem_unit))
        .ok_or("No unit")
        .map(|unit_i| &UNIT2FACTOR[unit_i])?;

    // Extract the value part of the memory size
    let mem_val = match mem_str[0..mem_str.len() - mem_unit.len()]
        .trim()
        .parse::<usize>()
    {
        Err(_) => {
            return Err("No number");
        }
        Ok(mem_val) => mem_val,
    };

    Ok(mem_val * unit_factor)
}

fn get_u64_id_high_and_low(id: &OcclumMetaID) -> (u64, u64) {
    let id_high = u64::from_str_radix(id.high.trim_start_matches("0x"), 16)
        .expect("64 bit hex string ID required, such as 0x1234567812345678");
    let id_low = u64::from_str_radix(id.low.trim_start_matches("0x"), 16)
        .expect("64 bit hex string ID required, such as 0x1234567812345678");

    (id_high, id_low)
}

// Return a tuple (EnableKSS, ISVEXTPRODID_H, ISVEXTPRODID_L)
fn parse_kss_conf(occlum_config: &OcclumConfiguration) -> (u32, u64, u64) {
    match occlum_config.metadata.enable_kss {
        true => {
            let ext_prod_id = get_u64_id_high_and_low(&occlum_config.metadata.ext_prod_id);

            (1, ext_prod_id.0, ext_prod_id.1)
        }
        false => (0, 0, 0),
    }
}

fn gen_app_config(
    entry_points: serde_json::Value,
    mount_conf: Vec<OcclumMount>,
    occlum_conf_user_fs_mac: String,
    occlum_conf_init_fs_mac: String,
    image_encrypted: bool,
) -> Result<serde_json::Value, &'static str> {
    let mut app_config: serde_json::Value = json!({
        "app": [
        {
            "stage": "init",
            "entry_points": [
                "/bin"
            ],
            "mount": [
                {
                    "target": "/",
                    "type": "unionfs",
                    "options": {
                        "layers": [
                            {
                                "target": "/",
                                "type": "sefs",
                                "source": "./build/initfs/__ROOT",
                                "options": {
                                    "MAC": ""
                                }
                            },
                            {
                                "target": "/",
                                "type": "sefs",
                                "source": "./run/initfs/__ROOT"
                            }
                        ]
                    }
                },
                {
                    "target": "/proc",
                    "type": "procfs"
                },
                {
                    "target": "/dev",
                    "type": "devfs"
                }
            ]
        },
        {
            "stage": "app",
            "entry_points": [],
            "encrypted": false,
            "mount": [
                {
                    "target": "/proc",
                    "type": "procfs"
                },
                {
                    "target": "/dev",
                    "type": "devfs"
                }
            ]
        }]
    });

    // Update init root mount fs MAC
    *app_config
        .pointer_mut("/app/0/mount/0/options/layers/0/options/MAC")
        .unwrap() = serde_json::Value::String(occlum_conf_init_fs_mac);

    // Update app entry points
    *app_config.pointer_mut("/app/1/entry_points").unwrap() = entry_points;

    debug!("User provided root mount config: {:?}", mount_conf);
    let mut root_mount_config = mount_conf;

    //Check the validity of the user provided root mount
    let root_mc = &mut root_mount_config
        .iter_mut()
        .find(|m| m.target == String::from("/") && m.type_ == String::from("unionfs"))
        .ok_or("the root UnionFS is not valid")?;
    if root_mc.options.layers.is_none() {
        return Err("the root UnionFS must be given layers");
    }
    let mut root_image_sefs_mc = root_mc
        .options
        .layers
        .as_mut()
        .unwrap()
        .iter_mut()
        .find(|m| {
            m.target == String::from("/")
                && m.type_ == String::from("sefs")
                && m.options.mac.is_some()
        })
        .ok_or("the image SEFS in layers is not valid")?;
    // Update app root mount fs MAC
    root_image_sefs_mc.options.mac = Some(occlum_conf_user_fs_mac);

    // Combine the user provided mount
    let mut mount_json = serde_json::to_value(root_mount_config).unwrap();
    let mut mount_array = mount_json.as_array_mut().unwrap();
    app_config["app"][1]["mount"]
        .as_array_mut()
        .unwrap()
        .append(&mut mount_array);

    // Update app encrypted tag
    *app_config.pointer_mut("/app/1/encrypted").unwrap() = image_encrypted.into();

    debug!("Occlum.json app config:\n{:?}", app_config);

    Ok(app_config["app"].to_owned())
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumConfiguration {
    resource_limits: OcclumResourceLimits,
    process: OcclumProcess,
    entry_points: serde_json::Value,
    env: serde_json::Value,
    #[serde(default)]
    untrusted_unix_socks: Option<Vec<UntrustedUnixSock>>,
    metadata: OcclumMetadata,
    mount: Vec<OcclumMount>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumResourceLimits {
    num_of_cpus: u32,
    kernel_space_stack_size: String,
    kernel_space_heap_size: DynamicMemorySize,
    user_space_size: DynamicMemorySize,
}

#[derive(Debug, PartialEq, Deserialize)]
struct DynamicMemorySize {
    init: String,
    max: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
struct OcclumProcess {
    default_stack_size: String,
    default_heap_size: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
struct UntrustedUnixSock {
    host: serde_json::Value,  // host path
    libos: serde_json::Value, // libos path
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumMetaID {
    high: String,
    low: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumMetadata {
    product_id: u32,
    version_number: u32,
    debuggable: bool,
    enable_kss: bool,
    ext_prod_id: OcclumMetaID,
    pkru: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OcclumMount {
    #[serde(rename = "type")]
    type_: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    options: OcclumMountOptions,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct OcclumMountOptions {
    #[serde(rename = "MAC")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<OcclumMount>>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub temporary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub async_sfs_total_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_cache_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    autokey_policy: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sefs_cache_size: Option<String>,
}

#[inline]
fn is_false(v: &bool) -> bool {
    !(*v)
}

#[inline]
fn is_default(option: &OcclumMountOptions) -> bool {
    let default_option: OcclumMountOptions = Default::default();
    option == &default_option
}

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Serialize)]
struct EnclaveConfiguration {
    ProdID: u32,
    ISVSVN: u32,
    StackMaxSize: u64,
    StackMinSize: u64,
    HeapInitSize: u64,
    HeapMaxSize: u64,
    HeapMinSize: u64,
    TCSNum: u32,
    TCSMaxNum: u32,
    TCSMinPool: u32,
    TCSPolicy: u32,
    DisableDebug: u32,
    MiscSelect: String,
    MiscMask: String,
    ReservedMemMaxSize: u64,
    ReservedMemMinSize: u64,
    ReservedMemInitSize: u64,
    ReservedMemExecutable: u32,
    // TODO: Enable this field when EDMM support is ready
    // UserRegionSize is the size of the region where users can
    // operate on using the EDMM APIs introduced since Intel SGX SDK 2.18
    // UserRegionSize: u64,
    EnableKSS: u32,
    ISVEXTPRODID_H: u64,
    ISVEXTPRODID_L: u64,
    ISVFAMILYID_H: u64,
    ISVFAMILYID_L: u64,
    PKRU: u32,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
struct InternalResourceLimits {
    user_space_init_size: String,
    user_space_max_size: String,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
struct InternalOcclumJson {
    resource_limits: InternalResourceLimits,
    process: OcclumProcess,
    env: serde_json::Value,
    untrusted_unix_socks: Option<serde_json::Value>,
    app: serde_json::Value,
}
