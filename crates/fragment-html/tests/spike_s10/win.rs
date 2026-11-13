//! FFI Win32 du spike S10 : TOUT l'`unsafe` Win32 du spike est ici (préfigure `hote/win32.rs`, ADR-019).
//! user32 / gdi32 / kernel32 déclarés à la main, comme S2 ; les HWND circulent en `isize`.
#![allow(unsafe_code, dead_code)]

use std::ffi::c_void;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
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

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
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
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn FindWindowExW(parent: isize, after: isize, class: *const u16, title: *const u16) -> isize;
    fn ClientToScreen(h: isize, p: *mut Point) -> i32;
    fn GetWindowRect(h: isize, r: *mut Rect) -> i32;
    fn GetDC(h: isize) -> isize;
    fn ReleaseDC(h: isize, dc: isize) -> i32;
    fn DestroyWindow(h: isize) -> i32;
    fn GetDpiForWindow(h: isize) -> u32;
    fn SetWindowPos(h: isize, after: isize, x: i32, y: i32, w: i32, hh: i32, flags: u32) -> i32;
    fn SetWindowRgn(h: isize, rgn: isize, redraw: i32) -> i32;
    fn ShowWindow(h: isize, cmd: i32) -> i32;
    fn IsWindowVisible(h: isize) -> i32;
    fn IsIconic(h: isize) -> i32;
    fn GetForegroundWindow() -> isize;
    fn GetFocus() -> isize;
    fn SetFocus(h: isize) -> isize;
    fn PostMessageW(h: isize, m: u32, w: usize, l: isize) -> i32;
    fn EnumChildWindows(
        parent: isize,
        f: unsafe extern "system" fn(isize, isize) -> i32,
        l: isize,
    ) -> i32;
    fn GetClassNameW(h: isize, buf: *mut u16, n: i32) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn PrintWindow(h: isize, dc: isize, flags: u32) -> i32;
    fn GetWindowThreadProcessId(h: isize, pid: *mut u32) -> u32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn CreateRectRgn(l: i32, t: i32, r: i32, b: i32) -> isize;
    fn CreateCompatibleDC(dc: isize) -> isize;
    fn DeleteDC(dc: isize) -> i32;
    fn CreateDIBSection(
        dc: isize,
        bmi: *const BitmapInfoHeader,
        usage: u32,
        bits: *mut *mut c_void,
        section: isize,
        offset: u32,
    ) -> isize;
    fn SelectObject(dc: isize, o: isize) -> isize;
    fn DeleteObject(o: isize) -> i32;
    fn BitBlt(
        dst: isize,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: isize,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
}

pub const SWP_NOSIZE: u32 = 0x1;
pub const SWP_NOMOVE: u32 = 0x2;
pub const SWP_NOZORDER: u32 = 0x4;
pub const SWP_NOACTIVATE: u32 = 0x10;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_SHOWMINNOACTIVE: i32 = 7;
pub const WM_LBUTTONDOWN: u32 = 0x201;
pub const WM_LBUTTONUP: u32 = 0x202;
pub const WM_KEYDOWN: u32 = 0x100;
pub const WM_KEYUP: u32 = 0x101;
pub const WM_CHAR: u32 = 0x102;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn find_window(title: &str) -> isize {
    let t = wide(title);
    // SÛRETÉ : chaîne UTF-16 terminée par 0, vivante pendant l'appel.
    unsafe { FindWindowW(std::ptr::null(), t.as_ptr()) }
}

/// Le premier enfant DIRECT de `parent` de la classe `class` (0 si aucun).
pub fn find_child(parent: isize, class: &str) -> isize {
    let c = wide(class);
    // SÛRETÉ : idem ; `parent` peut être invalide, l'API rend alors 0.
    unsafe { FindWindowExW(parent, 0, c.as_ptr(), std::ptr::null()) }
}

unsafe extern "system" fn collecte(h: isize, l: isize) -> i32 {
    // SÛRETÉ : `l` est le `&mut Vec` passé par `descendants`, vivant pendant `EnumChildWindows`.
    let v = unsafe { &mut *(l as *mut Vec<(isize, String)>) };
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, buf.as_mut_ptr(), 128) };
    v.push((h, String::from_utf16_lossy(&buf[..n.max(0) as usize])));
    1
}

/// Tous les descendants (classe comprise), y compris ceux d'un autre processus (WebView2).
pub fn descendants(hwnd: isize) -> Vec<(isize, String)> {
    let mut v: Vec<(isize, String)> = Vec::new();
    // SÛRETÉ : le pointeur sur `v` ne sert que pendant l'appel synchrone.
    unsafe { EnumChildWindows(hwnd, collecte, &mut v as *mut _ as isize) };
    v
}

pub fn pid_of(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    // SÛRETÉ : sortie dans une variable locale.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

pub fn client_origin(hwnd: isize) -> Option<(i32, i32)> {
    let mut p = Point { x: 0, y: 0 };
    // SÛRETÉ : sortie dans une variable locale.
    (unsafe { ClientToScreen(hwnd, &mut p) } != 0).then_some((p.x, p.y))
}

pub fn window_rect(hwnd: isize) -> Option<Rect> {
    let mut r = Rect::default();
    // SÛRETÉ : sortie dans une variable locale.
    (unsafe { GetWindowRect(hwnd, &mut r) } != 0).then_some(r)
}

pub fn dpi(hwnd: isize) -> u32 {
    // SÛRETÉ : lecture seule, HWND invalide → 0.
    unsafe { GetDpiForWindow(hwnd) }
}

pub fn set_pos(hwnd: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> bool {
    // SÛRETÉ : pas de pointeur ; toujours SANS activation chez l'appelant.
    unsafe { SetWindowPos(hwnd, 0, x, y, w, h, flags | SWP_NOACTIVATE) != 0 }
}

/// Rogne `hwnd` au rectangle (coordonnées FENÊTRE), ou retire la région (`None`).
pub fn set_region(hwnd: isize, r: Option<Rect>) -> bool {
    // SÛRETÉ : la région créée appartient au système après `SetWindowRgn` (ne pas la libérer).
    unsafe {
        let rgn = match r {
            Some(r) => CreateRectRgn(r.left, r.top, r.right, r.bottom),
            None => 0,
        };
        SetWindowRgn(hwnd, rgn, 1) != 0
    }
}

pub fn show(hwnd: isize, cmd: i32) {
    // SÛRETÉ : pas de pointeur ; seules des commandes SANS activation sont utilisées.
    unsafe { ShowWindow(hwnd, cmd) };
}

pub fn is_visible(hwnd: isize) -> bool {
    // SÛRETÉ : lecture seule.
    unsafe { IsWindowVisible(hwnd) != 0 }
}

pub fn is_iconic(hwnd: isize) -> bool {
    // SÛRETÉ : lecture seule.
    unsafe { IsIconic(hwnd) != 0 }
}

pub fn foreground() -> isize {
    // SÛRETÉ : lecture seule.
    unsafe { GetForegroundWindow() }
}

pub fn focus() -> isize {
    // SÛRETÉ : lecture seule (focus du fil courant).
    unsafe { GetFocus() }
}

pub fn set_focus(hwnd: isize) -> isize {
    // SÛRETÉ : focus LOCAL au fil ; ne change pas le premier plan d'un fil inactif.
    unsafe { SetFocus(hwnd) }
}

pub fn post(hwnd: isize, msg: u32, w: usize, l: isize) -> bool {
    // SÛRETÉ : message posté sans pointeur dans `w`/`l`.
    unsafe { PostMessageW(hwnd, msg, w, l) != 0 }
}

/// Le rectangle du bureau virtuel (tous les moniteurs) : x, y, l, h.
pub fn virtual_screen() -> (i32, i32, i32, i32) {
    // SÛRETÉ : lecture seule.
    unsafe {
        (
            GetSystemMetrics(76),
            GetSystemMetrics(77),
            GetSystemMetrics(78),
            GetSystemMetrics(79),
        )
    }
}

pub fn destroy(hwnd: isize) {
    // SÛRETÉ : fenêtre créée par ce fil.
    unsafe { DestroyWindow(hwnd) };
}

/// Un POPUP possédé par `owner`, jamais activé (NOACTIVATE + TOOLWINDOW), fond vert. Pixels écran.
pub fn create_popup_host(owner: isize, r: (i32, i32, i32, i32)) -> isize {
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const WS_CLIPCHILDREN: u32 = 0x0200_0000;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
    // SÛRETÉ : structure de classe initialisée sur place ; chaînes vivantes pendant les appels.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("FragmentSpikeS10Hote");
        let wc = WndClassW {
            style: 0,
            wnd_proc: DefWindowProcW,
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: 0,
            background: CreateSolidBrush(0x0000_FF00),
            menu: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let t = wide("spike-s10-popup");
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            t.as_ptr(),
            WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN,
            r.0,
            r.1,
            r.2,
            r.3,
            owner,
            0,
            instance,
            std::ptr::null(),
        )
    }
}

/// Une image BGRA 32 bits, rangée de haut en bas.
pub struct Image {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
}

impl Image {
    pub fn rgb(&self, x: i32, y: i32) -> Option<(u8, u8, u8)> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        let p = *self.px.get((y * self.w + x) as usize)?;
        Some((
            ((p >> 16) & 0xFF) as u8,
            ((p >> 8) & 0xFF) as u8,
            (p & 0xFF) as u8,
        ))
    }
}

/// Une surface DIB 32 bits (haut en bas) dans un DC mémoire ; `f` y dessine, puis on recopie.
fn with_dib(w: i32, h: i32, f: impl FnOnce(isize) -> bool) -> Option<Image> {
    if w <= 0 || h <= 0 {
        return None;
    }
    // SÛRETÉ : DC et bitmap créés, sélectionnés puis libérés ici ; `bits` pointe sur w*h*4 octets
    // tant que la bitmap vit, et la copie a lieu avant `DeleteObject`.
    unsafe {
        let ecran = GetDC(0);
        let mem = CreateCompatibleDC(ecran);
        let bmi = BitmapInfoHeader {
            size: 40,
            width: w,
            height: -h,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(mem, &bmi, 0, &mut bits, 0, 0);
        let mut out = None;
        if bmp != 0 && !bits.is_null() {
            let old = SelectObject(mem, bmp);
            if f(mem) {
                let n = (w * h) as usize;
                let src = std::slice::from_raw_parts(bits as *const u32, n);
                out = Some(Image {
                    w,
                    h,
                    px: src.to_vec(),
                });
            }
            SelectObject(mem, old);
            DeleteObject(bmp);
        }
        DeleteDC(mem);
        ReleaseDC(0, ecran);
        out
    }
}

/// Le bureau COMPOSÉ (DWM) sur un rectangle écran.
pub fn capture_screen(x: i32, y: i32, w: i32, h: i32) -> Option<Image> {
    const SRCCOPY: u32 = 0x00CC_0020;
    const CAPTUREBLT: u32 = 0x4000_0000;
    with_dib(w, h, |mem| {
        // SÛRETÉ : DC de l'écran pris et rendu dans la fermeture.
        unsafe {
            let ecran = GetDC(0);
            let ok = BitBlt(mem, 0, 0, w, h, ecran, x, y, SRCCOPY | CAPTUREBLT) != 0;
            ReleaseDC(0, ecran);
            ok
        }
    })
}

/// `PrintWindow(PW_RENDERFULLCONTENT)` : l'image d'une fenêtre, même hors écran ou recouverte.
pub fn print_window(hwnd: isize) -> Option<Image> {
    const PW_RENDERFULLCONTENT: u32 = 2;
    let r = window_rect(hwnd)?;
    with_dib(r.right - r.left, r.bottom - r.top, |mem| {
        // SÛRETÉ : `mem` est le DC mémoire de `with_dib`.
        unsafe { PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT) != 0 }
    })
}

/// `lParam` d'un message souris : (x, y) client en pixels physiques.
pub fn lparam_xy(x: i32, y: i32) -> isize {
    (((y as u32 & 0xFFFF) << 16) | (x as u32 & 0xFFFF)) as isize
}

// ───────────────────────────── COM : WebView2 (webview2-com, windows) ─────────────────────────────

/// Résultat d'une capture `CapturePreview` : octets PNG ou erreur.
pub type Capture = std::sync::Arc<std::sync::Mutex<Option<Result<Vec<u8>, String>>>>;

fn lire_flux(flux: &windows::Win32::System::Com::IStream) -> Result<Vec<u8>, String> {
    use windows::Win32::System::Com::{STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};
    let mut st = STATSTG::default();
    // SÛRETÉ : `flux` est un IStream vivant ; `st` et `buf` sont des sorties locales de la bonne taille.
    unsafe {
        flux.Stat(&mut st, STATFLAG_NONAME)
            .map_err(|e| e.to_string())?;
        flux.Seek(0, STREAM_SEEK_SET, None)
            .map_err(|e| e.to_string())?;
        let n = st.cbSize as usize;
        let mut buf = vec![0u8; n];
        let mut lus = 0u32;
        flux.Read(buf.as_mut_ptr().cast(), n as u32, Some(&mut lus))
            .ok()
            .map_err(|e| e.to_string())?;
        buf.truncate(lus as usize);
        Ok(buf)
    }
}

/// `ICoreWebView2::CapturePreview(PNG)` : le contenu de la page, sans passer par l'écran.
/// Le PNG arrive dans `slot` (rappel sur le fil UI, pompé par la boucle de GPUI).
pub fn capture_preview(wv: &wry::WebView, slot: Capture) -> Result<(), String> {
    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    use wry::WebViewExtWindows as _;
    let coeur = wv.webview();
    // SÛRETÉ : flux mémoire possédé par COM (compté), libéré avec la dernière référence.
    let flux =
        unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true) }.map_err(|e| e.to_string())?;
    let f2 = flux.clone();
    let handler = CapturePreviewCompletedHandler::create(Box::new(move |r| {
        let res = r.map_err(|e| e.to_string()).and_then(|()| lire_flux(&f2));
        if let Ok(mut g) = slot.lock() {
            *g = Some(res);
        }
        Ok(())
    }));
    // SÛRETÉ : interface COM vivante (tenue par wry), flux et rappel comptés par COM.
    unsafe {
        coeur.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &flux,
            &handler,
        )
    }
    .map_err(|e| e.to_string())
}

/// `ICoreWebView2Controller::MoveFocus(PROGRAMMATIC)`.
pub fn move_focus(wv: &wry::WebView) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
    use wry::WebViewExtWindows as _;
    // SÛRETÉ : contrôleur vivant (tenu par wry).
    unsafe {
        wv.controller()
            .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
    }
    .map_err(|e| e.to_string())
}

/// Au-dessus de tout SANS activation (mesures sur le bureau composé), ou retour à la normale.
pub fn set_topmost(hwnd: isize, on: bool) -> bool {
    let apres: isize = if on { -1 } else { -2 };
    // SÛRETÉ : pas de pointeur ; SWP_NOACTIVATE : le premier plan ne change pas.
    unsafe {
        SetWindowPos(
            hwnd,
            apres,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) != 0
    }
}
