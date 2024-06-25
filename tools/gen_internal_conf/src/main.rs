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

const MISC_FOR_EDMM_PLATFORM: (&str, &str) = ("1", "0xFFFFFFFF");
const MISC_FOR_NON_EDMM_PLATFORM: (&str, &str) = ("0", "0");

struct DefaultConfig {
    // Corresponds to HeapMaxSize in Enclave.xml
    kernel_heap_max_size: &'static str,
    user_space_max_size: &'static str,
    tcs_init_num: u32,
    // Corresponds to TCSMaxNum in Enclave.xml
    tcs_max_num: u32,
    // Extra user region memory for SDK
    extra_user_region_for_sdk: &'static str,
}

impl DefaultConfig {
    fn init() -> Self {
        Self {
            kernel_heap_max_size: "1024MB",
            user_space_max_size: "16GB",
            tcs_init_num: 16,
            tcs_max_num: 4096,
            extra_user_region_for_sdk: "1GB",
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
                        let err_message = f + " is not exist";
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

    // If env is set, or Occlum.json `enable_edmm` field is set to true, EDMM is enabled.
    let instance_is_for_edmm_platform = {
        match std::env::var("INSTANCE_IS_FOR_EDMM_PLATFORM") {
            Ok(val) => val == "YES" || occlum_config.feature.enable_edmm,
            _ => unreachable!(),
        }
    };

    // Match subcommand
    if let Some(sub_matches) = matches.subcommand_matches("gen_conf") {
        let occlum_conf_user_fs_mac = sub_matches.value_of("user_fs_mac").unwrap();
        debug!("Occlum config user FS MAC {:?}", occlum_conf_user_fs_mac);

        let occlum_conf_init_fs_mac = sub_matches.value_of("init_fs_mac").unwrap();
        debug!("Occlum config init FS MAC {:?}", occlum_conf_init_fs_mac);

        let occlum_json_file_path = sub_matches.value_of("output_json").unwrap();
        debug!(
            "Generated Occlum user config (json) file name {:?}",
            occlum_json_file_path
        );

        let enclave_config_file_path = sub_matches.value_of("sdk_xml").unwrap();
        debug!(
            "Enclave config (xml) file name {:?}",
            enclave_config_file_path
        );

        println!(
            "Build on platform {} EDMM support",
            if instance_is_for_edmm_platform {
                "WITH"
            } else {
                "WITHOUT"
            }
        );

        debug!(
            "Enable IO_Uring feature with {:?} instances",
            occlum_config.feature.io_uring
        );

        debug!(
            "user config init num of threads = {:?}",
            occlum_config.resource_limits.init_num_of_threads
        );

        // For init TCS number, try to use the values provided by users. If not provided, use the default value
        let (tcs_init_num, tcs_min_pool, tcs_max_num) = {
            if instance_is_for_edmm_platform {
                let tcs_init_num = if let Some(ref init_num_of_threads) =
                    occlum_config.resource_limits.init_num_of_threads
                {
                    *init_num_of_threads
                } else {
                    // The user doesn't provide a value
                    std::cmp::min(
                        DEFAULT_CONFIG.tcs_init_num,
                        occlum_config.resource_limits.max_num_of_threads,
                    )
                };

                // For platforms with EDMM support, use the max value
                let tcs_max_num = std::cmp::max(
                    occlum_config.resource_limits.max_num_of_threads,
                    DEFAULT_CONFIG.tcs_max_num,
                );

                (tcs_init_num, tcs_init_num, tcs_max_num)
            } else {
                // For platforms without EDMM support (including SIM mode), use the "max_num_of_threads" provided by user
                let tcs_max_num = occlum_config.resource_limits.max_num_of_threads;
                (tcs_max_num, tcs_max_num, tcs_max_num)
            }
        };

        debug!(
            "tcs init num: {}, tcs_min_pool: {}, tcs_max_num: {}",
            tcs_init_num, tcs_min_pool, tcs_max_num
        );
        if tcs_init_num > tcs_max_num {
            println!(
                "init_num_of_threads: {:?}, max_num_of_threads: {:?}, wrong configuration",
                occlum_config.resource_limits.init_num_of_threads,
                occlum_config.resource_limits.max_num_of_threads,
            );
            return;
        }

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

        let (kernel_heap_init_size, kernel_heap_max_size) = {
            let heap_init_size = {
                let heap_init_size =
                    parse_memory_size(&occlum_config.resource_limits.kernel_space_heap_size);
                if heap_init_size.is_err() {
                    println!(
                        "The kernel_space_heap_size \"{}\" is not correct.",
                        occlum_config.resource_limits.kernel_space_heap_size
                    );
                    return;
                }
                heap_init_size.unwrap()
            };

            let optional_config_heap_max_size = {
                if let Some(ref heap_max_size) =
                    occlum_config.resource_limits.kernel_space_heap_max_size
                {
                    let config_kernel_heap_max_size = parse_memory_size(&heap_max_size);
                    if config_kernel_heap_max_size.is_err() {
                        println!(
                            "The kernel_space_heap_max_size \"{}\" is not correct.",
                            heap_max_size
                        );
                        return;
                    }
                    config_kernel_heap_max_size.ok()
                } else {
                    None
                }
            };

            debug!(
                "optional_config_heap_max_size = {:?}",
                optional_config_heap_max_size
            );

            match optional_config_heap_max_size {
                Some(heap_max_size) => {
                    if instance_is_for_edmm_platform {
                        let heap_max_size = std::cmp::max(
                            heap_max_size,
                            parse_memory_size(DEFAULT_CONFIG.kernel_heap_max_size).unwrap(),
                        );
                        (heap_init_size, heap_max_size)
                    } else {
                        // User specified heap_max but no EDMM support, use user specified as the heap value
                        (heap_max_size, heap_max_size)
                    }
                }
                None => {
                    if instance_is_for_edmm_platform {
                        let heap_max_size = std::cmp::max(
                            heap_init_size,
                            parse_memory_size(DEFAULT_CONFIG.kernel_heap_max_size).unwrap(),
                        );
                        (heap_init_size, heap_max_size)
                    } else {
                        (heap_init_size, heap_init_size)
                    }
                }
            }
        };
        if kernel_heap_init_size > kernel_heap_max_size {
            println!(
                "kernel_space_heap_size: {:?}, kernel_space_heap_max_size: {:?}, wrong configuration",
                occlum_config.resource_limits.kernel_space_heap_size, occlum_config.resource_limits.kernel_space_heap_max_size,
            );
            return;
        }
        debug!(
            "kernel heap init size = {}, kernel heap max size = {}",
            kernel_heap_init_size, kernel_heap_max_size
        );
        assert!(kernel_heap_max_size >= kernel_heap_init_size);

        let (config_user_space_init_size, config_user_space_max_size) = {
            let user_space_init_size = {
                let user_space_init_size =
                    parse_memory_size(&occlum_config.resource_limits.user_space_size);
                if user_space_init_size.is_err() {
                    println!(
                        "The user_space_size \"{}\" is not correct.",
                        occlum_config.resource_limits.user_space_size
                    );
                    return;
                }
                user_space_init_size.unwrap()
            };

            let optional_config_user_space_max_size = {
                if let Some(ref user_space_max_size) =
                    occlum_config.resource_limits.user_space_max_size
                {
                    let config_user_space_max_size = parse_memory_size(&user_space_max_size);
                    if config_user_space_max_size.is_err() {
                        println!(
                            "The user_space_max_size \"{}\" is not correct.",
                            user_space_max_size
                        );
                        return;
                    }
                    config_user_space_max_size.ok()
                } else {
                    None
                }
            };
            debug!(
                "optional_config_user_space_max_size = {:?}",
                optional_config_user_space_max_size
            );

            let user_space_max_size = match optional_config_user_space_max_size {
                Some(user_space_max_size) => {
                    if instance_is_for_edmm_platform {
                        std::cmp::max(
                            user_space_max_size,
                            parse_memory_size(DEFAULT_CONFIG.user_space_max_size).unwrap(),
                        )
                    } else {
                        // Without EDMM support, just use user-provided value
                        user_space_max_size
                    }
                }
                None => {
                    if instance_is_for_edmm_platform {
                        std::cmp::max(
                            user_space_init_size,
                            parse_memory_size(DEFAULT_CONFIG.user_space_max_size).unwrap(),
                        )
                    } else {
                        user_space_init_size
                    }
                }
            };
            (user_space_init_size, user_space_max_size)
        };
        if config_user_space_init_size > config_user_space_max_size {
            println!(
                "user_space_size: {:?}, user_space_max_size: {:?}, wrong configuration",
                occlum_config.resource_limits.user_space_size,
                occlum_config.resource_limits.user_space_max_size,
            );
            return;
        }

        debug!(
            "config user space init size = {},config user space max size = {}",
            config_user_space_init_size, config_user_space_max_size
        );
        assert!(config_user_space_init_size <= config_user_space_max_size);

        // Calculate the actual memory size for different regions
        let (reserved_mem_size, user_region_mem_size) = {
            if instance_is_for_edmm_platform {
                // For platforms with EDMM support, we need extra memory for SDK usage. This might be fixed by SGX SDK in the future.
                let extra_user_region = parse_memory_size(DEFAULT_CONFIG.extra_user_region_for_sdk);
                if extra_user_region.is_err() {
                    println!("The extra_user_region_for_sdk in default config is not correct.");
                    return;
                }
                let user_region_mem_size =
                    if config_user_space_max_size == config_user_space_init_size {
                        // SDK still need user region to track the EMA.
                        config_user_space_max_size
                    } else {
                        config_user_space_max_size + extra_user_region.unwrap()
                    };

                (
                    config_user_space_init_size as u64,
                    Some(user_region_mem_size as u64),
                )
            } else {
                // For platforms without EDMM support, use the max value for the user space
                let reserved_mem_size = config_user_space_max_size;
                (reserved_mem_size as u64, None)
            }
        };

        debug!(
            "reserved memory size = {:?}, user_region_memory size = {:?}",
            reserved_mem_size, user_region_mem_size
        );

        #[cfg(feature = "ms_buffer")]
        let marshal_buffer_size = if occlum_config.resource_limits.marshal_buffer_size.is_some() {
            let marshal_buffer_size = parse_memory_size(
                occlum_config
                    .resource_limits
                    .marshal_buffer_size
                    .as_ref()
                    .unwrap(),
            );
            if marshal_buffer_size.is_err() {
                println!(
                    "The marshal_buffer_size \"{}\" is not correct.",
                    occlum_config.resource_limits.marshal_buffer_size.unwrap()
                );
                return;
            }
            marshal_buffer_size.unwrap()
        } else {
            0x10_0000
        };

        // Check validity of cache and disk size in mount options
        const CACHE_PAGE_SIZE: usize = 0x1000;
        const MIN_CACHE_SIZE: usize = 48 * CACHE_PAGE_SIZE; // 192KB
        const MIN_DISK_SIZE: usize = 5 * 1024usize.pow(3); // 5GB
        for mount in &occlum_config.mount {
            if let Some(cache_size_str) = mount.options.cache_size.as_ref() {
                let cache_size = {
                    let cache_size = parse_memory_size(cache_size_str);
                    if cache_size.is_err() {
                        println!("The cache_size \"{}\" is not correct.", cache_size_str);
                        return;
                    }
                    cache_size.unwrap()
                };
                if cache_size < MIN_CACHE_SIZE
                    || cache_size % CACHE_PAGE_SIZE != 0
                    || cache_size > kernel_heap_max_size
                {
                    println!(
                        "Invalid cache_size \"{}\". The cache_size must be 1. larger than the minimum size \"{}\", \
                        2. aligned with cache page size \"{}\", \
                        3. smaller than the kernel_heap_max_size \"{}\".",
                        cache_size, MIN_CACHE_SIZE, CACHE_PAGE_SIZE, kernel_heap_max_size
                    );
                    return;
                }
            }
            if mount.type_ == String::from("ext2") && mount.options.disk_size.is_none() {
                println!("The disk_size must be specified for Ext2.");
                return;
            }
            if let Some(disk_size_str) = mount.options.disk_size.as_ref() {
                let disk_size = {
                    let disk_size = parse_memory_size(disk_size_str);
                    if disk_size.is_err() {
                        println!("The disk_size \"{}\" is not correct.", disk_size_str);
                        return;
                    }
                    disk_size.unwrap()
                };
                if disk_size < MIN_DISK_SIZE {
                    println!(
                        "The disk_size \"{}\" is too small, minimum size is \"{}\".",
                        disk_size, MIN_DISK_SIZE
                    );
                    return;
                }
            }
        }

        let kss_tuple = parse_kss_conf(&occlum_config);

        let (misc_select, misc_mask) = if instance_is_for_edmm_platform {
            MISC_FOR_EDMM_PLATFORM
        } else {
            MISC_FOR_NON_EDMM_PLATFORM
        };
        debug!(
            "misc_select = {:?}, misc_mask = {:?}",
            misc_select, misc_mask
        );

        // Generate the enclave configuration
        let sgx_enclave_configuration = EnclaveConfiguration {
            ProdID: occlum_config.metadata.product_id,
            ISVSVN: occlum_config.metadata.version_number,
            StackMaxSize: stack_max_size.unwrap() as u64,
            StackMinSize: stack_max_size.unwrap() as u64, // just use the same size as max size
            HeapInitSize: kernel_heap_init_size as u64,
            HeapMaxSize: kernel_heap_max_size as u64,
            HeapMinSize: kernel_heap_init_size as u64,
            TCSNum: tcs_init_num,
            TCSMinPool: tcs_min_pool,
            TCSMaxNum: tcs_max_num,
            TCSPolicy: 0,
            DisableDebug: match occlum_config.metadata.debuggable {
                true => 0,
                false => 1,
            },
            MiscSelect: misc_select.to_string(),
            MiscMask: misc_mask.to_string(),
            ReservedMemMaxSize: reserved_mem_size,
            ReservedMemMinSize: reserved_mem_size,
            ReservedMemInitSize: reserved_mem_size,
            ReservedMemExecutable: 1,
            UserRegionSize: user_region_mem_size,
            #[cfg(feature = "ms_buffer")]
            MarshalBufferSize: marshal_buffer_size as u64,
            EnableKSS: kss_tuple.0,
            ISVEXTPRODID_H: kss_tuple.1,
            ISVEXTPRODID_L: kss_tuple.2,
            ISVFAMILYID_H: kss_tuple.3,
            ISVFAMILYID_L: kss_tuple.4,
            PKRU: occlum_config.feature.pkru,
            AMX: occlum_config.feature.amx,
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
            );
            if app_config.is_err() {
                println!("Mount configuration invalid: {:?}", app_config);
                return;
            }
            app_config.unwrap()
        };

        // If the user doesn't provide a value, set it false unless it is release enclave.
        // If the user provides a value, just use it.
        let disable_log = {
            if occlum_config.metadata.disable_log.is_none() {
                if occlum_config.metadata.debuggable {
                    false
                } else {
                    true
                }
            } else {
                occlum_config.metadata.disable_log.unwrap()
            }
        };

        let occlum_json_config = InternalOcclumJson {
            resource_limits: InternalResourceLimits {
                user_space_init_size: config_user_space_init_size.to_string() + "B",
                user_space_max_size: config_user_space_max_size.to_string() + "B",
            },
            process: OcclumProcess {
                default_stack_size: occlum_config.process.default_stack_size,
                default_heap_size: occlum_config.process.default_heap_size,
                default_mmap_size: occlum_config.process.default_mmap_size,
            },
            env: occlum_config.env,
            disable_log: disable_log,
            app: app_config,
            feature: occlum_config.feature.clone(),
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

// Return a tuple (EnableKSS, ISVEXTPRODID_H, ISVEXTPRODID_L, ISVFAMILYID_H, ISVFAMILYID_L)
fn parse_kss_conf(occlum_config: &OcclumConfiguration) -> (u32, u64, u64, u64, u64) {
    match occlum_config.metadata.enable_kss {
        true => {
            let ext_prod_id = get_u64_id_high_and_low(&occlum_config.metadata.ext_prod_id);
            let family_id = get_u64_id_high_and_low(&occlum_config.metadata.family_id);

            (1, ext_prod_id.0, ext_prod_id.1, family_id.0, family_id.1)
        }
        false => (0, 0, 0, 0, 0),
    }
}

fn gen_app_config(
    entry_points: serde_json::Value,
    mount_conf: Vec<OcclumMount>,
    occlum_conf_user_fs_mac: String,
    occlum_conf_init_fs_mac: String,
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
    let root_image_sefs_mc = root_mc
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

    debug!("Occlum.json app config:\n{:?}", app_config);

    Ok(app_config["app"].to_owned())
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumConfiguration {
    resource_limits: OcclumResourceLimits,
    process: OcclumProcess,
    entry_points: serde_json::Value,
    env: serde_json::Value,
    metadata: OcclumMetadata,
    feature: OcclumFeature,
    mount: Vec<OcclumMount>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct OcclumResourceLimits {
    #[serde(default)]
    init_num_of_threads: Option<u32>,
    max_num_of_threads: u32,
    kernel_space_heap_size: String,
    #[serde(default)]
    kernel_space_heap_max_size: Option<String>,
    kernel_space_stack_size: String,
    user_space_size: String,
    #[serde(default)]
    user_space_max_size: Option<String>,
    #[cfg(feature = "ms_buffer")]
    marshal_buffer_size: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
struct OcclumProcess {
    default_stack_size: String,
    default_heap_size: String,
    default_mmap_size: String,
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
    #[serde(default)]
    disable_log: Option<bool>,
    enable_kss: bool,
    family_id: OcclumMetaID,
    ext_prod_id: OcclumMetaID,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
struct OcclumFeature {
    #[serde(default)]
    amx: u32,
    #[serde(default)]
    pkru: u32,
    #[serde(default)]
    io_uring: u32,
    #[serde(default)]
    enable_edmm: bool,
    #[serde(default)]
    enable_posix_shm: bool,
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
    pub cache_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_size: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    UserRegionSize: Option<u64>,
    #[cfg(feature = "ms_buffer")]
    MarshalBufferSize: u64,
    EnableKSS: u32,
    ISVEXTPRODID_H: u64,
    ISVEXTPRODID_L: u64,
    ISVFAMILYID_H: u64,
    ISVFAMILYID_L: u64,
    PKRU: u32,
    AMX: u32,
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
    disable_log: bool,
    app: serde_json::Value,
    feature: OcclumFeature,
}
