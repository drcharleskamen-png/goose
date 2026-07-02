//! Background stdin reader that lets users type steering messages while the
//! agent is actively working.
//!
//! While `process_agent_response` streams agent events, the rustyline editor
//! is not running, so nothing is reading stdin. This module spawns a thread
//! that puts the terminal into non-canonical (raw-ish) mode with echo off and
//! reads keystrokes as they arrive, forwarding edit/submit events over a
//! channel. The session loop renders a live compose line (so typing is
//! visible) and queues submitted lines onto the agent's pending-steer queue
//! via [`goose::agents::Agent::steer`].
//!
//! The reader must never fight over the terminal with other readers:
//! - It is pausable (see [`SteerControl::pause`]) so interactive prompts such
//!   as tool confirmations (cliclack) can take over stdin; the original
//!   terminal attributes are restored while paused.
//! - It stops and restores the terminal when the control handle is dropped,
//!   before rustyline resumes reading input.
//!
//! ISIG is left enabled so Ctrl+C still raises SIGINT and cancellation works.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// How long the reader thread polls stdin per iteration. Kept short so that
/// stopping (drop) and pausing stay responsive.
#[cfg(unix)]
const POLL_INTERVAL_MS: u64 = 50;

/// Events emitted by the steer reader.
#[derive(Debug)]
pub enum SteerEvent {
    /// The compose buffer changed (keystroke, backspace, kill-line). Contains
    /// the full current buffer so the caller can redraw the input line.
    Edit(String),
    /// The user pressed Enter on a non-empty line.
    Submit(String),
}

/// Spawn the steering stdin reader.
///
/// When `enabled` is false (non-interactive sessions, json output modes,
/// stdin not a tty) no thread is spawned and the receiver never yields,
/// which makes it safe to use as an inert `tokio::select!` branch.
pub fn spawn_steer_reader(enabled: bool) -> (SteerControl, mpsc::UnboundedReceiver<SteerEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let idle = Arc::new(AtomicBool::new(false));

    let handle = if enabled {
        spawn_reader_thread(tx.clone(), stop.clone(), paused.clone(), idle.clone())
    } else {
        None
    };

    (
        SteerControl {
            stop,
            paused,
            idle,
            handle,
            _tx: tx,
        },
        rx,
    )
}

pub struct SteerControl {
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    // Keeps the channel open even when no reader thread was spawned (or the
    // thread exited on EOF) so the receiver stays pending instead of
    // resolving to `None` in a tight loop.
    _tx: mpsc::UnboundedSender<SteerEvent>,
}

impl SteerControl {
    /// Pause reading so another component (e.g. a cliclack confirmation
    /// prompt) can consume stdin with normal terminal attributes. Waits
    /// briefly for the reader thread to acknowledge so an in-flight poll
    /// settles and the terminal is restored before the other reader starts.
    pub async fn pause(&self) {
        if self.handle.is_none() {
            return;
        }
        self.paused.store(true, Ordering::SeqCst);
        for _ in 0..40 {
            if self.idle.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Resume reading after a [`Self::pause`].
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }
}

impl Drop for SteerControl {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Guard that switches stdin to non-canonical mode with echo off so the
/// reader sees individual keystrokes and they don't get echoed into the
/// agent's streamed output. Original attributes are restored on drop and
/// while paused. ISIG stays enabled so Ctrl+C keeps working.
#[cfg(unix)]
struct TermGuard {
    fd: libc::c_int,
    original: libc::termios,
    active: bool,
}

#[cfg(unix)]
impl TermGuard {
    fn new(fd: libc::c_int) -> Option<Self> {
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
            return None;
        }
        Some(Self {
            fd,
            original,
            active: false,
        })
    }

    fn enter_raw(&mut self) {
        if self.active {
            return;
        }
        let mut raw = self.original;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &raw) } == 0 {
            self.active = true;
        }
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.original) };
        self.active = false;
    }
}

#[cfg(unix)]
impl Drop for TermGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Tracks ANSI escape sequences (arrow keys etc.) so they can be swallowed
/// instead of ending up in the compose buffer.
#[cfg(unix)]
#[derive(PartialEq)]
enum EscState {
    None,
    Esc,
    Csi,
}

#[cfg(unix)]
fn spawn_reader_thread(
    tx: mpsc::UnboundedSender<SteerEvent>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
) -> Option<std::thread::JoinHandle<()>> {
    Some(std::thread::spawn(move || {
        let fd: libc::c_int = libc::STDIN_FILENO;
        let mut guard = TermGuard::new(fd);
        let mut compose = String::new();
        // Bytes of a UTF-8 sequence still in flight.
        let mut pending: Vec<u8> = Vec::new();
        let mut esc = EscState::None;

        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            if paused.load(Ordering::SeqCst) {
                if let Some(g) = guard.as_mut() {
                    g.restore();
                }
                idle.store(true, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            idle.store(false, Ordering::SeqCst);
            if let Some(g) = guard.as_mut() {
                g.enter_raw();
            }

            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, POLL_INTERVAL_MS as libc::c_int) };
            if ret < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }
            if ret == 0 {
                continue;
            }
            if pfd.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
                break;
            }
            if pfd.revents & (libc::POLLIN | libc::POLLHUP) == 0 {
                continue;
            }

            let mut buf = [0u8; 1024];
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                // EOF or read error: stop reading; the session loop keeps
                // running and the channel stays open via the control handle.
                break;
            }

            let mut changed = false;
            for &byte in &buf[..n as usize] {
                // Swallow ANSI escape sequences (arrow keys etc.).
                match esc {
                    EscState::Esc => {
                        esc = if byte == b'[' || byte == b'O' {
                            EscState::Csi
                        } else {
                            EscState::None
                        };
                        continue;
                    }
                    EscState::Csi => {
                        if (0x40..=0x7e).contains(&byte) {
                            esc = EscState::None;
                        }
                        continue;
                    }
                    EscState::None => {}
                }

                match byte {
                    0x1b => {
                        esc = EscState::Esc;
                        pending.clear();
                    }
                    b'\r' | b'\n' => {
                        pending.clear();
                        let line = compose.trim().to_string();
                        compose.clear();
                        changed = true;
                        if !line.is_empty() && tx.send(SteerEvent::Submit(line)).is_err() {
                            return;
                        }
                    }
                    0x7f | 0x08 => {
                        pending.clear();
                        if compose.pop().is_some() {
                            changed = true;
                        }
                    }
                    0x15 => {
                        // Ctrl+U: kill line
                        pending.clear();
                        if !compose.is_empty() {
                            compose.clear();
                            changed = true;
                        }
                    }
                    b'\t' => {
                        pending.clear();
                    }
                    b if b < 0x20 => {
                        // Other control chars: ignore.
                        pending.clear();
                    }
                    b => {
                        pending.push(b);
                        match std::str::from_utf8(&pending) {
                            Ok(s) => {
                                compose.push_str(s);
                                pending.clear();
                                changed = true;
                            }
                            Err(e) if e.error_len().is_some() => {
                                // Invalid sequence: drop it.
                                pending.clear();
                            }
                            Err(_) => {
                                // Incomplete UTF-8: wait for more bytes.
                            }
                        }
                    }
                }
            }

            if changed && tx.send(SteerEvent::Edit(compose.clone())).is_err() {
                return;
            }
        }
    }))
}

#[cfg(not(unix))]
fn spawn_reader_thread(
    _tx: mpsc::UnboundedSender<SteerEvent>,
    _stop: Arc<AtomicBool>,
    _paused: Arc<AtomicBool>,
    _idle: Arc<AtomicBool>,
) -> Option<std::thread::JoinHandle<()>> {
    // Mid-run steering input is not supported on non-unix platforms yet:
    // there is no way to poll console input without conflicting with output
    // rendering and rustyline.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_reader_never_yields() {
        let (_control, mut rx) = spawn_steer_reader(false);
        let result = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(
            result.is_err(),
            "a disabled steer reader must stay pending so it is inert in select! loops"
        );
    }

    #[tokio::test]
    async fn disabled_reader_pause_resume_are_noops() {
        let (control, _rx) = spawn_steer_reader(false);
        control.pause().await;
        control.resume();
    }
}
