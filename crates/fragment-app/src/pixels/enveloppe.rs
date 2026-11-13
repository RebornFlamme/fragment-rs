//! L'identité des glyphes peints, lue SANS fork de GPUI (fr-p3.4, ADR-009).
//!
//! La scène peinte ne dit pas QUEL glyphe porte un sprite : `SubpixelSprite` et `MonochromeSprite`
//! n'ont que leur tuile d'atlas, et la table clé → tuile de l'atlas est privée, comme le cache des
//! lignes mises en page de la frame (`LineLayoutCache`, `pub(crate)`). ★ Les deux se lisent pourtant
//! par les traits PUBLICS de la plateforme : `fragment-pixels` lance l'application sur une plateforme
//! ENVELOPPÉE qui délègue tout à celle de Windows et relève au passage :
//! - dans le système de texte, chaque ligne mise en page (`layout_line`) : la police et l'id de chaque
//!   glyphe, et l'index de son caractère dans la chaîne, donc (police, glyphe) → caractère ;
//! - dans l'atlas de chaque fenêtre, chaque tuile rendue pour un glyphe (`get_or_insert_with`) :
//!   tuile → (police, glyphe).
//!
//! Un sprite de la scène se lit alors exactement : sa tuile donne son glyphe, son glyphe son
//! caractère. Rien n'est modifié : chaque appel est rendu tel que la plateforme de Windows le rend.
//!
//! ★ La délégation recopie les traits de `gpui-pre =0.3.12` (épinglé, ADR-001), méthodes par défaut
//! comprises : une méthode par défaut non déléguée remplacerait celle de Windows. Une montée de GPUI
//! qui ajoute une méthode casse la compilation ici, jamais en silence. Deux méthodes par défaut
//! restent NON déléguées, sans effet sur l'image : `update_jump_list` (liste de raccourcis de la
//! barre des tâches, son type demanderait `smallvec`) et `as_test` (fenêtre de test, `None` sous
//! Windows comme par défaut ; `TestWindow` n'est pas nommable hors de GPUI).

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use futures::channel::oneshot;
use gpui_kit::*;

/// (police, glyphe) → caractère(s), relevé dans toutes les lignes mises en page du processus.
fn caracteres() -> &'static Mutex<HashMap<(usize, u32), String>> {
    static CARACTERES: OnceLock<Mutex<HashMap<(usize, u32), String>>> = OnceLock::new();
    CARACTERES.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    /// L'atlas enveloppé de chaque fenêtre ouverte (la peinture et la capture vivent sur le fil
    /// principal).
    static ATLAS: RefCell<HashMap<WindowId, Arc<Atlas>>> = RefCell::new(HashMap::new());
}

/// L'application de `fragment-pixels`, sur la plateforme enveloppée.
pub fn application() -> Application {
    Application::with_platform(Rc::new(Plateforme(gpui_kit::platform::current_platform(
        false,
    ))))
}

/// Le caractère du glyphe peint avec la tuile `tuile` dans la fenêtre `fenetre`, s'il est connu.
pub fn caractere(fenetre: WindowId, tuile: &AtlasTile) -> Option<String> {
    let glyphe = ATLAS.with(|a| {
        a.borrow()
            .get(&fenetre)
            .and_then(|atlas| atlas.glyphe(tuile))
    })?;
    caracteres()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&glyphe)
        .cloned()
}

/// Relève (police, glyphe) → caractère(s) dans une ligne mise en page : le glyphe couvre les octets
/// de son index à celui du glyphe suivant (une ligature « fi » en couvre deux).
fn relever(texte: &str, ligne: &LineLayout) {
    let mut glyphes: Vec<(usize, usize, u32)> = ligne
        .runs
        .iter()
        .flat_map(|r| r.glyphs.iter().map(move |g| (g.index, r.font_id.0, g.id.0)))
        .collect();
    glyphes.sort_by_key(|g| g.0);
    let mut table = caracteres().lock().unwrap_or_else(|e| e.into_inner());
    for (k, &(index, police, glyphe)) in glyphes.iter().enumerate() {
        let fin = glyphes
            .iter()
            .skip(k + 1)
            .map(|g| g.0)
            .find(|&i| i > index)
            .unwrap_or(texte.len());
        let Some(c) = texte.get(index..fin).filter(|c| !c.is_empty()) else {
            continue;
        };
        // ★ Un même glyphe relevé sur un seul caractère l'emporte sur une grappe (un glyphe suivi
        // d'une marque de largeur nulle) : c'est son identité propre.
        let entree = table
            .entry((police, glyphe))
            .or_insert_with(|| c.to_owned());
        if c.chars().count() < entree.chars().count() {
            *entree = c.to_owned();
        }
    }
}

/// La plateforme de Windows, enveloppée.
struct Plateforme(Rc<dyn Platform>);

/// Une fenêtre de Windows, enveloppée : son atlas est relevé.
struct Fenetre(Box<dyn PlatformWindow>, Arc<Atlas>);

/// Le système de texte de Windows, enveloppé : ses lignes mises en page sont relevées.
struct Texte(Arc<dyn PlatformTextSystem>);

/// L'atlas d'une fenêtre, enveloppé : tuile → (police, glyphe).
struct Atlas {
    interieur: Arc<dyn PlatformAtlas>,
    tuiles: Mutex<HashMap<(AtlasTextureId, u32), (usize, u32)>>,
}

impl Atlas {
    fn glyphe(&self, tuile: &AtlasTile) -> Option<(usize, u32)> {
        self.tuiles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(tuile.texture_id, tuile.tile_id.0))
            .copied()
    }
}

impl PlatformAtlas for Atlas {
    fn get_or_insert_with<'a>(
        &self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let glyphe = match &key {
            AtlasKey::Glyph(p) => Some((p.font_id.0, p.glyph_id.0)),
            _ => None,
        };
        let tuile = self.interieur.get_or_insert_with(key, build)?;
        if let Some(t) = &tuile {
            let mut tuiles = self.tuiles.lock().unwrap_or_else(|e| e.into_inner());
            // ★ Une tuile libérée peut resservir, à un SVG comme à un autre glyphe : la dernière
            // insertion fait foi.
            match glyphe {
                Some(g) => tuiles.insert((t.texture_id, t.tile_id.0), g),
                None => tuiles.remove(&(t.texture_id, t.tile_id.0)),
            };
        }
        Ok(tuile)
    }

    fn remove(&self, key: &AtlasKey) {
        self.interieur.remove(key)
    }

    fn contains(&self, key: &AtlasKey) -> bool {
        self.interieur.contains(key)
    }
}

impl PlatformTextSystem for Texte {
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        let ligne = self.0.layout_line(text, font_size, runs);
        relever(text, &ligne);
        ligne
    }

    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.0.add_fonts(fonts)
    }
    fn set_missing_glyph_sink(&self, sink: Option<Arc<dyn MissingGlyphSink>>) {
        self.0.set_missing_glyph_sink(sink)
    }
    fn all_font_names(&self) -> Vec<String> {
        self.0.all_font_names()
    }
    fn font_id(&self, descriptor: &Font) -> Result<FontId> {
        self.0.font_id(descriptor)
    }
    fn prewarm_fonts(&self, font_ids: &[FontId]) {
        self.0.prewarm_fonts(font_ids)
    }
    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.0.font_metrics(font_id)
    }
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        self.0.typographic_bounds(font_id, glyph_id)
    }
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        self.0.advance(font_id, glyph_id)
    }
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.0.glyph_for_char(font_id, ch)
    }
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        self.0.glyph_raster_bounds(params)
    }
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        self.0.rasterize_glyph(params, raster_bounds)
    }
    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.0.recommended_rendering_mode(font_id, font_size)
    }
    fn glyph_dilation_for_color(&self, color: Hsla) -> u8 {
        self.0.glyph_dilation_for_color(color)
    }
}

impl Platform for Plateforme {
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        Arc::new(Texte(self.0.text_system()))
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let fenetre = self.0.open_window(handle, options)?;
        // ★ `sprite_atlas` rend un `Arc<dyn PlatformAtlas>` (trait de GPUI) : l'atlas de Windows
        // n'est pas `Send` non plus, il ne quitte jamais le fil principal.
        #[expect(
            clippy::arc_with_non_send_sync,
            reason = "type imposé par PlatformWindow::sprite_atlas"
        )]
        let atlas = Arc::new(Atlas {
            interieur: fenetre.sprite_atlas(),
            tuiles: Mutex::new(HashMap::new()),
        });
        ATLAS.with(|a| a.borrow_mut().insert(handle.window_id(), atlas.clone()));
        Ok(Box::new(Fenetre(fenetre, atlas)))
    }
    fn platform_info(&self) -> PlatformInfo {
        self.0.platform_info()
    }
    fn background_executor(&self) -> BackgroundExecutor {
        self.0.background_executor()
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        self.0.foreground_executor()
    }
    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        self.0.run(on_finish_launching)
    }
    fn quit(&self) {
        self.0.quit()
    }
    fn set_initial_windowing(&self, request: WindowingRequest) {
        self.0.set_initial_windowing(request)
    }
    fn request_windowing(&self, request: WindowingRequest) -> Task<anyhow::Result<()>> {
        self.0.request_windowing(request)
    }
    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<OsString>) {
        self.0.restart(binary_path, arguments)
    }
    fn activate(&self, ignoring_other_apps: bool) {
        self.0.activate(ignoring_other_apps)
    }
    fn set_activation_policy(&self, policy: ActivationPolicy) {
        self.0.set_activation_policy(policy)
    }
    fn hide(&self) {
        self.0.hide()
    }
    fn hide_other_apps(&self) {
        self.0.hide_other_apps()
    }
    fn unhide_other_apps(&self) {
        self.0.unhide_other_apps()
    }
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.0.displays()
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.0.primary_display()
    }
    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.on_displays_changed(callback)
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.0.active_window()
    }
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.0.window_stack()
    }
    fn is_screen_capture_supported(&self) -> bool {
        self.0.is_screen_capture_supported()
    }
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.0.screen_capture_sources()
    }
    fn window_appearance(&self) -> WindowAppearance {
        self.0.window_appearance()
    }
    fn set_window_appearance(&self, appearance: Option<WindowAppearance>) {
        self.0.set_window_appearance(appearance)
    }
    fn button_layout(&self) -> Option<WindowButtonLayout> {
        self.0.button_layout()
    }
    fn open_url(&self, url: &str) {
        self.0.open_url(url)
    }
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.0.on_open_urls(callback)
    }
    fn register_url_scheme(&self, url: &str) -> Task<Result<()>> {
        self.0.register_url_scheme(url)
    }
    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        self.0.prompt_for_paths(options)
    }
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        self.0.prompt_for_new_path(directory, suggested_name)
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.0.can_select_mixed_files_and_dirs()
    }
    fn reveal_path(&self, path: &Path) {
        self.0.reveal_path(path)
    }
    fn open_with_system(&self, path: &Path) {
        self.0.open_with_system(path)
    }
    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.on_quit(callback)
    }
    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.0.on_reopen(callback)
    }
    fn on_system_sleep(&self, callback: Box<dyn FnMut()>) {
        self.0.on_system_sleep(callback)
    }
    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        self.0.on_system_wake(callback)
    }
    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.0.on_app_lifecycle(callback)
    }
    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        self.0.on_memory_warning(callback)
    }
    fn gestures(&self) -> Option<Rc<dyn PlatformGestures>> {
        self.0.gestures()
    }
    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        self.0.set_menus(menus, keymap)
    }
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.0.get_menus()
    }
    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        self.0.set_dock_menu(menu, keymap)
    }
    fn perform_dock_menu_action(&self, action: usize) {
        self.0.perform_dock_menu_action(action)
    }
    fn add_recent_document(&self, path: &Path) {
        self.0.add_recent_document(path)
    }
    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.0.on_app_menu_action(callback)
    }
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.0.on_will_open_app_menu(callback)
    }
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.0.on_validate_app_menu_command(callback)
    }
    fn thermal_state(&self) -> ThermalState {
        self.0.thermal_state()
    }
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.0.on_thermal_state_change(callback)
    }
    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        self.0.prevent_idle_sleep(reason)
    }
    fn power_mode(&self) -> PowerMode {
        self.0.power_mode()
    }
    fn on_power_mode_change(&self, callback: Box<dyn FnMut()>) {
        self.0.on_power_mode_change(callback)
    }
    fn set_app_identity(&self, identifier: &str, name: &str) {
        self.0.set_app_identity(identifier, name)
    }
    fn show_system_notification(&self, notification: SystemNotification) {
        self.0.show_system_notification(notification)
    }
    fn dismiss_system_notification(&self, tag: &str) {
        self.0.dismiss_system_notification(tag)
    }
    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(SystemNotificationResponse)>,
    ) {
        self.0.on_system_notification_response(callback)
    }
    fn compositor_name(&self) -> &'static str {
        self.0.compositor_name()
    }
    fn graphical_environment(&self) -> Option<GraphicalEnvironment> {
        self.0.graphical_environment()
    }
    fn app_path(&self) -> Result<PathBuf> {
        self.0.app_path()
    }
    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.0.path_for_auxiliary_executable(name)
    }
    fn set_cursor_style(&self, style: CursorStyle) {
        self.0.set_cursor_style(style)
    }
    fn hide_cursor_until_mouse_moves(&self) {
        self.0.hide_cursor_until_mouse_moves()
    }
    fn is_cursor_visible(&self) -> bool {
        self.0.is_cursor_visible()
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        self.0.should_auto_hide_scrollbars()
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.0.read_from_clipboard()
    }
    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.0.write_to_clipboard(item)
    }
    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        self.0.read_from_clipboard_async()
    }
    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        self.0.write_credentials(url, username, password)
    }
    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        self.0.read_credentials(url)
    }
    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        self.0.delete_credentials(url)
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.0.keyboard_layout()
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.0.keyboard_mapper()
    }
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.0.on_keyboard_layout_change(callback)
    }
}

impl raw_window_handle::HasWindowHandle for Fenetre {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
}

impl raw_window_handle::HasDisplayHandle for Fenetre {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.0.display_handle()
    }
}

impl PlatformWindow for Fenetre {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.1.clone()
    }
    fn bounds(&self) -> Bounds<Pixels> {
        self.0.bounds()
    }
    fn is_maximized(&self) -> bool {
        self.0.is_maximized()
    }
    fn window_bounds(&self) -> WindowBounds {
        self.0.window_bounds()
    }
    fn content_size(&self) -> Size<Pixels> {
        self.0.content_size()
    }
    fn visual_viewport_bounds(&self) -> Bounds<Pixels> {
        self.0.visual_viewport_bounds()
    }
    fn on_visual_viewport_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.on_visual_viewport_changed(callback)
    }
    fn prepare_frame(&self) -> bool {
        self.0.prepare_frame()
    }
    fn resize(&mut self, size: Size<Pixels>) {
        self.0.resize(size)
    }
    fn scale_factor(&self) -> f32 {
        self.0.scale_factor()
    }
    fn appearance(&self) -> WindowAppearance {
        self.0.appearance()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.0.display()
    }
    fn mouse_position(&self) -> Point<Pixels> {
        self.0.mouse_position()
    }
    fn modifiers(&self) -> Modifiers {
        self.0.modifiers()
    }
    fn capslock(&self) -> Capslock {
        self.0.capslock()
    }
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.0.set_input_handler(input_handler)
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.take_input_handler()
    }
    fn set_text_input_configuration(&mut self, configuration: TextInputConfiguration) {
        self.0.set_text_input_configuration(configuration)
    }
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        self.0.prompt(level, msg, detail, answers)
    }
    fn activate(&self) {
        self.0.activate()
    }
    fn request_attention(&self) {
        self.0.request_attention()
    }
    fn is_active(&self) -> bool {
        self.0.is_active()
    }
    fn visibility(&self) -> WindowVisibility {
        self.0.visibility()
    }
    fn is_hovered(&self) -> bool {
        self.0.is_hovered()
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.0.background_appearance()
    }
    fn set_title(&mut self, title: &str) {
        self.0.set_title(title)
    }
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.0.set_background_appearance(background_appearance)
    }
    fn minimize(&self) {
        self.0.minimize()
    }
    fn zoom(&self) {
        self.0.zoom()
    }
    fn toggle_fullscreen(&self) {
        self.0.toggle_fullscreen()
    }
    fn is_fullscreen(&self) -> bool {
        self.0.is_fullscreen()
    }
    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        self.0.frame_waker()
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.on_request_frame(callback)
    }
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.0.on_input(callback)
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.on_active_status_change(callback)
    }
    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.0.on_visibility_change(callback)
    }
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.on_hover_status_change(callback)
    }
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.on_resize(callback)
    }
    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.on_moved(callback)
    }
    fn on_display_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.on_display_changed(callback)
    }
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.on_should_close(callback)
    }
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.0.on_hit_test_window_control(callback)
    }
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.on_close(callback)
    }
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.on_appearance_changed(callback)
    }
    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.on_button_layout_changed(callback)
    }
    fn draw(&self, scene: &Scene) {
        self.0.draw(scene)
    }
    fn schedule_frame(&self) {
        self.0.schedule_frame()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        self.0.is_subpixel_rendering_supported()
    }
    fn get_title(&self) -> String {
        self.0.get_title()
    }
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.0.tabbed_windows()
    }
    fn tab_bar_visible(&self) -> bool {
        self.0.tab_bar_visible()
    }
    fn set_edited(&mut self, edited: bool) {
        self.0.set_edited(edited)
    }
    fn set_document_path(&self, path: Option<&std::path::Path>) {
        self.0.set_document_path(path)
    }
    fn toggle_simple_fullscreen(&self) {
        self.0.toggle_simple_fullscreen()
    }
    fn is_simple_fullscreen(&self) -> bool {
        self.0.is_simple_fullscreen()
    }
    fn show_character_palette(&self) {
        self.0.show_character_palette()
    }
    fn titlebar_double_click(&self, is_resizable: bool, is_minimizable: bool) {
        self.0.titlebar_double_click(is_resizable, is_minimizable)
    }
    fn on_move_tab_to_new_window(&self, callback: Box<dyn FnMut()>) {
        self.0.on_move_tab_to_new_window(callback)
    }
    fn on_merge_all_windows(&self, callback: Box<dyn FnMut()>) {
        self.0.on_merge_all_windows(callback)
    }
    fn on_select_previous_tab(&self, callback: Box<dyn FnMut()>) {
        self.0.on_select_previous_tab(callback)
    }
    fn on_select_next_tab(&self, callback: Box<dyn FnMut()>) {
        self.0.on_select_next_tab(callback)
    }
    fn on_toggle_tab_bar(&self, callback: Box<dyn FnMut()>) {
        self.0.on_toggle_tab_bar(callback)
    }
    fn merge_all_windows(&self) {
        self.0.merge_all_windows()
    }
    fn move_tab_to_new_window(&self) {
        self.0.move_tab_to_new_window()
    }
    fn toggle_window_tab_overview(&self) {
        self.0.toggle_window_tab_overview()
    }
    fn set_tabbing_identifier(&self, identifier: Option<String>) {
        self.0.set_tabbing_identifier(identifier)
    }
    fn native_window_state(&self) -> Option<Vec<u8>> {
        self.0.native_window_state()
    }
    fn restore_native_window_state(&self, state: &[u8]) {
        self.0.restore_native_window_state(state)
    }
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        self.0.get_raw_handle()
    }
    fn inner_window_bounds(&self) -> WindowBounds {
        self.0.inner_window_bounds()
    }
    fn request_decorations(&self, decorations: WindowDecorations) {
        self.0.request_decorations(decorations)
    }
    fn show_window_menu(&self, position: Point<Pixels>) {
        self.0.show_window_menu(position)
    }
    fn start_window_move(&self) {
        self.0.start_window_move()
    }
    fn can_start_external_drag(&self) -> bool {
        self.0.can_start_external_drag()
    }
    fn start_external_drag(&self, payload: &ExternalDragPayload) -> bool {
        self.0.start_external_drag(payload)
    }
    fn start_window_resize(&self, edge: ResizeEdge) {
        self.0.start_window_resize(edge)
    }
    fn set_exclusive_zone(&self, zone: Pixels) {
        self.0.set_exclusive_zone(zone)
    }
    fn set_input_region(&self, region: Option<&[Bounds<Pixels>]>) {
        self.0.set_input_region(region)
    }
    fn window_decorations(&self) -> Decorations {
        self.0.window_decorations()
    }
    fn set_app_id(&mut self, app_id: &str) {
        self.0.set_app_id(app_id)
    }
    fn map_window(&mut self) -> anyhow::Result<()> {
        self.0.map_window()
    }
    fn window_controls(&self) -> WindowControls {
        self.0.window_controls()
    }
    fn set_client_inset(&self, inset: Pixels) {
        self.0.set_client_inset(inset)
    }
    fn set_client_corner_radii(&self, radii: Corners<Pixels>) {
        self.0.set_client_corner_radii(radii)
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0.gpu_specs()
    }
    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.0.update_ime_position(bounds)
    }
    fn insets(&self) -> WindowInsets {
        self.0.insets()
    }
    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.0.on_insets_changed(callback)
    }
    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        self.0.set_back_handler(callback)
    }
    fn set_back_enabled(&self, enabled: bool) {
        self.0.set_back_enabled(enabled)
    }
    fn show_soft_keyboard(&self) {
        self.0.show_soft_keyboard()
    }
    fn hide_soft_keyboard(&self) {
        self.0.hide_soft_keyboard()
    }
    fn text_input_state_changed(&self, change: TextInputStateChange) {
        self.0.text_input_state_changed(change)
    }
    fn play_system_bell(&self) {
        self.0.play_system_bell()
    }
    fn a11y_init(&self, callbacks: A11yCallbacks) {
        self.0.a11y_init(callbacks)
    }
    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        self.0.a11y_tree_update(tree_update)
    }
    fn a11y_update_window_bounds(&self) {
        self.0.a11y_update_window_bounds()
    }
    fn render_to_image(&self, scene: &Scene) -> Result<image::RgbaImage> {
        self.0.render_to_image(scene)
    }
}
