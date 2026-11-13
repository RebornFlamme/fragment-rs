//! Scénario du spike S10 (hôte webview de T9), partagé par `tests/spike_s10.rs` et `examples/spike_s10.rs`.
//! Windows seulement. Voir `docs/spikes/S10-hote-webview.md`.
//!
//! Une fenêtre GPUI (jamais activée : `focus: false`) porte :
//! - une webview ENFANT (`gpui_webview::WebView`, HWND enfant de la fenêtre GPUI) ;
//! - une webview dans un POPUP POSSÉDÉ (l'architecture de repli de S2), placé à chaque image par un `canvas` ;
//! - un témoin GPUI vert, au même `top` que les deux pages, et une barre GPUI rouge en haut.
//!
//! Le scénario mesure sur le bureau composé (BitBlt) : visibilité, suivi au défilement (retard en images),
//! rognage (`SetWindowRgn`), menu GPUI au-dessus, déplacement / minimisation, DPI, capture
//! (`CapturePreview` contre `PrintWindow`), focus et saisie par messages Win32, isolement (3 variantes),
//! mémoire et temps d'ouverture de 1, 5, 10 webviews. Tout l'`unsafe` est dans `win.rs`.
#![allow(clippy::expect_used, clippy::unwrap_used, dead_code)]

#[path = "win.rs"]
pub mod win;

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fragment_text::html_bridge::{inject_html_bridge, page_servable};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Context, Entity, IntoElement,
    ParentElement as _, Render, Styled as _, TitlebarOptions, Window, WindowBounds, WindowKind,
    WindowOptions, canvas, div, point, px, rgb, size,
};
use gpui_webview::WebView;
use serde_json::Value;
use wry::http::{Request, Response};
use wry::{WebContext, WebViewBuilder};

pub const PONT_S10: &str = include_str!("pont-s10.js");
pub const TITRE: &str = "spike-s10";
const DONNEES: &str = "fragment-spike-s10-donnees";

// ───────────────────────────── géométrie logique (px) ─────────────────────────────

const LARGEUR: f32 = 900.;
const HAUTEUR: f32 = 640.;
const BARRE_H: f32 = 60.;
const PAGE_TOP: f32 = 300.;
const PAGE_H: f32 = 300.;
const ENFANT_X: f32 = 40.;
const ENFANT_W: f32 = 500.;
const TEMOIN_X: f32 = 560.;
const TEMOIN_W: f32 = 20.;
const POPUP_X: f32 = 600.;
const POPUP_W: f32 = 280.;
/// Vitesse du défilement simulé, en px logiques par image ; amplitude d'un aller.
const VITESSE: u32 = 4;
const AMPLITUDE: u32 = 200;
const CYCLES: u32 = 4;

// ───────────────────────────── protocole fragment-local ─────────────────────────────

/// CSP de base : rien ne sort de `fragment-local` (le réseau sortant est coupé par la politique).
pub const CSP_BASE: &str = "default-src 'none'; \
     script-src 'unsafe-inline' http://fragment-local.localhost; \
     style-src 'unsafe-inline' http://fragment-local.localhost; \
     img-src http://fragment-local.localhost data:; \
     font-src http://fragment-local.localhost data:; \
     connect-src http://fragment-local.localhost";

/// La page hôte de la variante 3 : une iframe `sandbox="allow-scripts"` et un relais vers `window.ipc`
/// qui n'accepte que les messages `fragmentHtml` venus de CETTE iframe.
pub fn page_hote(src: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;height:100%}}\
         iframe{{border:0;width:100%;height:100%;display:block}}</style></head><body>\
         <iframe id=\"f\" sandbox=\"allow-scripts\" src=\"{src}\"></iframe><script>\
         var f=document.getElementById('f');\
         addEventListener('message',function(e){{if(e.source!==f.contentWindow)return;var m=e.data;\
         if(!m||m.fragmentHtml!==1)return;window.ipc.postMessage(JSON.stringify(m));}});\
         </script></body></html>"
    )
}

/// La CSP d'une réponse selon la variante d'isolement (`v` de l'URL) : 1 = aucune, 2 = page de premier
/// niveau avec `sandbox allow-scripts` (origine opaque), 3 = page dans l'iframe de l'hôte.
pub fn csp_pour(variante: &str, hote: bool) -> Option<String> {
    if hote {
        return Some(
            "default-src 'none'; frame-src http://fragment-local.localhost; \
             script-src 'unsafe-inline'; style-src 'unsafe-inline'"
                .into(),
        );
    }
    match variante {
        "2" => Some(format!("sandbox allow-scripts; {CSP_BASE}")),
        "3" => Some(CSP_BASE.into()),
        _ => None,
    }
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            out.push(u8::from_str_radix(s.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Un chemin de requête → un fichier DANS le coffre, ou rien (même règle que S2).
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
        Err(_) => Some(out),
    }
}

type Reponse = Response<Cow<'static, [u8]>>;

fn reponse(status: u16, mime: &str, csp: Option<String>, body: Vec<u8>) -> Reponse {
    let mut b = Response::builder()
        .status(status)
        .header("Content-Type", mime);
    if let Some(c) = csp {
        b = b.header("Content-Security-Policy", c);
    }
    b.body(Cow::Owned(body))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

fn servir(vault: &Path, req: &Request<Vec<u8>>) -> Reponse {
    let chemin = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("");
    let variante = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("v="))
        .unwrap_or("1")
        .to_string();
    if chemin == "/__hote.html" {
        let src = query
            .split('&')
            .find_map(|kv| kv.strip_prefix("p="))
            .unwrap_or("isolement.html");
        let html = page_hote(&format!("/{src}?v=3"));
        return reponse(
            200,
            "text/html; charset=utf-8",
            csp_pour(&variante, true),
            html.into_bytes(),
        );
    }
    let decode = percent_decode(&chemin).unwrap_or_default();
    if !page_servable(&decode) {
        return reponse(403, "text/plain", None, b"refuse".to_vec());
    }
    let Some(fichier) = resolve_in_vault(vault, &chemin) else {
        return reponse(403, "text/plain", None, b"hors coffre".to_vec());
    };
    let Ok(octets) = std::fs::read(&fichier) else {
        return reponse(404, "text/plain", None, b"absent".to_vec());
    };
    let ext = fichier
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => {
            let html = inject_html_bridge(&String::from_utf8_lossy(&octets), PONT_S10);
            reponse(
                200,
                "text/html; charset=utf-8",
                csp_pour(&variante, false),
                html.into_bytes(),
            )
        }
        _ => reponse(200, "application/octet-stream", None, octets),
    }
}

fn url(page: &str) -> String {
    format!("http://fragment-local.localhost/{page}")
}

// ───────────────────────────── messages ─────────────────────────────

#[derive(Default)]
pub struct Shared {
    /// (source, message) dans l'ordre d'arrivée.
    pub messages: Vec<(String, Value, Instant)>,
}

type Partage = Arc<Mutex<Shared>>;

fn compter(sh: &Partage, source: &str, kind: &str) -> usize {
    sh.lock()
        .map(|g| {
            g.messages
                .iter()
                .filter(|(s, m, _)| s == source && m["type"] == kind && m["fragmentHtml"] == 1)
                .count()
        })
        .unwrap_or(0)
}

fn dernier(sh: &Partage, source: &str, kind: &str) -> Option<(Value, Instant)> {
    let g = sh.lock().ok()?;
    g.messages
        .iter()
        .rev()
        .find(|(s, m, _)| s == source && m["type"] == kind && m["fragmentHtml"] == 1)
        .map(|(_, m, t)| (m.clone(), *t))
}

fn constructeur<'a>(
    ctx: &'a mut WebContext,
    vault: &Path,
    source: &str,
    sh: &Partage,
    page: String,
) -> WebViewBuilder<'a> {
    let (vault, source, sh) = (vault.to_path_buf(), source.to_string(), sh.clone());
    WebViewBuilder::new_with_web_context(ctx)
        .with_custom_protocol("fragment-local".into(), move |_id, req| {
            servir(&vault, &req)
        })
        .with_ipc_handler(move |req: Request<String>| {
            if let Ok(v) = serde_json::from_str::<Value>(req.body())
                && let Ok(mut g) = sh.lock()
            {
                g.messages.push((source.clone(), v, Instant::now()));
            }
        })
        .with_focused(false)
        .with_url(url(&page))
}

/// Un HWND nu présenté à wry comme parent.
struct Hwnd(isize);

impl raw_window_handle::HasWindowHandle for Hwnd {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let nz = std::num::NonZeroIsize::new(self.0)
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        let raw = raw_window_handle::RawWindowHandle::Win32(
            raw_window_handle::Win32WindowHandle::new(nz),
        );
        // SÛRETÉ : le HWND vit tant que le popup n'est pas détruit (fin du scénario).
        #[allow(unsafe_code)]
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

/// Une webview wry dans un popup possédé par `owner`, au rectangle écran `r` (pixels physiques).
fn ouvrir_popup(
    ctx: &mut WebContext,
    owner: isize,
    r: (i32, i32, i32, i32),
    vault: &Path,
    source: &str,
    sh: &Partage,
    page: String,
) -> Result<(isize, wry::WebView), String> {
    let hwnd = win::create_popup_host(owner, r);
    if hwnd == 0 {
        return Err("create_popup_host a échoué".into());
    }
    let wv = constructeur(ctx, vault, source, sh, page)
        .with_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(0, 0).into(),
            size: wry::dpi::PhysicalSize::new(r.2 as u32, r.3 as u32).into(),
        })
        .build_as_child(&Hwnd(hwnd))
        .map_err(|e| format!("build_as_child : {e}"))?;
    Ok((hwnd, wv))
}

// ───────────────────────────── la vue GPUI ─────────────────────────────

pub struct Scene {
    enfant: Option<Entity<WebView>>,
    offset: f32,
    frame: Option<u32>,
    /// Amplitude d'un aller de l'animation (px logiques).
    ampl: u32,
    pub images: u32,
    /// (popup, fenêtre GPUI) : le popup est replacé à chaque image par un `canvas`.
    popup: Option<(isize, isize)>,
    /// Rogner les deux pages sous la barre (`SetWindowRgn`).
    rogner: bool,
    /// Le conteneur `WRY_WEBVIEW` de la webview enfant.
    conteneur: isize,
    scale: f32,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.images += 1;
        if let Some(f) = self.frame.as_mut() {
            *f += 1;
            let periode = 2 * self.ampl / VITESSE;
            let ph = *f % periode;
            let pas = if ph < periode / 2 { ph } else { periode - ph };
            self.offset = (pas * VITESSE) as f32;
            if *f >= CYCLES * periode {
                self.frame = None;
                self.offset = 0.;
            }
            window.request_animation_frame();
        }
        let top = PAGE_TOP - self.offset;
        let s = self.scale;
        let (rogner, conteneur, popup) = (self.rogner, self.conteneur, self.popup);
        // Région d'une page dont le haut est à `top` : la partie sous la barre, en pixels FENÊTRE.
        let region = move |w: f32| {
            (rogner && top < BARRE_H).then(|| win::Rect {
                left: 0,
                top: ((BARRE_H - top) * s).round() as i32,
                right: (w * s).round() as i32,
                bottom: (PAGE_H * s).round() as i32,
            })
        };
        let place = |x: f32, w: f32| {
            div()
                .absolute()
                .left(px(x))
                .top(px(top))
                .w(px(w))
                .h(px(PAGE_H))
        };
        let mut root = div().relative().size_full().bg(rgb(0x202020));
        if let Some(wv) = &self.enfant {
            root = root.child(place(ENFANT_X, ENFANT_W).child(wv.clone()));
        }
        root.child(place(TEMOIN_X, TEMOIN_W).bg(rgb(0x00ff00)))
            .child(
                place(POPUP_X, POPUP_W).child(
                    canvas(
                        move |b, _, _| {
                            if conteneur != 0 {
                                win::set_region(conteneur, region(ENFANT_W));
                            }
                            let Some((p, g)) = popup else { return };
                            let Some((ox, oy)) = win::client_origin(g) else {
                                return;
                            };
                            win::set_pos(
                                p,
                                ox + (f32::from(b.origin.x) * s).round() as i32,
                                oy + (f32::from(b.origin.y) * s).round() as i32,
                                (f32::from(b.size.width) * s).round() as i32,
                                (f32::from(b.size.height) * s).round() as i32,
                                win::SWP_NOZORDER,
                            );
                            win::set_region(p, region(POPUP_W));
                        },
                        |_, _, _, _| {},
                    )
                    .size_full(),
                ),
            )
            // La barre est déclarée en DERNIER : en GPUI elle est au-dessus de tout.
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .w(px(LARGEUR))
                    .h(px(BARRE_H))
                    .bg(rgb(0xff0000)),
            )
    }
}

struct Menu;

impl Render for Menu {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0xffff00))
    }
}

// ───────────────────────────── rapport ─────────────────────────────

#[derive(Default, Debug)]
pub struct Report {
    pub error: Option<String>,
    pub mesures: Vec<(String, String)>,
}

impl Report {
    fn note(&mut self, k: &str, v: impl std::fmt::Debug) {
        let s = format!("{v:?}");
        println!("[S10] {k} = {s}");
        self.mesures.push((k.to_string(), s));
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.mesures
            .iter()
            .find(|(c, _)| c == k)
            .map(|(_, v)| v.as_str())
    }
}

// ───────────────────────────── pilotage ─────────────────────────────

async fn pause(cx: &mut AsyncApp, ms: u64) {
    cx.background_executor()
        .timer(Duration::from_millis(ms))
        .await;
}

async fn wait_for(cx: &mut AsyncApp, ms: u64, f: impl Fn() -> bool) -> bool {
    let debut = Instant::now();
    while debut.elapsed() < Duration::from_millis(ms) {
        if f() {
            return true;
        }
        pause(cx, 10).await;
    }
    f()
}

async fn eval(cx: &mut AsyncApp, wv: &wry::WebView, js: &str) -> Option<String> {
    let slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let s = slot.clone();
    wv.evaluate_script_with_callback(js, move |r| {
        if let Ok(mut g) = s.lock() {
            *g = Some(r);
        }
    })
    .ok()?;
    let s2 = slot.clone();
    wait_for(cx, 5000, move || {
        s2.lock().map(|g| g.is_some()).unwrap_or(false)
    })
    .await;
    slot.lock().ok().and_then(|g| g.clone())
}

/// Capture `CapturePreview` : (ms, octets, pixel au centre) ou erreur.
async fn apercu(
    cx: &mut AsyncApp,
    wv: &wry::WebView,
) -> Result<(u128, usize, (u8, u8, u8)), String> {
    let slot: win::Capture = Arc::new(Mutex::new(None));
    let t0 = Instant::now();
    win::capture_preview(wv, slot.clone())?;
    let s2 = slot.clone();
    wait_for(cx, 5000, move || {
        s2.lock().map(|g| g.is_some()).unwrap_or(false)
    })
    .await;
    let ms = t0.elapsed().as_millis();
    let png = slot
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .unwrap_or(Err("délai".into()))?;
    let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let p = img.get_pixel(img.width() / 2, img.height() / 2);
    Ok((ms, png.len(), (p[0], p[1], p[2])))
}

fn proche(p: Option<(u8, u8, u8)>, c: (u8, u8, u8)) -> bool {
    p.is_some_and(|(r, g, b)| {
        (r as i32 - c.0 as i32).abs() < 24
            && (g as i32 - c.1 as i32).abs() < 24
            && (b as i32 - c.2 as i32).abs() < 24
    })
}

const BLEU: (u8, u8, u8) = (30, 144, 255);
const VERT: (u8, u8, u8) = (0, 255, 0);

/// Mémoire des processus WebView2 de CE scénario (dossier de données propre) : (Mo de working set, Mo privés, n).
fn memoire_webview2() -> Option<(f64, f64, usize)> {
    let cmd = format!(
        "$p = Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | \
         Where-Object {{ $_.CommandLine -like '*{DONNEES}*' }}; \
         $ws = ($p | Measure-Object WorkingSetSize -Sum).Sum; \
         $pv = ($p | Measure-Object PrivatePageCount -Sum).Sum; \
         Write-Output \"$ws;$pv;$(@($p).Count)\""
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut it = s.trim().split(';');
    let ws: f64 = it.next()?.parse().ok()?;
    let pv: f64 = it.next()?.parse().ok()?;
    let n: usize = it.next()?.parse().ok()?;
    Some((ws / 1_048_576., pv / 1_048_576., n))
}

/// Premier rang (depuis `y0`) de la colonne `x` de l'image qui a la couleur `c`.
fn bord(img: &win::Image, x: i32, y0: i32, c: (u8, u8, u8)) -> Option<i32> {
    (y0..img.h).find(|&y| proche(img.rgb(x, y), c))
}

pub struct Ctx {
    pub scene: Entity<Scene>,
    pub enfant: Entity<WebView>,
    pub shared: Partage,
    pub vault: PathBuf,
    pub web: Rc<RefCell<WebContext>>,
    pub report: Arc<Mutex<Report>>,
    pub interactif: bool,
}

async fn drive(cx: &mut AsyncApp, c: Ctx) -> Report {
    let mut r = Report::default();
    let fg0 = win::foreground();
    let t0 = Instant::now();
    let sh = c.shared.clone();
    if !wait_for(cx, 30_000, move || compter(&sh, "enfant", "height") > 0).await {
        r.error = Some("aucun `height` de la webview enfant en 30 s".into());
        return r;
    }
    r.note(
        "ouverture_enfant_ms (fenêtre → 1er height)",
        t0.elapsed().as_millis(),
    );
    let g = win::find_window(TITRE);
    let conteneur = win::find_child(g, "WRY_WEBVIEW");
    let scale = cx.update(|cx| c.scene.read(cx).scale);
    let s = scale;
    r.note("dpi", win::dpi(g));
    r.note("echelle", scale);
    r.note("conteneur_wry_trouve", conteneur != 0);
    let Some((ox, oy)) = win::client_origin(g) else {
        r.error = Some("fenêtre GPUI introuvable".into());
        return r;
    };
    let ecran = |lx: f32, ly: f32| (ox + (lx * s).round() as i32, oy + (ly * s).round() as i32);
    let pixel = |lx: f32, ly: f32| {
        let (x, y) = ecran(lx, ly);
        win::capture_screen(x, y, 1, 1).and_then(|i| i.rgb(0, 0))
    };

    // Popup possédé (repli de S2) : créé à sa place, puis replacé à chaque image par le canvas.
    let (px0, py0) = ecran(POPUP_X, PAGE_TOP);
    let popup = ouvrir_popup(
        &mut c.web.borrow_mut(),
        g,
        (
            px0,
            py0,
            (POPUP_W * s).round() as i32,
            (PAGE_H * s).round() as i32,
        ),
        &c.vault,
        "popup",
        &c.shared,
        "defile.html".into(),
    );
    let (popup_hwnd, popup_wv) = match popup {
        Ok(p) => p,
        Err(e) => {
            r.error = Some(e);
            return r;
        }
    };
    cx.update(|cx| {
        c.scene.update(cx, |sc, cx| {
            sc.popup = Some((popup_hwnd, g));
            sc.conteneur = conteneur;
            cx.notify();
        })
    });
    let sh = c.shared.clone();
    wait_for(cx, 20_000, move || compter(&sh, "popup", "height") > 0).await;
    pause(cx, 1200).await;

    if c.interactif {
        interactif(cx, &c, &mut r).await;
        return r;
    }

    // ★ Les fenêtres de l'utilisateur recouvrent la nôtre (jamais activée) : au-dessus de tout, SANS
    // activation, le temps des mesures sur le bureau composé (phases 1 à 5), puis retour à la normale.
    r.note("0_popup_visible_avant_topmost", win::is_visible(popup_hwnd));
    r.note("0_topmost_sans_activation", win::set_topmost(g, true));
    // ★ Un popup possédé NON topmost passe SOUS son propriétaire devenu topmost (mesuré) : on le suit.
    win::set_topmost(popup_hwnd, true);
    pause(cx, 500).await;

    // ── 1. Visibilité (gpui-webview 0.9.1 / gpui-pre 0.3.12) ──
    r.note(
        "1_enfant_centre (bleu = page visible)",
        pixel(ENFANT_X + 250., PAGE_TOP + 150.),
    );
    r.note(
        "1_temoin_gpui (vert)",
        pixel(TEMOIN_X + 10., PAGE_TOP + 150.),
    );
    r.note("1_barre_gpui (rouge)", pixel(450., 30.));
    r.note(
        "1_popup_centre (bleu)",
        pixel(POPUP_X + 140., PAGE_TOP + 150.),
    );

    // ── 2. Suivi au défilement : retard en images, enfant et popup contre le témoin GPUI ──
    let (zx, zy) = ecran(ENFANT_X + 250., BARRE_H);
    let (zx2, _) = ecran(POPUP_X + 140., BARRE_H);
    let zw = zx2 - zx + 1;
    let zh = ((HAUTEUR - BARRE_H) * s).round() as i32;
    let col_enfant = 0;
    let col_temoin = ((TEMOIN_X + 10. - (ENFANT_X + 250.)) * s).round() as i32;
    let col_popup = zw - 1;
    let images0 = cx.update(|cx| c.scene.read(cx).images);
    cx.update(|cx| {
        c.scene.update(cx, |sc, cx| {
            sc.frame = Some(0);
            cx.notify();
        })
    });
    // L'échantillonneur tourne jusqu'à la FIN de l'animation (la cadence réelle décide de la durée).
    let fini = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let fini2 = fini.clone();
    let echant = std::thread::spawn(move || {
        let mut v = Vec::new();
        while !fini2.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(img) = win::capture_screen(zx, zy, zw, zh) {
                v.push((
                    bord(&img, col_enfant, 0, BLEU),
                    bord(&img, col_temoin, 0, VERT),
                    bord(&img, col_popup, 0, BLEU),
                ));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        v
    });
    let t_anim = Instant::now();
    let sc = c.scene.clone();
    let anime = |cx: &mut AsyncApp, sc: &Entity<Scene>| cx.update(|cx| sc.read(cx).frame.is_some());
    while anime(cx, &sc) && t_anim.elapsed() < Duration::from_secs(40) {
        pause(cx, 20).await;
    }
    fini.store(true, std::sync::atomic::Ordering::Relaxed);
    wait_for(cx, 2000, || echant.is_finished()).await;
    let echantillons = echant.join().unwrap_or_default();
    let images = cx.update(|cx| c.scene.read(cx).images) - images0;
    let ips = images as f64 / t_anim.elapsed().as_secs_f64();
    r.note("2_images_par_seconde", format!("{ips:.1}"));
    r.note("2_echantillons", echantillons.len());
    let pas = VITESSE as f32 * s;
    let mut histo = |nom: &str, i: usize| {
        let mut ecarts: Vec<i32> = echantillons
            .iter()
            .filter_map(|e| {
                let (a, t, p) = *e;
                let page = if i == 0 { a } else { p };
                Some((page? - t?).abs())
            })
            .collect();
        ecarts.sort_unstable();
        let n = ecarts.len().max(1);
        let images: Vec<f32> = ecarts.iter().map(|&d| d as f32 / pas).collect();
        let zero = images.iter().filter(|&&x| x < 0.5).count();
        let un = images.iter().filter(|&&x| (0.5..1.5).contains(&x)).count();
        let plus = images.iter().filter(|&&x| x >= 1.5).count();
        let max = images.last().copied().unwrap_or(0.);
        let med = images.get(images.len() / 2).copied().unwrap_or(0.);
        r.note(
            nom,
            format!(
                "n={} retard 0 image {:.0}% / 1 image {:.0}% / ≥2 images {:.0}% ; médiane {med:.2} ; max {max:.2} images",
                ecarts.len(),
                100. * zero as f32 / n as f32,
                100. * un as f32 / n as f32,
                100. * plus as f32 / n as f32
            ),
        );
    };
    histo("2_suivi_enfant", 0);
    histo("2_suivi_popup", 1);

    // ── 3. Rognage sous la barre GPUI ──
    cx.update(|cx| {
        c.scene.update(cx, |sc, cx| {
            sc.offset = 280.;
            cx.notify();
        })
    });
    pause(cx, 400).await;
    r.note(
        "3_sans_region_enfant_sous_barre (bleu = la page couvre la barre)",
        pixel(ENFANT_X + 250., 40.),
    );
    r.note("3_sans_region_popup_sous_barre", pixel(POPUP_X + 140., 40.));
    cx.update(|cx| {
        c.scene.update(cx, |sc, cx| {
            sc.rogner = true;
            cx.notify();
        })
    });
    pause(cx, 400).await;
    r.note(
        "3_region_enfant_sous_barre (rouge = rognée)",
        pixel(ENFANT_X + 250., 40.),
    );
    r.note(
        "3_region_enfant_dessous (bleu)",
        pixel(ENFANT_X + 250., 200.),
    );
    r.note(
        "3_region_popup_sous_barre (rouge)",
        pixel(POPUP_X + 140., 40.),
    );
    r.note("3_region_popup_dessous (bleu)", pixel(POPUP_X + 140., 200.));
    // En défilement (le haut des pages descend jusqu'à 20 px, sous la barre) : la page passe-t-elle
    // sur la barre ? Sans région (témoin), puis avec la région recalculée à chaque image.
    let (bx, by) = ecran(ENFANT_X + 250., BARRE_H - 4.);
    let (bx2, _) = ecran(POPUP_X + 140., 0.);
    for rogner in [false, true] {
        cx.update(|cx| {
            c.scene.update(cx, |sc, cx| {
                sc.rogner = rogner;
                sc.ampl = 280;
                sc.frame = Some(0);
                sc.offset = 0.;
                cx.notify();
            })
        });
        let fini = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fini2 = fini.clone();
        let fuite = std::thread::spawn(move || {
            let (mut n, mut bleu_e, mut bleu_p) = (0, 0, 0);
            while !fini2.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(img) = win::capture_screen(bx, by, bx2 - bx + 1, 1) {
                    n += 1;
                    bleu_e += proche(img.rgb(0, 0), BLEU) as u32;
                    bleu_p += proche(img.rgb(img.w - 1, 0), BLEU) as u32;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            (n, bleu_e, bleu_p)
        });
        let t = Instant::now();
        while cx.update(|cx| c.scene.read(cx).frame.is_some())
            && t.elapsed() < Duration::from_secs(40)
        {
            pause(cx, 20).await;
        }
        fini.store(true, std::sync::atomic::Ordering::Relaxed);
        wait_for(cx, 2000, || fuite.is_finished()).await;
        r.note(
            &format!("3_en_mouvement_rogner={rogner} (échantillons, page bleue sur la barre : enfant, popup)"),
            fuite.join().unwrap_or_default(),
        );
    }
    cx.update(|cx| c.scene.update(cx, |sc, _| sc.ampl = AMPLITUDE));
    cx.update(|cx| {
        c.scene.update(cx, |sc, cx| {
            sc.frame = None;
            sc.offset = 0.;
            sc.rogner = false;
            cx.notify();
        })
    });
    pause(cx, 300).await;

    // ── 4. Un menu GPUI (fenêtre PopUp, sans activation) au-dessus des deux pages ──
    let (mx, my) = (ox as f32 / s + 400., oy as f32 / s + 400.);
    let menu: Option<AnyWindowHandle> = cx.update(|cx| {
        let options = WindowOptions {
            titlebar: None,
            focus: false,
            kind: WindowKind::PopUp,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(mx), px(my)),
                size(px(300.), px(80.)),
            ))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Menu))
            .ok()
            .map(|(h, _)| h)
    });
    pause(cx, 600).await;
    r.note("4_menu_sur_enfant (jaune)", pixel(450., 440.));
    r.note("4_menu_sur_popup (jaune)", pixel(650., 440.));
    r.note("4_enfant_hors_menu (bleu)", pixel(100., 560.));
    if let Some(h) = menu {
        let _ = cx.update_window(h, |_, window, _| window.remove_window());
    }
    pause(cx, 200).await;

    // ── 5. Déplacement et minimisation de la fenêtre GPUI (sans activation) ──
    let avant_g = win::window_rect(g).unwrap_or_default();
    let avant_c = win::window_rect(conteneur).unwrap_or_default();
    let avant_p = win::window_rect(popup_hwnd).unwrap_or_default();
    win::set_pos(
        g,
        avant_g.left + 80,
        avant_g.top + 40,
        0,
        0,
        win::SWP_NOSIZE | win::SWP_NOZORDER,
    );
    pause(cx, 400).await;
    let dc = win::window_rect(conteneur).unwrap_or_default();
    let dp = win::window_rect(popup_hwnd).unwrap_or_default();
    r.note(
        "5_deplacement_enfant (dx, dy attendu 80, 40)",
        (dc.left - avant_c.left, dc.top - avant_c.top),
    );
    r.note(
        "5_deplacement_popup_sans_rendu (dx, dy)",
        (dp.left - avant_p.left, dp.top - avant_p.top),
    );
    let t_notify = Instant::now();
    cx.update(|cx| c.scene.update(cx, |_, cx| cx.notify()));
    let cible = (avant_p.left + 80, avant_p.top + 40);
    wait_for(cx, 1000, move || {
        win::window_rect(popup_hwnd).is_some_and(|r| (r.left, r.top) == cible)
    })
    .await;
    let dp = win::window_rect(popup_hwnd).unwrap_or_default();
    r.note(
        "5_deplacement_popup_apres_notify (dx, dy ; ms)",
        (
            dp.left - avant_p.left,
            dp.top - avant_p.top,
            t_notify.elapsed().as_millis(),
        ),
    );
    win::set_pos(
        g,
        avant_g.left,
        avant_g.top,
        0,
        0,
        win::SWP_NOSIZE | win::SWP_NOZORDER,
    );
    cx.update(|cx| c.scene.update(cx, |_, cx| cx.notify()));
    pause(cx, 300).await;
    win::show(g, win::SW_SHOWMINNOACTIVE);
    pause(cx, 600).await;
    r.note(
        "5_minimisee (iconique, popup visible, conteneur visible)",
        (
            win::is_iconic(g),
            win::is_visible(popup_hwnd),
            win::is_visible(conteneur),
        ),
    );
    win::show(g, win::SW_SHOWNOACTIVATE);
    cx.update(|cx| c.scene.update(cx, |_, cx| cx.notify()));
    pause(cx, 800).await;
    r.note(
        "5_restauree (iconique, popup visible)",
        (win::is_iconic(g), win::is_visible(popup_hwnd)),
    );
    r.note(
        "5_restauree_enfant (bleu)",
        pixel(ENFANT_X + 250., PAGE_TOP + 150.),
    );
    r.note(
        "5_restauree_popup (bleu)",
        pixel(POPUP_X + 140., PAGE_TOP + 150.),
    );

    win::set_topmost(g, false);
    win::set_topmost(popup_hwnd, false);
    r.note("5_premier_plan_inchange", win::foreground() == fg0);

    // ── 6. DPI ──
    r.note(
        "6_page_enfant (innerWidth x innerHeight, dpr)",
        eval(
            cx,
            c.enfant.read_with(cx, |w, _| w.handle()).raw(),
            "innerWidth+'x'+innerHeight+' dpr='+devicePixelRatio",
        )
        .await,
    );
    let rc = win::window_rect(conteneur).unwrap_or_default();
    r.note(
        "6_conteneur_physique (l x h) contre logique x échelle",
        (
            rc.right - rc.left,
            rc.bottom - rc.top,
            (ENFANT_W * s).round(),
            (PAGE_H * s).round(),
        ),
    );

    // ── 7. Capture pour le harnais pixels ──
    let h_enfant = c.enfant.read_with(cx, |w, _| w.handle());
    r.note(
        "7_capturepreview_enfant (ms, octets, centre)",
        apercu(cx, h_enfant.raw()).await,
    );
    r.note("7_capturepreview_popup", apercu(cx, &popup_wv).await);
    let t = Instant::now();
    let pw = win::print_window(conteneur);
    r.note(
        "7_printwindow_conteneur_enfant (ms, centre)",
        (
            t.elapsed().as_millis(),
            pw.as_ref().and_then(|i| i.rgb(i.w / 2, i.h / 2)),
        ),
    );
    let t = Instant::now();
    let pw = win::print_window(g);
    let ms = t.elapsed().as_millis();
    let gr = win::window_rect(g).unwrap_or_default();
    let (cxx, cyy) = ecran(ENFANT_X + 250., PAGE_TOP + 150.);
    let (tx, ty) = ecran(TEMOIN_X + 10., PAGE_TOP + 150.);
    r.note(
        "7_printwindow_fenetre_gpui (ms, page, témoin)",
        (
            ms,
            pw.as_ref().and_then(|i| i.rgb(cxx - gr.left, cyy - gr.top)),
            pw.as_ref().and_then(|i| i.rgb(tx - gr.left, ty - gr.top)),
        ),
    );
    let t = Instant::now();
    let pw = win::print_window(popup_hwnd);
    r.note(
        "7_printwindow_popup (ms, centre)",
        (
            t.elapsed().as_millis(),
            pw.as_ref().and_then(|i| i.rgb(i.w / 2, i.h / 2)),
        ),
    );
    // Région : CapturePreview ignore-t-il la région du HWND ? (moitié haute rognée)
    let hc = ((PAGE_H * s) / 2.).round() as i32;
    win::set_region(
        conteneur,
        Some(win::Rect {
            left: 0,
            top: hc,
            right: (ENFANT_W * s).round() as i32,
            bottom: (PAGE_H * s).round() as i32,
        }),
    );
    pause(cx, 200).await;
    r.note(
        "7_capturepreview_enfant_rogne (centre)",
        apercu(cx, h_enfant.raw()).await,
    );
    let pw = win::print_window(conteneur);
    r.note(
        "7_printwindow_enfant_rogne (haut, bas)",
        (
            pw.as_ref().and_then(|i| i.rgb(i.w / 2, 10)),
            pw.as_ref().and_then(|i| i.rgb(i.w / 2, i.h - 10)),
        ),
    );
    win::set_region(conteneur, None);
    // Hors écran (mode invisible du harnais) : le popup quitte le bureau virtuel.
    cx.update(|cx| {
        c.scene.update(cx, |sc, cx| {
            sc.popup = None;
            cx.notify();
        })
    });
    let (vx, vy, _, _) = win::virtual_screen();
    win::set_pos(
        popup_hwnd,
        vx - 4000,
        vy,
        0,
        0,
        win::SWP_NOSIZE | win::SWP_NOZORDER,
    );
    pause(cx, 500).await;
    r.note(
        "7_capturepreview_popup_hors_ecran",
        apercu(cx, &popup_wv).await,
    );
    let pw = win::print_window(popup_hwnd);
    r.note(
        "7_printwindow_popup_hors_ecran (centre)",
        pw.as_ref().and_then(|i| i.rgb(i.w / 2, i.h / 2)),
    );
    let _ = popup_wv.set_visible(false);
    pause(cx, 500).await;
    r.note(
        "7_capturepreview_webview_cachee",
        apercu(cx, &popup_wv).await,
    );
    let _ = popup_wv.set_visible(true);

    // ── 8. Focus et saisie par messages Win32 (sans activer la fenêtre) ──
    focus(cx, &c, conteneur, g, &mut r).await;

    // ── 9. Isolement : 3 variantes, popups hors écran ──
    let mut isos = Vec::new();
    for (n, page) in [
        ("iso1", "isolement.html?v=1&reseau=0"),
        ("iso2", "isolement.html?v=2"),
        ("iso3", "__hote.html?v=3&p=isolement.html"),
    ] {
        match ouvrir_popup(
            &mut c.web.borrow_mut(),
            g,
            (vx - 4000, vy + 400, 300, 200),
            &c.vault,
            n,
            &c.shared,
            page.into(),
        ) {
            Ok(p) => isos.push(p),
            Err(e) => r.note(&format!("9_{n}_erreur"), e),
        }
    }
    let sh = c.shared.clone();
    wait_for(cx, 15_000, move || {
        ["iso1", "iso2", "iso3"]
            .iter()
            .all(|n| compter(&sh, n, "isolement") > 0)
    })
    .await;
    for n in ["iso1", "iso2", "iso3"] {
        r.note(
            &format!("9_{n}"),
            dernier(&c.shared, n, "isolement").map(|(m, _)| m.to_string()),
        );
    }
    for n in ["iso1", "iso2", "iso3"] {
        r.note(
            &format!("9_{n}_ipc_direct_recu"),
            compter(&c.shared, n, "ipc-direct"),
        );
    }
    for (h, wv) in isos {
        drop(wv);
        win::destroy(h);
    }

    // ── 10. Mémoire et temps d'ouverture de 1, 5, 10 webviews ──
    pause(cx, 1500).await;
    r.note(
        "10_memoire_base (Mo WS, Mo privés, processus)",
        memoire_webview2(),
    );
    let mut lot: Vec<(isize, wry::WebView)> = Vec::new();
    for cible in [1usize, 5, 10] {
        let t = Instant::now();
        let debut = lot.len();
        for k in debut..cible {
            let src = format!("lot{k}");
            if let Ok(p) = ouvrir_popup(
                &mut c.web.borrow_mut(),
                g,
                (vx - 4000, vy + 800, 400, 300),
                &c.vault,
                &src,
                &c.shared,
                "defile.html".into(),
            ) {
                lot.push(p);
            }
        }
        let t_build = t.elapsed().as_millis();
        let sh = c.shared.clone();
        wait_for(cx, 30_000, move || {
            (debut..cible).all(|k| compter(&sh, &format!("lot{k}"), "height") > 0)
        })
        .await;
        let t_pret = t.elapsed().as_millis();
        pause(cx, 1500).await;
        r.note(
            &format!("10_lot_{cible} (ms construction, ms jusqu'au dernier height, mémoire)"),
            (t_build, t_pret, memoire_webview2()),
        );
    }
    for (h, wv) in lot {
        drop(wv);
        win::destroy(h);
    }

    let fg = win::foreground();
    r.note(
        "premier_plan_intact (inchangé, à nous)",
        (fg == fg0, win::pid_of(fg) == std::process::id()),
    );
    drop(popup_wv);
    win::destroy(popup_hwnd);
    r
}

/// Focus et saisie dans la webview enfant : clic et frappe POSTÉS au HWND de rendu de Chromium.
async fn focus(cx: &mut AsyncApp, c: &Ctx, conteneur: isize, g: isize, r: &mut Report) {
    let h = c.enfant.read_with(cx, |w, _| w.handle());
    let avant = compter(&c.shared, "enfant", "hello");
    let _ = h.raw().load_url(&url("saisie.html"));
    let sh = c.shared.clone();
    wait_for(cx, 10_000, move || compter(&sh, "enfant", "hello") > avant).await;
    pause(cx, 500).await;
    let fg = win::foreground();
    let enfants = win::descendants(conteneur);
    let rwh = enfants
        .iter()
        .find(|(_, c)| c == "Chrome_RenderWidgetHostHWND")
        .map(|(h, _)| *h)
        .unwrap_or(0);
    r.note(
        "8_hwnd_descendants (classe, même processus ?)",
        enfants
            .iter()
            .map(|(h, cl)| format!("{cl}:{}", win::pid_of(*h) == std::process::id()))
            .collect::<Vec<_>>(),
    );
    if rwh == 0 {
        r.note("8_rwh", "Chrome_RenderWidgetHostHWND introuvable");
    }
    let s = cx.update(|cx| c.scene.read(cx).scale);
    let (x, y) = ((60. * s).round() as i32, (35. * s).round() as i32);
    win::post(rwh, win::WM_LBUTTONDOWN, 1, win::lparam_xy(x, y));
    win::post(rwh, win::WM_LBUTTONUP, 0, win::lparam_xy(x, y));
    pause(cx, 500).await;
    r.note("8_apres_clic_poste", eval(cx, h.raw(), "__etat()").await);
    for ch in ['a', 'b'] {
        win::post(rwh, win::WM_CHAR, ch as usize, 1);
    }
    pause(cx, 400).await;
    r.note("8_apres_wm_char", eval(cx, h.raw(), "__etat()").await);
    let avant = compter(&c.shared, "enfant", "key");
    // VK_ESCAPE, code de balayage 0x01.
    win::post(rwh, win::WM_KEYDOWN, 0x1B, 1 | (0x01 << 16));
    win::post(
        rwh,
        win::WM_KEYUP,
        0x1B,
        1 | (0x01 << 16) | (0xC000_0000u32 as isize),
    );
    let sh = c.shared.clone();
    wait_for(cx, 1500, move || compter(&sh, "enfant", "key") > avant).await;
    r.note(
        "8_echap_poste → message key",
        dernier(&c.shared, "enfant", "key").map(|(m, _)| m.to_string()),
    );
    // Focus programmatique : MoveFocus puis retour à GPUI.
    r.note("8_focus_fil_avant (== fenêtre GPUI ?)", win::focus() == g);
    r.note("8_move_focus", win::move_focus(h.raw()));
    pause(cx, 300).await;
    let f = win::focus();
    r.note(
        "8_focus_fil_apres_movefocus (gpui, conteneur, descendant webview)",
        (
            f == g,
            f == conteneur,
            win::descendants(conteneur).iter().any(|(h, _)| *h == f),
        ),
    );
    r.note(
        "8_etat_page_apres_movefocus",
        eval(cx, h.raw(), "__etat()").await,
    );
    let _ = h.raw().focus_parent();
    pause(cx, 300).await;
    r.note(
        "8_focus_fil_apres_focus_parent (== gpui)",
        win::focus() == g,
    );
    r.note("8_premier_plan_inchange", win::foreground() == fg);
}

/// Mode `--interactif` : la page de saisie dans l'enfant, 60 s pour la souris, le clavier et l'IME réels.
async fn interactif(cx: &mut AsyncApp, c: &Ctx, r: &mut Report) {
    let h = c.enfant.read_with(cx, |w, _| w.handle());
    let _ = h.raw().load_url(&url("saisie.html"));
    println!(
        "[S10] INTERACTIF : cliquez dans le champ, tapez (IME compris), Échap, puis cliquez la barre rouge. 60 s."
    );
    for _ in 0..12 {
        pause(cx, 5000).await;
        let etat = eval(cx, h.raw(), "__etat()").await;
        println!("[S10] {etat:?}");
    }
    r.note("interactif_fin", eval(cx, h.raw(), "__etat()").await);
    r.note(
        "interactif_touches_recues",
        c.shared
            .lock()
            .map(|g| {
                g.messages
                    .iter()
                    .filter(|(s, m, _)| s == "enfant" && m["type"] == "key")
                    .map(|(_, m, _)| m["key"].to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );
}

// ───────────────────────────── point d'entrée ─────────────────────────────

pub fn run(interactif: bool) -> Report {
    let report = Arc::new(Mutex::new(Report::default()));
    let report_out = report.clone();
    let vault = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/spike_s10/coffre");
    let donnees = std::env::temp_dir().join(DONNEES);

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let shared: Partage = Arc::new(Mutex::new(Shared::default()));
        let web = Rc::new(RefCell::new(WebContext::new(Some(donnees))));
        let mut enfant = None;
        let mut erreur = None;
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(TITRE.into()),
                ..Default::default()
            }),
            // ★ Jamais au premier plan : SW_SHOWNOACTIVATE chez GPUI (focus: false), sauf en interactif.
            focus: interactif,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(40.), px(40.)),
                size(px(LARGEUR), px(HAUTEUR)),
            ))),
            ..Default::default()
        };
        let opened = gpui_kit::open_window(options, cx, |window, cx| {
            let scale = window.scale_factor();
            let mut contexte = web.borrow_mut();
            let b = constructeur(
                &mut contexte,
                &vault,
                "enfant",
                &shared,
                "defile.html".into(),
            );
            match WebView::build(b, window, cx) {
                Ok(w) => enfant = Some(cx.new(|_| w)),
                Err(e) => erreur = Some(format!("WebView::build : {e}")),
            }
            cx.new(|_| Scene {
                enfant: enfant.clone(),
                offset: 0.,
                frame: None,
                ampl: AMPLITUDE,
                images: 0,
                popup: None,
                rogner: false,
                conteneur: 0,
                scale,
            })
        });
        let (scene, enfant) = match (opened, enfant) {
            (Ok((_, scene)), Some(e)) => (scene, e),
            (o, _) => {
                if let Ok(mut g) = report.lock() {
                    g.error = Some(erreur.unwrap_or_else(|| {
                        format!("open_window : {:?}", o.err().map(|e| e.to_string()))
                    }));
                }
                cx.quit();
                return;
            }
        };
        let ctx = Ctx {
            scene,
            enfant,
            shared,
            vault: vault.clone(),
            web,
            report: report.clone(),
            interactif,
        };
        cx.spawn(async move |cx| {
            let rep = ctx.report.clone();
            let r = drive(cx, ctx).await;
            if let Ok(mut g) = rep.lock() {
                *g = r;
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });

    report_out
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}
