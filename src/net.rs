//! LAN multiplayer plumbing for the "Race" game mode.
//!
//! Two pieces:
//!
//! * **Discovery** — every instance broadcasts a UDP `HELLO` packet a couple of
//!   times per second on `255.255.255.255:47821` and listens for the same
//!   packets coming back. UDP sockets are bound with `SO_REUSEADDR` so multiple
//!   processes on the *same host* can all participate. Each instance has a
//!   random 64-bit id and ignores `HELLO`s carrying its own id.
//!
//! * **Race** — every instance also keeps a TCP listener on an ephemeral port
//!   (announced in the `HELLO`). Clicking a peer in the lobby opens a TCP
//!   connection; the side that *accepts* picks the text and sends `START`,
//!   then both sides exchange `PROGRESS` and `DONE` messages.
//!
//! All wire messages are plain text, tab-separated, one per line. The protocol
//! is intentionally tiny — see the module-level comment in `main.rs` for the
//! grammar.
//!
//! Background threads communicate with the UI thread via a single
//! `mpsc::channel<NetEvent>`. The UI polls it from its existing 100 ms timer.
//!
//! No external dependencies are required beyond `socket2` (only used to enable
//! `SO_REUSEADDR` / `SO_BROADCAST`, which `std::net::UdpSocket` doesn't expose).
//!
//! This module is intentionally tolerant of network errors — anything
//! unexpected results in a `Status` event and (for the race connection) a
//! graceful teardown, rather than a panic.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use socket2::{Domain, Protocol, Socket, Type};

const DISCOVERY_PORT: u16 = 47821;
const HELLO_INTERVAL: Duration = Duration::from_millis(1500);
const PEER_TIMEOUT: Duration = Duration::from_millis(6000);
const TCP_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum time the server side waits for the user to accept an incoming race.
const INCOMING_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

// ---------------------------------------------------------------------------
//  Public types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Peer {
    pub id: String,
    pub name: String,
    /// TCP address the peer is listening on (for initiating races).
    pub tcp_addr: SocketAddr,
    pub last_seen: Instant,
}

/// Events pushed from background threads to the UI thread.
#[derive(Debug)]
pub enum NetEvent {
    /// Peer table changed (someone joined, left, or renamed).
    PeersUpdated(Vec<Peer>),
    /// Another peer has connected and is asking to race. The UI should show
    /// an accept/decline prompt and call [`NetService::accept_incoming_race`]
    /// or [`NetService::reject_incoming_race`].
    IncomingRaceRequest { opponent: Peer },
    /// We just accepted an incoming race and we are the "server"; the text we
    /// picked is included.
    RaceStartedAsServer { text: String, opponent: Peer },
    /// We just connected to a peer and they replied with `START`.
    RaceStartedAsClient { text: String, opponent: Peer },
    /// Opponent's typing progress.
    OpponentProgress { position: usize, errors: usize },
    /// Opponent finished — final WPM and accuracy as reported by them.
    OpponentFinished { wpm: f64, accuracy: f64 },
    /// The race connection closed (peer hung up, network error, …).
    OpponentDisconnected,
    /// The remote peer rejected our race invitation.
    InviteRejected {
        #[allow(dead_code)]
        opponent_id: String,
    },
    /// Free-form status string for the UI to display.
    Status(String),
}

// ---------------------------------------------------------------------------
//  Service
// ---------------------------------------------------------------------------

pub struct NetService {
    #[allow(dead_code)]
    pub my_id: String,
    pub tcp_port: u16,
    pub event_rx: Receiver<NetEvent>,
    inner: Arc<Inner>,
}

struct Inner {
    my_id: String,
    /// Current display name. Wrapped in a Mutex so it can be changed at
    /// runtime via [`NetService::set_name`] without restarting threads.
    name: Mutex<String>,
    tcp_port: u16,
    running: AtomicBool,
    /// Discovered peers keyed by id.
    peers: Mutex<HashMap<String, Peer>>,
    /// Active race connection. We only support one race at a time.
    race: Mutex<Option<RaceHandle>>,
    /// In-flight incoming race request waiting for the user's accept/decline.
    pending_request: Mutex<Option<PendingRequest>>,
    /// Lamport-ish counter we bump every time we mutate `peers` so the
    /// discovery thread can decide when to push a `PeersUpdated` event.
    peers_version: AtomicU64,
    /// Cloned into every worker thread so they can publish events back to the UI.
    event_tx: Sender<NetEvent>,
}

/// Bookkeeping for an active race connection.
struct RaceHandle {
    /// Write half of the TCP stream (we wrap it in a Mutex so the UI thread
    /// can call `send_progress` / `send_done` safely).
    writer: Arc<Mutex<TcpStream>>,
}

/// Bookkeeping for an unanswered incoming race request. The server thread
/// blocks on the condvar until the user clicks Accept or Decline (or the
/// request times out).
struct PendingRequest {
    decision: Arc<(Mutex<Option<bool>>, Condvar)>,
}

impl NetService {
    /// Spawns the discovery + listener threads and returns a handle.
    ///
    /// `display_name` is what other peers see in their lobby. It can be
    /// changed later via [`Self::set_name`].
    pub fn start(display_name: String) -> std::io::Result<Self> {
        let my_id = random_id();
        let (event_tx, event_rx) = channel();

        // ----- TCP listener (always on, accepts incoming races) -------------
        let tcp_listener = TcpListener::bind("0.0.0.0:0")?;
        tcp_listener.set_nonblocking(false)?;
        let tcp_port = tcp_listener.local_addr()?.port();

        // ----- UDP discovery socket (SO_REUSEADDR + SO_BROADCAST) -----------
        let udp_recv = make_discovery_socket()?;
        // Best-effort: tell the kernel we're a broadcaster. Some BSD-like
        // stacks need this to be set even on the *send* socket; we use the
        // same socket for both.
        udp_recv.set_broadcast(true).ok();

        let inner = Arc::new(Inner {
            my_id: my_id.clone(),
            name: Mutex::new(display_name),
            tcp_port,
            running: AtomicBool::new(true),
            peers: Mutex::new(HashMap::new()),
            race: Mutex::new(None),
            pending_request: Mutex::new(None),
            peers_version: AtomicU64::new(0),
            event_tx: event_tx.clone(),
        });

        // ----- Threads ------------------------------------------------------
        spawn_discovery_recv(udp_recv.try_clone()?, inner.clone());
        spawn_discovery_send(udp_recv, inner.clone());
        spawn_peer_reaper(inner.clone());
        spawn_tcp_accept(tcp_listener, inner.clone());

        Ok(Self {
            my_id,
            tcp_port,
            event_rx,
            inner,
        })
    }

    pub fn current_peers(&self) -> Vec<Peer> {
        let peers = self.inner.peers.lock().unwrap();
        peers.values().cloned().collect()
    }

    /// Update the display name advertised in HELLO broadcasts. Takes effect
    /// on the next broadcast tick (within ~1.5 s).
    pub fn set_name(&self, new_name: String) {
        let trimmed = new_name.trim();
        let resolved = if trimmed.is_empty() {
            "Player".to_string()
        } else {
            trimmed.to_string()
        };
        *self.inner.name.lock().unwrap() = resolved;
    }

    /// Open a race connection to `peer_id`. The remote side becomes the
    /// "server" and picks the text. Returns immediately; the actual outcome
    /// is delivered as a `NetEvent` (RaceStartedAsClient on success,
    /// InviteRejected if the remote declined, or OpponentDisconnected on
    /// network failure).
    pub fn invite(&self, peer_id: &str) -> std::io::Result<()> {
        let peer = {
            let peers = self.inner.peers.lock().unwrap();
            peers
                .get(peer_id)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "unknown peer"))?
        };
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("keystroke-race-client".into())
            .spawn(move || {
                if let Err(e) = run_race_client(&inner, peer.clone()) {
                    let _ = inner
                        .event_tx
                        .send(NetEvent::Status(format!("Race connection failed: {}", e)));
                    let _ = inner.event_tx.send(NetEvent::OpponentDisconnected);
                    inner.race.lock().unwrap().take();
                }
            })?;
        Ok(())
    }

    /// Respond to an in-flight [`NetEvent::IncomingRaceRequest`]. No-op if no
    /// request is pending.
    pub fn accept_incoming_race(&self) {
        self.respond_to_pending(true);
    }

    /// Decline an in-flight [`NetEvent::IncomingRaceRequest`]. No-op if no
    /// request is pending.
    pub fn reject_incoming_race(&self) {
        self.respond_to_pending(false);
    }

    fn respond_to_pending(&self, accept: bool) {
        // Clone the decision Arc out of the mutex so we don't hold the outer
        // lock while signalling the condvar (the waiter re-takes the outer
        // lock to clear the pending slot, which would deadlock otherwise).
        let decision = {
            let guard = self.inner.pending_request.lock().unwrap();
            guard.as_ref().map(|p| p.decision.clone())
        };
        if let Some(d) = decision {
            let (mtx, cv) = &*d;
            *mtx.lock().unwrap() = Some(accept);
            cv.notify_all();
        }
    }

    /// Send a `PROGRESS` update to the opponent. Best-effort.
    pub fn send_progress(&self, position: usize, errors: usize) {
        self.write_line(&format!("PROGRESS\t{}\t{}\n", position, errors));
    }

    /// Send a final `DONE` to the opponent. Best-effort.
    pub fn send_done(&self, wpm: f64, accuracy: f64) {
        self.write_line(&format!(
            "DONE\t{}\t{}\n",
            (wpm * 100.0).round() as i64,
            (accuracy * 100.0).round() as i64
        ));
    }

    /// Tear down any active race. Idempotent.
    pub fn quit_race(&self) {
        self.write_line("QUIT\n");
        if let Some(h) = self.inner.race.lock().unwrap().take() {
            // Closing the writer half will cause the reader thread to exit.
            let _ = h.writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
        }
        // Also reject any pending incoming request so we don't leak the
        // waiting server thread.
        self.respond_to_pending(false);
    }

    fn write_line(&self, line: &str) {
        let race = self.inner.race.lock().unwrap();
        if let Some(handle) = race.as_ref() {
            let stream = handle.writer.clone();
            // Best-effort write; ignore the result.
            let mut s = stream.lock().unwrap();
            let _ = s.write_all(line.as_bytes());
            let _ = s.flush();
        }
    }
}

impl Drop for NetService {
    fn drop(&mut self) {
        self.inner.running.store(false, Ordering::SeqCst);
        // Tearing down the active race makes the worker threads exit promptly.
        self.quit_race();
    }
}

// ---------------------------------------------------------------------------
//  Discovery — socket setup
// ---------------------------------------------------------------------------

fn make_discovery_socket() -> std::io::Result<std::net::UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // Allow multiple processes on the same host to bind the discovery port.
    sock.set_reuse_address(true)?;
    // On Unix-likes, also enable SO_REUSEPORT when available so all bound
    // sockets actually *receive* incoming packets (not just one).
    #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
    {
        let _ = sock.set_reuse_port(true);
    }
    sock.set_broadcast(true)?;
    let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT));
    sock.bind(&bind_addr.into())?;
    Ok(sock.into())
}

// ---------------------------------------------------------------------------
//  Discovery — threads
// ---------------------------------------------------------------------------

fn spawn_discovery_send(sock: std::net::UdpSocket, inner: Arc<Inner>) {
    thread::Builder::new()
        .name("keystroke-net-broadcast".into())
        .spawn(move || {
            let bcast = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, DISCOVERY_PORT));
            // Tell the UI what we know about ourselves.
            {
                let name = inner.name.lock().unwrap().clone();
                let _ = inner.event_tx.send(NetEvent::Status(format!(
                    "Listening on TCP port {} as \"{}\"",
                    inner.tcp_port, name
                )));
            }
            while inner.running.load(Ordering::Relaxed) {
                // Build the payload fresh each tick so renames take effect.
                let name = inner.name.lock().unwrap().clone();
                let payload =
                    format!("HELLO\t{}\t{}\t{}\n", inner.my_id, inner.tcp_port, name);
                let _ = sock.send_to(payload.as_bytes(), bcast);
                thread::sleep(HELLO_INTERVAL);
            }
        })
        .expect("spawn broadcast thread");
}

fn spawn_discovery_recv(sock: std::net::UdpSocket, inner: Arc<Inner>) {
    thread::Builder::new()
        .name("keystroke-net-recv".into())
        .spawn(move || {
            // Use a read timeout so we can periodically re-check `running`.
            let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 1024];
            while inner.running.load(Ordering::Relaxed) {
                let (n, from) = match sock.recv_from(&mut buf) {
                    Ok(x) => x,
                    Err(_) => continue,
                };
                let line = match std::str::from_utf8(&buf[..n]) {
                    Ok(s) => s.trim_end_matches(['\r', '\n']),
                    Err(_) => continue,
                };
                let Some(hello) = parse_hello(line) else { continue };
                if hello.id == inner.my_id {
                    continue; // ourselves
                }
                let peer_ip = match from {
                    SocketAddr::V4(v4) => *v4.ip(),
                    // IPv6 broadcast isn't a thing; skip for now.
                    SocketAddr::V6(_) => continue,
                };
                let peer = Peer {
                    id: hello.id.clone(),
                    name: hello.name,
                    tcp_addr: SocketAddr::V4(SocketAddrV4::new(peer_ip, hello.tcp_port)),
                    last_seen: Instant::now(),
                };
                {
                    let mut peers = inner.peers.lock().unwrap();
                    let changed = match peers.get(&hello.id) {
                        Some(p) => p.name != peer.name || p.tcp_addr != peer.tcp_addr,
                        None => true,
                    };
                    peers.insert(hello.id, peer);
                    if changed {
                        inner.peers_version.fetch_add(1, Ordering::Relaxed);
                        let snapshot: Vec<Peer> = peers.values().cloned().collect();
                        let _ = inner.event_tx.send(NetEvent::PeersUpdated(snapshot));
                    }
                }
            }
        })
        .expect("spawn recv thread");
}

fn spawn_peer_reaper(inner: Arc<Inner>) {
    thread::Builder::new()
        .name("keystroke-net-reaper".into())
        .spawn(move || {
            while inner.running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1000));
                let mut changed = false;
                let mut peers = inner.peers.lock().unwrap();
                let before = peers.len();
                peers.retain(|_, p| p.last_seen.elapsed() < PEER_TIMEOUT);
                if peers.len() != before {
                    changed = true;
                }
                if changed {
                    inner.peers_version.fetch_add(1, Ordering::Relaxed);
                    let snapshot: Vec<Peer> = peers.values().cloned().collect();
                    let _ = inner.event_tx.send(NetEvent::PeersUpdated(snapshot));
                }
            }
        })
        .expect("spawn reaper thread");
}

// ---------------------------------------------------------------------------
//  TCP race connections — accept side (we are the server)
// ---------------------------------------------------------------------------

fn spawn_tcp_accept(listener: TcpListener, inner: Arc<Inner>) {
    thread::Builder::new()
        .name("keystroke-tcp-accept".into())
        .spawn(move || {
            // Non-blocking-with-timeout via set_read_timeout doesn't work on
            // TcpListener; instead we set the accept call non-blocking and
            // poll. Keeps shutdown responsive when `running` flips false.
            let _ = listener.set_nonblocking(true);
            while inner.running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let inner = inner.clone();
                        thread::Builder::new()
                            .name("keystroke-race-server".into())
                            .spawn(move || {
                                if let Err(e) = run_race_server(&inner, stream) {
                                    let _ = inner
                                        .event_tx
                                        .send(NetEvent::Status(format!("Race ended: {}", e)));
                                    let _ = inner.event_tx.send(NetEvent::OpponentDisconnected);
                                    inner.race.lock().unwrap().take();
                                }
                            })
                            .ok();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(150));
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        })
        .expect("spawn accept thread");
}

/// Server-side race handler: the remote peer connected to us. We expect their
/// `HELLO`, then prompt the user to accept the race. Once accepted, we pick
/// a text and send `START`. After that we relay `PROGRESS` / `DONE` lines.
fn run_race_server(inner: &Arc<Inner>, mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(TCP_READ_TIMEOUT))?;
    // Refuse new races if one is already active or another prompt is open.
    if inner.race.lock().unwrap().is_some() || inner.pending_request.lock().unwrap().is_some() {
        let _ = stream.write_all(b"REJECT\tbusy\n");
        return Ok(());
    }

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let Some(hello) = parse_hello_tcp(line.trim_end()) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected HELLO",
        ));
    };

    let opponent = Peer {
        id: hello.id,
        name: hello.name,
        tcp_addr: stream.peer_addr()?,
        last_seen: Instant::now(),
    };

    // ----- Ask the UI to accept or decline ---------------------------------
    let decision = Arc::new((Mutex::new(None::<bool>), Condvar::new()));
    {
        let mut pending = inner.pending_request.lock().unwrap();
        *pending = Some(PendingRequest {
            decision: decision.clone(),
        });
    }
    let _ = inner.event_tx.send(NetEvent::IncomingRaceRequest {
        opponent: opponent.clone(),
    });

    let accepted = wait_for_decision(&decision, INCOMING_REQUEST_TIMEOUT);
    // Clear the pending slot, regardless of outcome.
    inner.pending_request.lock().unwrap().take();

    if !accepted {
        let _ = stream.write_all(b"REJECT\tdeclined\n");
        return Ok(());
    }

    // ----- Race accepted: pick a text and send START -----------------------
    let text = crate::texts::pick_random_text().to_string();

    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    {
        let mut race = inner.race.lock().unwrap();
        *race = Some(RaceHandle {
            writer: writer.clone(),
        });
    }

    let start = format!("START\t{}\n", text.replace('\t', " ").replace('\n', " "));
    writer.lock().unwrap().write_all(start.as_bytes())?;
    writer.lock().unwrap().flush()?;

    let _ = inner.event_tx.send(NetEvent::RaceStartedAsServer {
        text,
        opponent: opponent.clone(),
    });

    relay_race_messages(reader, inner)
}

/// Blocks the current thread until the decision is recorded or the timeout
/// elapses. Returns the user's choice, defaulting to `false` on timeout.
fn wait_for_decision(decision: &Arc<(Mutex<Option<bool>>, Condvar)>, timeout: Duration) -> bool {
    let (mtx, cv) = &**decision;
    let guard = mtx.lock().unwrap();
    let (guard, _) = cv
        .wait_timeout_while(guard, timeout, |opt| opt.is_none())
        .unwrap();
    guard.unwrap_or(false)
}

/// Client-side: we connect to a peer that's currently listening, send our
/// `HELLO`, then wait for `START` (accept) or `REJECT` (decline).
fn run_race_client(inner: &Arc<Inner>, peer: Peer) -> std::io::Result<()> {
    if inner.race.lock().unwrap().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "already in a race",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&peer.tcp_addr, Duration::from_secs(5))?;
    // The remote side may take a while if the user is reading the prompt;
    // give them ~50 s before we give up waiting for their decision.
    stream.set_read_timeout(Some(Duration::from_secs(55)))?;
    let hello = {
        let name = inner.name.lock().unwrap().clone();
        format!("HELLO\t{}\t{}\n", inner.my_id, name)
    };
    stream.write_all(hello.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let line = line.trim_end();
    let text = if let Some(rest) = line.strip_prefix("START\t") {
        rest.to_string()
    } else if line.starts_with("REJECT") || line.starts_with("QUIT") {
        let _ = inner.event_tx.send(NetEvent::InviteRejected {
            opponent_id: peer.id.clone(),
        });
        return Ok(());
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected START or REJECT",
        ));
    };

    // Once racing, we want lower latency on PROGRESS reads.
    stream.set_read_timeout(Some(TCP_READ_TIMEOUT))?;

    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    {
        let mut race = inner.race.lock().unwrap();
        *race = Some(RaceHandle { writer });
    }
    let _ = inner.event_tx.send(NetEvent::RaceStartedAsClient {
        text,
        opponent: peer,
    });

    relay_race_messages(reader, inner)
}

/// Shared "pump" used by both server and client roles after the handshake.
fn relay_race_messages(
    mut reader: BufReader<TcpStream>,
    inner: &Arc<Inner>,
) -> std::io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            // EOF
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        let mut parts = line.split('\t');
        match parts.next() {
            Some("PROGRESS") => {
                let pos: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let err: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let _ = inner.event_tx.send(NetEvent::OpponentProgress {
                    position: pos,
                    errors: err,
                });
            }
            Some("DONE") => {
                let wpm_x100: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let acc_x100: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let _ = inner.event_tx.send(NetEvent::OpponentFinished {
                    wpm: wpm_x100 as f64 / 100.0,
                    accuracy: acc_x100 as f64 / 100.0,
                });
            }
            Some("QUIT") => break,
            _ => { /* ignore unknown frame */ }
        }
    }
    inner.race.lock().unwrap().take();
    let _ = inner.event_tx.send(NetEvent::OpponentDisconnected);
    Ok(())
}

// ---------------------------------------------------------------------------
//  Wire parsing
// ---------------------------------------------------------------------------

struct ParsedHello {
    id: String,
    tcp_port: u16,
    name: String,
}

/// Parse a UDP `HELLO` frame: `HELLO\t<id>\t<tcp_port>\t<name>`.
/// Name is everything after the third tab and may contain spaces.
fn parse_hello(line: &str) -> Option<ParsedHello> {
    let mut it = line.splitn(4, '\t');
    if it.next()? != "HELLO" {
        return None;
    }
    let id = it.next()?.to_string();
    let tcp_port: u16 = it.next()?.parse().ok()?;
    let name = it.next().unwrap_or("Player").to_string();
    if id.is_empty() {
        return None;
    }
    Some(ParsedHello {
        id,
        tcp_port,
        name,
    })
}

/// Parse a TCP `HELLO` frame (no port field): `HELLO\t<id>\t<name>`.
fn parse_hello_tcp(line: &str) -> Option<ParsedHello> {
    let mut it = line.splitn(3, '\t');
    if it.next()? != "HELLO" {
        return None;
    }
    let id = it.next()?.to_string();
    let name = it.next().unwrap_or("Player").to_string();
    if id.is_empty() {
        return None;
    }
    Some(ParsedHello {
        id,
        tcp_port: 0,
        name,
    })
}

// ---------------------------------------------------------------------------
//  Random id
// ---------------------------------------------------------------------------

/// Cheap, dependency-free random hex id. Mixes the system time with the
/// process address space so two instances on the same host get different ids.
fn random_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as u64;
    // Use the address of a stack variable as a second entropy source.
    let stack_var = 0u8;
    let addr = (&stack_var as *const u8) as usize as u64;
    let mut x = nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(addr);
    // Splitmix64 finalizer, then take 16 hex digits.
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    format!("{:016x}", x)
}

/// First N characters of a peer id, used to disambiguate same-host duplicates
/// in the displayed name.
#[allow(dead_code)]
pub fn short_id(id: &str) -> &str {
    &id[..id.len().min(4)]
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hello_udp_basic() {
        let h = parse_hello("HELLO\tabcd1234\t5555\tAlice").unwrap();
        assert_eq!(h.id, "abcd1234");
        assert_eq!(h.tcp_port, 5555);
        assert_eq!(h.name, "Alice");
    }

    #[test]
    fn parse_hello_udp_name_with_spaces() {
        let h = parse_hello("HELLO\tx\t1\tAlice the Great").unwrap();
        assert_eq!(h.name, "Alice the Great");
    }

    #[test]
    fn parse_hello_udp_rejects_garbage() {
        assert!(parse_hello("GARBAGE").is_none());
        assert!(parse_hello("HELLO\t\t1\tname").is_none()); // empty id
        assert!(parse_hello("HELLO\tid\tnotaport\tname").is_none());
    }

    #[test]
    fn parse_hello_tcp_basic() {
        let h = parse_hello_tcp("HELLO\tabcd\tBob").unwrap();
        assert_eq!(h.id, "abcd");
        assert_eq!(h.name, "Bob");
    }

    #[test]
    fn random_ids_are_unique_within_a_burst() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            assert!(seen.insert(random_id()));
        }
    }

    #[test]
    fn short_id_handles_short_inputs() {
        assert_eq!(short_id("abc"), "abc");
        assert_eq!(short_id("abcdef"), "abcd");
        assert_eq!(short_id(""), "");
    }
}
