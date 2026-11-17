use std::fs::File;
use std::io::BufReader;
use std::sync::Mutex;
use std::thread;

use rodio::{Decoder, DeviceSinkBuilder, Player};

static PLAYER: Mutex<Option<Player>> = Mutex::new(None);
/// Kept alive so the audio thread (and its device sink) persists.
static _KEEPALIVE: Mutex<Option<std::sync::mpsc::Sender<()>>> = Mutex::new(None);

fn ensure_init() {
    let mut player_lock = PLAYER.lock().unwrap();
    if player_lock.is_some() {
        return;
    }

    let (keep_tx, keep_rx) = std::sync::mpsc::channel::<()>();
    let (player_tx, player_rx) = std::sync::mpsc::channel::<Player>();

    thread::spawn(move || {
        let mut device = DeviceSinkBuilder::open_default_sink().expect("audio output");
        // Reopened on every track, and rodio logs each close to stderr.
        device.log_on_drop(false);
        let player = Player::connect_new(device.mixer());
        player.pause();
        let _ = player_tx.send(player);
        let _ = keep_rx.recv();
    });

    *player_lock = Some(player_rx.recv().expect("player from audio thread"));
    *_KEEPALIVE.lock().unwrap() = Some(keep_tx);
}

fn with_player<F, R>(f: F) -> R
where
    F: FnOnce(&Player) -> R,
    R: Default,
{
    ensure_init();
    let lock = PLAYER.lock().unwrap();
    match lock.as_ref() {
        Some(player) => f(player),
        None => R::default(),
    }
}
pub fn music_play(path: &str) -> Result<(), String> {
    ensure_init();

    let file = File::open(path).map_err(|e| format!("open: {e}"))?;
    let source = Decoder::new(BufReader::new(file)).map_err(|e| format!("decode: {e}"))?;

    // Stop + recreate the player (stop() invalidates it for further use)
    {
        let mut lock = PLAYER.lock().unwrap();
        if let Some(ref player) = *lock {
            player.stop();
        }
        *lock = None;
    }
    *_KEEPALIVE.lock().unwrap() = None;

    ensure_init();
    with_player(|player| {
        player.append(source);
        player.play();
    });
    Ok(())
}
pub fn music_pause() {
    with_player(|player| player.pause());
}
pub fn music_resume() {
    with_player(|player| player.play());
}
pub fn music_stop() {
    with_player(|player| player.stop());
}
pub fn music_is_finished() -> bool {
    with_player(|player| player.empty())
}
