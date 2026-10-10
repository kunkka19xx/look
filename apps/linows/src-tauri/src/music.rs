use std::fs::File;
use std::io::BufReader;
use std::sync::Mutex;
use std::thread;

use rodio::{Decoder, DeviceSinkBuilder, Player};

static SINK: Mutex<Option<Player>> = Mutex::new(None);
/// Kept alive so the audio thread (and its device sink) persists.
static _KEEPALIVE: Mutex<Option<std::sync::mpsc::Sender<()>>> = Mutex::new(None);

/// Ensures the audio output device and sink thread are initialized.
fn ensure_init() {
    let mut sink_lock = SINK.lock().unwrap();
    if sink_lock.is_some() {
        return;
    }

    let (keep_tx, keep_rx) = std::sync::mpsc::channel::<()>();
    let (sink_tx, sink_rx) = std::sync::mpsc::channel::<Player>();

    thread::spawn(move || {
        let mut device = DeviceSinkBuilder::open_default_sink().expect("audio output");
        device.log_on_drop(false);
        let sink = Player::connect_new(device.mixer());
        sink.pause();
        let _ = sink_tx.send(sink);
        let _ = keep_rx.recv();
    });

    *sink_lock = Some(sink_rx.recv().expect("sink from audio thread"));
    *_KEEPALIVE.lock().unwrap() = Some(keep_tx);
}

/// Executes a closure against the active audio player sink.
fn with_sink<F, R>(f: F) -> R
where
    F: FnOnce(&Player) -> R,
    R: Default,
{
    ensure_init();
    let lock = SINK.lock().unwrap();
    match lock.as_ref() {
        Some(sink) => f(sink),
        None => R::default(),
    }
}

/// Loads and begins playback of an audio file at the specified path.
#[tauri::command]
pub fn music_play(path: String) -> Result<(), String> {
    ensure_init();

    let file = File::open(&path).map_err(|e| format!("open: {e}"))?;
    let source = Decoder::new(BufReader::new(file)).map_err(|e| format!("decode: {e}"))?;

    // Stop + recreate sink (stop() invalidates the sink for further use)
    {
        let mut lock = SINK.lock().unwrap();
        if let Some(ref sink) = *lock {
            sink.stop();
        }
        *lock = None;
    }
    *_KEEPALIVE.lock().unwrap() = None;

    ensure_init();
    with_sink(|sink| {
        sink.append(source);
        sink.play();
    });
    Ok(())
}

/// Pauses current music playback.
#[tauri::command]
pub fn music_pause() {
    with_sink(|sink| sink.pause());
}

/// Resumes current music playback.
#[tauri::command]
pub fn music_resume() {
    with_sink(|sink| sink.play());
}

/// Stops music playback.
#[tauri::command]
pub fn music_stop() {
    with_sink(|sink| sink.stop());
}

/// Returns true if music playback has ended.
#[tauri::command]
pub fn music_is_finished() -> bool {
    with_sink(|sink| sink.empty())
}
