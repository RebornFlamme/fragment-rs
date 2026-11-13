//! Le lecteur de scènes de `fragment-pixels` : il rejoue `pixels/scenes/*.json` sur une vraie
//! fenêtre GPUI et rend l'image capturée.
//!
//! Ce fichier est compilé DEUX fois : par le binaire `fragment-pixels` (`#[path]`) et comme racine
//! de la cible de test `pixels` (voir `Cargo.toml`), de sorte que `cargo test --workspace` exerce le
//! schéma et la vue jouet sans la feature `pixels`. ★ D'où les chemins `super::` seuls : jamais
//! `crate::pixels::…`, qui n'existerait que dans l'un des deux.
//!
//! # Contrat avec les modules de rendu
//! Le `CommandManager` de la scène est garni par `fragment_ui::scene::commandes_de_scene`. GPUI n'a
//! pas d'arguments de commande ; le lecteur publie donc sur `commandes.events()` :
//! - `pixels:args` (une `serde_json::Value`) juste AVANT une `command` qui a des `args` ;
//! - `pixels:layout` (la `Value` de l'arbre) pour l'action `layout` ;
//! - `pixels:open` (`{"path", "split"}`) pour l'action `open`.
//!
//! Une action `layout` ou `open` que personne n'écoute est une erreur franche, pas un oubli muet.

// ★ Une partie de l'API (champs lus par xtask, helpers de test) n'est pas appelée par le binaire.
#![allow(dead_code)]

pub mod actions;
pub mod capture;
pub mod entree;
pub mod fenetres;

use std::time::Duration;

use anyhow::{Context as _, bail};
use fragment_theme::Mode;
use fragment_ui::theme_global;
use gpui_kit::{AnyWindowHandle, AsyncApp};
use image::RgbaImage;
use serde_json::{Value, json};

use actions::{Action, Cible, Scene, ThemeScene, VueScene};

/// Variable d'environnement du mode de rendu du texte des captures (fr-p4.1.3, mesure).
pub const VAR_MODE_TEXTE: &str = "FRAGMENT_PIXELS_TEXTE";

/// Mode de rendu du texte des captures. Par défaut, le gris. `subpixel` donne le ClearType de
/// GPUI, que Chromium emploie sur les surfaces opaques non composées (docs/mesures/rasterisation-glyphes.md).
pub fn mode_texte(valeur: Option<&str>) -> gpui_kit::TextRenderingMode {
    match valeur.map(str::trim) {
        Some(v) if v.eq_ignore_ascii_case("subpixel") => gpui_kit::TextRenderingMode::Subpixel,
        _ => gpui_kit::TextRenderingMode::Grayscale,
    }
}

/// Délai laissé à la première fenêtre pour poser sa première image stable.
const ATTENTE_INITIALE: Duration = Duration::from_millis(1500);
/// `wait-stable` : une pause après les deux images, comme `calme()` côté Fragment.
const PAUSE_STABLE: Duration = Duration::from_millis(150);
/// Une image à 60 Hz.
const IMAGE: Duration = Duration::from_millis(17);

/// Les fenêtres et le gestionnaire de commandes d'une scène en cours.
struct Session {
    commandes: std::rc::Rc<fragment_core::CommandManager>,
    /// ★ Le manager ne tient son hôte que par un lien faible : on le garde vivant ici.
    _hote: Option<std::rc::Rc<fenetres::HoteScene>>,
    /// L'application, pour une scène `"vue": "app"`.
    app: Option<fenetres::ScenesApp>,
    principale: AnyWindowHandle,
    reglages: Option<AnyWindowHandle>,
}

impl Session {
    fn fenetre(&self, cible: Cible) -> anyhow::Result<AnyWindowHandle> {
        match cible {
            Cible::Main => Ok(self.principale),
            Cible::Settings => self
                .reglages
                .context("`target: settings` sans `settings_window` dans la scène"),
        }
    }
}

fn mode(theme: ThemeScene) -> Mode {
    match theme {
        ThemeScene::Dark => Mode::Sombre,
        ThemeScene::Light => Mode::Clair,
    }
}

/// Rejoue `scene` dans le thème `theme` et rend l'image capturée (la fenêtre de réglages si la
/// scène en déclare une, sinon la fenêtre principale).
///
/// ★ Les fenêtres ne sont PAS fermées : fermer la dernière fait quitter l'application, et les
/// scènes suivantes n'étaient jamais capturées (constaté en CI, 01/11). `render_to_image` rend la
/// scène de CETTE fenêtre : le recouvrement n'y change rien.
pub async fn jouer(
    scene: &Scene,
    theme: ThemeScene,
    cx: &mut AsyncApp,
) -> anyhow::Result<RgbaImage> {
    let session = cx.update(|cx| -> anyhow::Result<Session> {
        // ★ Chromium (le harnais de référence) peint le texte en niveaux de gris ; GPUI choisit le sous-pixel
        // (franges colorées) sur le moteur wgpu. Sans ce réglage, chaque bord de glyphe diffère de 100+.
        // fr-p4.1.3 : `FRAGMENT_PIXELS_TEXTE=subpixel` capture en ClearType, pour la mesure seulement.
        cx.set_text_rendering_mode(mode_texte(std::env::var(VAR_MODE_TEXTE).ok().as_deref()));
        theme_global::installer(cx, mode(theme));
        // ★ L'échelle est un global de l'application, qui survit d'une scène à l'autre : le zoom
        // posé par une scène d'app (`app:zoom-in`, 110 %) agrandissait les surfaces flottantes des
        // scènes suivantes (menu 209 px au lieu de 190, run 43023627987). Chaque scène repart à
        // 100 % ; une scène d'app réinstalle le sien à sa première synchronisation.
        fragment_ui::echelle::Echelle::installer(cx, fragment_core::ZOOM_DEFAUT);
        let (commandes, hote, app, principale) = if scene.vue == VueScene::App {
            let app = fenetres::ouvrir_app(cx, scene.fenetre, theme)?;
            let commandes = app.application.noyau.commandes();
            let principale = app.application.principale;
            (commandes, None, Some(app), principale)
        } else {
            let (commandes, hote) = fenetres::commandes_de_scene();
            let principale = fenetres::ouvrir_principale(cx, scene.vue, scene.fenetre)?;
            (commandes, Some(hote), None, principale)
        };
        let reglages = scene
            .reglages
            .map(|t| fenetres::ouvrir_reglages(cx, t, app.as_ref().map(|a| &*a.application)))
            .transpose()?;
        Ok(Session {
            commandes,
            _hote: hote,
            app,
            principale,
            reglages,
        })
    })?;
    cx.background_executor().timer(ATTENTE_INITIALE).await;
    for (i, action) in scene.actions.iter().enumerate() {
        executer(&session, action, cx).await.with_context(|| {
            format!("scène « {} » : action n°{} ({action:?})", scene.nom, i + 1)
        })?;
    }
    let cible = session.reglages.unwrap_or(session.principale);
    let image = capture::capturer(cible, cx)?;
    capture::verifier(&image, scene, scene.vue == VueScene::Mire)?;
    Ok(image)
}

/// Publie `donnees` sur l'événement `nom`, s'il a au moins un écouteur.
fn publier(session: &Session, nom: &str, donnees: &Value) -> anyhow::Result<()> {
    let evenements = session.commandes.events();
    if evenements.listener_count(nom) == 0 {
        bail!("aucun module n'écoute « {nom} » (le décor correspondant n'est pas encore monté)");
    }
    evenements.trigger(nom, donnees);
    Ok(())
}

async fn executer(session: &Session, action: &Action, cx: &mut AsyncApp) -> anyhow::Result<()> {
    match action {
        Action::Command { id, args } => {
            cx.update(|cx| -> anyhow::Result<()> {
                if session.commandes.find_command(id).is_none() {
                    bail!("commande inconnue « {id} » (aucun module n'a enregistré cet id)");
                }
                if let Some(a) = args {
                    session.commandes.events().trigger("pixels:args", a);
                }
                if !session.commandes.execute_command_by_id(id, None) {
                    bail!("la commande « {id} » n'a pas tourné");
                }
                cx.refresh_windows();
                Ok(())
            })?;
            redessiner(session, cx)
        }
        Action::Open { path, split } => {
            publier(
                session,
                "pixels:open",
                &json!({ "path": path, "split": split }),
            )?;
            redessiner(session, cx)
        }
        Action::Layout { tree } => {
            publier(session, "pixels:layout", tree)?;
            redessiner(session, cx)
        }
        Action::Click {
            x,
            y,
            button,
            count,
            target,
        } => {
            let f = session.fenetre(*target)?;
            cx.update(|cx| {
                f.update(cx, |_, w, cx| {
                    entree::cliquer(w, cx, *x, *y, *button, *count)
                })
            })?;
            Ok(())
        }
        Action::Hover { x, y, target } => {
            let f = session.fenetre(*target)?;
            cx.update(|cx| f.update(cx, |_, w, cx| entree::deplacer(w, cx, *x, *y, None)))?;
            Ok(())
        }
        Action::Key { keys, target } => {
            let f = session.fenetre(*target)?;
            let touche = actions::Touche::parser(keys).map_err(anyhow::Error::msg)?;
            cx.update(|cx| f.update(cx, |_, w, cx| entree::toucher(w, cx, &touche)))?;
            Ok(())
        }
        Action::Type { text, target } => {
            let f = session.fenetre(*target)?;
            cx.update(|cx| f.update(cx, |_, w, cx| entree::taper(w, cx, text)))?;
            Ok(())
        }
        Action::Drag {
            from,
            to,
            release,
            steps,
            button,
            target,
        } => {
            let f = session.fenetre(*target)?;
            cx.update(|cx| {
                f.update(cx, |_, w, cx| {
                    entree::glisser(w, cx, *from, *to, *button, *steps, *release);
                })
            })?;
            Ok(())
        }
        Action::Wait { ms } => {
            cx.background_executor()
                .timer(Duration::from_millis(*ms))
                .await;
            Ok(())
        }
        Action::WaitStable => {
            // ★ Même sens que `calme()` côté Fragment : polices prêtes (elles sont enregistrées au
            // démarrage), deux images, 150 ms, et le pointeur NE BOUGE PAS (le garer est l'affaire
            // de la scène : `hover 2,2`).
            for _ in 0..2 {
                redessiner(session, cx)?;
                cx.background_executor().timer(IMAGE).await;
            }
            cx.background_executor().timer(PAUSE_STABLE).await;
            redessiner(session, cx)
        }
    }
}

/// Peint une image sur chaque fenêtre de la session.
fn redessiner(session: &Session, cx: &mut AsyncApp) -> anyhow::Result<()> {
    let fenetres = [Some(session.principale), session.reglages];
    cx.update(|cx| -> anyhow::Result<()> {
        // Le point de synchronisation de l'application : le zoom posé par une commande s'installe
        // avant l'image, comme après une frappe dans le binaire.
        if let Some(a) = &session.app {
            a.application.synchroniser(cx);
        }
        for f in fenetres.into_iter().flatten() {
            f.update(cx, |_, w, cx| entree::redessiner(w, cx))?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    //! La vue jouet réagit au clic, à la touche et au glisser, sur le TestAppContext de GPUI
    //! (même `dispatch_event` que dans la vraie fenêtre : `entree.rs` ne distingue pas les deux).
    use gpui_kit::{TestAppContext, VisualTestContext};

    use super::actions::{Bouton, Point as P, Touche};
    use super::entree;
    use super::fenetres::{Jouet, pt};
    use gpui_kit::Entity;

    fn jouet(cx: &mut TestAppContext) -> (Entity<Jouet>, &mut VisualTestContext) {
        let (vue, vcx) = cx.add_window_view(Jouet::new);
        vcx.update(entree::redessiner);
        (vue, vcx)
    }

    #[test]
    fn le_mode_texte_reste_en_gris_sauf_subpixel_explicite() {
        use super::mode_texte;
        use gpui_kit::TextRenderingMode as M;
        assert_eq!(mode_texte(None), M::Grayscale);
        assert_eq!(mode_texte(Some("")), M::Grayscale);
        assert_eq!(mode_texte(Some("gris")), M::Grayscale);
        assert_eq!(mode_texte(Some("subpixel")), M::Subpixel);
        assert_eq!(mode_texte(Some(" SubPixel ")), M::Subpixel);
    }

    #[test]
    fn le_jouet_recoit_un_clic_gauche_et_un_clic_droit() {
        let mut cx = TestAppContext::single();
        let (vue, vcx) = jouet(&mut cx);
        vcx.update(|w, cx| {
            entree::cliquer(w, cx, 100.0, 100.0, Bouton::Left, 1);
            entree::cliquer(w, cx, 200.0, 100.0, Bouton::Right, 1);
        });
        vue.read_with(&*vcx, |j, _| {
            assert_eq!((j.appuis, j.relaches, j.appuis_droit), (1, 1, 1));
            assert!(!j.maintenu());
        });
    }

    #[test]
    fn le_jouet_recoit_le_texte_et_les_touches_nommees() {
        let mut cx = TestAppContext::single();
        let (vue, vcx) = jouet(&mut cx);
        vcx.update(|w, cx| {
            entree::taper(w, cx, "Ab c");
            entree::toucher(w, cx, &Touche::parser("Enter").unwrap());
        });
        vue.read_with(&*vcx, |j, _| assert_eq!(j.touches, "Ab c<enter>"));
    }

    #[test]
    fn le_jouet_suit_le_survol() {
        let mut cx = TestAppContext::single();
        let (vue, vcx) = jouet(&mut cx);
        vcx.update(|w, cx| entree::deplacer(w, cx, 321.0, 123.0, None));
        vue.read_with(&*vcx, |j, _| assert_eq!(j.curseur, Some(pt(321.0, 123.0))));
    }

    /// ★ La preuve du `drag` (T1.md, risque 4) : le bouton reste MAINTENU entre l'appui et le
    /// relâchement, le déplacement est continu (une étape par image), et la vue voit chaque point.
    #[test]
    fn drag_garde_le_pointeur_maintenu_entre_deux_etats() {
        let mut cx = TestAppContext::single();
        let (vue, vcx) = jouet(&mut cx);
        let (from, to) = (P { x: 200.0, y: 300.0 }, P { x: 700.0, y: 500.0 });
        // Phase 1 : appui + déplacements, SANS relâcher.
        vcx.update(|w, cx| entree::glisser(w, cx, from, to, Bouton::Left, 8, false));
        vue.read_with(&*vcx, |j, _| {
            assert!(j.maintenu(), "le bouton doit rester enfoncé");
            assert_eq!((j.appuis, j.relaches), (1, 0));
            assert_eq!(j.glisse_vers, Some(pt(700.0, 500.0)));
            // Départ + 8 étapes : la trace est continue, pas un saut.
            assert_eq!(j.trace.len(), 9);
            assert_eq!(j.trace[0], pt(200.0, 300.0));
            assert_eq!(j.trace[4], pt(450.0, 400.0));
        });
        // Phase 2 : relâchement au point d'arrivée.
        vcx.update(|w, cx| entree::relacher(w, cx, to.x, to.y, Bouton::Left, 1));
        vue.read_with(&*vcx, |j, _| {
            assert!(!j.maintenu());
            assert_eq!((j.appuis, j.relaches), (1, 1));
            assert_eq!(j.glisse_vers, Some(pt(700.0, 500.0)));
        });
    }

    #[test]
    fn drag_avec_relachement_va_jusqu_au_bout() {
        let mut cx = TestAppContext::single();
        let (vue, vcx) = jouet(&mut cx);
        let (from, to) = (P { x: 10.0, y: 10.0 }, P { x: 60.0, y: 10.0 });
        vcx.update(|w, cx| entree::glisser(w, cx, from, to, Bouton::Left, 5, true));
        vue.read_with(&*vcx, |j, _| {
            assert!(!j.maintenu());
            assert_eq!((j.appuis, j.relaches), (1, 1));
            assert_eq!(j.trace.len(), 6);
        });
    }
}
