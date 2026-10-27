//! Leak probe: open and close a bare window N times, printing the glibc heap
//! and total PSS after each cycle. Isolates gpui itself from the launcher.
//! `cargo run --example window_cycle -p look-gpui -- 20`
//! `WC_HIDDEN=1` never maps the window; `WC_OPAQUE=1` drops transparency;
//! `WC_W`/`WC_H` set the size; `WC_WAIT_MS` the pause after open and close;
//! `WC_IDLE_S` idles that long after the cycles, sampling every 30 s, then trims.
//!
//! Linux only: it reads `/proc/self/smaps` and trims the glibc heap.

#[cfg(target_os = "linux")]
mod probe {
    use std::time::Duration;

    use gpui::{
        App, Bounds, Context, LinuxWindowBackground, Render, Window, WindowBounds, WindowKind,
        WindowOptions, div, prelude::*, px, rgb, size,
    };
    use gpui_platform::application;

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .bg(rgb(0x202020))
                .text_color(rgb(0xffffff))
                .child("window_cycle")
        }
    }

    fn mem() -> (f64, f64) {
        let mut heap = 0u64;
        let mut pss = 0u64;
        let mut in_heap = false;
        for line in std::fs::read_to_string("/proc/self/smaps").unwrap().lines() {
            if line
                .split_whitespace()
                .next()
                .is_some_and(|f| f.contains('-') && f.len() > 8)
            {
                in_heap = line.ends_with("[heap]");
            } else if let Some(kb) = line.strip_prefix("Pss:") {
                let kb: u64 = kb.trim().trim_end_matches(" kB").parse().unwrap();
                pss += kb;
                if in_heap {
                    heap += kb;
                }
            }
        }
        (heap as f64 / 1024.0, pss as f64 / 1024.0)
    }

    fn dim(var: &str, default: f32) -> f32 {
        std::env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    pub fn main() {
        let cycles: usize = std::env::args()
            .nth(1)
            .and_then(|a| a.parse().ok())
            .unwrap_or(20);
        application().run(move |cx: &mut App| {
            cx.set_quit_mode(gpui::QuitMode::Explicit);
            cx.spawn(async move |cx| {
                let (h, p) = mem();
                println!("cycle 0  heap {h:.1} MB  pss {p:.1} MB");
                for i in 1..=cycles {
                    let bounds = cx.update(|cx| {
                        Bounds::centered(
                            None,
                            size(px(dim("WC_W", 700.)), px(dim("WC_H", 500.))),
                            cx,
                        )
                    });
                    let handle = cx
                        .open_window(
                            WindowOptions {
                                titlebar: None,
                                window_bounds: Some(WindowBounds::Windowed(bounds)),
                                app_id: Some("window_cycle".into()),
                                linux_window_background: if std::env::var_os("WC_OPAQUE").is_some()
                                {
                                    LinuxWindowBackground::Opaque
                                } else {
                                    LinuxWindowBackground::Transparent
                                },
                                kind: WindowKind::Normal,
                                focus: true,
                                show: std::env::var_os("WC_HIDDEN").is_none(),
                                is_movable: false,
                                is_resizable: false,
                                ..Default::default()
                            },
                            |_, cx| cx.new(|_| Blank),
                        )
                        .expect("open window");
                    cx.background_executor()
                        .timer(Duration::from_millis(dim("WC_WAIT_MS", 1200.) as u64))
                        .await;
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    cx.background_executor()
                        .timer(Duration::from_millis(dim("WC_WAIT_MS", 1200.) as u64))
                        .await;
                    let (h, p) = mem();
                    println!("cycle {i}  heap {h:.1} MB  pss {p:.1} MB");
                }
                // Idle watch: does anything come back on its own, or with a trim?
                let idle = dim("WC_IDLE_S", 0.) as u64;
                for t in (30..=idle).step_by(30) {
                    cx.background_executor()
                        .timer(Duration::from_secs(30))
                        .await;
                    let (h, p) = mem();
                    println!("idle {t}s  heap {h:.1} MB  pss {p:.1} MB");
                }
                if idle > 0 {
                    unsafe extern "C" {
                        fn malloc_trim(pad: usize) -> std::ffi::c_int;
                    }
                    unsafe { malloc_trim(0) };
                    let (h, p) = mem();
                    println!("after malloc_trim  heap {h:.1} MB  pss {p:.1} MB");
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    }
}

#[cfg(target_os = "linux")]
fn main() {
    probe::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("window_cycle is a Linux-only probe");
}
