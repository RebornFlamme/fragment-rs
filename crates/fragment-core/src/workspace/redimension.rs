// Port de app/src/ui/core/workspace/resizeGesture.ts (la part pure), avec les calculs des
// poignées de WorkspaceSplit.attachResize et de Workspace.attachSidebarResize.
//
// Trois choses, sans DOM ni horloge :
// 1. le transfert de flex-grow entre deux panes voisins quand on tire leur poignée ;
// 2. la largeur d'un dock qu'on tire, bornée ;
// 3. LE GEL DES FEUILLES pendant le geste : un automate qui reçoit les boîtes mesurées et les
//    instants des images, et répond par les boîtes à imposer au contenu de chaque feuille.
//    La couche de rendu mesure, applique et rappelle ; le cœur décide.
//
// Pourquoi le gel (mesuré dans Fragment sur un vrai document : PDF de 291 pages en split, une
// conversation de vingt messages pleins de formules au dock) : faire refluer le contenu de
// chaque feuille à chaque image du geste coûtait une image sur trois au-delà de 17 ms ; figer
// la boîte du contenu ramène au plancher. Le contenu ne reflue qu'à bon escient.
//
// ★ LA RÈGLE DU MAXIMUM (règle de l'utilisateur) : pendant le geste, le contenu d'une feuille
//   garde la PLUS GRANDE boîte qu'il a atteinte. Une feuille qui rétrécit sous ce maximum est
//   seulement rognée : rien à refluer, pas de trou. Une feuille qui dépasse son maximum
//   (« territoire neuf ») reflue tout de suite à sa nouvelle place. Sur un aller-retour, seul
//   l'aller reflue : au retour, chaque côté revient dans une zone que son contenu couvre déjà.
//
// ★ LE GARDE-FOU : refluer en direct coûte ce que coûte la vue. Si l'image qui suit un reflux,
//   OU la suivante, dépasse le budget, la feuille est marquée « trou » jusqu'à la fin du
//   geste : elle ne suit plus en direct (une bande de fond apparaît là où elle grandit), et ne
//   rattrape sa place qu'à une PAUSE du pointeur ou au relâché. Les DEUX images comptent : sur
//   le vrai document, c'est la seconde qui débordait (mesures différées des éditeurs).
//
// ★ LA PAUSE se compte en IMAGES, pas au minuteur : 100 ms sans mouvement, constatées à une
//   image qui suit une image COURTE. Un minuteur compte le temps mural, et une image lourde
//   lui faisait croire à un arrêt du pointeur → nouveau reflux complet → cycle. Et la pause
//   ne comble QUE les trous : refaire refluer une feuille rognée ne montre rien de plus.
//
// Aucun nom de vue : une feuille est une clé `K` et deux boîtes (celle de la vue entière,
// en-tête compris, et celle de son contenu).

use std::collections::HashSet;
use std::hash::Hash;

use super::arbre::{Arbre, Cote, ItemId};

/// Taille minimale d'un pane pendant qu'on tire sa poignée (px).
pub const TAILLE_MIN_PANE: f64 = 100.0;
/// Bornes de la largeur d'un dock tiré à la main (px).
pub const LARGEUR_DOCK_MIN: f64 = 150.0;
pub const LARGEUR_DOCK_MAX: f64 = 500.0;
/// Calme (ms) après lequel les feuilles « trou » rattrapent leur place.
pub const PAUSE_GESTE_MS: f64 = 100.0;
/// Budget d'une image contenant un reflux en direct (ms) : au-delà, la feuille devient un trou.
pub const BUDGET_MS: f64 = 20.0;
/// En deçà de ce gain (px), une boîte n'est pas considérée comme plus grande (bruit de mesure).
const TOLERANCE_PX: f64 = 0.5;

/// Le geste sur une poignée de split, capturé à l'appui : les tailles À L'ÉCRAN des deux
/// voisins et leur flex-grow. La SOMME des deux flex-grow reste constante : les autres panes
/// ne bougent pas (leurs enfants sont `flex: 1 1 0%`, taille ∝ flex-grow).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GesteSplit {
    depart: f64,
    taille_avant: f64,
    taille_apres: f64,
    somme_grow: f64,
}

impl GesteSplit {
    /// `depart` : la coordonnée du pointeur à l'appui, sur l'axe du split (x pour un split
    /// vertical, y pour un horizontal).
    pub fn new(
        depart: f64,
        taille_avant: f64,
        taille_apres: f64,
        grow_avant: f64,
        grow_apres: f64,
    ) -> Self {
        Self {
            depart,
            taille_avant,
            taille_apres,
            somme_grow: grow_avant + grow_apres,
        }
    }

    /// Les nouveaux flex-grow (avant, après) pour le pointeur en `position`. Chaque voisin
    /// garde au moins `TAILLE_MIN_PANE`.
    pub fn grows(&self, position: f64) -> (f64, f64) {
        let total = self.taille_avant + self.taille_apres;
        if total.is_nan() || total <= 0.0 {
            let moitie = self.somme_grow / 2.0;
            return (moitie, moitie);
        }
        // Même ordre que Fragment : la borne haute d'abord, puis la basse (pas de `clamp`, qui
        // refuserait des bornes croisées quand les deux panes sont déjà sous le minimum).
        let d = (position - self.depart)
            .min(self.taille_apres - TAILLE_MIN_PANE)
            .max(-(self.taille_avant - TAILLE_MIN_PANE));
        (
            self.somme_grow * (self.taille_avant + d) / total,
            self.somme_grow * (self.taille_apres - d) / total,
        )
    }
}

/// La largeur d'un dock pour le pointeur en `x`, bornée à [150, 500] px. Le dock gauche
/// grandit vers la droite, le dock droit vers la gauche.
pub fn largeur_dock(cote: Cote, largeur_depart: f64, x_depart: f64, x: f64) -> f64 {
    let w = match cote {
        Cote::Gauche => largeur_depart + (x - x_depart),
        Cote::Droite => largeur_depart - (x - x_depart),
    };
    // Bornes constantes et ordonnées : `clamp` ne peut pas paniquer.
    w.clamp(LARGEUR_DOCK_MIN, LARGEUR_DOCK_MAX)
}

impl<F> Arbre<F> {
    /// Applique un pas du geste de poignée entre `avant` et `apres` (deux enfants voisins d'un
    /// même split). Le modèle change ; la sauvegarde attend le relâché (`demander_sauvegarde`).
    pub fn appliquer_geste_split(
        &mut self,
        avant: ItemId,
        apres: ItemId,
        geste: &GesteSplit,
        position: f64,
    ) {
        let (ga, gb) = geste.grows(position);
        self.poser_grow(avant, ga);
        self.poser_grow(apres, gb);
    }
}

/// Une boîte (px).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Boite {
    pub largeur: f64,
    pub hauteur: f64,
}

impl Boite {
    pub fn new(largeur: f64, hauteur: f64) -> Self {
        Self { largeur, hauteur }
    }
}

/// Ce que la couche de rendu mesure d'une feuille à l'appui.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesure<K> {
    pub cle: K,
    /// La boîte du contenu de la vue (sous l'en-tête).
    pub contenu: Boite,
    /// La boîte de la vue entière : sa différence avec le contenu est la part de l'en-tête.
    pub vue: Boite,
}

#[derive(Debug, Clone)]
struct Gelee<K> {
    cle: K,
    /// Le maximum atteint : la boîte imposée au contenu.
    boite: Boite,
    /// Ce que l'en-tête prend : place du contenu = boîte de la vue − delta.
    delta: Boite,
}

#[derive(Debug, Clone)]
struct Verification<K> {
    t0: f64,
    t1: Option<f64>,
    refluees: Vec<K>,
}

/// L'automate du gel. Usage : `debut` à l'appui, puis à chaque image `image` (l'horloge),
/// et après chaque écriture de la géométrie `mouvement` ; `fin` au relâché. Chaque appel rend
/// les boîtes à IMPOSER (clé, boîte) ; `fin` rend les clés à libérer (un seul reflux).
#[derive(Debug, Clone)]
pub struct GelDesFeuilles<K> {
    gelees: Vec<Gelee<K>>,
    trous: HashSet<K>,
    verifications: Vec<Verification<K>>,
    dernier_mouvement: f64,
    derniere_image: f64,
    en_cours: bool,
}

impl<K: Clone + Eq + Hash> Default for GelDesFeuilles<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + Eq + Hash> GelDesFeuilles<K> {
    pub fn new() -> Self {
        Self {
            gelees: Vec::new(),
            trous: HashSet::new(),
            verifications: Vec::new(),
            dernier_mouvement: 0.0,
            derniere_image: 0.0,
            en_cours: false,
        }
    }

    /// À l'appui : fige toutes les feuilles mesurées. Une feuille sans boîte (onglet caché,
    /// dock replié) n'a rien à figer. Rend les boîtes imposées.
    pub fn debut(
        &mut self,
        mesures: impl IntoIterator<Item = Mesure<K>>,
        maintenant: f64,
    ) -> Vec<(K, Boite)> {
        self.trous.clear();
        self.verifications.clear();
        self.gelees = mesures
            .into_iter()
            .filter(|m| m.contenu.largeur > 0.0 && m.contenu.hauteur > 0.0)
            .map(|m| Gelee {
                delta: Boite::new(
                    m.vue.largeur - m.contenu.largeur,
                    m.vue.hauteur - m.contenu.hauteur,
                ),
                boite: m.contenu,
                cle: m.cle,
            })
            .collect();
        self.dernier_mouvement = maintenant;
        self.derniere_image = maintenant;
        self.en_cours = true;
        self.gelees
            .iter()
            .map(|g| (g.cle.clone(), g.boite))
            .collect()
    }

    /// Après chaque écriture de la géométrie par le geste : les feuilles en territoire neuf
    /// refluent à leur nouvelle place (rendues), les autres restent rognées ; la pause est
    /// réarmée. `vue` rend la boîte ACTUELLE de la vue d'une feuille.
    pub fn mouvement(
        &mut self,
        vue: impl Fn(&K) -> Option<Boite>,
        maintenant: f64,
    ) -> Vec<(K, Boite)> {
        self.dernier_mouvement = maintenant;
        if !self.en_cours {
            return Vec::new();
        }
        let mut refluees = Vec::new();
        for g in &mut self.gelees {
            if self.trous.contains(&g.cle) {
                continue;
            }
            let Some(v) = vue(&g.cle) else {
                continue;
            };
            if let Some(b) = agrandie(g, v) {
                g.boite = b;
                refluees.push((g.cle.clone(), b));
            }
        }
        if !refluees.is_empty() {
            self.verifications.push(Verification {
                t0: maintenant,
                t1: None,
                refluees: refluees.iter().map(|(k, _)| k.clone()).collect(),
            });
        }
        refluees
    }

    /// Une image à l'instant `t` (l'horodatage d'image du rendu). Tient le garde-fou (les
    /// deux images qui suivent un reflux), puis la pause : 100 ms de calme constatées à une
    /// image courte comblent les trous. Rend les boîtes des trous comblés.
    pub fn image(&mut self, t: f64, vue: impl Fn(&K) -> Option<Boite>) -> Vec<(K, Boite)> {
        if !self.en_cours {
            return Vec::new();
        }
        let mut restantes = Vec::new();
        for mut v in std::mem::take(&mut self.verifications) {
            match v.t1 {
                None => {
                    v.t1 = Some(t);
                    restantes.push(v);
                }
                Some(t1) => {
                    if (t1 - v.t0).max(t - t1) > BUDGET_MS {
                        self.trous.extend(v.refluees);
                    }
                }
            }
        }
        self.verifications = restantes;

        let mut comblees = Vec::new();
        if t - self.dernier_mouvement >= PAUSE_GESTE_MS && t - self.derniere_image <= BUDGET_MS {
            comblees = self.combler(&vue);
            self.dernier_mouvement = t;
        }
        self.derniere_image = t;
        comblees
    }

    fn combler(&mut self, vue: &impl Fn(&K) -> Option<Boite>) -> Vec<(K, Boite)> {
        let mut out = Vec::new();
        for g in &mut self.gelees {
            if !self.trous.contains(&g.cle) {
                continue;
            }
            let Some(v) = vue(&g.cle) else {
                continue;
            };
            if let Some(b) = agrandie(g, v) {
                g.boite = b;
                out.push((g.cle.clone(), b));
            }
        }
        out
    }

    /// Au relâché : tout est libéré, un seul reflux. Rend les clés à libérer.
    pub fn fin(&mut self) -> Vec<K> {
        self.en_cours = false;
        self.trous.clear();
        self.verifications.clear();
        std::mem::take(&mut self.gelees)
            .into_iter()
            .map(|g| g.cle)
            .collect()
    }

    /// La boîte imposée au contenu d'une feuille pendant le geste (`None` : non figée).
    pub fn boite_figee(&self, cle: &K) -> Option<Boite> {
        self.gelees.iter().find(|g| &g.cle == cle).map(|g| g.boite)
    }

    /// Vrai si la feuille a été marquée « trou » par le garde-fou.
    pub fn est_trou(&self, cle: &K) -> bool {
        self.trous.contains(cle)
    }

    pub fn nombre_gelees(&self) -> usize {
        self.gelees.len()
    }

    pub fn en_cours(&self) -> bool {
        self.en_cours
    }
}

/// La nouvelle boîte si la place du contenu dépasse son maximum (sur un axe au moins), le
/// maximum ne descendant jamais ; `None` si rien ne grandit.
fn agrandie<K>(g: &Gelee<K>, vue: Boite) -> Option<Boite> {
    let place = Boite::new(vue.largeur - g.delta.largeur, vue.hauteur - g.delta.hauteur);
    let w = if place.largeur > g.boite.largeur + TOLERANCE_PX {
        place.largeur
    } else {
        g.boite.largeur
    };
    let h = if place.hauteur > g.boite.hauteur + TOLERANCE_PX {
        place.hauteur
    } else {
        g.boite.hauteur
    };
    let b = Boite::new(w, h);
    (b != g.boite).then_some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Ajoutés : les calculs de poignée n'ont pas de test TS ───

    #[test]
    fn geste_split_garde_la_somme_et_borne_au_minimum() {
        let g = GesteSplit::new(500.0, 500.0, 500.0, 1.0, 1.0);
        let (a, b) = g.grows(600.0);
        assert!((a + b - 2.0).abs() < 1e-12);
        assert!((a - 1.2).abs() < 1e-12);
        // Tiré au-delà : le voisin garde 100 px.
        let (a, b) = g.grows(5000.0);
        assert!((a - 1.8).abs() < 1e-12 && (b - 0.2).abs() < 1e-12);
        let (a, _) = g.grows(-5000.0);
        assert!((a - 0.2).abs() < 1e-12);
    }

    #[test]
    fn geste_split_sans_taille_partage_a_parts_egales() {
        let g = GesteSplit::new(0.0, 0.0, 0.0, 0.6, 1.4);
        assert_eq!(g.grows(10.0), (1.0, 1.0));
    }

    #[test]
    fn largeur_dock_suit_le_cote_et_reste_bornee() {
        assert_eq!(largeur_dock(Cote::Gauche, 300.0, 300.0, 350.0), 350.0);
        assert_eq!(largeur_dock(Cote::Droite, 300.0, 700.0, 650.0), 350.0);
        assert_eq!(
            largeur_dock(Cote::Droite, 300.0, 700.0, 0.0),
            LARGEUR_DOCK_MAX
        );
        assert_eq!(
            largeur_dock(Cote::Gauche, 300.0, 300.0, 0.0),
            LARGEUR_DOCK_MIN
        );
    }

    #[test]
    fn debut_ignore_les_feuilles_sans_boite() {
        let mut gel = GelDesFeuilles::new();
        let figees = gel.debut(
            [
                Mesure {
                    cle: 1,
                    contenu: Boite::new(100.0, 100.0),
                    vue: Boite::new(100.0, 130.0),
                },
                Mesure {
                    cle: 2,
                    contenu: Boite::new(0.0, 0.0),
                    vue: Boite::new(0.0, 0.0),
                },
            ],
            0.0,
        );
        assert_eq!(figees, vec![(1, Boite::new(100.0, 100.0))]);
        assert_eq!(gel.fin(), vec![1]);
        assert!(!gel.en_cours());
    }
}
