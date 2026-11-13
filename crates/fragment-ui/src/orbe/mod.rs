// Port de app/src/ui/views/agent/trace/orb.ts (la boucle de l'orbe, réécrite pour GPUI) ; le
// moteur vient de « Thinking Orbs » (Jakub Antalik, MIT, https://libraries.dev/orbs, commit
// d'origine bad63221 de github.com/Jakubantalik/Libraries.dev, packages/thinking-orbs/src),
// traduit en Rust dans `noyau`, `reglages`, `sphere`, `formes`, `echarpe` et `morph`. La notice
// MIT du moteur est dans `LICENSE-THINKING-ORBS`, à côté de ce fichier.

//! L'orbe de l'en-tête de la trace (tâche `fr-t7-orbe`).
//!
//! Neuf animations de points en 3D, une par geste de l'agent ([`OrbState`]) : la géométrie d'un
//! instant est une fonction PURE de (forme, côté, temps, réglages) — [`image`] — puis le
//! [`plan`] la traduit en disques et traits colorés que la vue peint sur une toile GPUI.
//!
//! - Horloge PARTAGÉE ([`HorlogeOrbes`]) : changer d'état ne fait pas sauter l'animation, et
//!   deux orbes à l'écran tournent ensemble.
//! - Mouvement réduit : une image fixe, toujours la même ([`T_FIGE`]).
//! - Hors écran (hors du masque de contenu de la fenêtre) : rien n'est peint ni redemandé.
//! - Encre : la couleur du texte du chat (`--chat-fg`), donnée par la trace ; la rampe de
//!   profondeur se fait sur elle, inversée en thème sombre. Aucun littéral de couleur.

mod echarpe;
mod formes;
mod morph;
pub mod noyau;
pub mod reglages;
mod sphere;
mod vue;

use std::cell::Cell;

use fragment_agent::trace_model::OrbState;
use fragment_core::CommandManager;

pub use noyau::{ImageOrbe, Point, Segment};
pub use reglages::{Forme, Reglages, Resolu, Taille, forme_de, resoudre};
pub use vue::{Orbes, element_orbe, visible};

/// Le temps (secondes) de l'image fixe du mouvement réduit, celui de la bibliothèque d'origine.
pub const T_FIGE: f64 = 0.6;

/// Tous les états, dans l'ordre de la bibliothèque (la liste des réglages de l'agent).
pub const ETATS: [OrbState; 9] = [
    OrbState::Working,
    OrbState::Searching,
    OrbState::Solving,
    OrbState::Listening,
    OrbState::Connecting,
    OrbState::Weaving,
    OrbState::Composing,
    OrbState::Breathing,
    OrbState::Shaping,
];

/// Le nom montré d'une orbe (pour la choisir dans les réglages de l'agent).
#[must_use]
pub fn libelle(etat: OrbState) -> &'static str {
    match etat {
        OrbState::Working => "Orbites",
        OrbState::Searching => "Globe",
        OrbState::Solving => "Résolution",
        OrbState::Listening => "Onde",
        OrbState::Connecting => "Constellation",
        OrbState::Weaving => "Tresse",
        OrbState::Composing => "Écharpe",
        OrbState::Breathing => "Anneau",
        OrbState::Shaping => "Formes",
    }
}

/// La géométrie d'une forme à l'instant `t` (secondes déjà multipliées par la vitesse).
#[must_use]
pub fn image(forme: Forme, cote: f64, t: f64, o: &Reglages) -> ImageOrbe {
    match forme {
        Forme::Orbites => formes::image_orbites(cote, t, o),
        Forme::Globe => sphere::image_globe(cote, t, o),
        Forme::Rubik => sphere::image_rubik(cote, t, o),
        Forme::Onde => sphere::image_onde(cote, t, o),
        Forme::Toile => formes::image_toile(cote, t, o),
        Forme::Tresse => formes::image_tresse(cote, t, o),
        // L'anneau partage la géométrie de l'écharpe : `face_on` fait la différence.
        Forme::Echarpe | Forme::Anneau => echarpe::image_echarpe(cote, t, o),
        Forme::Morph => morph::image_morph(cote, t, o),
    }
}

/// L'image d'un état de l'agent à l'horloge partagée `secondes`, sur un côté `cote`.
#[must_use]
pub fn image_etat(etat: OrbState, cote: f32, secondes: f64) -> ImageOrbe {
    let r = resoudre(etat, Taille::proche(cote));
    image(r.forme, f64::from(cote), secondes * r.vitesse, &r.reglages)
}

/// Une marque à peindre : coordonnées locales à la toile, couleur en canaux 0–1 `[r, v, b, a]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Marque {
    Disque {
        x: f32,
        y: f32,
        r: f32,
        couleur: [f32; 4],
    },
    Trait {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        epaisseur: f32,
        couleur: [f32; 4],
    },
}

/// Le plan de peinture d'une image : les traits d'abord (les nœuds se posent dessus), puis les
/// disques dans l'ordre du tri. `teinte` : l'encre du thème en octets (sans son opacité).
#[must_use]
pub fn plan(image: &ImageOrbe, sombre: bool, teinte: Option<[u8; 3]>) -> Vec<Marque> {
    let mut marques = Vec::with_capacity(image.points.len() + image.segments.len());
    for s in &image.segments {
        marques.push(Marque::Trait {
            x1: s.x1 as f32,
            y1: s.y1 as f32,
            x2: s.x2 as f32,
            y2: s.y2 as f32,
            epaisseur: s.w as f32,
            couleur: noyau::encre(s.blanc, s.a.unwrap_or(1.0), sombre, teinte),
        });
    }
    for p in &image.points {
        marques.push(Marque::Disque {
            x: p.x as f32,
            y: p.y as f32,
            r: p.r as f32,
            couleur: noyau::encre(p.blanc, p.a.unwrap_or(1.0), sombre, teinte),
        });
    }
    marques
}

/// L'horloge commune à toutes les orbes : des secondes depuis la première image peinte. Le temps
/// est injecté (`maintenant`, en millisecondes) : les tests le figent.
#[derive(Debug, Default)]
pub struct HorlogeOrbes {
    origine: Cell<Option<i64>>,
}

impl HorlogeOrbes {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Les secondes écoulées à `maintenant` (ms) ; le premier appel fixe l'origine.
    pub fn secondes(&self, maintenant: i64) -> f64 {
        let origine = match self.origine.get() {
            Some(o) => o,
            None => {
                self.origine.set(Some(maintenant));
                maintenant
            }
        };
        // Une horloge murale qui recule (réglage de l'heure) ne fait pas tourner à l'envers.
        (maintenant - origine).max(0) as f64 / 1000.0
    }
}

/// Enregistre les commandes de scène `pixels:orbe` : aucune, les scènes de T7 coupent l'orbe
/// (docs/tranches/T7.md §5).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
