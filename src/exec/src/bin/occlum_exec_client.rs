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
use futures::stream::StreamExt;
use grpc::prelude::*;
use grpc::ClientConf;
use occlum_exec::occlum_exec::{
    ExecComm, GetResultRequest, GetResultResponse_ExecutionStatus, HealthCheckRequest,
    HealthCheckResponse_ServingStatus, StopRequest,
};
use occlum_exec::occlum_exec_grpc::OcclumExecClient;
use occlum_exec::{
    DEFAULT_CLIENT_FILE, DEFAULT_SERVER_FILE, DEFAULT_SERVER_TIMER, DEFAULT_SOCK_FILE,
};
use protobuf::RepeatedField;
use sendfd::SendWithFd;
use std::cmp;
use std::env;
use std::os::unix::net::UnixListener;
use std::process;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use tempdir::TempDir;

use signal_hook::iterator::Signals;
use signal_hook::SIGUSR1;

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
) -> Result<u32, String> {
    debug!("exec_command {:?} {:?}", command, parameters);

    let mut parameter_list = RepeatedField::default();
    for p in parameters {
        parameter_list.push(p.to_string());
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
                ExecComm {
                    process_id: process::id(),
                    command: command.to_string(),
                    parameters: parameter_list,
                    sockpath: String::from(sockpath.as_path().to_str().unwrap()),
                    ..Default::default()
                },
            )
            .drop_metadata(),
    ); // Drop response metadata

    match resp {
        Ok(resp) => {
            sendfd_thread.join().unwrap();
            Ok(resp.process_id)
        }
        Err(_) => Err(String::from("failed to send request.")),
    }
}

/// Starts the server if the server is not running
fn start_server(client: &OcclumExecClient, server_name: &str) -> Result<u32, String> {
    let mut server_launched = false;
    let mut server_connection_retry_time = 0;

    loop {
        let resp = executor::block_on(
            client
                .status_check(
                    grpc::RequestOptions::new(),
                    HealthCheckRequest {
                        process_id: 0,
                        ..Default::default()
                    },
                )
                .join_metadata_result(),
        );

        server_connection_retry_time += 1;

        match resp {
            Ok((_, resp, _)) => {
                match resp.status {
                    HealthCheckResponse_ServingStatus::NOT_SERVING => {
                        return Err("no process".to_string())
                    }
                    _ => {}
                };
                debug!("server is running.");
                return Ok(0);
            }
            Err(_resp) => {
                if !server_launched {
                    debug!("server is not running, try to launch the server.");
                    match Command::new(server_name).stdout(Stdio::null()).spawn() {
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
                    if server_connection_retry_time < 100 {
                        //wait server 100 millis
                        thread::sleep(time::Duration::from_millis(100));
                        continue;
                    }
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

/// Sends heart beats to server. When server responses NOT_SERVING, the app exit with return value.
fn start_heart_beat(client: &OcclumExecClient, process_id: u32) {
    let process_stopped = Arc::new(Mutex::new(false));
    let c_process_stopped = process_stopped.clone();

    let (mut req, resp) =
        executor::block_on(client.heart_beat(grpc::RequestOptions::new())).unwrap();

    thread::spawn(move || {
        loop {
            thread::sleep(time::Duration::from_millis(500));
            match *c_process_stopped.lock().unwrap() {
                true => {
                    //the application stopped
                    break;
                }
                false => {
                    executor::block_on(req.wait()).unwrap();
                    req.send_data(HealthCheckRequest {
                        process_id: process_id,
                        ..Default::default()
                    })
                    .expect("send failed");
                }
            };
        }
        req.finish().expect("req finish failed");
    });

    let mut responses = resp.drop_metadata();
    'a: loop {
        while let Some(message) = executor::block_on(responses.next()) {
            let status = match message {
                Ok(m) => m.status,
                Err(_e) => {
                    //stop the client for any issue
                    //Todo: How to report the crash issue?
                    HealthCheckResponse_ServingStatus::NOT_SERVING
                }
            };

            if status != HealthCheckResponse_ServingStatus::SERVING {
                //the application has stopped
                *process_stopped.lock().unwrap() = true;
                break 'a;
            }

            thread::sleep(time::Duration::from_millis(100));
        }
    }
}

//Gets the application return value
fn get_return_value(client: &OcclumExecClient, process_id: &u32) -> Result<i32, ()> {
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

fn main() -> Result<(), i32> {
    env_logger::init();

    let matches = App::new("Occlum")
        .version("0.1.0")
        .subcommand(
            App::new("start").about(
                "Start the Occlum server. If the server already running, immediately return.",
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

    let args: Vec<String> = env::args().collect();

    let mut sock_file = String::from(args[0].as_str());
    let sock_file = str::replace(
        sock_file.as_mut_str(),
        DEFAULT_CLIENT_FILE,
        DEFAULT_SOCK_FILE,
    );

    let client = OcclumExecClient::new_plain_unix(&sock_file, ClientConf::new())
        .expect("failed to create UDS client");

    if let Some(ref _matches) = matches.subcommand_matches("start") {
        //get the server name with the first args
        let mut server_name = String::from(args[0].as_str());
        let server_name = str::replace(
            server_name.as_mut_str(),
            DEFAULT_CLIENT_FILE,
            DEFAULT_SERVER_FILE,
        );

        if let Err(s) = start_server(&client, &server_name) {
            debug!("start_server failed {}", s);
            return Err(-1);
        }
    } else if let Some(ref matches) = matches.subcommand_matches("stop") {
        let stop_time = matches.value_of("time").unwrap().parse::<u32>().unwrap();
        stop_server(&client, stop_time);
    } else if let Some(ref matches) = matches.subcommand_matches("exec") {
        let cmd_args: Vec<&str> = match matches
            .values_of("args")
            .map(|vals| vals.collect::<Vec<_>>())
        {
            Some(p) => p,
            //Already set the min_values to 1. So it could not be here
            _ => panic!(),
        };

        let (cmd, args) = cmd_args.split_first().unwrap();

        match exec_command(&client, cmd, args) {
            Ok(process_id) => {
                let signals = Signals::new(&[SIGUSR1]).unwrap();
                let signal_thread = thread::spawn(move || {
                    for sig in signals.forever() {
                        debug!("Received signal {:?}", sig);
                        break;
                    }
                });

                //Notifies the server, if client killed by KILL
                start_heart_beat(&client, process_id.clone());

                signal_thread.join().unwrap();
                let result = get_return_value(&client, &process_id).unwrap();

                if result != 0 {
                    return Err(result);
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
