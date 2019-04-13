/// Code for handling sockets and tls
/// The code in this file is based on rustls example code

use std::sync::Arc;

use mio;
use mio::tcp::{TcpListener, TcpStream, Shutdown};

use std::fs;
use std::io;
use vecio::Rawv;
use std::net;
use std::io::{Write, Read, BufReader};
use std::collections::HashMap;

use rustls;

use rustls::{Session, NoClientAuth};

// Token for our listening socket.
const LISTENER: mio::Token = mio::Token(0);

/// This binds together a TCP listening socket, some outstanding
/// connections, and a TLS server configuration.
struct TlsServer {
    server: TcpListener,
    connections: HashMap<mio::Token, Connection>,
    next_id: usize,
    tls_config: Arc<rustls::ServerConfig>
}

impl TlsServer {
    fn new(server: TcpListener, tls_config: Arc<rustls::ServerConfig>) -> TlsServer {
        TlsServer {
            server,
            connections: HashMap::new(),
            next_id: 2,
            tls_config,
        }
    }

    fn accept(&mut self, poll: &mut mio::Poll) -> bool {
        // accept on TcpListener to get socket (TcpStream) and remote address
        match self.server.accept() {
            Ok((socket, addr)) => {
                println!("Accepting new connection from {:?}", addr);

                // tls session handle
                let tls_session = rustls::ServerSession::new(&self.tls_config);

                // token for this connection
                let token = mio::Token(self.next_id);
                self.next_id += 1;

                // create a mapping between the token and a Connection
                self.connections.insert(token, Connection::new(socket, token, tls_session));
                // register stuff with the mio::Poll so that further events from this client go to conn_event()
                self.connections[&token].register(poll);
                true
            }
            Err(e) => {
                println!("encountered error while accepting connection; err={:?}", e);
                false
            }
        }
    }

    fn conn_event(&mut self, poll: &mut mio::Poll, event: &mio::Event) {
        let token = event.token();

        if self.connections.contains_key(&token) {
            // look up the connection based on the token
            self.connections
                .get_mut(&token)
                .unwrap()
                // do something with the event
                .ready(poll, event);

            // if that resulted in the socket closing, remove the connection
            if self.connections[&token].is_closed() {
                self.connections.remove(&token);
            }
        }
    }
}

/// This is a connection which has been accepted by the server,
/// and is currently being served.
///
/// It has a TCP-level stream, a TLS-level session, and some
/// other state/metadata.
struct Connection {
    socket: TcpStream,                  // open socket with client
    token: mio::Token,                  // unique token
    closing: bool,                      //
    closed: bool,                       //
    tls_session: rustls::ServerSession,
}

/// This glues our `rustls::WriteV` trait to `vecio::Rawv`.
pub struct WriteVAdapter<'a> {
    rawv: &'a mut dyn Rawv
}

impl<'a> WriteVAdapter<'a> {
    pub fn new(rawv: &'a mut dyn Rawv) -> WriteVAdapter<'a> {
        WriteVAdapter { rawv }
    }
}

impl<'a> rustls::WriteV for WriteVAdapter<'a> {
    fn writev(&mut self, bytes: &[&[u8]]) -> io::Result<usize> {
        self.rawv.writev(bytes)
    }
}

impl Connection {
    fn new(socket: TcpStream,
           token: mio::Token,
           tls_session: rustls::ServerSession)
           -> Connection {
        Connection {
            socket,
            token,
            closing: false,
            closed: false,
            tls_session,
        }
    }

    /// We're a connection, and we have something to do.
    fn ready(&mut self, poll: &mut mio::Poll, ev: &mio::Event) {
        // If we're readable: read some TLS.  Then
        // see if that yielded new plaintext.
        if ev.readiness().is_readable() {
            // reading tls sets up tls_session to read like a socket
            self.do_tls_read();
            // read from the tls_session
            self.plain_read();
        }

        if ev.readiness().is_writable() {
            self.do_tls_write_and_handle_error();
        }

        if self.closing && !self.tls_session.wants_write() {
            let _ = self.socket.shutdown(Shutdown::Both);
            self.closed = true;
        } else {
            self.reregister(poll);
        }
    }

    fn do_tls_read(&mut self) {
        // Read some TLS data from the socket
        let rc = self.tls_session.read_tls(&mut self.socket); // returns a Result with usize
        if rc.is_err() {
            let err = rc.unwrap_err();

            if let io::ErrorKind::WouldBlock = err.kind() {
                return;
            }

            println!("read error {:?}", err);
            self.closing = true;
            return;
        }

        if rc.unwrap() == 0 {
            println!("eof");
            self.closing = true;
            return;
        }

        // Process newly-received TLS messages.
        let processed = self.tls_session.process_new_packets();
        if processed.is_err() {
            println!("cannot process packet: {:?}", processed);
            self.closing = true;
            return;
        }
    }

    fn plain_read(&mut self) {
        // Read and process all available plaintext.
        let mut buf = Vec::new();

        let rc = self.tls_session.read_to_end(&mut buf);
        if rc.is_err() {
            println!("plaintext read failed: {:?}", rc);
            return;
        }

        // if we got something, respond based on server type
        if !buf.is_empty() {
            //println!("plaintext read {:?}", buf.len());
            let response = crate::http::get_response(&buf);
            self.tls_session
                .write_all(&response)
                .unwrap();
            self.closing = true;
            self.tls_session.send_close_notify();
        }
    }

    #[cfg(target_os = "windows")]
    fn tls_write(&mut self) -> io::Result<usize> {
        self.tls_session.write_tls(&mut self.socket)
    }

    #[cfg(not(target_os = "windows"))]
    fn tls_write(&mut self) -> io::Result<usize> {
        self.tls_session.writev_tls(&mut WriteVAdapter::new(&mut self.socket))
    }

    fn do_tls_write_and_handle_error(&mut self) {
        let rc = self.tls_write();
        if rc.is_err() {
            println!("write failed {:?}", rc);
            self.closing = true;
            return;
        }
    }

    fn register(&self, poll: &mut mio::Poll) {
        // register our socket with the poll, our token, what events we want
        poll.register(&self.socket,
                      self.token,
                      self.event_set(),
                      // oneshot means we deregister ourselves from the poll after getting an event
                      mio::PollOpt::level() | mio::PollOpt::oneshot())
            .unwrap();

    }

    // similar to above...
    fn reregister(&self, poll: &mut mio::Poll) {
        poll.reregister(&self.socket,
                        self.token,
                        self.event_set(),
                        mio::PollOpt::level() | mio::PollOpt::oneshot())
            .unwrap();

    }

    /// What IO events we're currently waiting for,
    /// based on wants_read/wants_write.
    fn event_set(&self) -> mio::Ready {
        let rd = self.tls_session.wants_read();
        let wr = self.tls_session.wants_write();

        if rd && wr {
            mio::Ready::readable() | mio::Ready::writable()
        } else if wr {
            mio::Ready::writable()
        } else {
            mio::Ready::readable()
        }
    }

    fn is_closed(&self) -> bool {
        self.closed
    }
}

fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
    let certfile = fs::File::open(filename).expect("cannot open certificate file");
    let mut reader = BufReader::new(certfile);
    rustls::internal::pemfile::certs(&mut reader).unwrap()
}

fn load_private_key(filename: &str) -> rustls::PrivateKey {
    let rsa_keys = {
        let keyfile = fs::File::open(filename)
            .expect("cannot open private key file");
        let mut reader = BufReader::new(keyfile);
        rustls::internal::pemfile::rsa_private_keys(&mut reader)
            .expect("file contains invalid rsa private key")
    };

    let pkcs8_keys = {
        let keyfile = fs::File::open(filename)
            .expect("cannot open private key file");
        let mut reader = BufReader::new(keyfile);
        rustls::internal::pemfile::pkcs8_private_keys(&mut reader)
            .expect("file contains invalid pkcs8 private key (encrypted keys not supported)")
    };

    // prefer to load pkcs8 keys
    if !pkcs8_keys.is_empty() {
        pkcs8_keys[0].clone()
    } else {
        assert!(!rsa_keys.is_empty());
        rsa_keys[0].clone()
    }
}

fn load_ocsp(filename: &Option<String>) -> Vec<u8> {
    let mut ret = Vec::new();

    if let &Some(ref name) = filename {
        fs::File::open(name)
            .expect("cannot open ocsp file")
            .read_to_end(&mut ret)
            .unwrap();
    }

    ret
}

pub fn get_listener(port: u16) -> TcpListener {
    // create socket address; an ip and a port number
    let addr = net::SocketAddr::new(net::IpAddr::V4(net::Ipv4Addr::new(0, 0, 0, 0)), port);
    // create a listener that is bound to this port
    TcpListener::bind(&addr).expect("Couldn't bind to port")
}

pub fn start(listener: TcpListener) {
    // create TLS server config
    let mut config = rustls::ServerConfig::new(NoClientAuth::new());
    config.key_log = Arc::new(rustls::KeyLogFile::new());

    // TODO get from config
    let certs = load_certs("cert/certificate.pem");
    let privkey = load_private_key("cert/key.pem");
    let ocsp = load_ocsp(&Option::None);

    config.set_single_cert_with_ocsp_and_sct(certs, privkey, ocsp, vec![])
        .expect("bad certificates/private key");

    let mut poll = mio::Poll::new()
        .unwrap();

    // register listener with poll
    poll.register(&listener,
                  LISTENER,                 // mio Token to be returned on a readiness event
                  mio::Ready::readable(),   // generate an event when socket is readable
                  mio::PollOpt::level())    // always generate events when there's data in the socket
        .unwrap();

    // create server object...

    let config = Arc::new(config);
    let mut tlsserv = TlsServer::new(listener, config);

    // receives readiness events from poll
    let mut events = mio::Events::with_capacity(256);

    loop {
        // wait for readiness event, with no timeout
        poll.poll(&mut events, None)
            .unwrap();

        for event in events.iter() {
            match event.token() {
                // LISTENER token is used to accept() new connections
                LISTENER => {
                    if !tlsserv.accept(&mut poll) {
                        break;
                    }
                }
                // conn_event() processes existing connections
                _ => tlsserv.conn_event(&mut poll, &event)
            }
        }
    }
}