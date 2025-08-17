/// https://users.rust-lang.org/t/reading-from-pipe-via-stdin-in-binary/133088/10

use std::io;
use std::io::stdin;
use std::os::fd::{AsFd, AsRawFd};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener};

use nix;
use nix::sys::socket::getpeername;
use nix::unistd::dup2_stdin;

pub fn init_fcgi() -> io::Result<UnixListener> {
    if getpeername::<()>(stdin().as_raw_fd()) != Err(nix::Error::ENOTCONN) {
        return Err(io::Error::other(
            "Not a FastCGI application (FD-0 is not a listener socket)",
        ));
    }
    let file = File::open("/dev/null")?;
    let socket_fd = stdin().as_fd().try_clone_to_owned()?;
    dup2_stdin(file)?; // atomically replace stdin
    Ok(UnixListener::from(socket_fd))
}
/*
fn main() {
    // create dummy listener to test without FastCGI
    dup2_stdin(
        UnixListener::bind_addr(&SocketAddr::from_abstract_name("fcgi-test").unwrap()).unwrap(),
    )
    .unwrap();
*/
