//! Local transport for the engine process. The length-prefixed protocol in
//! [`crate::ipc`] needs one duplex byte stream with a read timeout, and every
//! supported platform has one — just not the same one.
//!
//! * Unix: an `AF_UNIX` socket path, exactly as the engine has always used.
//! * Windows: a loopback TCP socket. There is no stable `AF_UNIX` there, so
//!   the engine binds `127.0.0.1` on an ephemeral port and publishes that
//!   endpoint to its parent through a file.
//!
//! [`EngineEndpoint`] is the serializable form of "where do I connect": the
//! engine writes it, the UI reads it, and neither side hard-codes a
//! platform-specific address. The endpoint of a live listener is observable
//! through [`std::net::TcpListener::local_addr`], so the loopback listener
//! cannot be handed out before bind assigns it.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// A connected, timeout-capable engine stream.
pub trait EngineStream: Read + Write {
    /// Both transports expose a blocking-read timeout, which is what lets the
    /// engine serve loop and the UI drain loop stay platform-neutral.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;
}

impl<S: EngineStream + ?Sized> EngineStream for Box<S> {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        (**self).set_read_timeout(timeout)
    }
}

#[cfg(unix)]
impl EngineStream for UnixStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        UnixStream::set_read_timeout(self, timeout)
    }
}

impl EngineStream for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }
}

/// Any reachable engine endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineEndpoint {
    /// Unix socket file, serialized as `unix:/tmp/revo-engine-1234.sock`.
    Path(PathBuf),
    /// Loopback TCP address, serialized as `tcp:127.0.0.1:41234`.
    Socket(SocketAddr),
}

impl EngineEndpoint {
    /// Endpoint to bind when the caller did not choose one: a socket path in
    /// the temp directory on Unix, any free loopback port elsewhere.
    pub fn auto() -> Self {
        #[cfg(unix)]
        {
            Self::Path(
                std::env::temp_dir().join(format!("revo-engine-{}.sock", std::process::id())),
            )
        }
        #[cfg(not(unix))]
        {
            Self::Socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        }
    }

    /// Wire form stored in endpoint files and `REVO_ENGINE_SOCK`.
    pub fn to_string(&self) -> String {
        match self {
            Self::Path(path) => format!("unix:{}", path.display()),
            Self::Socket(address) => format!("tcp:{address}"),
        }
    }

    /// Parses [`EngineEndpoint::to_string`]. A value with no scheme is read as
    /// a socket path so `--socket <path>` and existing `REVO_ENGINE_SOCK`
    /// values keep working.
    pub fn parse(value: &str) -> io::Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty engine endpoint",
            ));
        }
        if let Some(address) = value.strip_prefix("tcp:") {
            let parsed: SocketAddr = address
                .parse()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if !parsed.ip().is_loopback() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "engine endpoints must be loopback addresses",
                ));
            }
            return Ok(Self::Socket(parsed));
        }
        if let Some(path) = value.strip_prefix("unix:") {
            return Ok(Self::Path(PathBuf::from(path)));
        }
        Ok(Self::Path(PathBuf::from(value)))
    }
}

/// Listening engine endpoint handing out connected streams.
pub enum EngineListener {
    #[cfg(unix)]
    Unix(UnixListener, PathBuf),
    Tcp(TcpListener),
}

impl EngineListener {
    /// Binds `endpoint`. A zero TCP port resolves to the assigned port, which
    /// [`Self::endpoint`] then reports.
    pub fn bind(endpoint: &EngineEndpoint) -> io::Result<Self> {
        match endpoint {
            #[cfg(unix)]
            EngineEndpoint::Path(path) => {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                // A stale socket file from a crashed engine would block bind().
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
                Ok(Self::Unix(UnixListener::bind(path)?, path.clone()))
            }
            #[cfg(not(unix))]
            EngineEndpoint::Path(path) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "unix socket endpoints are unavailable on this platform: {}",
                    path.display()
                ),
            )),
            EngineEndpoint::Socket(address) => Ok(Self::Tcp(TcpListener::bind(address)?)),
        }
    }

    /// The endpoint to connect to, after any ephemeral port was assigned.
    pub fn endpoint(&self) -> EngineEndpoint {
        match self {
            #[cfg(unix)]
            Self::Unix(_, path) => EngineEndpoint::Path(path.clone()),
            Self::Tcp(listener) => EngineEndpoint::Socket(
                listener
                    .local_addr()
                    .expect("a bound TCP listener always reports its address"),
            ),
        }
    }

    /// Removes the filesystem artifact of a socket endpoint. The engine calls
    /// this once a client is connected: the path is not reusable anyway and a
    /// leftover file would mislead a later run.
    pub fn release_endpoint_file(&self) {
        #[cfg(unix)]
        if let Self::Unix(_, path) = self {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn accept(&self) -> io::Result<Box<dyn EngineStream + Send>> {
        match self {
            #[cfg(unix)]
            Self::Unix(listener, _) => listener
                .accept()
                .map(|(stream, _)| Box::new(stream) as Box<dyn EngineStream + Send>),
            Self::Tcp(listener) => listener
                .accept()
                .map(|(stream, _)| Box::new(stream) as Box<dyn EngineStream + Send>),
        }
    }
}

/// Connects to one endpoint.
pub fn connect(endpoint: &EngineEndpoint) -> io::Result<Box<dyn EngineStream + Send>> {
    match endpoint {
        #[cfg(unix)]
        EngineEndpoint::Path(path) => Ok(Box::new(UnixStream::connect(path)?)),
        #[cfg(not(unix))]
        EngineEndpoint::Path(path) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "unix socket endpoints are unavailable on this platform: {}",
                path.display()
            ),
        )),
        EngineEndpoint::Socket(address) => Ok(Box::new(TcpStream::connect(address)?)),
    }
}

/// Publishes the bound endpoint for a spawning parent to read.
pub fn write_endpoint_file(path: &Path, endpoint: &EngineEndpoint) -> io::Result<()> {
    std::fs::write(path, endpoint.to_string())
}

/// Reads a published endpoint. `Ok(None)` means the engine has not written it
/// yet, so the caller keeps polling under its own deadline.
pub fn read_endpoint_file(path: &Path) -> io::Result<Option<EngineEndpoint>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    EngineEndpoint::parse(&text).map(Some)
}

/// Two connected streams for tests, without spawning anything.
pub fn test_pair() -> io::Result<(Box<dyn EngineStream + Send>, Box<dyn EngineStream + Send>)> {
    #[cfg(unix)]
    {
        let (client, server) = UnixStream::pair()?;
        return Ok((Box::new(client), Box::new(server)));
    }
    #[cfg(not(unix))]
    {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let address = listener.local_addr()?;
        let client = TcpStream::connect(address)?;
        let (server, _) = listener.accept()?;
        Ok((Box::new(client), Box::new(server)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_round_trip_through_their_wire_form() {
        let path = EngineEndpoint::Path(PathBuf::from("/tmp/engine.sock"));
        assert_eq!(EngineEndpoint::parse(&path.to_string()).unwrap(), path);

        let socket = EngineEndpoint::Socket("127.0.0.1:45123".parse().unwrap());
        assert_eq!(EngineEndpoint::parse(&socket.to_string()).unwrap(), socket);
    }

    #[test]
    fn unprefixed_endpoints_stay_socket_paths() {
        assert_eq!(
            EngineEndpoint::parse("/tmp/legacy.sock").unwrap(),
            EngineEndpoint::Path(PathBuf::from("/tmp/legacy.sock"))
        );
    }

    #[test]
    fn tcp_endpoints_refuse_non_loopback_addresses() {
        let error = EngineEndpoint::parse("tcp:0.0.0.0:41234").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn empty_endpoint_is_a_parse_error() {
        assert!(EngineEndpoint::parse("   ").is_err());
    }

    #[test]
    fn a_bound_listener_reports_a_reconnectable_endpoint() {
        let listener =
            EngineListener::bind(&EngineEndpoint::auto()).expect("the platform endpoint binds");
        let endpoint = listener.endpoint();
        assert_eq!(
            EngineEndpoint::parse(&endpoint.to_string()).unwrap(),
            endpoint,
            "the published endpoint must survive its own wire form"
        );
    }

    #[test]
    fn frames_flow_over_a_test_pair_in_both_directions() {
        let (mut client, mut server) = test_pair().expect("transport pair");
        client.write_all(b"ping").expect("client write");
        client.flush().expect("client flush");
        let mut buffer = [0u8; 4];
        server.read_exact(&mut buffer).expect("server read");
        assert_eq!(&buffer, b"ping");

        server.write_all(b"pong").expect("server write");
        server.flush().expect("server flush");
        client.read_exact(&mut buffer).expect("client read");
        assert_eq!(&buffer, b"pong");
    }

    #[test]
    fn read_timeout_fires_on_an_idle_stream() {
        let (mut client, _server) = test_pair().expect("transport pair");
        client
            .set_read_timeout(Some(Duration::from_millis(30)))
            .expect("timeout set");
        let mut buffer = [0u8; 1];
        let error = client
            .read(&mut buffer)
            .expect_err("an idle stream must time out");
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
    }

    #[test]
    fn endpoint_file_is_pollable_until_the_engine_writes_it() {
        let directory = std::env::temp_dir().join(format!(
            "revo-transport-test-{}-{}",
            std::process::id(),
            file_index()
        ));
        std::fs::create_dir_all(&directory).expect("test directory");
        let file = directory.join("engine.endpoint");
        let _ = std::fs::remove_file(&file);
        assert!(read_endpoint_file(&file)
            .expect("a missing file is not an error")
            .is_none());

        let endpoint = EngineEndpoint::Socket("127.0.0.1:41999".parse().unwrap());
        write_endpoint_file(&file, &endpoint).expect("endpoint published");
        assert_eq!(
            read_endpoint_file(&file).expect("endpoint read").unwrap(),
            endpoint
        );
        let _ = std::fs::remove_file(&directory);
    }

    fn file_index() -> u32 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}
