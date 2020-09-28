extern crate clap;
extern crate env_logger;
extern crate log;
extern crate regex;
extern crate serde;
extern crate serde_derive;
extern crate serde_xml_rs;

use clap::{App, Arg};
use log::debug;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::path::Path;

// This is not really used anymore.
// Just keep it here in case we need it in the future.
const OCCLUM_INSTANCE_DIR: &str = ".";

fn main() {
    env_logger::init();

    let matches = App::new("gen_internal_conf")
        .version("0.2.0")
        // Input: JSON file which users may change
        .arg(
            Arg::with_name("user_json")
                .long("user_json")
                .value_name("input: user_json")
                .required(true)
                .validator(|f| match Path::new(&f).exists() {
                    true => Ok(()),
                    false => {
                        let err_message = String::from(f) + " is not exist";
                        Err(err_message)
                    }
                })
                .takes_value(true),
        )
        // Input: Secure Occlum FS image MAC
        .arg(
            Arg::with_name("fs_mac")
                .long("fs_mac")
                .value_name("input: fs_mac")
                .required(true)
                .takes_value(true),
        )
        // Output: XML file used by Intel SGX SDK
        .arg(
            Arg::with_name("sdk_xml")
                .long("sdk_xml")
                .value_name("output: sdk_xml")
                .required(true)
                .validator(|f| match File::create(f) {
                    Ok(_e) => Ok(()),
                    Err(e) => Err(e.to_string()),
                })
                .takes_value(true),
        )
        // Output: JSON file used by libOS and users shouldn't touch
        .arg(
            Arg::with_name("sys_json")
                .long("sys_json")
                .value_name("output: sys_json")
                .required(true)
                .validator(|f| match File::create(f) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                })
                .takes_value(true),
        )
        .get_matches();

    let occlum_config_file_path = matches.value_of("user_json").unwrap();
    debug!(
        "Occlum config (json) file name {:?}",
        occlum_config_file_path
    );

    let occlum_conf_root_fs_mac = matches.value_of("fs_mac").unwrap();
    debug!(
        "Occlum config root FS MAC {:?}",
        occlum_conf_root_fs_mac
    );

    let enclave_config_file_path = matches.value_of("sdk_xml").unwrap();
    debug!(
        "Enclave config (xml) file name {:?}",
        enclave_config_file_path
    );

    let occlum_internal_json_file_path = matches.value_of("sys_json").unwrap();
    debug!(
        "Genereated Occlum internal config (json) file name {:?}",
        occlum_internal_json_file_path
    );

    // Read the occlum configuration file
    let occlum_config_file =
        File::open(occlum_config_file_path).expect("The Occlum configuration file does not exist");
    let occlum_config: OcclumConfiguration = serde_json::from_reader(occlum_config_file)
        .expect("It is not a valid Occlum configuration file.");
    debug!("The occlum config is:{:?}", occlum_config);

    // get the kernel stack size
    let stack_max_size = parse_memory_size(&occlum_config.resource_limits.kernel_space_stack_size);
    if stack_max_size.is_err() {
        println!(
            "The kernel_space_stack_size \"{}\" is not correct.",
            occlum_config.resource_limits.kernel_space_stack_size
        );
        return;
    }

    // get the kernel heap size
    let heap_max_size = parse_memory_size(&occlum_config.resource_limits.kernel_space_heap_size);
    if heap_max_size.is_err() {
        println!(
            "The kernel_space_heap_size \"{}\" is not correct.",
            occlum_config.resource_limits.kernel_space_heap_size
        );
        return;
    }

    // get the user space size
    let user_space_size = parse_memory_size(&occlum_config.resource_limits.user_space_size);
    if user_space_size.is_err() {
        println!(
            "The user_space_size \"{}\" is not correct.",
            occlum_config.resource_limits.user_space_size
        );
        return;
    }

    // Generate the enclave configuration
    let sgx_enclave_configuration = EnclaveConfiguration {
        ProdID: occlum_config.metadata.product_id,
        ISVSVN: occlum_config.metadata.version_number,
        StackMaxSize: stack_max_size.unwrap() as u64,
        StackMinSize: stack_max_size.unwrap() as u64, // just use the same size as max size
        HeapMaxSize: heap_max_size.unwrap() as u64,
        HeapMinSize: heap_max_size.unwrap() as u64, // just use the same size as max size
        TCSNum: occlum_config.resource_limits.max_num_of_threads,
        TCSPolicy: 1,
        DisableDebug: match occlum_config.metadata.debuggable {
            true => 0,
            false => 1,
        },
        MiscSelect: "0".to_string(),
        MiscMask: "0xFFFFFFFF".to_string(),
        ReservedMemMaxSize: user_space_size.unwrap() as u64,
        ReservedMemMinSize: user_space_size.unwrap() as u64,
        ReservedMemInitSize: user_space_size.unwrap() as u64,
        ReservedMemExecutable: 1,
    };

    let enclave_config = serde_xml_rs::to_string(&sgx_enclave_configuration).unwrap();
    debug!("The enclave config:{:?}", enclave_config);

    // Generate internal Occlum.json - "sys_json"
    let internal_occlum_json_config = InternalOcclumJson {
        resource_limits: InternalResourceLimits {
            user_space_size: occlum_config.resource_limits.user_space_size.to_string(),
        },
        process: OcclumProcess {
            default_stack_size: occlum_config.process.default_stack_size,
            default_heap_size: occlum_config.process.default_heap_size,
            default_mmap_size: occlum_config.process.default_mmap_size,
        },
        entry_points: occlum_config.entry_points,
        env: occlum_config.env,
        mount: gen_mount_config(occlum_conf_root_fs_mac.to_string()),
    };
    let internal_occlum_json_str =
        serde_json::to_string_pretty(&internal_occlum_json_config).unwrap();
    debug!("The internal Occlum.json config:\n{:?}", internal_occlum_json_str);

    // Update the output file
    let mut enclave_config_file = File::create(enclave_config_file_path)
        .expect("Could not open the target Enclave configuration file.");
    enclave_config_file
        .write_all(enclave_config.as_bytes())
        .expect("Failed to update the Enclave configuration file.");

    let mut internal_occlum_json = File::create(occlum_internal_json_file_path)
        .expect("Could not open the internal Occlum.json file.");
    internal_occlum_json
        .write_all(internal_occlum_json_str.as_bytes())
        .expect("Failed to update the internal Occlum.json file.");
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
        .ok_or_else(|| "No unit")
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

fn gen_mount_config(occlum_conf_root_fs_mac: String) -> serde_json::Value {
    let mut internal_mount_config: serde_json::Value = json!({
        "mount": [
            {
                "target": "/",
                "type": "unionfs",
                "options": {
                    "layers": [
                        {
                            "target": "/",
                            "type": "sefs",
                            "source": "",
                            "options": {
                                "integrity_only": true,
                                "MAC": ""
                            }
                        },
                        {
                            "target": "/",
                            "type": "sefs",
                            "source": ""
                        }
                    ]
                }
            },
            {
                "target": "/host",
                "type": "hostfs",
                "source": "."
            },
            {
                "target": "/tmp",
                "type": "sefs",
                "source": "",
                "options": {
                    "temporary": true
                }
            }
        ]
    });

    let unionfs_base_source_path = format!("{}{}", OCCLUM_INSTANCE_DIR, "/build/mount/__ROOT");
    let unionfs_run_source_path = format!("{}{}", OCCLUM_INSTANCE_DIR, "/run/mount/__ROOT");
    let tmp_run_source_path = format!("{}{}", OCCLUM_INSTANCE_DIR, "/run/mount/tmp");

    *internal_mount_config
        .pointer_mut("/mount/0/options/layers/0/source")
        .unwrap() = serde_json::Value::String(unionfs_base_source_path);
    *internal_mount_config
        .pointer_mut("/mount/0/options/layers/0/options/MAC")
        .unwrap() = serde_json::Value::String(occlum_conf_root_fs_mac);
    *internal_mount_config
        .pointer_mut("/mount/0/options/layers/1/source")
        .unwrap() = serde_json::Value::String(unionfs_run_source_path);
    *internal_mount_config
        .pointer_mut("/mount/2/source")
        .unwrap() = serde_json::Value::String(tmp_run_source_path);

    debug!("internal Occlum.json mount config:\n{:?}", internal_mount_config);

    internal_mount_config["mount"].to_owned()
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumConfiguration {
    resource_limits: OcclumResourceLimits,
    process: OcclumProcess,
    entry_points: serde_json::Value,
    env: serde_json::Value,
    metadata: OcclumMetadata,
    mount: serde_json::Value,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumResourceLimits {
    max_num_of_threads: u32,
    kernel_space_heap_size: String,
    kernel_space_stack_size: String,
    user_space_size: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct OcclumProcess {
    default_stack_size: String,
    default_heap_size: String,
    default_mmap_size: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumMetadata {
    product_id: u32,
    version_number: u32,
    debuggable: bool,
}

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Serialize)]
struct EnclaveConfiguration {
    ProdID: u32,
    ISVSVN: u32,
    StackMaxSize: u64,
    StackMinSize: u64,
    HeapMaxSize: u64,
    HeapMinSize: u64,
    TCSNum: u32,
    TCSPolicy: u32,
    DisableDebug: u32,
    MiscSelect: String,
    MiscMask: String,
    ReservedMemMaxSize: u64,
    ReservedMemMinSize: u64,
    ReservedMemInitSize: u64,
    ReservedMemExecutable: u32,
}

#[derive(Debug, PartialEq, Serialize)]
struct InternalResourceLimits {
    user_space_size: String,
}

#[derive(Debug, PartialEq, Serialize)]
struct InternalOcclumJson {
    resource_limits: InternalResourceLimits,
    process: OcclumProcess,
    entry_points: serde_json::Value,
    env: serde_json::Value,
    mount: serde_json::Value,
}
