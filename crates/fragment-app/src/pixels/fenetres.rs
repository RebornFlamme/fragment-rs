//! Les fenêtres des scènes et les vues de contrôle : la mire, le jouet, la fenêtre de réglages vide.
//!
//! ★ Couleurs littérales assumées pour `Mire` et `Jouet` : ce sont des vues de TEST, hors thème par
//! nature (comme la mire de la phase 0). Les vraies vues lisent les jetons de `fragment-theme`.

use std::rc::{Rc, Weak};

use fragment_core::{
    CommandHost, CommandManager, EditorContext, HotkeyManager, InputEvent as EvenementCoeur,
};
use fragment_ui::theme_global;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Pixels, Point, Render, Styled as _, Window, WindowOptions, div, point, px,
    rgb,
};

use super::actions::{Taille, ThemeScene, VueScene};

/// Mire de contrôle : aplats, trait de 1 px, rayon, texte. Elle prouve que le rasterizeur
/// (WARP sur le runner) dessine vraiment quads et glyphes : une fenêtre vide n'a qu'une couleur.
pub struct Mire;

impl Render for Mire {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .p(px(40.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .w(px(200.0))
                    .h(px(80.0))
                    .bg(rgb(0x7c3aed))
                    .rounded(px(12.0)),
            )
            .child(div().w(px(300.0)).h(px(1.0)).bg(rgb(0xffffff)))
            .child(
                div()
                    .text_color(rgb(0xdcddde))
                    .text_size(px(24.0))
                    .child("Fragment — mire 0123456789"),
            )
    }
}

/// La vue jouet : elle retient ce qu'on lui a fait (clics, frappes, glisser) et le DESSINE, de sorte
/// qu'une capture prouve que l'entrée simulée est arrivée jusqu'à une vraie vue GPUI.
///
/// ★ Le glisser est la partie délicate (T1.md, risque 4) : `appui` reste renseigné tant que le
/// bouton est maintenu, et le carré suit `glisse_vers`. Capturer ENTRE l'appui et le relâchement
/// montre un carré orange (maintenu) ; après le relâchement, un carré vert.
pub struct Jouet {
    focus: FocusHandle,
    /// Nombre d'appuis du bouton gauche reçus.
    pub appuis: usize,
    /// Nombre de relâchements consécutifs à un appui.
    pub relaches: usize,
    /// Appuis du bouton droit.
    pub appuis_droit: usize,
    /// Le texte reçu au clavier (les touches nommées s'écrivent `<enter>`).
    pub touches: String,
    /// Le dernier point survolé.
    pub curseur: Option<Point<Pixels>>,
    /// Où le bouton a été enfoncé ; `Some` tant qu'il est maintenu.
    pub appui: Option<Point<Pixels>>,
    /// Le dernier point atteint par le glisser.
    pub glisse_vers: Option<Point<Pixels>>,
    /// Les points successifs d'un glisser en cours (pour prouver la continuité).
    pub trace: Vec<Point<Pixels>>,
}

impl Jouet {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            focus,
            appuis: 0,
            relaches: 0,
            appuis_droit: 0,
            touches: String::new(),
            curseur: None,
            appui: None,
            glisse_vers: None,
            trace: Vec::new(),
        }
    }

    /// Vrai si le bouton est maintenu.
    pub fn maintenu(&self) -> bool {
        self.appui.is_some()
    }
}

impl Render for Jouet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut racine = div()
            .id("jouet")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xdcddde))
            .text_size(px(20.0))
            .p(px(24.0))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                // ★ GPUI renseigne `key_char` même pour Entrée (« \n ») : on ne garde le texte que
                // pour une vraie touche de caractère (ou espace), les autres s'écrivent <nom>.
                let k = &ev.keystroke;
                match &k.key_char {
                    Some(c) if k.key.chars().count() == 1 || k.key == "space" => {
                        this.touches.push_str(c)
                    }
                    _ => this.touches.push_str(&format!("<{}>", k.key)),
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    this.appuis += 1;
                    this.appui = Some(ev.position);
                    this.glisse_vers = Some(ev.position);
                    this.trace = vec![ev.position];
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.appuis_droit += 1;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                this.curseur = Some(ev.position);
                if ev.pressed_button == Some(MouseButton::Left) && this.appui.is_some() {
                    this.glisse_vers = Some(ev.position);
                    this.trace.push(ev.position);
                }
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseUpEvent, _, cx| {
                    if this.appui.take().is_some() {
                        this.relaches += 1;
                        this.glisse_vers = Some(ev.position);
                    }
                    cx.notify();
                }),
            )
            .child(format!(
                "appuis {} · relâchés {} · droit {} · touches « {} »",
                self.appuis, self.relaches, self.appuis_droit, self.touches
            ));
        if let Some(p) = self.glisse_vers {
            let couleur = if self.maintenu() { 0xf59e0b } else { 0x22c55e };
            racine = racine.child(
                div()
                    .absolute()
                    .left(p.x - px(24.0))
                    .top(p.y - px(24.0))
                    .w(px(48.0))
                    .h(px(48.0))
                    .rounded(px(8.0))
                    .bg(rgb(couleur)),
            );
        }
        racine
    }
}

/// Le contenu de la fenêtre de réglages tant que la tâche des réglages ne l'a pas remplacé :
/// un fond de thème, rien d'autre.
pub struct Vide;

impl Render for Vide {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fond =
            theme_global::couleur(theme_global::theme_ou_defaut(cx).palette.background_primary);
        div().size_full().bg(fond)
    }
}

/// L'hôte minimal du `CommandManager` d'une scène : pas d'éditeur, pas de raccourcis.
pub struct HoteScene;

impl CommandHost for HoteScene {
    fn set_last_event(&self, _evt: Option<EvenementCoeur>) {}
    fn active_editor(&self) -> Option<EditorContext> {
        None
    }
    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
        None
    }
}

/// Un `CommandManager` neuf, garni des commandes `pixels:*` de tous les modules de rendu.
/// Rend aussi l'hôte, que l'appelant doit garder vivant (le manager n'en tient qu'un lien faible).
pub fn commandes_de_scene() -> (Rc<CommandManager>, Rc<HoteScene>) {
    let hote = Rc::new(HoteScene);
    let hote_dyn: Rc<dyn CommandHost> = hote.clone();
    let faible: Weak<dyn CommandHost> = Rc::downgrade(&hote_dyn);
    let commandes = CommandManager::new(faible);
    fragment_ui::scene::commandes_de_scene(&commandes);
    (commandes, hote)
}

/// Options d'une fenêtre de scène (centrée, titrée « Fragment »).
pub fn options(cx: &App, taille: Taille) -> WindowOptions {
    fragment_app::options_fenetre(
        cx,
        fragment_app::TITRE,
        taille.width as f32,
        taille.height as f32,
    )
}

/// La version que montrent les références (le build de dev de Fragment) : un réglage de la scène,
/// comme les dates figées, pas la version de fragment-rs.
pub const VERSION_DES_REFERENCES: &str = "0.0.0";

/// Ce qu'une scène `"vue": "app"` monte : l'application entière, en mémoire.
pub struct ScenesApp {
    pub application: Rc<fragment_app::Application>,
    /// Les abonnements de scène (`pixels:layout`, `pixels:epingler`…), vivants avec la scène.
    _abonnements: Vec<fragment_core::EventRef>,
}

/// Démarre l'application sur `options` (stockages en mémoire, version des références, coffre de la
/// scène s'il y en a un : voir `super::coffre_de_scene`) dans une fenêtre de `taille`, et branche les actions de scène sur SON workspace et SES commandes.
///
/// ★ Les commandes `pixels:*` de la coquille (onglets) sont enregistrées dans le gestionnaire de
///   l'application, pas dans un gestionnaire à part : une scène d'app exécute les VRAIES commandes
///   (`app:zoom-in`). Les fixtures de la palette (`palette::commandes_de_scene`) n'y vont pas : elles
///   doublonneraient les commandes `app:*` réelles.
pub fn ouvrir_app(
    cx: &mut App,
    taille: Taille,
    theme: ThemeScene,
    options: &fragment_app::boot::Options,
) -> anyhow::Result<ScenesApp> {
    // ★ Les surfaces flottantes vivent dans des magasins de fil (menus, notices, infobulle) que
    // chaque `commandes_de_scene` vide : un gestionnaire jetable les remet à zéro, sans quoi le menu
    // d'onglet d'une scène resterait ouvert dans la suivante.
    drop(commandes_de_scene());
    let application = fragment_app::monter(cx, options, taille.width as f32, taille.height as f32)?;
    // Le harnais de Fragment fixe le schéma de couleurs de la scène (`baseColorScheme`), que montre
    // l'onglet Appearance : « Dark » ou « Light », pas « Adapt to system ».
    application.noyau.apparence.set_schema(match theme {
        ThemeScene::Dark => "dark",
        ThemeScene::Light => "light",
    });
    let commandes = application.noyau.commandes();
    fragment_ui::onglets::commandes_de_scene(&commandes);
    let ws = fragment_core::App::workspace(&*application.noyau.app).clone();
    let mut abonnements = fragment_ui::onglets::brancher_scene(&commandes, &ws);
    abonnements.push(fragment_ui::splits::brancher_scene(&commandes, &ws));
    Ok(ScenesApp {
        application,
        _abonnements: abonnements,
    })
}

/// Ouvre la fenêtre principale d'une scène dont la vue n'est PAS l'application.
pub fn ouvrir_principale(
    cx: &mut App,
    vue: VueScene,
    taille: Taille,
) -> anyhow::Result<AnyWindowHandle> {
    let opts = options(cx, taille);
    let fenetre = match vue {
        VueScene::App => anyhow::bail!("la vue « app » s'ouvre par `ouvrir_app`"),
        VueScene::Mire => gpui_kit::open_window(opts, cx, |_, cx| cx.new(|_| Mire))?.0,
        VueScene::Jouet => {
            gpui_kit::open_window(opts, cx, |window, cx| cx.new(|cx| Jouet::new(window, cx)))?.0
        }
        VueScene::Flottantes => {
            gpui_kit::open_window(opts, cx, |_, cx| cx.new(|_| fragment_ui::flottant::Decor))?.0
        }
    };
    Ok(fenetre)
}

/// Ouvre la fenêtre de réglages (2e fenêtre, 1000 × 700 dans les scènes de Fragment) : celle de
/// l'application si la scène en a une, sinon un fond de thème.
pub fn ouvrir_reglages(
    cx: &mut App,
    taille: Taille,
    application: Option<&fragment_app::Application>,
) -> anyhow::Result<AnyWindowHandle> {
    if let Some(a) = application {
        return a.ouvrir_reglages(cx, (taille.width as f32, taille.height as f32));
    }
    let opts = options(cx, taille);
    let (fenetre, _) = gpui_kit::open_window(opts, cx, |_, cx| cx.new(|_| Vide))?;
    Ok(fenetre)
}

/// Un point logique, pour les tests.
pub fn pt(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}
