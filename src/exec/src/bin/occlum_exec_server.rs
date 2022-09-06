extern crate futures;
extern crate grpc;
extern crate libc;
extern crate occlum_exec;
extern crate protobuf;
#[macro_use]
extern crate log;
use clap::{App, Arg};
use futures::executor;
use grpc::prelude::*;
use grpc::ClientConf;
use occlum_exec::occlum_exec::HealthCheckRequest;
use occlum_exec::occlum_exec_grpc::{OcclumExecClient, OcclumExecServer};
use occlum_exec::server::{OcclumExecImpl, ServerStatus};
use occlum_exec::DEFAULT_SOCK_FILE;
use std::env;
use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

//Checks the server status, if the server is running return true, else recover the socket file and return false.
fn is_server_running(sock_file: &str) -> bool {
    if let Err(e) = std::fs::File::open(sock_file) {
        debug!("failed to open the sock_file {:?}", e);

        if e.kind() == std::io::ErrorKind::NotFound {
            return false;
        }
    }

    let client = OcclumExecClient::new_plain_unix(sock_file, ClientConf::new())
        .expect("failed to create UDS client");

    let resp = executor::block_on(
        client
            .status_check(
                grpc::RequestOptions::new(),
                HealthCheckRequest {
                    ..Default::default()
                },
            )
            .join_metadata_result(),
    );

    if let Ok(_) = resp {
        debug!("another server is running.");
        true
    } else {
        debug!("delete the useless socket file.");
        std::fs::remove_file(sock_file).expect("could not remove socket file");
        false
    }
}

fn main() -> Result<(), i32> {
    let matches = App::new("Occlum_server")
        .version("0.1.0")
        .arg(
            Arg::with_name("instance_dir")
                .short('d')
                .long("instance_dir")
                .takes_value(true)
                .default_value("./")
                .help("The Occlum instance dir."),
        )
        .get_matches();

    // Set the instance_dir as the current dir
    let instance_dir = Path::new(matches.value_of("instance_dir").unwrap());
    assert!(env::set_current_dir(&instance_dir).is_ok());

    //If the server already startted, then return
    if is_server_running(DEFAULT_SOCK_FILE) {
        println!("server stared");
        return Ok(());
    }

    let server_status = Arc::new((Mutex::new(ServerStatus::default()), Condvar::new()));

    let service_def = OcclumExecServer::new_service_def(
        OcclumExecImpl::new_and_save_execution_lock(server_status.clone()),
    );
    let mut server_builder = grpc::ServerBuilder::new_plain();
    server_builder.add_service(service_def);
    match server_builder.http.set_unix_addr(DEFAULT_SOCK_FILE) {
        Ok(_) => {}
        Err(e) => {
            debug!("{:?}", e);
            return Err(-1);
        }
    };

    if let Ok(server) = server_builder.build() {
        if let Err(_) = rust_occlum_pal_init() {
            let (status, _) = &*server_status;
            status.lock().unwrap().set_error();
            return Err(-1);
        }
        //server is running
        println!("server stared on addr {}", server.local_addr());
        let (lock, cvar) = &*server_status;
        let mut status = lock.lock().unwrap();
        // *server_stopped = false;
        status.set_running();
        while status.is_running() {
            status = cvar.wait(status).unwrap();
        }
        rust_occlum_pal_destroy()?;
        println!("server stopped");
    } else {
        println!("server build failed");
        return Err(-1);
    }

    Ok(())
}

extern "C" {
    /*
     * @brief Initialize an Occlum enclave
     *
     * @param attr  Mandatory input. Attributes for Occlum.
     *
     * @retval If 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_init(attr: *const occlum_pal_attr_t) -> i32;

    /*
     * @brief Destroy the Occlum enclave
     *
     * @retval if 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_destroy() -> i32;
}

#[repr(C)]
/// Occlum PAL attributes. Defined by occlum pal.
pub struct occlum_pal_attr_t {
    /// Occlum instance directory.
    ///
    /// Specifies the path of an Occlum instance directory, which is usually created with the
    /// `occlum new` command. The default value is "."; that is, the current working directory
    /// is the Occlum instance directory.
    pub instance_dir: *const libc::c_char,
    /// Log level.
    ///
    /// Specifies the log level of Occlum LibOS. Valid values: "off", "error",
    /// "warn", "info", and "trace". Case insensitive.
    ///
    /// Optional field. If NULL, the LibOS will treat it as "off".
    pub log_level: *const libc::c_char,
}

/// Loads and initializes the Occlum enclave image
fn rust_occlum_pal_init() -> Result<(), i32> {
    let instance_dir = OsString::from(".\0");
    let mut log_level = OsString::from("off\0");
    if let Some(val) = env::var_os("OCCLUM_LOG_LEVEL") {
        log_level = val;
        log_level.push("\0");
    };
    debug!("{:?} {:?}", instance_dir, log_level);

    let occlum_pal_attribute = occlum_pal_attr_t {
        instance_dir: CStr::from_bytes_with_nul(instance_dir.as_bytes())
            .unwrap()
            .as_ptr(),
        log_level: CStr::from_bytes_with_nul(log_level.as_bytes())
            .unwrap()
            .as_ptr(),
    };
    let rust_object = Box::new(&occlum_pal_attribute);

    let ret = unsafe { occlum_pal_init(*rust_object) };
    match ret {
        0 => Ok(()),
        _ => Err(ret),
    }
}

///Destroyes the Occlum enclave image
fn rust_occlum_pal_destroy() -> Result<(), i32> {
    let ret = unsafe { occlum_pal_destroy() };
    match ret {
        0 => Ok(()),
        _ => Err(ret),
    }
}
