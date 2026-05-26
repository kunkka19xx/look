use std::fs::File;
use std::io::BufReader;
use std::sync::Mutex;
use std::thread;

use rodio::{Decoder, OutputStream, Sink};

static SINK: Mutex<Option<Sink>> = Mutex::new(None);
/// Kept alive so the audio thread (and OutputStream) persists.
static _KEEPALIVE: Mutex<Option<std::sync::mpsc::Sender<()>>> = Mutex::new(None);

fn ensure_init() {
    let mut sink_lock = SINK.lock().unwrap();
    if sink_lock.is_some() {
        return;
    }

    let (keep_tx, keep_rx) = std::sync::mpsc::channel::<()>();
    let (sink_tx, sink_rx) = std::sync::mpsc::channel::<Sink>();

    thread::spawn(move || {
        let (_stream, handle) = OutputStream::try_default().expect("audio output");
        let sink = Sink::try_new(&handle).expect("audio sink");
        sink.pause();
        let _ = sink_tx.send(sink);
        let _ = keep_rx.recv();
    });

    *sink_lock = Some(sink_rx.recv().expect("sink from audio thread"));
    *_KEEPALIVE.lock().unwrap() = Some(keep_tx);
}

fn with_sink<F, R>(f: F) -> R
where
    F: FnOnce(&Sink) -> R,
    R: Default,
{
    ensure_init();
    let lock = SINK.lock().unwrap();
    match lock.as_ref() {
        Some(sink) => f(sink),
        None => R::default(),
    }
}

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

#[tauri::command]
pub fn music_pause() {
    with_sink(|sink| sink.pause());
}

#[tauri::command]
pub fn music_resume() {
    with_sink(|sink| sink.play());
}

#[tauri::command]
pub fn music_stop() {
    with_sink(|sink| sink.stop());
}

#[tauri::command]
pub fn music_is_finished() -> bool {
    with_sink(|sink| sink.empty())
}
