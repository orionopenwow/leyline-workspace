//! The byte stream a login or world session runs over (5 Nov, the phone's solo play): a TCP connection on a PC, or
//! an in-memory stream when Den Lite runs inside a web page (web.rs: the browser client's socket is a fake one that
//! hands its bytes to the page's own Den Lite). The sessions (login.rs, world.rs) only read, write, clone the
//! writing half, and close; everything else about a connection is set up where it is accepted.
//!
//! [`pair`] is a connected in-memory pair (two threads, a condvar): the native tests run a whole session over it,
//! which is exactly what the page does over its own stream (web.rs), so the transport is tested without sockets.
//! A later transport (a friend's page over a WebRTC data channel, docs/PHONE-SOLO.md "Sky Island") is one more
//! [`Wire`].

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Condvar, Mutex};

/// One connection's bytes, both ways.
pub trait Wire: Read + Write + Send {
    /// A second handle on the same connection (the session's reader and its shared writer).
    fn clone_wire(&self) -> io::Result<Box<dyn Wire>>;
    /// Closes both ways: a read blocked on it returns, the other side sees the end.
    fn close(&self);
}

impl Wire for TcpStream {
    fn clone_wire(&self) -> io::Result<Box<dyn Wire>> {
        Ok(Box::new(self.try_clone()?))
    }
    fn close(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

/// What the sessions hold: any [`Wire`].
pub struct Stream(Box<dyn Wire>);

impl Stream {
    pub fn new(w: impl Wire + 'static) -> Stream {
        Stream(Box::new(w))
    }
    pub fn tcp(s: TcpStream) -> Stream {
        Stream(Box::new(s))
    }
    pub fn try_clone(&self) -> io::Result<Stream> {
        Ok(Stream(self.0.clone_wire()?))
    }
    /// As TcpStream::shutdown (the host's Kick): both ways, whatever `how` says.
    pub fn shutdown(&self, _how: Shutdown) -> io::Result<()> {
        self.0.close();
        Ok(())
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// One direction of an in-memory pair.
#[derive(Default)]
struct Pipe {
    bytes: VecDeque<u8>,
    closed: bool,
}

type Half = Arc<(Mutex<Pipe>, Condvar)>;

/// One end of [`pair`]: reads from one pipe, writes to the other.
pub struct MemEnd {
    rx: Half,
    tx: Half,
}

fn lock(h: &Half) -> std::sync::MutexGuard<'_, Pipe> {
    h.0.lock().unwrap_or_else(|e| e.into_inner())
}

impl Read for MemEnd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut p = lock(&self.rx);
        while p.bytes.is_empty() && !p.closed {
            p = self.rx.1.wait(p).unwrap_or_else(|e| e.into_inner());
        }
        let n = buf.len().min(p.bytes.len());
        for (i, b) in p.bytes.drain(..n).enumerate() {
            buf[i] = b;
        }
        Ok(n)
    }
}

impl Write for MemEnd {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut p = lock(&self.tx);
        if p.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"));
        }
        p.bytes.extend(buf);
        self.tx.1.notify_all();
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Wire for MemEnd {
    fn clone_wire(&self) -> io::Result<Box<dyn Wire>> {
        Ok(Box::new(MemEnd { rx: self.rx.clone(), tx: self.tx.clone() }))
    }
    fn close(&self) {
        for h in [&self.rx, &self.tx] {
            lock(h).closed = true;
            h.1.notify_all();
        }
    }
}

/// Two connected in-memory ends (a client's and a server's).
pub fn pair() -> (Stream, Stream) {
    let a: Half = Arc::new((Mutex::new(Pipe::default()), Condvar::new()));
    let b: Half = Arc::new((Mutex::new(Pipe::default()), Condvar::new()));
    (Stream::new(MemEnd { rx: a.clone(), tx: b.clone() }), Stream::new(MemEnd { rx: b, tx: a }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pair_carries_bytes_both_ways_in_order_and_ends_on_close() {
        let (mut c, mut s) = pair();
        c.write_all(b"hello ").unwrap();
        c.write_all(b"den").unwrap();
        let mut got = [0u8; 9];
        s.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"hello den");
        let mut w = s.try_clone().unwrap();
        let t = std::thread::spawn(move || {
            let mut b = [0u8; 4];
            c.read_exact(&mut b).unwrap();
            let mut rest = Vec::new();
            c.read_to_end(&mut rest).unwrap();
            (b, rest)
        });
        w.write_all(b"pong").unwrap();
        w.write_all(b"!").unwrap();
        s.shutdown(Shutdown::Both).unwrap();
        let (b, rest) = t.join().unwrap();
        assert_eq!(&b, b"pong");
        assert_eq!(rest, b"!", "what was written before the close still arrives");
        assert!(w.write_all(b"x").is_err(), "a closed stream refuses writes");
    }

    #[test]
    fn a_blocked_read_returns_when_the_other_side_closes() {
        let (c, mut s) = pair();
        let t = std::thread::spawn(move || {
            let mut b = [0u8; 1];
            s.read(&mut b).unwrap()
        });
        std::thread::sleep(std::time::Duration::from_millis(30));
        c.shutdown(Shutdown::Both).unwrap();
        assert_eq!(t.join().unwrap(), 0);
    }
}
