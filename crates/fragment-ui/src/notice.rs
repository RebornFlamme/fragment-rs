// Port de app/src/ui/core/Notice.ts et notice.css
//! Les notices : de petits toasts empilés en haut à droite, qui s'effacent seuls (ou au clic).
//!
//! Même contrat qu'Obsidian : `Notice::new(message, durée)`, `set_message`, `hide`. Une durée de
//! `0` rend la notice persistante (elle ne part qu'au clic) ; le défaut est de cinq secondes.
//!
//! ★ Écart assumé : Fragment fait glisser la notice vers la droite avant de la retirer (transition
//! de 400 ms) ; ici elle disparaît à l'image suivante, comme sous `prefers-reduced-motion`, régime
//! dans lequel les captures de référence sont prises.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use fragment_core::{Command, CommandManager};
use gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div,
};

use crate::echelle::u;
use crate::flottant::{bordure, chaine, ecouter_args, nombre, ombres, texte};
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::theme_global::{couleur, theme_ou_defaut};

/// La durée par défaut d'une notice (millisecondes), comme Obsidian.
pub const DUREE_DEFAUT_MS: u64 = 5000;

/// Largeur maximale du texte d'une notice (`max-width`, boîte de contenu).
const LARGEUR_MAX: f32 = 340.0;

/// Une notice affichée : son rang dans la pile est son ordre d'arrivée.
#[derive(Debug, Clone)]
struct Entree {
    id: u64,
    message: String,
    /// `None` : persistante.
    expire: Option<Instant>,
}

thread_local! {
    static PILE: RefCell<Vec<Entree>> = const { RefCell::new(Vec::new()) };
    static COMPTEUR: Cell<u64> = const { Cell::new(0) };
}

/// Une poignée sur une notice (comme l'objet `Notice` d'Obsidian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notice {
    id: u64,
}

impl Notice {
    /// Pose une notice. `duree_ms` : millisecondes avant disparition, `0` = persistante.
    ///
    /// Ne programme aucune échéance : [`Notice::purger`] retire ce qui a expiré, et
    /// [`Notice::afficher`] programme le réveil de la fenêtre.
    pub fn new(message: &str, duree_ms: u64) -> Self {
        let id = COMPTEUR.with(|c| {
            c.set(c.get() + 1);
            c.get()
        });
        let expire = (duree_ms > 0).then(|| Instant::now() + Duration::from_millis(duree_ms));
        PILE.with(|p| {
            p.borrow_mut().push(Entree {
                id,
                message: message.to_owned(),
                expire,
            });
        });
        Self { id }
    }

    /// Pose une notice, redessine les fenêtres et, si elle n'est pas persistante, programme sa
    /// disparition.
    pub fn afficher(cx: &mut App, message: &str, duree_ms: u64) -> Self {
        let notice = Self::new(message, duree_ms);
        if duree_ms > 0 {
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(duree_ms))
                    .await;
                notice.hide();
                cx.update(|cx| cx.refresh_windows());
            })
            .detach();
        }
        cx.refresh_windows();
        notice
    }

    /// Remplace le texte affiché. Sans effet si la notice est déjà fermée.
    pub fn set_message(&self, message: &str) -> &Self {
        PILE.with(|p| {
            if let Some(e) = p.borrow_mut().iter_mut().find(|e| e.id == self.id) {
                e.message = message.to_owned();
            }
        });
        self
    }

    /// Ferme la notice (idempotent).
    pub fn hide(&self) {
        PILE.with(|p| p.borrow_mut().retain(|e| e.id != self.id));
    }

    /// Cette notice est-elle encore affichée ?
    pub fn est_affichee(&self) -> bool {
        PILE.with(|p| p.borrow().iter().any(|e| e.id == self.id))
    }

    /// Les messages affichés, du plus ancien au plus récent (de haut en bas).
    pub fn messages() -> Vec<String> {
        PILE.with(|p| p.borrow().iter().map(|e| e.message.clone()).collect())
    }

    /// Retire les notices dont l'échéance est passée à `maintenant`.
    pub fn purger(maintenant: Instant) {
        PILE.with(|p| {
            p.borrow_mut()
                .retain(|e| e.expire.is_none_or(|t| t > maintenant));
        });
    }

    /// Ferme toutes les notices.
    pub fn tout_fermer() {
        PILE.with(|p| p.borrow_mut().clear());
    }
}

/// Enregistre la commande de scène `pixels:notice` (`{ "text": …, "duration": ms }`).
pub fn commandes_de_scene(commandes: &CommandManager) {
    // Chaque scène (et chaque thème) repart d'un magasin vide : le processus rejoue toutes les scènes.
    Notice::tout_fermer();
    let args = ecouter_args(commandes);
    let commande =
        Command::new("pixels:notice", "Pixels : poser une notice").with_callback(move || {
            let a = args.borrow();
            let texte = chaine(&a, "text").unwrap_or("Une notice de test");
            let duree = nombre(&a, "duration").unwrap_or(0.0).max(0.0) as u64;
            // Les deux thèmes d'une scène se jouent dans le même processus : la pile repart vide.
            if a.get("reset").and_then(serde_json::Value::as_bool) == Some(true) {
                Notice::tout_fermer();
            }
            Notice::new(texte, duree);
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La couche des notices : un conteneur en haut à droite, une carte par notice.
pub fn couche(cx: &App) -> AnyElement {
    Notice::purger(Instant::now());
    let entrees: Vec<Entree> = PILE.with(|p| p.borrow().clone());
    if entrees.is_empty() {
        return div().into_any_element();
    }
    let t = theme_ou_defaut(cx);
    let c = &t.palette;
    let f = &t.forme;
    // `max-width: 340px` s'applique à la boîte de CONTENU : on y ajoute rembourrage et bordure.
    let largeur_max = LARGEUR_MAX + 2.0 * f.size_2_7 + 2.0 * f.border_width;
    let cartes: Vec<AnyElement> = entrees
        .into_iter()
        .map(|e| {
            let id = e.id;
            // ★ `fr-t1.3` : une notice d'une ligne qui tient se peint à la largeur ENTIÈRE de Chromium (texte
            // exact + rembourrage + bordures, arrondi) ; GPUI arrondirait la feuille au pixel supérieur.
            // Sans retour à la ligne, la boîte est posée en `nowrap` (le texte dépasse de 0,3 px au plus).
            let texte_exact =
                largeur_texte(cx, &e.message, FontWeight::NORMAL, t.typo.font_ui_small);
            let tient = !e.message.contains('\n') && texte_exact <= LARGEUR_MAX;
            let carte = div()
                .id(("notice", id))
                .max_w(u(cx, largeur_max))
                .px(u(cx, f.size_2_7))
                // ★ Chromium cale la ligne de 16,8 px (12 × 1,4) vers le haut ; GPUI l'arrondit à 17 et la
                // pose un pixel plus bas. Un pixel de rembourrage passe donc du haut vers le bas : la
                // carte garde sa hauteur, le texte remonte.
                .pt(u(cx, f.size_2_5 - 1.0))
                .pb(u(cx, f.size_2_5 + 1.0))
                .rounded(u(cx, f.radius_m))
                .bg(couleur(c.background_secondary_alt))
                .text_color(couleur(c.text_normal))
                .shadow(ombres(cx, &c.shadow_s))
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, _: &mut Window, app: &mut App| {
                    Notice { id }.hide();
                    app.refresh_windows();
                })
                .child(e.message);
            let carte = if tient {
                carte.whitespace_nowrap().w(largeur_ajustee(
                    cx,
                    texte_exact,
                    2.0 * f.size_2_7 + 2.0 * f.border_width,
                ))
            } else {
                carte
            };
            let carte = bordure(carte, u(cx, f.border_width))
                .border_color(couleur(c.background_modifier_border));
            texte(carte, cx, t.typo.font_ui_small, 1.4).into_any_element()
        })
        .collect();
    // Le conteneur laisse passer les clics : il n'a ni fond ni gestionnaire.
    div()
        .absolute()
        .top(u(cx, 0.0))
        .right(u(cx, 0.0))
        .flex()
        .flex_col()
        .items_end()
        .gap(u(cx, f.size_4_2))
        .p(u(cx, f.size_4_4))
        .children(cartes)
        .into_any_element()
}
