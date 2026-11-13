// Port de app/src/ui/coreCommands/marginMode.ts (la classe `is-margin-mode` posée sur la feuille) et
// de marginMode.css (la réserve de 25 % ou 40 %, sa transition de 90 ms).
//
//! L'état de la marge de lecture, PAR FEUILLE et non persisté, et la réserve qu'il donne aux vues.
//!
//! Fragment pose une classe sur la feuille et laisse le CSS faire la transition. Ici l'état tient la
//! même chose : ouverte ou non, et l'avancement animé de la réserve. Une vue lit, à chaque image,
//! [`MargeDeLecture::reserve`] pour SA feuille et SON genre ([`Genre::Note`], [`Genre::Pdf`]) ; tant
//! que la réserve est en transition, elle redemande une image.
//!
//! ★ La réserve est celle de la FEUILLE, pas de ce qu'elle héberge : une vue embarquée ou dans un
//!   widget vit dans une AUTRE feuille et lit la sienne, jamais ouverte (les commandes refusent une
//!   feuille sans parent). Dans Fragment il fallait l'exclure du sélecteur (bug « le swipe
//!   s'applique aux vues embed ») ; ici la clé par feuille l'exclut d'office.
//! ★ Aller = retour : la même courbe et la même durée dans les deux sens, partant de la valeur
//!   COURANTE (un retour en pleine transition repart d'où il en est, sans saut). Dans Fragment, le
//!   retour « snappait » faute de `border-box` dans les deux états.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_core::{EventRef, Events, ItemId};
use fragment_theme::Theme;

use crate::clavier::balayage::{Horloge, horloge_reelle};

/// L'événement émis quand l'état d'une feuille change (donnée : son [`ItemId`]).
pub const CHANGEMENT: &str = "margin-change";

/// Ce que montre la vue : la réserve n'est pas la même.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Une note (ou toute vue à colonne) : 25 %, animée.
    Note,
    /// Un PDF : 40 % (il a déjà ses marges, demande de l'utilisateur), SANS transition (chaque image
    /// relancerait le zoom « ajusté »).
    Pdf,
}

impl Genre {
    /// `padding-right` de la zone qui défile en mode marge (marginMode.css, lignes 21 et 27).
    pub fn fraction(self) -> f32 {
        match self {
            Genre::Note => 0.25,
            Genre::Pdf => 0.40,
        }
    }

    fn anime(self) -> bool {
        matches!(self, Genre::Note)
    }
}

/// La réserve d'une feuille à cet instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reserve {
    /// La fraction de la largeur de la zone à réserver à droite.
    pub fraction: f32,
    /// Vrai tant que la transition court : la vue redemande une image.
    pub en_transition: bool,
}

/// L'avancement de la marge d'une feuille : 0 = fermée, 1 = ouverte.
#[derive(Clone, Copy, Debug)]
struct Feuille {
    ouverte: bool,
    /// D'où part la transition (l'avancement au moment du changement).
    depart: f32,
    /// Quand elle part (ms de l'horloge).
    debut: f64,
}

/// L'état de la marge de toutes les feuilles d'une fenêtre.
pub struct MargeDeLecture {
    feuilles: RefCell<HashMap<ItemId, Feuille>>,
    /// `--anim-duration-quick` (ms).
    duree: f64,
    /// `--anim-motion-smooth`.
    courbe: [f32; 4],
    horloge: Horloge,
    events: Events,
}

impl std::fmt::Debug for MargeDeLecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MargeDeLecture")
            .field("ouvertes", &self.feuilles.borrow().len())
            .finish_non_exhaustive()
    }
}

impl MargeDeLecture {
    /// Durée et courbe lues dans le thème (`--anim-duration-quick`, `--anim-motion-smooth`).
    pub fn new(theme: &Theme) -> Rc<Self> {
        Self::avec_horloge(theme, horloge_reelle())
    }

    pub fn avec_horloge(theme: &Theme, horloge: Horloge) -> Rc<Self> {
        Rc::new(Self {
            feuilles: RefCell::new(HashMap::new()),
            duree: f64::from(theme.forme.anim_duration_quick),
            courbe: theme.forme.anim_motion_smooth,
            horloge,
            events: Events::new(),
        })
    }

    /// La marge de cette feuille est-elle ouverte (la classe `is-margin-mode`) ?
    pub fn est_ouverte(&self, leaf: ItemId) -> bool {
        self.feuilles.borrow().get(&leaf).is_some_and(|f| f.ouverte)
    }

    /// Ouvre ou ferme la marge d'une feuille, avec transition.
    pub fn poser(&self, leaf: ItemId, on: bool) {
        self.changer(leaf, on, true);
    }

    /// Ouvre ou ferme sans transition (les scènes de pixels figent l'état final).
    pub fn poser_immediat(&self, leaf: ItemId, on: bool) {
        self.changer(leaf, on, false);
    }

    fn changer(&self, leaf: ItemId, on: bool, anime: bool) {
        let maintenant = (self.horloge)();
        let change = {
            let mut feuilles = self.feuilles.borrow_mut();
            let courant = feuilles
                .get(&leaf)
                .map_or(0.0, |f| self.avancement(f, maintenant));
            let avant = feuilles.get(&leaf).is_some_and(|f| f.ouverte);
            let depart = if anime { courant } else { cible(on) };
            if !on && depart <= 0.0 {
                // Fermée et au repos : rien à retenir (non persisté, aucune trace).
                feuilles.remove(&leaf);
            } else {
                feuilles.insert(
                    leaf,
                    Feuille {
                        ouverte: on,
                        depart,
                        debut: maintenant,
                    },
                );
            }
            avant != on
        };
        if change {
            self.events.trigger(CHANGEMENT, &leaf);
        }
    }

    /// Une feuille fermée (détachée) : on l'oublie.
    pub fn oublier(&self, leaf: ItemId) {
        self.feuilles.borrow_mut().remove(&leaf);
    }

    /// Le retrait du plugin : toutes les feuilles perdent leur marge, sans transition.
    pub fn tout_fermer(&self) {
        let ouvertes: Vec<ItemId> = {
            let mut feuilles = self.feuilles.borrow_mut();
            let o = feuilles
                .iter()
                .filter(|(_, f)| f.ouverte)
                .map(|(&l, _)| l)
                .collect();
            feuilles.clear();
            o
        };
        for leaf in ouvertes {
            self.events.trigger(CHANGEMENT, &leaf);
        }
    }

    /// La réserve de la feuille pour une vue de ce genre, à cet instant.
    pub fn reserve(&self, leaf: ItemId, genre: Genre) -> Reserve {
        let maintenant = (self.horloge)();
        let Some(f) = self.feuilles.borrow().get(&leaf).copied() else {
            return Reserve {
                fraction: 0.0,
                en_transition: false,
            };
        };
        if !genre.anime() {
            return Reserve {
                fraction: if f.ouverte { genre.fraction() } else { 0.0 },
                en_transition: false,
            };
        }
        let a = self.avancement(&f, maintenant);
        Reserve {
            fraction: genre.fraction() * a,
            en_transition: (a - cible(f.ouverte)).abs() > f32::EPSILON,
        }
    }

    /// S'abonne aux changements d'état (ouverte ↔ fermée) ; la donnée est l'[`ItemId`] de la feuille.
    pub fn on_change(&self, cb: impl Fn(ItemId) + 'static) -> EventRef {
        self.events.on(CHANGEMENT, move |d| {
            if let Some(&leaf) = d.downcast_ref::<ItemId>() {
                cb(leaf);
            }
        })
    }

    /// L'avancement animé de `f` à `maintenant`, entre `depart` et sa cible.
    fn avancement(&self, f: &Feuille, maintenant: f64) -> f32 {
        let fin = cible(f.ouverte);
        if self.duree <= 0.0 {
            return fin;
        }
        let x = ((maintenant - f.debut) / self.duree).clamp(0.0, 1.0) as f32;
        if x >= 1.0 {
            return fin;
        }
        f.depart + (fin - f.depart) * courbe_bezier(self.courbe, x)
    }
}

fn cible(on: bool) -> f32 {
    if on { 1.0 } else { 0.0 }
}

/// La courbe CSS `cubic-bezier(x1, y1, x2, y2)` en `x` ∈ [0, 1] : on cherche le paramètre dont
/// l'abscisse vaut `x` (dichotomie : x(t) est croissante pour x1, x2 ∈ [0, 1]), on rend son ordonnée.
pub fn courbe_bezier(c: [f32; 4], x: f32) -> f32 {
    let [x1, y1, x2, y2] = c;
    let b = |p1: f32, p2: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    };
    let (mut bas, mut haut) = (0.0_f32, 1.0_f32);
    for _ in 0..40 {
        let t = (bas + haut) / 2.0;
        if b(x1, x2, t) < x {
            bas = t;
        } else {
            haut = t;
        }
    }
    b(y1, y2, (bas + haut) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_courbe_smooth_part_vite_et_freine() {
        let c = [0.2, 0.0, 0.0, 1.0];
        assert!(courbe_bezier(c, 0.0).abs() < 1e-4);
        assert!((courbe_bezier(c, 1.0) - 1.0).abs() < 1e-4);
        // Au cinquième du temps, déjà près de la moitié du chemin.
        let y = courbe_bezier(c, 0.185);
        assert!(y > 0.4 && y < 0.5, "{y}");
    }
}
