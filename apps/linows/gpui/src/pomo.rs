//! The Pomodoro, kept for the process: the plan, the running session and
//! the background music outlive the window, since the launcher is rebuilt
//! on every summon. A thread ticks the running session every second, window
//! or no window, and sends the phase notifications; what the panel and the
//! slot draw is projected by the wall clock so it is exact at any instant.
//! The plan persists in `~/.look/config` under the keys macOS writes, so
//! both shells share one.

use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use linows_backend::config::{self, ConfigUpdate};
use linows_backend::{files, music};

const SESSIONS_KEY: &str = "pomo_sessions";
const STYLE_KEY: &str = "pomo_timer_style";
const MUSIC_FOLDER_KEY: &str = "pomo_music_folder";
pub const MINUTES_MIN: u32 = 1;
pub const MINUTES_MAX: u32 = 120;
pub const ENDING_SOON_SECS: u64 = 10;
const DEFAULT_FOCUS_MINUTES: u32 = 30;
const DEFAULT_BREAK_MINUTES: u32 = 5;
/// A track name longer than this shows its head and tail.
const TRACK_NAME_MAX: usize = 48;
const END_POLL_SECS: u64 = 1;
const TICK_SECS: u64 = 1;
const ALL_DONE: &str = "All sessions complete!";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Focus,
    Break,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "Focus",
            Self::Break => "Break",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Break => "break",
        }
    }

    fn parse(key: &str) -> Option<Self> {
        match key {
            "focus" => Some(Self::Focus),
            "break" => Some(Self::Break),
            _ => None,
        }
    }

    pub fn flipped(self) -> Self {
        match self {
            Self::Focus => Self::Break,
            Self::Break => Self::Focus,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub kind: Kind,
    pub minutes: u32,
    pub name: String,
}

impl Session {
    fn seconds(&self) -> u64 {
        u64::from(self.minutes) * 60
    }
}

fn default_sessions() -> Vec<Session> {
    let s = |kind, minutes, name: &str| Session {
        kind,
        minutes,
        name: name.to_string(),
    };
    vec![
        s(Kind::Focus, 30, "Deep Work"),
        s(Kind::Break, 5, "Short Break"),
        s(Kind::Focus, 30, "Review"),
        s(Kind::Break, 5, "Short Break"),
        s(Kind::Focus, 30, "Wrap Up"),
        s(Kind::Break, 15, "Long Break"),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Modern,
    Vintage,
    Minimal,
}

pub const STYLES: [Style; 3] = [Style::Modern, Style::Vintage, Style::Minimal];

impl Style {
    pub fn label(self) -> &'static str {
        match self {
            Self::Modern => "Modern Ring",
            Self::Vintage => "Vintage Dial",
            Self::Minimal => "Minimal Text",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::Vintage => "vintage",
            Self::Minimal => "minimal",
        }
    }

    fn parse(key: &str) -> Option<Self> {
        STYLES.iter().copied().find(|s| s.key() == key)
    }
}

/// What a tick found: the panel turns these into notifications.
pub enum Event {
    EndingSoon {
        kind: Kind,
        seconds_left: u64,
    },
    PhaseDone {
        done: Session,
        next: Option<Session>,
    },
}

/// The timer as the panel draws it, projected by the wall clock.
pub struct Status {
    pub active: Option<usize>,
    pub seconds_left: u64,
    pub total: u64,
    pub running: bool,
}

/// The timer as the launchpad slot shows it.
pub struct Snapshot {
    pub kind: Kind,
    pub index: usize,
    pub count: usize,
    pub seconds_left: u64,
    pub progress: f32,
}

pub struct MusicSnapshot {
    pub playing: bool,
    pub track: String,
}

pub struct Pomo {
    pub sessions: Vec<Session>,
    pub style: Style,
    pub music_folder: Option<String>,
    active: Option<usize>,
    seconds_left: u64,
    running: bool,
    last_tick: Option<Instant>,
    ending_soon_fired: bool,
    tracks: Vec<String>,
    track: Option<usize>,
    playing: bool,
    loaded: bool,
    tracks_scanned: bool,
    poller: bool,
    ticker: bool,
}

static STATE: Mutex<Pomo> = Mutex::new(Pomo {
    sessions: Vec::new(),
    style: Style::Modern,
    music_folder: None,
    active: None,
    seconds_left: 0,
    running: false,
    last_tick: None,
    ending_soon_fired: false,
    tracks: Vec::new(),
    track: None,
    playing: false,
    loaded: false,
    tracks_scanned: false,
    poller: false,
    ticker: false,
});

/// The state, read from the config on the first lock.
pub fn lock() -> MutexGuard<'static, Pomo> {
    let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
    if !guard.loaded {
        guard.load();
    }
    guard
}

/// `mm:ss`.
pub fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// The slot's view, or none while idle.
pub fn snapshot() -> Option<Snapshot> {
    lock().snapshot()
}

pub fn music_snapshot() -> Option<MusicSnapshot> {
    lock().music_snapshot()
}

impl Pomo {
    fn load(&mut self) {
        let entries = config::get_config().entries;
        let get = |key: &str| {
            entries
                .iter()
                .find(|e| e.key == key)
                .map(|e| e.value.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        self.sessions = get(SESSIONS_KEY)
            .and_then(|v| decode_sessions(&v))
            .unwrap_or_else(default_sessions);
        self.style = get(STYLE_KEY)
            .and_then(|v| Style::parse(&v))
            .unwrap_or(Style::Modern);
        self.music_folder = get(MUSIC_FOLDER_KEY);
        self.loaded = true;
    }

    /// The three keys, written over the config as macOS writes them.
    fn save(&self) {
        let updates = vec![
            ConfigUpdate {
                key: SESSIONS_KEY.into(),
                value: encode_sessions(&self.sessions),
            },
            ConfigUpdate {
                key: STYLE_KEY.into(),
                value: self.style.key().into(),
            },
            ConfigUpdate {
                key: MUSIC_FOLDER_KEY.into(),
                value: self.music_folder.clone().unwrap_or_default(),
            },
        ];
        if let Err(err) = config::set_config(updates) {
            eprintln!("[pomo] saving the plan: {err}");
        }
    }

    // --- Timer -------------------------------------------------------------------

    pub fn status(&self) -> Status {
        let total = self
            .active
            .and_then(|i| self.sessions.get(i))
            .or_else(|| self.sessions.first())
            .map_or(0, Session::seconds);
        Status {
            active: self.active,
            seconds_left: self.left_now(),
            total,
            running: self.running,
        }
    }

    /// Seconds left at this instant: the counter less what has run since
    /// the last tick credited it.
    fn left_now(&self) -> u64 {
        match (self.running, self.last_tick) {
            (true, Some(at)) => self.seconds_left.saturating_sub(at.elapsed().as_secs()),
            _ => self.seconds_left,
        }
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.active.and_then(|i| self.sessions.get(i))
    }

    fn snapshot(&self) -> Option<Snapshot> {
        let index = self.active?;
        let session = self.sessions.get(index)?;
        let total = session.seconds();
        let left = self.left_now();
        Some(Snapshot {
            kind: session.kind,
            index,
            count: self.sessions.len(),
            seconds_left: left,
            progress: if total == 0 {
                0.0
            } else {
                ((total - left) as f32 / total as f32).min(1.0)
            },
        })
    }

    /// Space: start the first session, pause, or resume.
    pub fn toggle(&mut self) {
        match self.active {
            None => {
                if self.sessions.is_empty() {
                    return;
                }
                self.active = Some(0);
                self.seconds_left = self.sessions[0].seconds();
                self.ending_soon_fired = false;
                self.run();
            }
            Some(_) if self.running => self.running = false,
            Some(_) => self.run(),
        }
    }

    fn run(&mut self) {
        self.running = true;
        self.last_tick = Some(Instant::now());
        self.ensure_ticker();
    }

    /// One thread for the process while a session runs: it credits the
    /// clock each second and sends the phase notifications, so the timer
    /// keeps time with the launcher hidden. Gone once the timer stops.
    fn ensure_ticker(&mut self) {
        if self.ticker {
            return;
        }
        self.ticker = true;
        std::thread::Builder::new()
            .name("look-pomo".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(TICK_SECS));
                    let mut pomo = lock();
                    if !pomo.running {
                        pomo.ticker = false;
                        return;
                    }
                    let events = pomo.tick();
                    drop(pomo);
                    for event in &events {
                        notify(event);
                    }
                }
            })
            .ok();
    }

    pub fn reset(&mut self) {
        self.running = false;
        self.active = None;
        self.seconds_left = 0;
        self.ending_soon_fired = false;
    }

    /// Skip: on to the next phase, or done, said like any other change.
    pub fn skip(&mut self) {
        if self.active.is_some() {
            notify(&self.advance());
        }
    }

    fn advance(&mut self) -> Event {
        let current = self.active.unwrap_or(0);
        let done = self.sessions[current].clone();
        let next = current + 1;
        if let Some(session) = self.sessions.get(next).cloned() {
            self.active = Some(next);
            self.seconds_left = session.seconds();
            self.ending_soon_fired = false;
            self.run();
            Event::PhaseDone {
                done,
                next: Some(session),
            }
        } else {
            self.reset();
            Event::PhaseDone { done, next: None }
        }
    }

    /// Credit the wall clock to the counter, whole seconds only so nothing
    /// is lost between ticks. A long gap runs through as many phases as it
    /// covers.
    pub fn tick(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        let (Some(_), true, Some(at)) = (self.active, self.running, self.last_tick) else {
            return events;
        };
        let mut elapsed = at.elapsed().as_secs();
        if elapsed == 0 {
            return events;
        }
        self.last_tick = Some(at + std::time::Duration::from_secs(elapsed));
        loop {
            if elapsed < self.seconds_left {
                self.seconds_left -= elapsed;
                break;
            }
            elapsed -= self.seconds_left;
            let event = self.advance();
            let done = !self.running;
            events.push(event);
            if done {
                return events;
            }
        }
        let kind = self.active_session().map(|s| s.kind);
        if !self.ending_soon_fired
            && self.seconds_left <= ENDING_SOON_SECS
            && let Some(kind) = kind
        {
            self.ending_soon_fired = true;
            events.push(Event::EndingSoon {
                kind,
                seconds_left: self.seconds_left,
            });
        }
        events
    }

    // --- Plan --------------------------------------------------------------------

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
        self.save();
    }

    /// A default session at the end; its index comes back for the panel to
    /// open.
    pub fn add_session(&mut self, kind: Kind) -> usize {
        let (minutes, name) = match kind {
            Kind::Focus => (DEFAULT_FOCUS_MINUTES, "Focus"),
            Kind::Break => (DEFAULT_BREAK_MINUTES, "Break"),
        };
        self.sessions.push(Session {
            kind,
            minutes,
            name: name.into(),
        });
        self.save();
        self.sessions.len() - 1
    }

    /// The last session stays: a plan with none has nothing to start.
    pub fn remove_session(&mut self, at: usize) {
        if self.sessions.len() <= 1 || at >= self.sessions.len() {
            return;
        }
        self.sessions.remove(at);
        if self.active.is_some_and(|i| i >= self.sessions.len()) {
            self.reset();
        }
        self.save();
    }

    pub fn flip_kind(&mut self, at: usize) {
        if let Some(session) = self.sessions.get_mut(at) {
            session.kind = session.kind.flipped();
            self.save();
        }
    }

    /// False when the name is blank, which the field flashes rather than keeps.
    pub fn rename(&mut self, at: usize, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        if let Some(session) = self.sessions.get_mut(at)
            && session.name != name
        {
            session.name = name.to_string();
            self.save();
        }
        true
    }

    /// False when the text is not a whole number of minutes in range.
    pub fn set_minutes(&mut self, at: usize, text: &str) -> bool {
        let Ok(minutes) = text.trim().parse::<u32>() else {
            return false;
        };
        if !(MINUTES_MIN..=MINUTES_MAX).contains(&minutes) {
            return false;
        }
        if let Some(session) = self.sessions.get_mut(at)
            && session.minutes != minutes
        {
            session.minutes = minutes;
            self.save();
        }
        true
    }

    // --- Music -------------------------------------------------------------------

    /// Read the folder's tracks once per process, shuffled. Blocking on the
    /// directory, so callers run it off the UI thread.
    pub fn restore_music(&mut self) {
        if self.tracks_scanned {
            return;
        }
        self.tracks_scanned = true;
        let Some(folder) = self.music_folder.clone() else {
            return;
        };
        let files = files::scan_music_folder(&folder);
        if files.is_empty() {
            self.music_folder = None;
            self.save();
            return;
        }
        self.tracks = shuffled(files);
        self.track = None;
    }

    /// A folder picked: its tracks replace the list, stopped.
    pub fn set_music_folder(&mut self, folder: String) {
        self.music_stop();
        self.tracks = shuffled(files::scan_music_folder(&folder));
        self.track = None;
        self.music_folder = Some(folder);
        self.tracks_scanned = true;
        self.save();
    }

    pub fn clear_music_folder(&mut self) {
        self.music_stop();
        self.tracks.clear();
        self.track = None;
        self.music_folder = None;
        self.tracks_scanned = true;
        self.save();
    }

    pub fn tracks_len(&self) -> usize {
        self.tracks.len()
    }

    pub fn music_playing(&self) -> bool {
        self.playing
    }

    /// The track's name for the panel and the tile, or none before play.
    pub fn track_name(&self) -> Option<String> {
        self.track
            .and_then(|i| self.tracks.get(i))
            .map(|path| short_track_name(path))
    }

    fn music_snapshot(&self) -> Option<MusicSnapshot> {
        Some(MusicSnapshot {
            playing: self.playing,
            track: self.track_name()?,
        })
    }

    /// Play or pause. The first press starts the list from the top.
    pub fn music_toggle(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        if self.playing {
            music::music_pause();
            self.playing = false;
        } else if self.track.is_none() {
            self.track = Some(0);
            self.play_current();
        } else {
            music::music_resume();
            self.playing = true;
            self.ensure_poller();
        }
    }

    fn play_current(&mut self) {
        let Some(path) = self.track.and_then(|i| self.tracks.get(i)) else {
            return;
        };
        if let Err(err) = music::music_play(path) {
            eprintln!("[music] play: {err}");
        }
        self.playing = true;
        self.ensure_poller();
    }

    fn music_stop(&mut self) {
        music::music_stop();
        self.playing = false;
        self.track = None;
    }

    pub fn music_next(&mut self) {
        self.music_step(1);
    }

    pub fn music_prev(&mut self) {
        self.music_step(-1);
    }

    fn music_step(&mut self, delta: isize) {
        let count = self.tracks.len() as isize;
        if count == 0 {
            return;
        }
        let at = self.track.map_or(0, |i| i as isize);
        self.track = Some((at + delta).rem_euclid(count) as usize);
        if self.playing {
            self.play_current();
        }
    }

    /// The home tile's transport, by the same words MPRIS takes.
    pub fn music_command(&mut self, command: &str) {
        match command {
            "playpause" => self.music_toggle(),
            "next" => self.music_next(),
            "previous" => self.music_prev(),
            _ => {}
        }
    }

    /// One thread for the process: when a track ends, the next plays, panel
    /// open or not.
    fn ensure_poller(&mut self) {
        if self.poller {
            return;
        }
        self.poller = true;
        std::thread::Builder::new()
            .name("look-music".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(END_POLL_SECS));
                    let mut pomo = lock();
                    if pomo.playing && music::music_is_finished() {
                        pomo.music_next();
                    }
                }
            })
            .ok();
    }
}

/// A desktop notification for a phase change, through the session's own
/// notifier.
fn notify(event: &Event) {
    let (title, body) = match event {
        Event::EndingSoon { kind, seconds_left } => (
            format!("{} ending soon", kind.label()),
            format!("{seconds_left}s remaining"),
        ),
        Event::PhaseDone { done, next } => (
            format!("{} done", done.kind.label()),
            match next {
                Some(next) => format!("Next: {} ({}m)", next.name, next.minutes),
                None => ALL_DONE.to_string(),
            },
        ),
    };
    #[cfg(target_os = "linux")]
    {
        let _ = linows_backend::platform::linux::host_command("notify-send")
            .args([&title, &body])
            .spawn();
    }
    #[cfg(not(target_os = "linux"))]
    eprintln!("[pomo] {title}: {body}");
}

/// `type:minutes:name` per session, names percent-encoded, joined by commas:
/// the macOS spelling.
fn encode_sessions(sessions: &[Session]) -> String {
    sessions
        .iter()
        .map(|s| format!("{}:{}:{}", s.kind.key(), s.minutes, percent_encode(&s.name)))
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_sessions(value: &str) -> Option<Vec<Session>> {
    let sessions: Vec<Session> = value
        .split(',')
        .filter_map(|token| {
            let mut parts = token.splitn(3, ':');
            let kind = Kind::parse(parts.next()?)?;
            let minutes: u32 = parts.next()?.parse().ok().filter(|m| *m > 0)?;
            let name = percent_decode(parts.next()?);
            Some(Session {
                kind,
                minutes,
                name,
            })
        })
        .collect();
    (!sessions.is_empty()).then_some(sessions)
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(value) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Fisher-Yates on a clock-seeded xorshift: a play order, not a lottery.
fn shuffled(mut list: Vec<String>) -> Vec<String> {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
        | 1;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for i in (1..list.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        list.swap(i, j);
    }
    list
}

/// The file's stem, with the middle elided past the cap.
fn short_track_name(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let base = match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    };
    let chars: Vec<char> = base.chars().collect();
    if chars.len() <= TRACK_NAME_MAX {
        return base.to_string();
    }
    let head = (TRACK_NAME_MAX - 1).div_ceil(2);
    let tail = (TRACK_NAME_MAX - 1) / 2;
    let mut out: String = chars[..head].iter().collect();
    out.push('\u{2026}');
    out.extend(&chars[chars.len() - tail..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_round_trip_in_the_macos_spelling() {
        let plan = vec![
            Session {
                kind: Kind::Focus,
                minutes: 25,
                name: "Deep, Work:1".into(),
            },
            Session {
                kind: Kind::Break,
                minutes: 5,
                name: "Nghỉ".into(),
            },
        ];
        let encoded = encode_sessions(&plan);
        assert!(encoded.starts_with("focus:25:Deep%2C%20Work%3A1,break:5:"));
        assert_eq!(decode_sessions(&encoded), Some(plan));
        assert_eq!(decode_sessions(""), None);
        assert_eq!(decode_sessions("nope:5:x"), None);
    }

    #[test]
    fn track_names_drop_the_extension_and_elide_the_middle() {
        assert_eq!(short_track_name("/m/Song Name.mp3"), "Song Name");
        let long = format!("/m/{}.flac", "a".repeat(60));
        let short = short_track_name(&long);
        assert_eq!(short.chars().count(), TRACK_NAME_MAX);
        assert!(short.contains('\u{2026}'));
    }

    #[test]
    fn a_shuffle_keeps_every_track() {
        let mut out = shuffled((0..20).map(|i| i.to_string()).collect());
        out.sort_by_key(|s| s.parse::<u32>().unwrap());
        assert_eq!(out, (0..20).map(|i| i.to_string()).collect::<Vec<_>>());
    }
}
