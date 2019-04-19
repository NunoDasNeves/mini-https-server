/// Code for handling http requests

use std::fs;
use std::path::{Path, PathBuf};

struct Response {
    status: u16,
    body: String,
}

impl Response {
    fn new(status: u16, body: String) -> Response {
        Response {
            status,
            body,
        }
    }

    fn get_bytes(&self) -> Vec<u8> {
        // TODO: proper headers
        let header_string = "Connection: close\r\n\r\n";
        format!(
            "HTTP/1.1 {} {}\r\n{}{}\r\n",
            self.status,
            self.status_text(),
            header_string,
            self.body
        ).into_bytes()
    }
    /// Map status code to status text
    fn status_text(&self) -> &str {
        match self.status {
            200 => "OK",
            301 => "Redirect",
            400 => "User Error",
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Server Error",
            _ => "Error"
        }
    }
}

pub struct HTTPHandler {
    root_path: PathBuf,
}

impl HTTPHandler {
    pub fn new(path: &str) -> HTTPHandler {
        let root_path = Path::new(path).to_path_buf();
        HTTPHandler {
            root_path 
        }
    }

    /// Parse HTTP and return a response
    pub fn get_response(&self, buf: &[u8]) -> Vec<u8> {

        // bytes to string
        let string = std::str::from_utf8(buf).unwrap();

        // first line
        let vec: Vec<&str> = string.split("\n").collect();

        if vec.len() < 1 {
            return Response::new(400, "".to_string()).get_bytes();
        }

        // split it up
        let vec: Vec<&str> = vec[0].split(" ").collect();

        if vec.len() < 3 {
            return Response::new(400, "".to_string()).get_bytes();
        }

        // print request
        println!("{} {} {}", vec[0], vec[1], vec[2]);

        // check that it's a basic GET request
        if vec[0] != "GET" {
            return Response::new(400, "".to_string()).get_bytes();
        }

        // get file path - TODO: make more secure using a whitelist of files or similar
        let mut path = String::from(vec[1]);
        if path == "/" {
            path = String::from("index.html");
        } else {
            // starts with "/"
            if path.starts_with("/") {
                path = String::from(&path[1..]);
            } else {
                return Response::new(400, "".to_string()).get_bytes();
            }
        }

        let mut path_path = self.root_path.clone();
        path_path.push(Path::new(&path));

        // read the html file, or fail
        if let Ok(html) = fs::read_to_string(path_path) {
            Response::new(200, html).get_bytes()
        } else {
            Response::new(404, "404 - Not found!".to_string()).get_bytes()
        }
    }
}