/// Code for initializing the server

extern crate daemonize;

use daemonize::Daemonize;

use std::fs::File;
use std::env;
use std::process;

mod connections;
mod http;

fn main() {

    // Parse options
    let args: Vec<String> = env::args().collect();
    let usage = format!("USAGE: {} [--version] | [--daemon]", &args[0]);

    if args.len() > 2 {
        println!("{}", usage);
        process::exit(1);
    }

    // bleh
    let mut listener;

    if args.len() == 2 {
        if &args[1] == "--version" {
            // create version string
            println!("{}", env!("CARGO_PKG_NAME").to_string() + ", version: " + env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }

        if &args[1] != "--daemon" {
            println!("{}", usage);
            process::exit(1);
        }

        // Make daemon

        // TODO move these to a config file or something
        let server_root_path = ".";
        let log_file = File::create("./log").unwrap();
        let log_file2 = log_file.try_clone().unwrap();
        let pid_file_path = "./pidfile";
        let user = "nuno";
        let group = "daemon";

        let daemonize = Daemonize::new()
            .pid_file(pid_file_path)
            .chown_pid_file(true)
            .working_directory(server_root_path)
            .user(user)
            .group(group)
            .stdout(log_file)
            .stderr(log_file2)
            .privileged_action(move || connections::get_listener(443));

        match daemonize.start() {
            Ok(tcp_listener) => {
                println!("Server daemonized");
                listener = Some(tcp_listener);
                },
            Err(e) => {
                eprintln!("Error, {}", e);
                process::exit(1);
            },
        }
    } else {
        listener = Some(connections::get_listener(5443));
    }

    if let Some(listener) = listener {
        connections::start(listener);
    }
}