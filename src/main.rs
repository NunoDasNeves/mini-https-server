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

    let html_path = "public_html";
    let handler = http::HTTPHandler::new(html_path);

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
        let server_root = ".";
        let log_file = File::create("./log").unwrap();
        let log_file2 = log_file.try_clone().unwrap();
        let pid_file_path = "./pidfile";
        let user = "nuno";
        let group = "daemon";

        let daemonize = Daemonize::new()
            .pid_file(pid_file_path)
            .working_directory(server_root)
            .user(user)
            .group(group)
            .stdout(log_file)
            .stderr(log_file2)
            .privileged_action(move || connections::TlsServer::new(443, handler));

        match daemonize.start() {
            Ok(mut tls_server) => {
                println!("Server daemonized");
                tls_server.start();
                },
            Err(e) => {
                eprintln!("Error, {}", e);
                process::exit(1);
            },
        }
    } else {
        let mut server = connections::TlsServer::new(5443, handler);
        server.start();
    }
}