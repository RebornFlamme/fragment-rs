// Port de app/src/ui/theme/ThemePlugin.ts (la partie qui touche l'écran : poser la feuille du
// thème, la garder en dernier, redessiner)
//
//! Les thèmes du coffre à l'écran : le [`ThemesCoffre`] (fragment-theme) calcule le `Theme` d'un
//! mode ; ce module le range dans le `Global` GPUI et redessine les fenêtres, sans rechargement.
//!
//! ★ Un changement (réglage, `theme.json` édité, thème retiré) arrive par `on_change`, qui n'a
//!   pas de `cx` : il lève un drapeau, qu'une tâche de fond lit toutes les [`PERIODE_MS`] pour
//!   réinstaller le thème. La même tâche donne la main à `sonder()` (le regroupement des
//!   événements du dossier des thèmes a son horloge dans le cœur, pas de minuteur).
//! ★ Le mode (sombre / clair) reste celui du global courant : le schéma de couleurs n'est pas
//!   l'affaire de ce module. Pour changer de mode SANS perdre le thème du coffre, passer par
//!   [`basculer_avec_themes`] (`theme_global::basculer` repart de la palette de Fragment).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use fragment_core::CommandManager;
use fragment_theme::{Mode, ThemeManager, ThemesCoffre};
use gpui_kit::App;

use crate::theme_global::ThemeGlobal;

/// Le rythme de la tâche de fond : au moins un tour par regroupement (50 ms).
pub const PERIODE_MS: u64 = 50;

/// Enregistre les commandes de scène `pixels:themes` : vide jusqu'à la tâche des scènes.
pub fn commandes_de_scene(_commandes: &CommandManager) {}

/// Le mode du thème actuellement installé (sombre si aucun).
fn mode_courant(cx: &App) -> Mode {
    cx.try_global::<ThemeGlobal>()
        .map_or(Mode::Sombre, |t| t.0.mode)
}

/// Installe le thème du coffre pour `mode` dans le global et redessine les fenêtres.
pub fn installer_theme_du_coffre(cx: &mut App, themes: &ThemesCoffre, mode: Mode) {
    cx.set_global(ThemeGlobal(themes.theme(mode)));
    cx.refresh_windows();
}

/// Réinstalle le thème du coffre dans le mode courant (un `theme.json` ou l'accent a changé).
pub fn appliquer_au_global(cx: &mut App, themes: &ThemesCoffre) {
    installer_theme_du_coffre(cx, themes, mode_courant(cx));
}

/// Passe au `mode` donné en gardant le thème du coffre. Rien à faire si c'est déjà le mode.
pub fn basculer_avec_themes(cx: &mut App, themes: &ThemesCoffre, mode: Mode) {
    if cx
        .try_global::<ThemeGlobal>()
        .is_some_and(|t| t.0.mode == mode)
    {
        return;
    }
    installer_theme_du_coffre(cx, themes, mode);
}

/// Monte le plugin : installe le thème du coffre, puis le tient à jour (tâche de fond).
///
/// La tâche s'arrête seule quand le dernier `Rc` du [`ThemesCoffre`] est lâché.
pub fn monter(cx: &mut App, themes: &Rc<ThemesCoffre>) {
    themes.appliquer();
    // Échec de lecture : la liste reste vide, ce n'est pas une panne (`refresh` du contrat).
    let _ = themes.refresh();
    appliquer_au_global(cx, themes);

    let sale = Rc::new(Cell::new(false));
    let drapeau = sale.clone();
    // ★ Le désabonnement n'est pas rendu : l'abonné vit autant que le plugin, et le drapeau est
    //   inoffensif une fois la tâche arrêtée.
    let _desabonner = themes.on_change(Box::new(move || drapeau.set(true)));
    let faible = Rc::downgrade(themes);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(PERIODE_MS))
                .await;
            let Some(themes) = faible.upgrade() else {
                break;
            };
            themes.sonder();
            if sale.replace(false) {
                cx.update(|cx| appliquer_au_global(cx, &themes));
            }
        }
    })
    .detach();
}
