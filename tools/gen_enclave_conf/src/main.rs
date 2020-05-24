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
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    env_logger::init();

    let matches = App::new("gen_enclave_conf")
        .version("0.1.0")
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
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
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .required(true)
                .validator(|f| match File::create(f) {
                    Ok(_e) => Ok(()),
                    Err(e) => Err(e.to_string()),
                })
                .takes_value(true),
        )
        .get_matches();

    let occlum_config_file_path = matches.value_of("input").unwrap();
    debug!(
        "Occlum config (json) file name {:?}",
        occlum_config_file_path
    );

    let enclave_config_file_path = matches.value_of("output").unwrap();
    debug!(
        "Enclave config (xml) file name {:?}",
        enclave_config_file_path
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

    let sgx_enclave_configuration = EnclaveConfiguration {
        ProdID: occlum_config.metadata.product_id,
        ISVSVN: occlum_config.metadata.version_number,
        StackMaxSize: stack_max_size.unwrap() as u64,
        HeapMaxSize: heap_max_size.unwrap() as u64,
        TCSNum: occlum_config.resource_limits.max_num_of_threads,
        TCSPolicy: 1,
        DisableDebug: match occlum_config.metadata.debuggable {
            true => 0,
            false => 1,
        },
        MiscSelect: "0".to_string(),
        MiscMask: "0xFFFFFFFF".to_string(),
    };

    // Generate the enclave configuration
    let enclave_config = serde_xml_rs::to_string(&sgx_enclave_configuration).unwrap();
    debug!("The enclave config:{:?}", enclave_config);

    // Update the output file
    let mut enclave_config_file = File::create(enclave_config_file_path)
        .expect("Could not open the target Enclave configuration file.");
    enclave_config_file
        .write_all(enclave_config.as_bytes())
        .expect("Failed to update the Enclave configuration file.");
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

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumConfiguration {
    metadata: OcclumMetadata,
    resource_limits: OcclumResourceLimits,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumMetadata {
    product_id: u32,
    version_number: u32,
    debuggable: bool,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumResourceLimits {
    max_num_of_threads: u32,
    kernel_space_heap_size: String,
    kernel_space_stack_size: String,
    user_space_size: String,
}

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Serialize)]
struct EnclaveConfiguration {
    ProdID: u32,
    ISVSVN: u32,
    StackMaxSize: u64,
    HeapMaxSize: u64,
    TCSNum: u32,
    TCSPolicy: u32,
    DisableDebug: u32,
    MiscSelect: String,
    MiscMask: String,
}
