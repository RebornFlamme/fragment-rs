//! Scénario du spike S2 (pages HTML vivantes), partagé par `tests/spike_s2.rs` et
//! `examples/spike_s2.rs`. Windows seulement.
//!
//! Une VRAIE fenêtre GPUI (gpui-kit, un seul GPUI) porte un `gpui_webview::WebView` (WebView2 via
//! lb-wry) qui charge une page du coffre servie par un protocole personnalisé `fragment-local`
//! (racine = `tests/fixtures/vault`, ressources web seulement, pont injecté en tête de `<head>`).
//! Le scénario mesure : JS vivant, pont `postMessage`, thème avant la 1re image, sandbox, puis le
//! point dur : un calque dessiné PAR-DESSUS la page (GPUI sous le HWND enfant ; popup layered au-dessus).
//!
//! ★ `unsafe` toléré dans un spike : FFI Win32 minimale (user32/gdi32), signalée dans `win`.
#![allow(unsafe_code, clippy::expect_used, clippy::unwrap_used, dead_code)]

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fragment_text::html_bridge::{inject_html_bridge, page_servable};
use gpui_kit::{
    App, AppContext as _, AsyncApp, Bounds, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, div, point, px, rgb, size,
};
use gpui_webview::{WebView, WebViewHandle};
use serde_json::{Value, json};
use wry::WebViewBuilder;
use wry::http::{Request, Response};

/// Le script du pont de Fragment, recopié tel quel (`app/src/shared/htmlBridge.ts`, `HTML_BRIDGE_SCRIPT`).
pub const BRIDGE_VERBATIM: &str = include_str!("../fixtures/htmlBridge.js");

/// Le pont n'est écrit que pour une IFRAME (`window.parent.postMessage`). Dans une webview de premier
/// niveau (wry), `window.parent === window` : le script s'arrêterait dès sa 1re ligne. Adaptation
/// minimale, deux lignes : on retire la garde et on envoie vers l'hôte par `window.ipc.postMessage`.
pub fn adapt_bridge(script: &str) -> String {
    script
        .replace("if (window.parent === window) return;", "")
        .replace(
            "const envoyer = parent.postMessage.bind(parent);",
            "const envoyer = (m) => window.ipc.postMessage(JSON.stringify(m));",
        )
}

// ───────────────────────────── sandbox : la racine `page` ─────────────────────────────

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Le chemin d'une requête `fragment-local` → un fichier DANS le coffre, ou rien. Refuse `..`, les
/// lecteurs (`C:`), les antislashs, et tout ce qui, une fois résolu (liens symboliques compris), sort de `root`.
pub fn resolve_in_vault(root: &Path, uri_path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(uri_path.trim_start_matches('/'))?;
    if decoded.contains(['\\', ':', '\0']) {
        return None;
    }
    let mut out = root.to_path_buf();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => return None,
            s => out.push(s),
        }
    }
    let racine = root.canonicalize().ok()?;
    match out.canonicalize() {
        Ok(c) if c.starts_with(&racine) => Some(c),
        Ok(_) => None,
        Err(_) => Some(out), // n'existe pas : 404 plus loin
    }
}

type Reponse = Response<Cow<'static, [u8]>>;

fn reponse(status: u16, mime: &str, body: Vec<u8>) -> Reponse {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .body(Cow::Owned(body))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

fn servir(vault: &Path, bridge: &str, req: &Request<Vec<u8>>) -> Reponse {
    let chemin = req.uri().path().to_string();
    let decode = percent_decode(&chemin).unwrap_or_default();
    if !page_servable(&decode) {
        return reponse(403, "text/plain", b"refuse".to_vec());
    }
    let Some(fichier) = resolve_in_vault(vault, &chemin) else {
        return reponse(403, "text/plain", b"hors coffre".to_vec());
    };
    let Ok(octets) = std::fs::read(&fichier) else {
        return reponse(404, "text/plain", b"absent".to_vec());
    };
    let ext = fichier
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => {
            let html = inject_html_bridge(&String::from_utf8_lossy(&octets), bridge);
            reponse(200, "text/html; charset=utf-8", html.into_bytes())
        }
        "js" => reponse(200, "text/javascript", octets),
        "css" => reponse(200, "text/css", octets),
        "txt" => reponse(200, "text/plain; charset=utf-8", octets),
        _ => reponse(200, "application/octet-stream", octets),
    }
}

// ───────────────────────────── FFI Win32 minimale ─────────────────────────────

#[cfg(windows)]
pub mod win {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }
    #[repr(C)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }
    #[repr(C)]
    struct WndClassW {
        style: u32,
        wnd_proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu: *const u16,
        class_name: *const u16,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(c: *const WndClassW) -> u16;
        fn CreateWindowExW(
            ex: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *const c_void,
        ) -> isize;
        fn DefWindowProcW(h: isize, m: u32, w: usize, l: isize) -> isize;
        fn ShowWindow(h: isize, cmd: i32) -> i32;
        fn SetLayeredWindowAttributes(h: isize, key: u32, alpha: u8, flags: u32) -> i32;
        fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        fn ClientToScreen(h: isize, p: *mut Point) -> i32;
        fn GetDC(h: isize) -> isize;
        fn ReleaseDC(h: isize, dc: isize) -> i32;
        fn DestroyWindow(h: isize) -> i32;
        fn GetDpiForWindow(h: isize) -> u32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn GetPixel(dc: isize, x: i32, y: i32) -> u32;
        fn CreateSolidBrush(color: u32) -> isize;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Le HWND d'une fenêtre de premier niveau par son titre.
    pub fn find_window(title: &str) -> isize {
        let t = wide(title);
        unsafe { FindWindowW(std::ptr::null(), t.as_ptr()) }
    }

    /// Le coin haut-gauche de la zone cliente, en pixels ÉCRAN.
    pub fn client_origin(hwnd: isize) -> Option<(i32, i32)> {
        let mut p = Point { x: 0, y: 0 };
        (unsafe { ClientToScreen(hwnd, &mut p) } != 0).then_some((p.x, p.y))
    }

    pub fn dpi(hwnd: isize) -> u32 {
        unsafe { GetDpiForWindow(hwnd) }
    }

    /// Le pixel de l'ÉCRAN composé (bureau, DWM compris), `None` si le bureau n'est pas lisible.
    pub fn screen_pixel(x: i32, y: i32) -> Option<(u8, u8, u8)> {
        unsafe {
            let dc = GetDC(0);
            if dc == 0 {
                return None;
            }
            let c = GetPixel(dc, x, y);
            ReleaseDC(0, dc);
            (c != 0xFFFF_FFFF).then_some((
                (c & 0xFF) as u8,
                ((c >> 8) & 0xFF) as u8,
                ((c >> 16) & 0xFF) as u8,
            ))
        }
    }

    /// Une fenêtre POPUP rouge, semi-transparente (WS_EX_LAYERED), SANS activation, possédée par `owner`
    /// (donc toujours au-dessus de lui ET de ses enfants, WebView2 compris). Pixels écran.
    pub fn create_overlay(owner: isize, x: i32, y: i32, w: i32, h: i32, alpha: u8) -> isize {
        const WS_POPUP: u32 = 0x8000_0000;
        const WS_EX_TOPMOST: u32 = 0x8;
        const WS_EX_TOOLWINDOW: u32 = 0x80;
        const WS_EX_LAYERED: u32 = 0x8_0000;
        const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
        const LWA_ALPHA: u32 = 2;
        const SW_SHOWNOACTIVATE: i32 = 4;
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide("FragmentSpikeS2Overlay");
            let wc = WndClassW {
                style: 0,
                wnd_proc: DefWindowProcW,
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: CreateSolidBrush(0x0000_00FF), // COLORREF 0x00BBGGRR : rouge
                menu: std::ptr::null(),
                class_name: class.as_ptr(),
            };
            RegisterClassW(&wc); // déjà enregistrée : sans conséquence
            let title = wide("overlay");
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                x,
                y,
                w,
                h,
                owner,
                0,
                instance,
                std::ptr::null(),
            );
            if hwnd != 0 {
                SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            hwnd
        }
    }

    pub fn destroy(hwnd: isize) {
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}

// ───────────────────────────── le rapport ─────────────────────────────

/// Les mesures d'UNE fenêtre (variante) : pixels ÉCRAN lus sur le bureau composé.
#[derive(Default, Debug, Clone)]
pub struct Variant {
    pub name: String,
    /// Carré GPUI hors webview : doit être rouge (prouve que le bureau est lisible).
    pub px_control: Option<(u8, u8, u8)>,
    /// Point de la webview SANS rien de GPUI dessus : bleu de la page si elle est visible.
    pub px_web_only: Option<(u8, u8, u8)>,
    /// Point où un carré GPUI rouge chevauche la webview : rouge = GPUI dessus, bleu = webview dessus.
    pub px_gpui_over_webview: Option<(u8, u8, u8)>,
    /// Même point `web_only`, avec un popup layered rouge (alpha 160) par-dessus.
    pub px_popup_over_webview: Option<(u8, u8, u8)>,
    /// Taille de la fenêtre du document côté page (`innerWidth x innerHeight`) : la webview a-t-elle ses bornes ?
    pub inner: Option<String>,
    pub screenshots: Vec<PathBuf>,
}

#[derive(Default, Debug)]
pub struct Report {
    pub error: Option<String>,
    pub scale: f32,
    pub dpi: u32,
    pub height_ms: Option<u128>,
    pub height: Option<i64>,
    pub js_value: Option<String>,
    pub theme_first: Option<String>,
    pub sandbox: Option<Value>,
    pub link: Option<Value>,
    pub key: Option<Value>,
    pub message_types: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Default)]
struct Shared {
    messages: Vec<Value>,
}

// ───────────────────────────── la vue GPUI ─────────────────────────────

// Géométrie logique (px) de la scène : x, y, w, h.
const WEB: (f32, f32, f32, f32) = (40., 60., 500., 300.);
const CALQUE_GPUI: (f32, f32, f32, f32) = (420., 240., 100., 100.); // chevauche le bas-droite de la webview
const TEMOIN_GPUI: (f32, f32, f32, f32) = (600., 60., 80., 80.); // hors webview
const WEB_ONLY: (f32, f32, f32, f32) = (100., 100., 80., 80.); // dans la webview, rien de GPUI

struct Page {
    webview: Option<Entity<WebView>>,
    opaque: bool,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let carre = |(x, y, w, h): (f32, f32, f32, f32)| {
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .bg(rgb(0xff0000))
        };
        let mut root = div().relative().size_full();
        if self.opaque {
            root = root.bg(rgb(0x202020));
        }
        if let Some(wv) = &self.webview {
            root = root.child(
                div()
                    .absolute()
                    .left(px(WEB.0))
                    .top(px(WEB.1))
                    .w(px(WEB.2))
                    .h(px(WEB.3))
                    .child(wv.clone()),
            );
        }
        // Le calque GPUI est déclaré APRÈS la webview : en GPUI il serait dessus.
        root.child(carre(CALQUE_GPUI)).child(carre(TEMOIN_GPUI))
    }
}

// ───────────────────────────── pilotage asynchrone ─────────────────────────────

async fn wait_for(cx: &mut AsyncApp, ms: u64, f: impl Fn() -> bool) -> bool {
    let debut = Instant::now();
    while debut.elapsed() < Duration::from_millis(ms) {
        if f() {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(40))
            .await;
    }
    f()
}

fn find_msg(shared: &Arc<Mutex<Shared>>, kind: &str) -> Option<Value> {
    let g = shared.lock().ok()?;
    g.messages
        .iter()
        .find(|m| m["type"] == kind && m["fragmentHtml"] == 1)
        .cloned()
}

async fn eval(cx: &mut AsyncApp, h: &WebViewHandle, js: &str) -> Option<String> {
    let slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let s = slot.clone();
    h.raw()
        .evaluate_script_with_callback(js, move |r| {
            if let Ok(mut g) = s.lock() {
                *g = Some(r);
            }
        })
        .ok()?;
    let slot2 = slot.clone();
    wait_for(cx, 8000, move || {
        slot2.lock().map(|g| g.is_some()).unwrap_or(false)
    })
    .await;
    slot.lock().ok().and_then(|g| g.clone())
}

/// Capture RÉELLE du bureau (CopyFromScreen) dans un PNG, hors du fil UI.
async fn screenshot(cx: &mut AsyncApp, out: &Path, name: &str) -> Option<PathBuf> {
    let _ = std::fs::create_dir_all(out);
    let fichier = out.join(format!("{name}.png"));
    let cmd = format!(
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
         $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds; \
         $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height; \
         $g=[System.Drawing.Graphics]::FromImage($bmp); \
         $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size); \
         $bmp.Save('{}')",
        fichier.display()
    );
    let ok = cx
        .background_executor()
        .spawn(async move {
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
        .await;
    (ok && fichier.exists()).then_some(fichier)
}

/// Une fenêtre GPUI + sa webview ; `Err` = message d'erreur du scénario.
struct Scene {
    handle: WebViewHandle,
    scale: f32,
}

fn open_scene(
    cx: &mut App,
    title: &str,
    transparent: bool,
    vault: &Path,
    bridge: &str,
    shared: &Arc<Mutex<Shared>>,
) -> Result<Scene, String> {
    let sh = shared.clone();
    let url = format!(
        "http://fragment-local.localhost/page.html?fragment-theme={}",
        percent_encode(&theme_json())
    );
    let mut handle = None;
    let mut build_error = None;
    let mut scale = 1.0_f32;
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(50.), px(50.)),
            size(px(900.), px(600.)),
        ))),
        window_background: if transparent {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        },
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        scale = window.scale_factor();
        let builder = WebViewBuilder::new()
            .with_custom_protocol("fragment-local".into(), {
                let vault = vault.to_path_buf();
                let bridge = bridge.to_string();
                move |_id, req| servir(&vault, &bridge, &req)
            })
            .with_ipc_handler(move |req: Request<String>| {
                if let Ok(v) = serde_json::from_str::<Value>(req.body()) {
                    if let Ok(mut g) = sh.lock() {
                        g.messages.push(v);
                    }
                }
            })
            .with_url(url);
        let webview = match WebView::build(builder, window, cx) {
            Ok(w) => {
                handle = Some(w.handle());
                Some(cx.new(|_| w))
            }
            Err(e) => {
                build_error = Some(format!("WebView::build : {e}"));
                None
            }
        };
        cx.new(|_| Page {
            webview,
            opaque: !transparent,
        })
    });
    if let Err(e) = opened {
        return Err(format!("open_window : {e:#}"));
    }
    match handle {
        Some(handle) => Ok(Scene { handle, scale }),
        None => Err(build_error.unwrap_or_else(|| "pas de webview".into())),
    }
}

/// Le point dur, pour la fenêtre `title` : lit les pixels ÉCRAN avec et sans popup layered.
async fn measure_overlay(
    cx: &mut AsyncApp,
    title: &str,
    scene: &Scene,
    out: &Path,
    dpi: &mut u32,
) -> Variant {
    let mut v = Variant {
        name: title.to_string(),
        ..Variant::default()
    };
    v.inner = eval(
        cx,
        &scene.handle,
        "innerWidth + 'x' + innerHeight + ' dpr=' + devicePixelRatio",
    )
    .await;
    cx.background_executor()
        .timer(Duration::from_millis(2000))
        .await;
    let hwnd = win::find_window(title);
    if hwnd == 0 {
        return v;
    }
    *dpi = win::dpi(hwnd);
    let Some((ox, oy)) = win::client_origin(hwnd) else {
        return v;
    };
    let s = scene.scale;
    let ecran = |lx: f32, ly: f32| (ox + (lx * s).round() as i32, oy + (ly * s).round() as i32);
    let centre = |r: (f32, f32, f32, f32)| ecran(r.0 + r.2 / 2., r.1 + r.3 / 2.);
    let (cx0, cy0) = centre(TEMOIN_GPUI);
    let (wx, wy) = centre(WEB_ONLY);
    let (ux, uy) = centre(CALQUE_GPUI);
    v.px_control = win::screen_pixel(cx0, cy0);
    v.px_web_only = win::screen_pixel(wx, wy);
    v.px_gpui_over_webview = win::screen_pixel(ux, uy);
    if let Some(p) = screenshot(cx, out, &format!("{title}-1-sans-popup")).await {
        v.screenshots.push(p);
    }

    // (a) popup layered possédé, au-dessus de la webview, sur le rectangle `WEB_ONLY`.
    let (x0, y0) = ecran(WEB_ONLY.0, WEB_ONLY.1);
    let (w, h) = (
        (WEB_ONLY.2 * s).round() as i32,
        (WEB_ONLY.3 * s).round() as i32,
    );
    let popup = win::create_overlay(hwnd, x0, y0, w, h, 160);
    cx.background_executor()
        .timer(Duration::from_millis(800))
        .await;
    v.px_popup_over_webview = win::screen_pixel(wx, wy);
    if let Some(p) = screenshot(cx, out, &format!("{title}-2-popup-layered")).await {
        v.screenshots.push(p);
    }
    if popup != 0 {
        win::destroy(popup);
    }
    v
}

struct Ctx {
    scene: Scene,
    shared: Arc<Mutex<Shared>>,
    report: Arc<Mutex<Report>>,
    out: PathBuf,
    vault: PathBuf,
    bridge: String,
}

async fn drive(cx: &mut AsyncApp, c: Ctx) {
    let t0 = Instant::now();
    let h = c.scene.handle.clone();
    let mut r = Report {
        scale: c.scene.scale,
        ..Report::default()
    };

    // 1. La page charge, le pont parle (hauteur) : mesure du temps jusqu'au 1er message utile.
    let shared = c.shared.clone();
    if wait_for(cx, 40_000, move || find_msg(&shared, "height").is_some()).await {
        r.height_ms = Some(t0.elapsed().as_millis());
        r.height = find_msg(&c.shared, "height").and_then(|m| m["height"].as_i64());
    } else {
        r.error = Some("aucun message `height` du pont en 40 s".into());
    }

    // 2. JS vivant (valeur calculée PAR la page, relue par l'hôte) et thème présent au 1er script.
    r.js_value = eval(cx, &h, "document.getElementById('out').textContent").await;
    r.theme_first = eval(cx, &h, "window.__first").await;

    // 3. Sandbox : sondes lancées par la page elle-même.
    for _ in 0..50 {
        if let Some(s) = eval(
            cx,
            &h,
            "window.__sandbox ? JSON.stringify(window.__sandbox) : ''",
        )
        .await
        {
            // WebView2 renvoie le JSON d'une chaîne : "\"{...}\"" ou "\"\"".
            if let Ok(inner) = serde_json::from_str::<String>(&s) {
                if !inner.is_empty() {
                    r.sandbox = serde_json::from_str(&inner).ok();
                    break;
                }
            }
        }
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;
    }

    // 4. Pont : lien et touche, déclenchés dans la page.
    let _ = eval(cx, &h, "document.getElementById('lien').click(); 1").await;
    let shared = c.shared.clone();
    wait_for(cx, 5000, move || find_msg(&shared, "link").is_some()).await;
    r.link = find_msg(&c.shared, "link");
    let _ = eval(
        cx,
        &h,
        "window.dispatchEvent(new KeyboardEvent('keydown',{key:'k',ctrlKey:true,bubbles:true})); 1",
    )
    .await;
    let shared = c.shared.clone();
    wait_for(cx, 5000, move || find_msg(&shared, "key").is_some()).await;
    r.key = find_msg(&c.shared, "key");
    if let Ok(g) = c.shared.lock() {
        for m in &g.messages {
            if let Some(t) = m["type"].as_str() {
                if !r.message_types.iter().any(|x| x == t) {
                    r.message_types.push(t.to_string());
                }
            }
        }
    }

    // 5. LE POINT DUR, variante A : fenêtre OPAQUE (la configuration naturelle de gpui-kit).
    let mut dpi = 0;
    let va = measure_overlay(cx, "spike-s2", &c.scene, &c.out, &mut dpi).await;
    r.variants.push(va);
    r.dpi = dpi;

    // Variante B : fenêtre TRANSPARENTE (fond GPUI absent) — la webview est-elle visible dessous,
    // et le calque GPUI passe-t-il alors AU-DESSUS d'elle ?
    let slot: Arc<Mutex<Option<Result<Scene, String>>>> = Arc::new(Mutex::new(None));
    let (s2, vault, bridge, shared) = (
        slot.clone(),
        c.vault.clone(),
        c.bridge.clone(),
        c.shared.clone(),
    );
    let _ = cx.update(move |cx| {
        let res = open_scene(cx, "spike-s2-transparent", true, &vault, &bridge, &shared);
        if let Ok(mut g) = s2.lock() {
            *g = Some(res);
        }
    });
    let scene_b = slot.lock().ok().and_then(|mut g| g.take());
    match scene_b {
        Some(Ok(sb)) => {
            // Laisser la 2e page charger.
            cx.background_executor()
                .timer(Duration::from_millis(3000))
                .await;
            let vb = measure_overlay(cx, "spike-s2-transparent", &sb, &c.out, &mut dpi).await;
            r.variants.push(vb);
        }
        Some(Err(e)) => {
            r.error
                .get_or_insert(format!("variante transparente : {e}"));
        }
        None => {}
    }

    if let Ok(mut g) = c.report.lock() {
        *g = r;
    }
}

// ───────────────────────────── point d'entrée ─────────────────────────────

fn theme_json() -> String {
    json!({
        "vars": {
            "--background-primary": "#1e90ff",
            "--text-normal": "#ffffff",
            "--font-text": "Segoe UI, sans-serif",
            "--font-text-size": "16px"
        },
        "scheme": "dark"
    })
    .to_string()
}

/// Lance l'application GPUI, déroule le scénario, rend le rapport. Bloquant (fil dédié, grosse pile).
pub fn run(out_dir: PathBuf) -> Report {
    let report = Arc::new(Mutex::new(Report::default()));
    let report_out = report.clone();
    let vault = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault");
    let bridge = adapt_bridge(BRIDGE_VERBATIM);

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let shared = Arc::new(Mutex::new(Shared::default()));
        let scene = match open_scene(cx, "spike-s2", false, &vault, &bridge, &shared) {
            Ok(s) => s,
            Err(e) => {
                if let Ok(mut g) = report.lock() {
                    g.error = Some(e);
                }
                cx.quit();
                return;
            }
        };
        let ctx = Ctx {
            scene,
            shared,
            report: report.clone(),
            out: out_dir,
            vault,
            bridge,
        };
        cx.spawn(async move |cx| {
            drive(cx, ctx).await;
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });

    report_out
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}
