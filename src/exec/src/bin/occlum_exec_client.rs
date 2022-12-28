extern crate clap;
extern crate env_logger;
extern crate futures;
extern crate grpc;
extern crate occlum_exec;
extern crate protobuf;
extern crate signal_hook;
#[macro_use]
extern crate log;

use clap::{App, Arg};
use futures::executor;
use grpc::prelude::*;
use grpc::ClientConf;
use occlum_exec::occlum_exec::{
    ExecCommRequest, ExecCommResponse_ExecutionStatus, GetResultRequest,
    GetResultResponse_ExecutionStatus, HealthCheckRequest, HealthCheckResponse_ServingStatus,
    KillProcessRequest, StopRequest,
};
use occlum_exec::occlum_exec_grpc::OcclumExecClient;
use occlum_exec::{DEFAULT_SERVER_FILE, DEFAULT_SERVER_TIMER, DEFAULT_SOCK_FILE};
use protobuf::RepeatedField;
use sendfd::SendWithFd;
use signal_hook::consts::{SIGINT, SIGKILL, SIGQUIT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::cmp;
use std::env;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use tempdir::TempDir;

/// Execute the command on server
///
/// # Examples
///
/// use occlum_exec::occlum_exec_grpc::OcclumExecClient;
///
/// let client = OcclumExecClient::new_plain_unix(&sock_file, ClientConf::new()).unwrap();
/// let let occlum_exec: Vec<String> = vec!["/bin/hello_world".to_String(), "".to_String()];
/// let process_id = exec_command(&client, &occlum_exec[0], &occlum_exec[1..]);
///
fn exec_command(
    client: &OcclumExecClient,
    command: &str,
    parameters: &[&str],
    envs: &[&str],
) -> Result<i32, String> {
    debug!("exec_command {:?} {:?} {:?}", command, parameters, envs);

    let mut parameter_list = RepeatedField::default();
    for p in parameters {
        parameter_list.push(p.to_string());
    }

    let mut environments_list = RepeatedField::default();
    for env in envs {
        environments_list.push(env.to_string());
    }

    let tmp_dir = TempDir::new("occlum_tmp").expect("create temp dir");
    let sockpath = tmp_dir.path().join("occlum.sock");

    let listener = UnixListener::bind(&sockpath).unwrap();

    //the thread would send the stdio to server
    let sendfd_thread = thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    debug!("server connected");
                    if let Ok(_) = stream.send_with_fd(&[0], &[0, 1, 2]) {
                        break;
                    }
                }
                Err(e) => {
                    debug!("connection failed: {}", e);
                }
            }
        }
    });

    let resp = executor::block_on(
        client
            .exec_command(
                grpc::RequestOptions::new(),
                ExecCommRequest {
                    process_id: process::id(),
                    command: command.to_string(),
                    parameters: parameter_list,
                    environments: environments_list,
                    sockpath: String::from(sockpath.as_path().to_str().unwrap()),
                    ..Default::default()
                },
            )
            .drop_metadata(),
    ); // Drop response metadata

    match resp {
        Ok(resp) => match resp.status {
            ExecCommResponse_ExecutionStatus::LAUNCH_FAILED => {
                Err(String::from("failed to launch the process."))
            }
            ExecCommResponse_ExecutionStatus::RUNNING => {
                sendfd_thread.join().unwrap();
                Ok(resp.process_id)
            }
        },
        Err(_) => Err(String::from("failed to send request.")),
    }
}

/// Starts the server if the server is not running
fn start_server(
    client: &OcclumExecClient,
    server_name: &str,
    num_vcpus: u32,
) -> Result<u32, String> {
    let mut server_launched = false;

    loop {
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

        match resp {
            Ok((_, resp, _)) => {
                if resp.status == HealthCheckResponse_ServingStatus::NOT_SERVING {
                    return Err("server is not running. It is not able to start.".to_string());
                }
                debug!("server is running.");
                return Ok(0);
            }
            Err(_resp) => {
                if !server_launched {
                    debug!("server is not running, try to launch the server.");
                    match Command::new(server_name)
                        .arg("-d")
                        .arg(env::current_dir().unwrap())
                        .arg("--cpus")
                        .arg(num_vcpus.to_string())
                        .stdout(Stdio::null())
                        .spawn()
                    {
                        Err(_r) => {
                            return Err("Failed to launch server".to_string());
                        }
                        Ok(_r) => {
                            server_launched = true;

                            //wait server 10 millis
                            thread::sleep(time::Duration::from_millis(100));
                            continue;
                        }
                    };
                } else {
                    return Err("Failed to launch server".to_string());
                }
            }
        };
    }
}

/// Stops the server with a timeout (seconds) specified
/// The timeout value should no larger than the default timeout value (30 seconds)
fn stop_server(client: &OcclumExecClient, time: u32) {
    let time = cmp::min(time, DEFAULT_SERVER_TIMER);
    if let Err(_) = executor::block_on(
        client
            .stop_server(
                grpc::RequestOptions::new(),
                StopRequest {
                    time: time,
                    ..Default::default()
                },
            )
            .join_metadata_result(),
    ) {
        debug!("The server is not running.");
    } else {
        debug!("The server has received the stop request.");
    }
}

//Gets the application return value
fn get_return_value(client: &OcclumExecClient, process_id: &i32) -> Result<i32, ()> {
    let resp = executor::block_on(
        client
            .get_result(
                grpc::RequestOptions::new(),
                GetResultRequest {
                    process_id: *process_id,
                    ..Default::default()
                },
            )
            .join_metadata_result(),
    );
    match resp {
        Ok((_, resp, _)) => {
            if resp.status == GetResultResponse_ExecutionStatus::STOPPED {
                Ok(resp.result)
            } else {
                Err(())
            }
        }
        Err(_) => Err(()),
    }
}

// Kill the process running in server
fn kill_process(client: &OcclumExecClient, process_id: &i32, signal: &i32) {
    if executor::block_on(
        client
            .kill_process(
                grpc::RequestOptions::new(),
                KillProcessRequest {
                    process_id: *process_id,
                    signal: *signal,
                    ..Default::default()
                },
            )
            .join_metadata_result(),
    )
    .is_err()
    {
        debug!("send signal failed");
    }
}

fn main() -> Result<(), i32> {
    env_logger::init();

    let matches = App::new("Occlum")
        .version("0.1.0")
        .arg(
            Arg::with_name("instance_dir")
                .short("d")
                .long("instance_dir")
                .takes_value(true)
                .default_value("./")
                .help("The Occlum instance dir."),
        )
        .subcommand(
            App::new("start").about(
                "Start the Occlum server. If the server already running, immediately return.",
            ).arg(
                Arg::with_name("cpus")
                    .long("cpus")
                    .takes_value(true)
                    .help("The number of vcpus")
                    .default_value("0")
                    .validator(|t| match t.parse::<u32>() {
                        Ok(_) => Ok(()),
                        Err(e) => Err(e.to_string()),
                        }),
                ),
        )
        .subcommand(
            App::new("stop")
                .about(
                    "Stop the Occlum server.",
                )
                .arg(
                    Arg::with_name("time")
                        .short("t")
                        .long("time")
                        .takes_value(true)
                        .help("Seconds to wait before killing the applications running on the Occlum server.")
                        .default_value("10")
                        .validator(|t| match t.parse::<u32>() {
                            Ok(_) => Ok(()),
                            Err(e) => Err(e.to_string()),
                        }),
                ),
        )
        .subcommand(
            App::new("exec")
                .about("Execute the command on server.")
                .arg(Arg::with_name("args").multiple(true).min_values(1).last(true).help("The arguments for the command")),
        )
        .get_matches();

    let env: Vec<String> = env::vars()
        .into_iter()
        .map(|(key, val)| format!("{}={}", key, val))
        .collect();

    // Set the instance_dir as the current dir
    let instance_dir = Path::new(matches.value_of("instance_dir").unwrap());
    assert!(env::set_current_dir(&instance_dir).is_ok());

    let client = OcclumExecClient::new_plain_unix(DEFAULT_SOCK_FILE, ClientConf::new())
        .expect("failed to create UDS client");

    if let Some(ref matches) = matches.subcommand_matches("start") {
        let num_vcpus = matches.value_of("cpus").unwrap().parse::<u32>().unwrap();
        if let Err(s) = start_server(&client, DEFAULT_SERVER_FILE, num_vcpus) {
            println!("start server failed {}", s);
            return Err(-1);
        }
        println!("server is running.");
    } else if let Some(ref matches) = matches.subcommand_matches("stop") {
        let stop_time = matches.value_of("time").unwrap().parse::<u32>().unwrap();
        stop_server(&client, stop_time);
        println!("server is stopping.");
    } else if let Some(ref matches) = matches.subcommand_matches("exec") {
        let mut cmd_args: Vec<&str> = match matches
            .values_of("args")
            .map(|vals| vals.collect::<Vec<_>>())
        {
            Some(p) => p,
            //Already set the min_values to 1. So it could not be here
            _ => panic!(),
        };

        let cmd = cmd_args[0];
        // Change cmd_args[0] from path name to program name
        cmd_args[0] = Path::new(cmd_args[0])
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let env: Vec<&str> = env.iter().map(|string| string.as_str()).collect();

        // Create the signal handler
        let process_killed = Arc::new(Mutex::new(false));
        let process_killed_clone = Arc::clone(&process_killed);
        let mut signals = Signals::new(&[SIGUSR1, SIGINT, SIGQUIT, SIGTERM]).unwrap();
        let signal_thread = thread::spawn(move || {
            for signal in signals.forever() {
                debug!("Received signal {:?}", signal);
                match signal {
                    SIGUSR1 => {
                        break;
                    }
                    SIGINT | SIGQUIT | SIGTERM => {
                        let mut process_killed = process_killed_clone.lock().unwrap();
                        *process_killed = true;
                        break;
                    }
                    _ => unreachable!(),
                }
            }
        });

        match exec_command(&client, cmd, &cmd_args, &env) {
            Ok(process_id) => {
                // the signal thread exit if server finished execution or user kill the client
                signal_thread.join().unwrap();

                // check the signal type:
                // if client killed by user, send SIGTERM and SIGKILL to server
                if *process_killed.lock().unwrap() {
                    // stop the process in server
                    kill_process(&client, &process_id, &SIGTERM);
                    kill_process(&client, &process_id, &SIGKILL);
                    return Err(-1);
                } else {
                    if let Ok(result) = get_return_value(&client, &process_id) {
                        if result != 0 {
                            return Err(result);
                        }
                    } else {
                        debug!("get the return value failed");
                        return Err(-1);
                    }
                }
            }
            Err(s) => {
                debug!("execute command failed {}", s);
                return Err(-1);
            }
        };
    } else {
        unreachable!();
    }

    Ok(())
}
