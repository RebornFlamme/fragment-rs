// Port de app/src/ui/core/tooltip.ts et tooltip.css
//! L'infobulle : le texte d'une cible apparaît à côté d'elle après un court délai.
//!
//! Le modèle ([`Tooltip`], [`Survol`]) ne connaît pas GPUI et prend l'horloge en paramètre
//! (`Instant`) : le délai se teste sans attendre. Une SEULE bulle de survol existe ([`Survol`]) ;
//! une [`Tooltip`] créée à la main n'a pas ce câblage et reste jusqu'à `hide`. Une variante change
//! le contenu par [`Tooltip::avec_contenu`] (le `fill()` de la classe d'origine).

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use fragment_core::{Command, CommandManager};
use gpui_kit::{
    Anchor, AnyElement, App, FontWeight, IntoElement, ParentElement as _, Styled as _, anchored,
    deferred, div, point,
};

use crate::echelle::u;
use crate::flottant::{bordure, chaine, ecouter_args, nombre, ombrer, texte};
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::theme_global::{couleur, theme_ou_defaut};

/// Délai avant l'apparition (millisecondes), si la cible n'en fixe pas.
pub const DELAI_DEFAUT_MS: u64 = 300;
/// L'écart entre la cible et la bulle.
pub const ECART: f32 = 6.0;
/// Le retrait minimal entre la bulle et le bord de la fenêtre.
pub const MARGE: f32 = 4.0;
/// Largeur maximale de la bulle.
pub const LARGEUR_MAX: f32 = 280.0;

/// Le côté de la cible où la bulle se pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    /// Sous la cible (défaut).
    #[default]
    Bottom,
    /// À droite.
    Right,
    /// À gauche.
    Left,
    /// Au-dessus.
    Top,
}

impl Placement {
    /// Le nom (`mod-<côté>` côté Fragment).
    pub fn nom(self) -> &'static str {
        match self {
            Self::Bottom => "bottom",
            Self::Right => "right",
            Self::Left => "left",
            Self::Top => "top",
        }
    }

    fn lire(s: &str) -> Self {
        match s {
            "right" => Self::Right,
            "left" => Self::Left,
            "top" => Self::Top,
            _ => Self::Bottom,
        }
    }
}

/// Un rectangle de la fenêtre (pixels logiques).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Bord gauche.
    pub left: f32,
    /// Bord haut.
    pub top: f32,
    /// Largeur.
    pub width: f32,
    /// Hauteur.
    pub height: f32,
}

impl Rect {
    /// Bord droit.
    pub fn right(self) -> f32 {
        self.left + self.width
    }
    /// Bord bas.
    pub fn bottom(self) -> f32 {
        self.top + self.height
    }
}

/// Les options de `set_tooltip`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Le côté ; défaut : dessous.
    pub placement: Option<Placement>,
    /// Le délai (ms) ; défaut : 300.
    pub delai_ms: Option<u64>,
}

/// L'identité d'une cible.
pub type CibleId = u64;

/// Ce que la bulle sait d'une cible (le `aria-label` et le `data-tooltip-*` de l'élément).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cible {
    /// Le texte ('' = pas d'infobulle).
    pub texte: String,
    /// Le côté.
    pub placement: Placement,
    /// Le délai propre, s'il y en a un.
    pub delai_ms: Option<u64>,
    /// Où elle est dans la fenêtre.
    pub rect: Rect,
    /// Est-elle encore dans le document ?
    pub connectee: bool,
}

/// Où poser une bulle de taille `bulle` (largeur, hauteur) à côté de `cible`, dans une fenêtre de
/// taille `fenetre` : arrondie au pixel, bornée à 4 px du bord.
pub fn position(
    cible: Rect,
    bulle: (f32, f32),
    placement: Placement,
    fenetre: (f32, f32),
) -> (f32, f32) {
    let (bw, bh) = bulle;
    let (x, y) = match placement {
        Placement::Right => (cible.right() + ECART, cible.top + (cible.height - bh) / 2.0),
        Placement::Left => (
            cible.left - ECART - bw,
            cible.top + (cible.height - bh) / 2.0,
        ),
        Placement::Top => (
            cible.left + (cible.width - bw) / 2.0,
            cible.top - ECART - bh,
        ),
        Placement::Bottom => (
            cible.left + (cible.width - bw) / 2.0,
            cible.bottom() + ECART,
        ),
    };
    let borne = |v: f32, max: f32| v.min(max).max(MARGE).round();
    (
        borne(x, fenetre.0 - bw - MARGE),
        borne(y, fenetre.1 - bh - MARGE),
    )
}

/// La largeur maximale à droite d'une cible collée au bord : la bulle RACCOURCIT (points de
/// suspension) plutôt que de revenir sur sa cible. `None` hors du placement à droite.
pub fn largeur_max(cible: Rect, placement: Placement, largeur_fenetre: f32) -> Option<f32> {
    (placement == Placement::Right)
        .then(|| (largeur_fenetre - cible.right() - ECART - MARGE).clamp(40.0, LARGEUR_MAX))
}

/// Le contenu d'une bulle pour sa cible (par défaut : le texte de la cible).
type Contenu = Box<dyn Fn(&Cible) -> String>;

/// Une bulle liée à une cible. Sans câblage de survol : elle reste jusqu'à [`Tooltip::hide`].
#[derive(Default)]
pub struct Tooltip {
    sur: Option<CibleId>,
    texte: String,
    placement: Placement,
    contenu: Option<Contenu>,
}

impl Tooltip {
    /// Une bulle vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Une variante : le contenu vient de `contenu` au lieu du texte de la cible.
    pub fn avec_contenu(contenu: impl Fn(&Cible) -> String + 'static) -> Self {
        Self {
            contenu: Some(Box::new(contenu)),
            ..Self::default()
        }
    }

    /// Montre la bulle à côté de `cible` (contenu relu). Une cible retirée la masque.
    pub fn show(&mut self, id: CibleId, cible: &Cible, placement: Option<Placement>) {
        if !cible.connectee {
            return self.hide();
        }
        self.sur = Some(id);
        self.placement = placement.unwrap_or(self.placement);
        self.texte = match &self.contenu {
            Some(f) => f(cible),
            None => cible.texte.clone(),
        };
    }

    /// Masque la bulle.
    pub fn hide(&mut self) {
        self.sur = None;
    }

    /// La cible actuelle (`None` si masquée).
    pub fn shown_on(&self) -> Option<CibleId> {
        self.sur
    }

    /// Le texte affiché.
    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// Le côté.
    pub fn placement(&self) -> Placement {
        self.placement
    }
}

/// Le survol : les cibles qui ont une infobulle, UNE bulle partagée et un minuteur.
#[derive(Default)]
pub struct Survol {
    cibles: HashMap<CibleId, Cible>,
    bulle: Tooltip,
    minuteur: Option<(CibleId, Instant)>,
}

impl Survol {
    /// Un survol sans cible.
    pub fn new() -> Self {
        Self::default()
    }

    /// Déclare une cible (dans le document, à `rect`).
    pub fn ajouter_cible(&mut self, id: CibleId, rect: Rect) {
        self.cibles.insert(
            id,
            Cible {
                rect,
                connectee: true,
                ..Cible::default()
            },
        );
    }

    /// La cible quitte le document.
    pub fn retirer(&mut self, id: CibleId) {
        if let Some(c) = self.cibles.get_mut(&id) {
            c.connectee = false;
        }
    }

    /// La cible `id`.
    pub fn cible(&self, id: CibleId) -> Option<&Cible> {
        self.cibles.get(&id)
    }

    /// Pose (ou change) l'infobulle d'une cible. Texte vide : plus d'infobulle. Le côté revient
    /// au défaut si `options` n'en donne pas. Affichée : la bulle suit le nouveau texte.
    pub fn set_tooltip(&mut self, id: CibleId, tooltip: &str, options: Options) {
        let Some(c) = self.cibles.get_mut(&id) else {
            return;
        };
        c.texte = tooltip.to_owned();
        c.placement = options.placement.unwrap_or_default();
        if options.delai_ms.is_some() {
            c.delai_ms = options.delai_ms;
        }
        if self.bulle.shown_on() == Some(id) {
            self.afficher(id);
        }
    }

    /// Montre tout de suite l'infobulle de `id` (texte = celui de la cible, sauf `tooltip`).
    pub fn display_tooltip(&mut self, id: CibleId, tooltip: Option<&str>, options: Options) {
        if tooltip.is_some() || options.placement.is_some() {
            let texte = tooltip
                .map(str::to_owned)
                .or_else(|| self.cibles.get(&id).map(|c| c.texte.clone()))
                .unwrap_or_default();
            self.set_tooltip(id, &texte, options);
        }
        self.afficher(id);
    }

    fn afficher(&mut self, id: CibleId) {
        match self.cibles.get(&id) {
            Some(c) if !c.texte.is_empty() && c.connectee => {
                let (c, placement) = (c.clone(), c.placement);
                self.bulle.show(id, &c, Some(placement));
            }
            _ => self.masquer(),
        }
    }

    fn masquer(&mut self) {
        self.minuteur = None;
        self.bulle.hide();
    }

    /// Le pointeur entre sur la cible : la bulle apparaîtra après son délai.
    pub fn pointer_enter(&mut self, id: CibleId, maintenant: Instant) {
        let delai = self
            .cibles
            .get(&id)
            .and_then(|c| c.delai_ms)
            .unwrap_or(DELAI_DEFAUT_MS);
        self.minuteur = Some((id, maintenant + Duration::from_millis(delai)));
    }

    /// L'horloge avance : montre la bulle si le délai est écoulé.
    pub fn poll(&mut self, maintenant: Instant) {
        if let Some((id, echeance)) = self.minuteur
            && maintenant >= echeance
        {
            self.minuteur = None;
            self.afficher(id);
        }
    }

    /// Le pointeur quitte la cible.
    pub fn pointer_leave(&mut self) {
        self.masquer();
    }

    /// Le pointeur appuie : la bulle part.
    pub fn pointer_down(&mut self) {
        self.masquer();
    }

    /// La bulle de survol.
    pub fn bulle(&self) -> &Tooltip {
        &self.bulle
    }
}

thread_local! {
    static SURVOL: RefCell<Survol> = RefCell::new(Survol::new());
}

/// Travaille sur le survol du fil (celui que [`couche`] dessine).
pub fn avec_survol<R>(f: impl FnOnce(&mut Survol) -> R) -> R {
    SURVOL.with(|s| f(&mut s.borrow_mut()))
}

/// Enregistre la commande de scène `pixels:tooltip`
/// (`{ "x", "y", "w", "h", "text", "placement" }`) : l'infobulle d'une cible invisible.
pub fn commandes_de_scene(commandes: &CommandManager) {
    // Chaque scène (et chaque thème) repart d'un magasin vide : le processus rejoue toutes les scènes.
    avec_survol(|s| *s = Survol::new());
    let args = ecouter_args(commandes);
    let commande =
        Command::new("pixels:tooltip", "Pixels : montrer l'infobulle").with_callback(move || {
            let a = args.borrow();
            let rect = Rect {
                left: nombre(&a, "x").unwrap_or(300.0),
                top: nombre(&a, "y").unwrap_or(200.0),
                width: nombre(&a, "w").unwrap_or(40.0),
                height: nombre(&a, "h").unwrap_or(20.0),
            };
            let placement = chaine(&a, "placement").map_or(Placement::Bottom, Placement::lire);
            let texte = chaine(&a, "text").unwrap_or("Calques");
            avec_survol(|s| {
                s.ajouter_cible(1, rect);
                s.display_tooltip(
                    1,
                    Some(texte),
                    Options {
                        placement: Some(placement),
                        delai_ms: None,
                    },
                );
            });
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La couche de l'infobulle : rien si la bulle de survol est masquée.
pub fn couche(cx: &App) -> AnyElement {
    let rendu = avec_survol(|s| {
        let id = s.bulle().shown_on()?;
        let c = s.cible(id)?;
        Some((c.rect, s.bulle().placement(), s.bulle().texte().to_owned()))
    });
    let Some((rect, placement, message)) = rendu else {
        return div().into_any_element();
    };
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let fenetre_w = 1280.0;
    let largeur = largeur_max(rect, placement, fenetre_w).unwrap_or(LARGEUR_MAX);
    // ★ `fr-t1.3` : une bulle qui tient se peint à la largeur ENTIÈRE de Chromium (texte exact + rembourrage +
    // bordures, arrondi), sans points de suspension ; GPUI arrondirait la feuille de texte au pixel supérieur.
    let ajout = 2.0 * f.size_4_2 + 2.0 * f.border_width;
    let texte_exact = largeur_texte(cx, &message, FontWeight::NORMAL, t.typo.font_ui_small);
    let tient = texte_exact + ajout <= largeur;
    // Chromium pose `left = Math.round(x)` (tooltip.ts) avec la largeur FRACTIONNAIRE de la bulle : on pose
    // l'arête gauche soi-même, entière, au lieu de laisser GPUI centrer une largeur entière.
    let gauche = |centre: f32| (centre - (texte_exact + ajout) / 2.0 + 0.5 + 1e-3).floor();
    let (ancre, x, y) = match placement {
        Placement::Bottom if tient => (
            Anchor::TopLeft,
            gauche(rect.left + rect.width / 2.0),
            rect.bottom() + ECART,
        ),
        Placement::Top if tient => (
            Anchor::BottomLeft,
            gauche(rect.left + rect.width / 2.0),
            rect.top - ECART,
        ),
        Placement::Bottom => (
            Anchor::TopCenter,
            rect.left + rect.width / 2.0,
            rect.bottom() + ECART,
        ),
        Placement::Top => (
            Anchor::BottomCenter,
            rect.left + rect.width / 2.0,
            rect.top - ECART,
        ),
        Placement::Right => (
            Anchor::LeftCenter,
            rect.right() + ECART,
            rect.top + rect.height / 2.0,
        ),
        Placement::Left => (
            Anchor::RightCenter,
            rect.left - ECART,
            rect.top + rect.height / 2.0,
        ),
    };
    let bulle = div()
        .max_w(u(cx, largeur))
        .px(u(cx, f.size_4_2))
        .py(u(cx, f.size_4_1))
        .rounded(u(cx, f.radius_sm))
        .bg(couleur(c.menu_bg))
        .text_color(couleur(c.text_normal))
        .whitespace_nowrap()
        .child(message);
    let bulle = if tient {
        bulle.w(largeur_ajustee(cx, texte_exact, ajout))
    } else {
        bulle.overflow_hidden().text_ellipsis()
    };
    let bulle =
        bordure(bulle, u(cx, f.border_width)).border_color(couleur(c.background_modifier_border));
    let bulle = ombrer(
        bulle,
        cx,
        &c.menu_shadow,
        u(cx, f.radius_sm),
        u(cx, f.border_width),
    );
    deferred(
        anchored()
            .anchor(ancre)
            .position(point(u(cx, x), u(cx, y)))
            .snap_to_window_with_margin(u(cx, MARGE))
            .child(texte(bulle, cx, t.typo.font_ui_small, 1.35)),
    )
    .into_any_element()
}
