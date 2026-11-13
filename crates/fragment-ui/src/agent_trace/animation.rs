// Port de app/src/ui/views/agent/trace/traceView.ts (TexteFondu, Odometre, repliable, geste)
// et de app/src/ui/views/agent/trace/trace.css (durées, courbe)
//! Les gestes de la trace, en modèle PUR : chaque geste est une petite machine d'états qui se
//! lit à un instant donné (`maintenant`, en millisecondes). Rien ne dépend de la fenêtre ni de
//! l'horloge : la vue passe l'heure, les tests passent la leur.
//!
//! ★ Écarts assumés (voir la PR) : GPUI n'a pas de flou (`filter: blur`) ; les entrées et les
//! sorties « depuis le flou » sont rendues par l'opacité seule. Les références de pixels sont
//! prises sous `prefers-reduced-motion`, régime où rien de tout cela ne bouge.

/// La courbe de la trace : celle du thème (`--anim-motion-smooth`), sortie rapide, arrivée douce.
pub const COURBE: [f32; 4] = [0.2, 0.0, 0.0, 1.0];

/// Durée de sortie d'un fondu flou (ms).
pub const SORTIE_MS: i64 = 100;
/// Durée d'entrée d'un fondu flou (ms).
pub const ENTREE_MS: i64 = 130;
/// Entrée d'une étape, roulement d'un chiffre (ms).
pub const ETAPE_MS: i64 = 160;
/// Un geste de l'utilisateur : nerveux, sans retard (ms).
pub const CLIC_MS: i64 = 110;
/// Repli d'une zone : hauteur (ms).
pub const REPLI_MS: i64 = 170;
/// Repli d'une zone : retard de la hauteur, le fondu passe d'abord (ms).
pub const REPLI_RETARD_MS: i64 = 60;
/// Dépli d'une zone : fondu du contenu (ms).
pub const DEPLI_FONDU_MS: i64 = 140;
/// Dépli d'une zone : retard du fondu, la hauteur passe d'abord (ms).
pub const DEPLI_RETARD_MS: i64 = 80;
/// Fondu rapide d'un geste (ms).
pub const FONDU_RAPIDE_MS: i64 = 90;
/// Durée de vie d'un texte qui sort avant d'être retiré (ms).
pub const RETRAIT_TEXTE_MS: i64 = 220;
/// Durée de vie d'un chiffre qui sort avant d'être retiré (ms).
pub const RETRAIT_CHIFFRE_MS: i64 = 200;
/// Le mode nerveux d'un geste dure ce temps (ms).
pub const GESTE_MS: i64 = 200;
/// Période du reflet qui glisse sur un libellé en cours (ms).
pub const REFLET_MS: i64 = 1800;
/// Période de l'arc qui tourne (ms).
pub const ARC_MS: i64 = 800;

/// Résout `t` (0..1) sur la courbe `cubic-bezier(x1, y1, x2, y2)`.
#[must_use]
pub fn courbe(c: [f32; 4], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let [x1, y1, x2, y2] = c;
    let eval = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    // Bissection de x(s) = t : x est monotone pour les courbes de CSS (x1, x2 dans 0..1).
    let (mut bas, mut haut) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let milieu = (bas + haut) / 2.0;
        if eval(x1, x2, milieu) < t {
            bas = milieu;
        } else {
            haut = milieu;
        }
    }
    eval(y1, y2, (bas + haut) / 2.0)
}

/// Progression 0..1 d'une transition de `duree` ms commencée à `debut`, après `retard` ms.
fn progres(debut: i64, retard: i64, duree: i64, maintenant: i64) -> f32 {
    if duree <= 0 {
        return 1.0;
    }
    ((maintenant - debut - retard) as f32 / duree as f32).clamp(0.0, 1.0)
}

/// L'avancement (0..1) lissé d'une entrée commencée à `debut` (étape qui entre depuis le flou).
#[must_use]
pub fn entree_etape(debut: i64, maintenant: i64) -> f32 {
    courbe(COURBE, progres(debut, 0, ETAPE_MS, maintenant))
}

/// Une couche d'un texte qui change en fondu croisé.
#[derive(Debug, Clone, PartialEq)]
struct Couche {
    texte: String,
    reflet: bool,
    /// Début de l'entrée (`None` : posée d'emblée).
    entree: Option<i64>,
    /// Début de la sortie (`None` : la couche courante).
    sortie: Option<i64>,
}

/// Ce que la vue peint d'une couche de texte.
#[derive(Debug, Clone, PartialEq)]
pub struct CoucheVue {
    pub texte: String,
    /// 0..1 (entrée, sortie) : le flou n'existe pas en GPUI, l'opacité le remplace.
    pub opacite: f32,
    /// Le libellé est en cours : le reflet glisse dessus.
    pub reflet: bool,
    /// La couche quitte l'écran.
    pub sortante: bool,
}

/// Un texte qui change en fondu croisé (deux couches superposées au plus).
#[derive(Debug, Clone, Default)]
pub struct TexteFondu {
    actuel: String,
    couches: Vec<Couche>,
}

impl TexteFondu {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pose le texte. `anime` faux ou mouvement réduit : remplacement net, sans couche sortante.
    pub fn set(&mut self, texte: &str, anime: bool, reflet: bool, reduit: bool, maintenant: i64) {
        let a_courante = self.couches.iter().any(|c| c.sortie.is_none());
        if texte == self.actuel && a_courante {
            // Même texte : seule la décoration (le reflet) suit.
            if let Some(c) = self.couches.iter_mut().find(|c| c.sortie.is_none()) {
                c.reflet = reflet;
            }
            return;
        }
        self.actuel = texte.to_string();
        let neuve = Couche {
            texte: texte.to_string(),
            reflet,
            entree: None,
            sortie: None,
        };
        if !a_courante || !anime || reduit {
            self.couches = vec![neuve];
            return;
        }
        for c in &mut self.couches {
            if c.sortie.is_none() {
                c.sortie = Some(maintenant);
            }
        }
        // Deux couches au plus : la plus ancienne des sortantes cède la place.
        while self.couches.len() > 1 {
            self.couches.remove(0);
        }
        self.couches.push(Couche {
            entree: Some(maintenant),
            ..neuve
        });
    }

    /// Les couches à peindre à `maintenant` (les sorties finies sont retirées).
    pub fn couches(&mut self, maintenant: i64) -> Vec<CoucheVue> {
        self.couches
            .retain(|c| c.sortie.is_none_or(|s| maintenant - s < RETRAIT_TEXTE_MS));
        self.couches
            .iter()
            .map(|c| {
                let (opacite, sortante) = match (c.sortie, c.entree) {
                    (Some(s), _) => (
                        1.0 - courbe(COURBE, progres(s, 0, SORTIE_MS, maintenant)),
                        true,
                    ),
                    (None, Some(e)) => {
                        (courbe(COURBE, progres(e, 0, ENTREE_MS, maintenant)), false)
                    }
                    (None, None) => (1.0, false),
                };
                CoucheVue {
                    texte: c.texte.clone(),
                    opacite,
                    reflet: c.reflet,
                    sortante,
                }
            })
            .collect()
    }

    /// Le texte posé (la couche qui ne sort pas).
    #[must_use]
    pub fn texte(&self) -> &str {
        &self.actuel
    }

    /// Nombre de couches vivantes.
    #[must_use]
    pub fn nb_couches(&self) -> usize {
        self.couches.len()
    }

    /// Un fondu est en cours à `maintenant` ?
    #[must_use]
    pub fn anime(&self, maintenant: i64) -> bool {
        self.couches.iter().any(|c| {
            c.sortie.is_some_and(|s| maintenant - s < RETRAIT_TEXTE_MS)
                || c.entree.is_some_and(|e| maintenant - e < ENTREE_MS)
        })
    }
}

/// Une case du chrono : le chiffre courant et, le temps de rouler, l'ancien.
#[derive(Debug, Clone, PartialEq)]
struct Case {
    courant: char,
    ancien: Option<char>,
    debut: i64,
}

/// Ce que la vue peint d'une case du chrono.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseVue {
    pub courant: char,
    pub ancien: Option<char>,
    /// 0..1 : le roulement (l'ancien monte et sort, le neuf arrive d'en bas).
    pub roulement: f32,
}

/// Un nombre dont les chiffres ROULENT quand ils changent (le chrono).
#[derive(Debug, Clone, Default)]
pub struct Odometre {
    texte: String,
    cases: Vec<Case>,
}

impl Odometre {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pose la valeur : seule une case dont le caractère change roule ; si le nombre de cases
    /// change (ou sans animation), tout est reconstruit net.
    pub fn set(&mut self, texte: &str, anime: bool, reduit: bool, maintenant: i64) {
        if texte == self.texte {
            return;
        }
        let avant: Vec<char> = self.texte.chars().collect();
        self.texte = texte.to_string();
        let neufs: Vec<char> = texte.chars().collect();
        if self.cases.len() != neufs.len() || !anime || reduit {
            self.cases = neufs
                .into_iter()
                .map(|courant| Case {
                    courant,
                    ancien: None,
                    debut: maintenant,
                })
                .collect();
            return;
        }
        for (i, c) in neufs.into_iter().enumerate() {
            if avant.get(i) == Some(&c) {
                continue;
            }
            self.cases[i] = Case {
                courant: c,
                ancien: avant.get(i).copied(),
                debut: maintenant,
            };
        }
    }

    /// Les cases à peindre à `maintenant` (un ancien chiffre trop vieux est retiré).
    pub fn cases(&mut self, maintenant: i64) -> Vec<CaseVue> {
        for c in &mut self.cases {
            if c.ancien.is_some() && maintenant - c.debut >= RETRAIT_CHIFFRE_MS {
                c.ancien = None;
            }
        }
        self.cases
            .iter()
            .map(|c| CaseVue {
                courant: c.courant,
                ancien: c.ancien,
                roulement: if c.ancien.is_some() {
                    courbe(COURBE, progres(c.debut, 0, ETAPE_MS, maintenant))
                } else {
                    1.0
                },
            })
            .collect()
    }

    /// Le texte du chrono (une case par caractère).
    #[must_use]
    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// Un chiffre roule à `maintenant` ?
    #[must_use]
    pub fn anime(&self, maintenant: i64) -> bool {
        self.cases
            .iter()
            .any(|c| c.ancien.is_some() && maintenant - c.debut < RETRAIT_CHIFFRE_MS)
    }
}

/// Une transition de hauteur et de fondu en cours.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Transition {
    debut: i64,
    ouvrant: bool,
    geste: bool,
    hauteur_depart: f32,
    opacite_depart: f32,
}

/// Une zone qui se déplie / se replie (grille 0fr ↔ 1fr), contenu en fondu.
#[derive(Debug, Clone, Default)]
pub struct Repliable {
    ouvert: bool,
    transition: Option<Transition>,
    /// Le mode nerveux court jusqu'à cet instant.
    geste_jusqua: i64,
}

impl Repliable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Un clic de l'utilisateur : la zone passe en mode NERVEUX le temps de la transition (110 ms,
    /// sans retard) — les durées des références valent pour ce qui bouge SEUL, pas pour un geste.
    pub fn geste(&mut self, maintenant: i64) {
        self.geste_jusqua = maintenant + GESTE_MS;
    }

    /// Le mode nerveux est-il posé (`is-gesture`) ?
    #[must_use]
    pub fn geste_actif(&self, maintenant: i64) -> bool {
        maintenant < self.geste_jusqua
    }

    #[must_use]
    pub fn ouvert(&self) -> bool {
        self.ouvert
    }

    /// Ouvre ou ferme ; une transition part de l'état visible à `maintenant`.
    pub fn set(&mut self, ouvert: bool, maintenant: i64) {
        if ouvert == self.ouvert {
            return;
        }
        let depart = (self.hauteur(maintenant), self.opacite(maintenant));
        self.ouvert = ouvert;
        self.transition = Some(Transition {
            debut: maintenant,
            ouvrant: ouvert,
            geste: self.geste_actif(maintenant),
            hauteur_depart: depart.0,
            opacite_depart: depart.1,
        });
    }

    /// Posée d'emblée, sans transition (premier rendu).
    pub fn poser(&mut self, ouvert: bool) {
        self.ouvert = ouvert;
        self.transition = None;
    }

    /// Durée et retard (ms) de la transition de HAUTEUR du moment : `(110, 0)` en geste.
    #[must_use]
    pub fn transition_hauteur(&self, maintenant: i64) -> (i64, i64) {
        if self.geste_actif(maintenant) {
            (CLIC_MS, 0)
        } else if self.ouvert {
            (REPLI_MS, 0)
        } else {
            (REPLI_MS, REPLI_RETARD_MS)
        }
    }

    fn cible(&self) -> f32 {
        if self.ouvert { 1.0 } else { 0.0 }
    }

    /// La fraction de la hauteur du contenu montrée (0 = fermée, 1 = pleine).
    #[must_use]
    pub fn hauteur(&self, maintenant: i64) -> f32 {
        let cible = self.cible();
        let Some(t) = self.transition else {
            return cible;
        };
        let (duree, retard) = if t.geste {
            (CLIC_MS, 0)
        } else if t.ouvrant {
            (REPLI_MS, 0)
        } else {
            (REPLI_MS, REPLI_RETARD_MS)
        };
        let e = courbe(COURBE, progres(t.debut, retard, duree, maintenant));
        t.hauteur_depart + (cible - t.hauteur_depart) * e
    }

    /// L'opacité du contenu (le fondu suit la hauteur à l'ouverture, la précède à la fermeture).
    #[must_use]
    pub fn opacite(&self, maintenant: i64) -> f32 {
        let cible = self.cible();
        let Some(t) = self.transition else {
            return cible;
        };
        let (duree, retard) = if t.geste {
            (FONDU_RAPIDE_MS, 0)
        } else if t.ouvrant {
            (DEPLI_FONDU_MS, DEPLI_RETARD_MS)
        } else {
            (SORTIE_MS, 0)
        };
        let e = courbe(COURBE, progres(t.debut, retard, duree, maintenant));
        t.opacite_depart + (cible - t.opacite_depart) * e
    }

    /// Une transition est en cours à `maintenant` ?
    #[must_use]
    pub fn anime(&self, maintenant: i64) -> bool {
        self.transition.is_some_and(|t| {
            maintenant
                < t.debut + REPLI_RETARD_MS.max(DEPLI_RETARD_MS) + REPLI_MS.max(DEPLI_FONDU_MS)
        })
    }
}

/// L'avancement (0..1) du reflet d'un libellé en cours : une bosse du gris vers l'encre.
/// ★ Le dégradé animé de la référence (texte en dégradé qui glisse) n'existe pas en GPUI : la
/// couleur du libellé entier oscille entre les deux jetons.
#[must_use]
pub fn reflet(maintenant: i64) -> f32 {
    let p = maintenant.rem_euclid(REFLET_MS) as f32 / REFLET_MS as f32;
    let d = (p - 0.5).abs() * 2.0;
    (1.0 - d).clamp(0.0, 1.0)
}

/// L'angle (degrés) de l'arc qui tourne à `maintenant`.
#[must_use]
pub fn angle_arc(maintenant: i64) -> f32 {
    maintenant.rem_euclid(ARC_MS) as f32 / ARC_MS as f32 * 360.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_courbe_est_monotone_et_borne_0_1() {
        assert!(courbe(COURBE, 0.0).abs() < 1e-3);
        assert!((courbe(COURBE, 1.0) - 1.0).abs() < 1e-3);
        let mut avant = 0.0;
        for i in 0..=20 {
            let v = courbe(COURBE, i as f32 / 20.0);
            assert!(v >= avant - 1e-4);
            avant = v;
        }
        // Sortie rapide : à mi-parcours, déjà bien avancée.
        assert!(courbe(COURBE, 0.5) > 0.7);
    }

    #[test]
    fn un_texte_qui_change_garde_au_plus_deux_couches_puis_une() {
        let mut t = TexteFondu::new();
        t.set("a", true, false, false, 0);
        t.set("b", true, false, false, 10);
        t.set("c", true, false, false, 20);
        assert!(t.couches(30).len() <= 2);
        assert_eq!(t.couches(30 + RETRAIT_TEXTE_MS).len(), 1);
        assert_eq!(t.texte(), "c");
    }

    #[test]
    fn mouvement_reduit_remplace_net() {
        let mut t = TexteFondu::new();
        t.set("a", true, false, true, 0);
        t.set("b", true, false, true, 10);
        assert_eq!(t.nb_couches(), 1);
        assert!(!t.anime(11));
    }

    #[test]
    fn le_chrono_ne_fait_rouler_que_la_case_qui_change() {
        let mut o = Odometre::new();
        o.set("1s", false, false, 0);
        o.set("2s", true, false, 1000);
        let cases = o.cases(1010);
        assert_eq!(cases[0].ancien, Some('1'));
        assert_eq!(cases[1].ancien, None);
        let cases = o.cases(1000 + RETRAIT_CHIFFRE_MS);
        assert_eq!(cases[0].ancien, None);
    }

    #[test]
    fn un_geste_bascule_en_mode_nerveux_110_ms_sans_retard() {
        let mut r = Repliable::new();
        r.geste(0);
        assert_eq!(r.transition_hauteur(0), (110, 0));
        assert!(r.geste_actif(100));
        assert!(!r.geste_actif(GESTE_MS));
    }

    #[test]
    fn le_repli_autonome_attend_le_fondu_puis_la_hauteur() {
        let mut r = Repliable::new();
        r.poser(true);
        r.set(false, 0);
        // Le fondu est déjà parti, la hauteur n'a pas bougé (retard de 60 ms).
        assert!(r.opacite(50) < 1.0);
        assert!((r.hauteur(50) - 1.0).abs() < 1e-6);
        assert!(r.hauteur(REPLI_RETARD_MS + REPLI_MS) < 1e-3);
    }
}
