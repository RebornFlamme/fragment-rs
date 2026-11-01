// Port de app/src/ui/core/keymap/swipe.ts
//! Le BALAYAGE horizontal à deux doigts — pur (swipe.test.ts).
//!
//! Un pavé tactile envoie ce geste comme une suite d'événements de molette
//! portant un `dx`, inertie comprise. On cumule tant que les événements se
//! suivent (< 150 ms) et restent horizontaux (|dx| > 2·|dy|) ; au-delà du seuil,
//! le geste déclenche UNE fois, puis on verrouille jusqu'à une pause (250 ms sans
//! événement) — sans quoi l'inertie redéclencherait.
//! ★ …OU jusqu'à un balayage dans l'AUTRE sens : l'inertie garde le signe du
//!   geste ; un delta de signe opposé est donc un nouveau geste. Sans ça, élargir
//!   puis rétrécir aussitôt ne faisait rien (les événements d'inertie s'enchaînent
//!   sans pause, le verrou ne tombait jamais — constaté par l'utilisateur).
//!
//! Sens : dx > 0 → `Right` (constaté sur le pavé de l'utilisateur : son balayage
//! vers la droite donne des dx positifs ; inversé à sa demande).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
}

/// Les « touches » d'un balayage reconnu (Keymap) : liées à des commandes comme une frappe.
pub const SWIPE_RIGHT: &str = "SwipeRight";
pub const SWIPE_LEFT: &str = "SwipeLeft";

/// Bas : le geste part dès le début du balayage (l'app doit être nerveuse).
pub const SWIPE_SEUIL: f64 = 60.0;
const ENCHAINEMENT: f64 = 150.0;
const PAUSE: f64 = 250.0;

/// Le `Math.sign` de JavaScript (0 pour 0, ±1 sinon).
fn signe(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[derive(Debug)]
pub struct SwipeDetector {
    cumul: f64,
    dernier: f64,
    verrou: bool,
    /// Le signe du dernier geste déclenché (pour reconnaître l'inverse).
    signe: f64,
}

impl Default for SwipeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SwipeDetector {
    pub fn new() -> Self {
        Self {
            cumul: 0.0,
            dernier: f64::NEG_INFINITY,
            verrou: false,
            signe: 0.0,
        }
    }

    /// Un événement de molette (dx, dy, horodatage ms) → le sens du geste s'il vient d'aboutir.
    pub fn push(&mut self, dx: f64, dy: f64, t: f64) -> Option<SwipeDirection> {
        let ecart = t - self.dernier;
        self.dernier = t;
        if self.verrou {
            // ≥ 4 : un rebond de fin d'inertie n'est pas un geste.
            let inverse = signe(dx) == -self.signe && dx.abs() >= 4.0 && dx.abs() > 2.0 * dy.abs();
            if !inverse && ecart < PAUSE {
                return None;
            }
            self.verrou = false;
            self.cumul = 0.0;
        }
        if ecart > ENCHAINEMENT {
            self.cumul = 0.0;
        }
        if dx.abs() <= 2.0 * dy.abs() {
            self.cumul = 0.0;
            return None;
        }
        self.cumul += dx;
        if self.cumul.abs() < SWIPE_SEUIL {
            return None;
        }
        let sens = if self.cumul > 0.0 {
            SwipeDirection::Right
        } else {
            SwipeDirection::Left
        };
        self.signe = signe(self.cumul);
        self.cumul = 0.0;
        self.verrou = true;
        Some(sens)
    }
}

#[cfg(test)]
mod tests {
    use super::SwipeDirection::{Left, Right};
    use super::*;

    /// Rejoue des deltas espacés de `pas` ms à partir de t0 ; rend les gestes déclenchés.
    fn jouer(
        d: &mut SwipeDetector,
        deltas: &[(f64, f64)],
        t0: f64,
        pas: f64,
    ) -> Vec<SwipeDirection> {
        deltas
            .iter()
            .enumerate()
            .filter_map(|(i, &(dx, dy))| d.push(dx, dy, t0 + i as f64 * pas))
            .collect()
    }

    fn n(k: usize, dx: f64, dy: f64) -> Vec<(f64, f64)> {
        vec![(dx, dy); k]
    }

    fn chaine(parties: &[Vec<(f64, f64)>]) -> Vec<(f64, f64)> {
        parties.concat()
    }

    #[test]
    fn delta_x_positif_right_une_seule_fois_inertie_comprise() {
        let deltas = chaine(&[n(10, 20.0, 0.0), n(20, 6.0, 0.0)]);
        assert_eq!(
            jouer(&mut SwipeDetector::new(), &deltas, 0.0, 16.0),
            vec![Right]
        );
    }

    #[test]
    fn delta_x_negatif_left() {
        assert_eq!(
            jouer(&mut SwipeDetector::new(), &n(10, -20.0, 0.0), 0.0, 16.0),
            vec![Left]
        );
    }

    #[test]
    fn surtout_vertical_ignore() {
        assert_eq!(
            jouer(&mut SwipeDetector::new(), &n(20, -20.0, 15.0), 0.0, 16.0),
            vec![]
        );
    }

    #[test]
    fn petits_mouvements_sous_le_seuil_ou_trop_espaces_ignores() {
        assert_eq!(
            jouer(&mut SwipeDetector::new(), &n(5, -10.0, 0.0), 0.0, 16.0),
            vec![]
        ); // 50 < 60
        assert_eq!(
            jouer(&mut SwipeDetector::new(), &n(10, -30.0, 0.0), 0.0, 400.0),
            vec![]
        );
    }

    #[test]
    fn elargir_puis_retrecir_aussitot_sans_pause_inertie_en_cours_le_sens_inverse_deverrouille() {
        let mut d = SwipeDetector::new();
        let deltas = chaine(&[n(10, 20.0, 0.0), n(5, 4.0, 0.0), n(10, -20.0, 0.0)]);
        assert_eq!(jouer(&mut d, &deltas, 0.0, 16.0), vec![Right, Left]);
    }

    #[test]
    fn deux_gestes_separes_par_une_pause_deux_declenchements() {
        let mut d = SwipeDetector::new();
        assert_eq!(jouer(&mut d, &n(10, 20.0, 0.0), 0.0, 16.0), vec![Right]);
        assert_eq!(jouer(&mut d, &n(10, -20.0, 0.0), 1000.0, 16.0), vec![Left]);
    }
}
