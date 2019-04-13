/// Code for handling http requests

use std::fs;

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

/// Parse HTTP and return a response
pub fn get_response(buf: &[u8]) -> Vec<u8> {

    // bytes to string
    let string = std::str::from_utf8(buf).unwrap();
    let vec: Vec<&str> = string.split(" ").collect();

    // check that it's a basic GET request
    if vec.len() < 3 || vec[0] != "GET" {
        return Response::new(400, "".to_string()).get_bytes();
    }

    // get file path - TODO: make more secure using a whitelist of files or similar
    let mut path = vec[1].to_string();
    if path == "/" {
        path = "/index.html".to_string();
    }

    println!("GET {}", path);

    path = format!("public_html{}", path);

    // read the html file, or fail
    if let Ok(html) = fs::read_to_string(path) {
        Response::new(200, html).get_bytes()
    } else {
        Response::new(404, "404 - Not found!".to_string()).get_bytes()
    }
}