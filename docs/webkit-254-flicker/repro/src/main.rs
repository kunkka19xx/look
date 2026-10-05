use gtk::prelude::*;
use gtk::gdk::RGBA;
use webkit2gtk::{WebView, WebViewExt};

const WIN_W: i32 = 1008;
const WIN_H: i32 = 672;

const HTML_TEMPLATE: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
html, body { margin: 0; background: __PAGE_BG__; }
body {
    width: 100vw; height: 100vh;
    display: flex; align-items: center; justify-content: center;
    font-family: sans-serif; color: #dcd7ba;
}
.root {
    width: 960px; height: 600px;
    border-radius: 14px;
    background: rgba(31, 31, 40, 0.85);
    overflow: hidden;
    display: flex; flex-direction: column;
}
.bar {
    height: 48px; flex: none;
    background: #2a2a37;
    display: flex; align-items: center; padding: 0 16px;
    font-size: 15px;
}
.cards {
    display: flex; gap: 12px; padding: 16px;
}
.card {
    width: 180px; height: 120px;
    background: #2a2a37;
    border-radius: 10px;
    display: flex; align-items: center; justify-content: center;
    font-size: 40px; font-weight: 600;
}

.root.in { animation: arrive 340ms cubic-bezier(0.22, 1, 0.36, 1) backwards; }
.root.in .bar { animation: spawn 460ms cubic-bezier(0.34, 1.4, 0.64, 1) backwards; }
.root.in .card {
    animation: tile 460ms cubic-bezier(0.34, 1.4, 0.64, 1) backwards;
    animation-delay: calc(var(--i) * 35ms);
}
.root.hidden { visibility: hidden; }

@keyframes arrive {
    from { opacity: 0; transform: scale(0.965); }
    to { opacity: 1; transform: none; }
}
@keyframes spawn {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: none; }
}
@keyframes tile {
    from { opacity: 0; transform: translateY(10px) scale(0.985); }
    to { opacity: 1; transform: none; }
}
</style>
</head>
<body>
<div class="root" id="root">
    <div class="bar">Search placeholder</div>
    <div class="cards">
        <div class="card" style="--i:0">11:48</div>
        <div class="card" style="--i:1">11:48</div>
        <div class="card" style="--i:2">11:48</div>
        <div class="card" style="--i:3">11:48</div>
        <div class="card" style="--i:4">11:48</div>
    </div>
</div>
<script>
const root = document.getElementById('root');
const CYCLE_MS = 2200;
const HIDDEN_MS = 600;
const FIRST_SHOW_MS = 1500;

function show() {
    root.classList.remove('hidden');
    void root.offsetWidth;
    root.classList.add('in');
}

function hide() {
    root.classList.add('hidden');
    root.classList.remove('in');
}

function cycle() {
    hide();
    setTimeout(show, HIDDEN_MS);
}

root.classList.add('hidden');
setTimeout(() => {
    show();
    setInterval(cycle, CYCLE_MS);
}, FIRST_SHOW_MS);
</script>
</body>
</html>
"#;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let argb = match mode.as_str() {
        "argb" => true,
        "opaque" => false,
        _ => {
            eprintln!("usage: repro <opaque|argb>");
            std::process::exit(2);
        }
    };

    gtk::init().expect("gtk init");

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("repro");
    window.set_decorated(false);
    window.set_default_size(WIN_W, WIN_H);
    window.set_position(gtk::WindowPosition::Center);

    if argb {
        if let Some(visual) = WidgetExt::screen(&window).and_then(|s| s.rgba_visual()) {
            window.set_visual(Some(&visual));
        }
        window.set_app_paintable(true);
    }

    let webview = WebView::new();
    if argb {
        webview.set_background_color(&RGBA::new(0.0, 0.0, 0.0, 0.0));
    }

    let page_bg = if argb { "transparent" } else { "#1f1f28" };
    let html = HTML_TEMPLATE.replace("__PAGE_BG__", page_bg);
    webview.load_html(&html, None);

    window.add(&webview);
    window.connect_destroy(|_| gtk::main_quit());
    window.show_all();
    gtk::main();
}
