// Port de app/src/ui/core/settings/components/SliderComponent.ts (le comportement) et de
// `input.slider` de settings.css (la géométrie).
//! Le curseur d'un réglage : le MODÈLE du geste, sans GPUI.
//!
//! Deux façons d'appliquer la valeur, comme `setInstant` du TS :
//! - `instant` : `on_change` à chaque déplacement (la taille de police) ;
//! - sinon, AU RELÂCHÉ seulement (le zoom). ★ Le zoom vaut pour toutes les fenêtres, celle des
//!   réglages comprise : appliqué pendant le glisser, la piste se redimensionnerait sous le
//!   curseur. L'étiquette, elle, suit le glisser.

/// Largeur de la piste (`input.slider { width }`), en px logiques.
pub const LARGEUR_PISTE: f32 = 150.0;
/// Hauteur de la piste.
pub const HAUTEUR_PISTE: f32 = 4.0;
/// Diamètre du pouce (`::-webkit-slider-thumb`).
pub const DIAMETRE_POUCE: f32 = 16.0;

/// L'état d'un curseur : sa valeur de référence et le glisser en cours.
#[derive(Debug, Clone, PartialEq)]
pub struct Curseur {
    min: f64,
    max: f64,
    pas: f64,
    instant: bool,
    /// La valeur du réglage (celle que le modèle porte).
    valeur: f64,
    /// La valeur suivie pendant un glisser.
    glisse: Option<f64>,
}

impl Curseur {
    pub fn new(min: f64, max: f64, pas: f64, instant: bool, valeur: f64) -> Self {
        let mut c = Self {
            min,
            max,
            pas: if pas > 0.0 { pas } else { 1.0 },
            instant,
            valeur: min,
            glisse: None,
        };
        c.valeur = c.caler(valeur);
        c
    }

    /// Ramène une valeur sur la grille du pas, dans les bornes (comme `<input type=range>`).
    pub fn caler(&self, v: f64) -> f64 {
        let n = ((v - self.min) / self.pas).round();
        (self.min + n * self.pas).clamp(self.min, self.max)
    }

    /// La valeur à montrer : celle du glisser s'il y en a un, sinon celle du réglage.
    pub fn valeur_affichee(&self) -> f64 {
        self.glisse.unwrap_or(self.valeur)
    }

    pub fn glisse(&self) -> bool {
        self.glisse.is_some()
    }

    /// La part de la piste parcourue, de 0 à 1.
    pub fn fraction(&self) -> f32 {
        let etendue = self.max - self.min;
        if etendue <= 0.0 {
            return 0.0;
        }
        ((self.valeur_affichee() - self.min) / etendue) as f32
    }

    /// L'abscisse du CENTRE du pouce depuis le bord gauche de la piste : il parcourt la piste
    /// moins son propre diamètre (Chromium), donc de 8 à 142 px.
    pub fn centre_pouce(fraction: f32) -> f32 {
        DIAMETRE_POUCE / 2.0 + fraction * (LARGEUR_PISTE - DIAMETRE_POUCE)
    }

    /// La valeur d'un pointeur à l'abscisse `x` (fenêtre), la piste commençant à `gauche`.
    pub fn valeur_a(&self, x: f32, gauche: f32) -> f64 {
        let course = LARGEUR_PISTE - DIAMETRE_POUCE;
        let fraction = f64::from(((x - gauche - DIAMETRE_POUCE / 2.0) / course).clamp(0.0, 1.0));
        self.caler(self.min + fraction * (self.max - self.min))
    }

    /// Appui sur la piste : le glisser commence. Rend la valeur à appliquer TOUT DE SUITE
    /// (mode instantané seulement).
    pub fn appuyer(&mut self, x: f32, gauche: f32) -> Option<f64> {
        self.deplacer(x, gauche)
    }

    /// Déplacement du pointeur pendant le glisser. Même contrat qu'`appuyer`.
    pub fn deplacer(&mut self, x: f32, gauche: f32) -> Option<f64> {
        let v = self.valeur_a(x, gauche);
        self.glisse = Some(v);
        self.instant.then_some(v)
    }

    /// Relâchement : rend la valeur à appliquer (mode au relâché, si elle diffère du réglage).
    pub fn relacher(&mut self) -> Option<f64> {
        let v = self.glisse.take()?;
        (!self.instant && (v - self.valeur).abs() > f64::EPSILON).then(|| {
            self.valeur = v;
            v
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoom() -> Curseur {
        Curseur::new(50.0, 200.0, 5.0, false, 100.0)
    }

    #[test]
    fn le_pouce_a_100_est_au_tiers_de_sa_course() {
        let c = zoom();
        assert!((c.fraction() - 1.0 / 3.0).abs() < 1e-6);
        // 8 + 134 / 3 = 52,67 + ... : le centre est à 52,67 px du bord, à 100 %.
        assert!((Curseur::centre_pouce(c.fraction()) - 52.666_67).abs() < 1e-3);
    }

    #[test]
    fn un_pointeur_se_cale_sur_le_pas_et_les_bornes() {
        let c = zoom();
        assert_eq!(c.valeur_a(-50.0, 0.0), 50.0);
        assert_eq!(c.valeur_a(500.0, 0.0), 200.0);
        // Au centre de la course : 125 %.
        assert_eq!(c.valeur_a(8.0 + 67.0, 0.0), 125.0);
        // Un pas de 5 : 126 retombe sur 125.
        assert_eq!(c.caler(126.0), 125.0);
    }

    #[test]
    fn le_mode_relache_attend_le_bouton() {
        let mut c = zoom();
        assert_eq!(c.appuyer(8.0 + 134.0 * 0.75, 0.0), None);
        assert_eq!(c.deplacer(8.0 + 134.0 * 0.75, 0.0), None);
        assert!(c.glisse());
        assert!(c.valeur_affichee() > 140.0);
        let v = c.relacher().expect("appliqué au relâché");
        assert_eq!(v, c.valeur_affichee());
        assert!(!c.glisse());
    }

    #[test]
    fn le_mode_instantane_applique_a_chaque_pas() {
        let mut c = Curseur::new(8.0, 30.0, 1.0, true, 16.0);
        assert!(c.deplacer(8.0, 0.0).is_some());
        assert_eq!(c.relacher(), None);
    }

    #[test]
    fn relacher_sans_changement_n_applique_rien() {
        let mut c = zoom();
        c.appuyer(Curseur::centre_pouce(c.fraction()), 0.0);
        assert_eq!(c.relacher(), None);
    }
}
