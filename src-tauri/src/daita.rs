use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

static ACTIVE: AtomicBool = AtomicBool::new(false);

const MIN_PACKET: usize = 64;
const MAX_PACKET: usize = 1420;
const MIN_INTERVAL_MS: u64 = 50;
const MAX_INTERVAL_MS: u64 = 500;
const DISCARD_PORT: u16 = 9;

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub fn start(gateway_ip: &str) -> Result<(), String> {
    if ACTIVE.load(Ordering::Relaxed) {
        return Ok(());
    }
    ACTIVE.store(true, Ordering::Relaxed);

    let target = format!("{}:{}", gateway_ip, DISCARD_PORT);
    let target_addr: std::net::SocketAddr = target
        .parse()
        .map_err(|e| format!("invalid gateway address: {e}"))?;

    thread::spawn(move || {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[DAITA] failed to bind UDP socket: {e}");
                ACTIVE.store(false, Ordering::Relaxed);
                return;
            }
        };
        socket.set_nonblocking(false).ok();

        while ACTIVE.load(Ordering::Relaxed) {
            let size = random_range(MIN_PACKET, MAX_PACKET);
            let buf = vec![0u8; size];
            let _ = socket.send_to(&buf, target_addr);

            let delay = random_range(MIN_INTERVAL_MS as usize, MAX_INTERVAL_MS as usize);
            thread::sleep(Duration::from_millis(delay as u64));
        }
    });

    Ok(())
}

pub fn stop() {
    ACTIVE.store(false, Ordering::Relaxed);
}

fn random_range(min: usize, max: usize) -> usize {
    let bytes = rand::random::<[u8; 2]>();
    let val = u16::from_le_bytes(bytes) as usize;
    min + (val % (max - min + 1))
}
