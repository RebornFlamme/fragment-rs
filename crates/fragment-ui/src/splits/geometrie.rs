// Port de app/src/ui/workspace.css (`.workspace`, `.workspace-ribbon`, `.workspace-sidedock`,
// `.workspace-sidebar-resize-handle`, `.workspace-root`, `.workspace-split`, `.workspace-resize-handle`)
// et de la répartition flex de WorkspaceSplit.ts.
//
//! La mise en page du workspace, SANS GPUI : la place de chaque nœud de l'arbre dans la fenêtre.
//!
//! Fragment laisse ce calcul au navigateur (flexbox) ; ici on le fait nous-mêmes, une fois par image,
//! depuis le modèle du cœur et la taille de la fenêtre. Trois raisons :
//! - **le pixel** : Chromium cumule les longueurs exactes et n'arrondit qu'à la peinture, arête par
//!   arête, l'égalité vers le haut ; GPUI arrondit autrement (voir `docs/constantes-rendu.md`,
//!   « Positions fractionnaires »). Un split 25/50/25 a des arêtes fractionnaires : on les calcule
//!   exactes et on les cale comme Chromium ([`arrondi`]) ;
//! - **le hit-test** : la pile sous un point (glisser d'onglet, espace des boutons de fenêtre) se lit
//!   dans le même calcul, sans attendre une mesure de l'image précédente ;
//! - **le gel du geste** : la boîte de chaque feuille APRÈS un pas du geste est connue tout de suite.
//!
//! Le rendu ([`super::rendre_workspace`]) pose chaque pile, chaque poignée et chaque dock en position
//! ABSOLUE à son cadre calé : l'arbre des éléments n'a pas à refléter celui des splits.

use std::collections::HashMap;

use fragment_core::{Arbre, Cadre, Cote, ItemId, SplitDirection};

/// La largeur d'un dock que personne n'a encore tiré (`--sidebar-width`, workspace.css:52).
pub const LARGEUR_DOCK_DEFAUT: f64 = 260.0;
/// L'épaisseur du trait d'une poignée entre deux panes (workspace.css:191-192).
pub const EPAISSEUR_POIGNEE: f64 = 1.0;
/// La zone attrapable d'une poignée déborde de 3 px de chaque côté du trait (workspace.css:196-197).
pub const DEBORD_POIGNEE: f64 = 3.0;
/// Le trait survolé ou tiré s'élargit à 3 px (workspace.css:214, 218).
pub const TRAIT_POIGNEE_ACTIVE: f64 = 3.0;

/// Les longueurs du thème dont dépend la mise en page (en px logiques, avant zoom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mesures {
    /// `--header-height` : la largeur du ruban, la hauteur des barres d'onglets.
    pub entete: f64,
    /// `--border-width` : la bordure intérieure d'un dock.
    pub bordure: f64,
    /// Le facteur de zoom : l'arrondi se fait en pixels de l'écran.
    pub zoom: f64,
    /// La hauteur du pied du dock gauche (le sélecteur de coffre, `VaultSwitcher`), 0 sans pied :
    /// les piles du dock s'arrêtent au-dessus.
    pub pied_gauche: f64,
}

impl Default for Mesures {
    fn default() -> Self {
        Self {
            entete: 36.0,
            bordure: 1.0,
            zoom: 1.0,
            pied_gauche: 0.0,
        }
    }
}

/// Une poignée entre deux enfants voisins d'un split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Poignee {
    /// Le split qui porte la poignée.
    pub split: ItemId,
    /// L'enfant avant (à gauche, ou au-dessus).
    pub avant: ItemId,
    /// L'enfant après.
    pub apres: ItemId,
    /// La direction du split : `Vertical` = trait vertical (enfants côte à côte).
    pub direction: SplitDirection,
    /// Le trait de 1 px, calé (le repère de la zone attrapable).
    pub trait_: Cadre,
    /// Le trait de 1 px à sa position EXACTE, souvent fractionnaire : c'est elle que Chromium
    /// peint (voir [`couverture`]).
    pub exact: Cadre,
}

impl Poignee {
    /// La zone attrapable (7 px de large) : le trait, plus 3 px de chaque côté.
    pub fn zone(&self) -> Cadre {
        let t = self.trait_;
        match self.direction {
            SplitDirection::Vertical => Cadre::new(
                t.x - DEBORD_POIGNEE,
                t.y,
                t.largeur + 2.0 * DEBORD_POIGNEE,
                t.hauteur,
            ),
            SplitDirection::Horizontal => Cadre::new(
                t.x,
                t.y - DEBORD_POIGNEE,
                t.largeur,
                t.hauteur + 2.0 * DEBORD_POIGNEE,
            ),
        }
    }
}

/// La poignée d'un dock : de largeur nulle, posée sur la frontière entre le dock et le centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoigneeDock {
    pub dock: ItemId,
    pub cote: Cote,
    /// L'abscisse de la frontière.
    pub x: f64,
    pub y: f64,
    pub hauteur: f64,
}

impl PoigneeDock {
    /// La zone attrapable : 3 px de chaque côté de la frontière (workspace.css:133).
    pub fn zone(&self) -> Cadre {
        Cadre::new(
            self.x - DEBORD_POIGNEE,
            self.y,
            2.0 * DEBORD_POIGNEE,
            self.hauteur,
        )
    }
}

/// Un dock visible : sa boîte entière (bordure comprise) et la part de ses piles (sans la bordure).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CadreDock {
    pub dock: ItemId,
    pub cote: Cote,
    /// La boîte entière, bordure comprise (`getBoundingClientRect` du dock).
    pub boite: Cadre,
    /// La place des piles (la boîte moins la bordure).
    pub contenu: Cadre,
    /// Le trait de la bordure (à droite du dock gauche, à gauche du dock droit).
    pub bordure: Cadre,
}

/// La mise en page d'une image : un cadre calé par nœud visible, les poignées, les docks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Disposition {
    /// Toute la fenêtre (le `.workspace`).
    pub fenetre: Cadre,
    /// Le ruban, à gauche.
    pub ruban: Cadre,
    cadres: HashMap<ItemId, Cadre>,
    /// Les piles visibles, dans l'ordre du parcours (centre, dock gauche, dock droit).
    pub piles: Vec<ItemId>,
    pub poignees: Vec<Poignee>,
    pub poignees_docks: Vec<PoigneeDock>,
    pub docks: Vec<CadreDock>,
    /// Le pied du dock gauche (sélecteur de coffre), calé, s'il est à l'écran.
    pub pied_gauche: Option<Cadre>,
}

impl Disposition {
    /// Le cadre calé d'un nœud ; `None` s'il n'est pas à l'écran (dock replié, onglet caché).
    pub fn cadre(&self, id: ItemId) -> Option<Cadre> {
        self.cadres.get(&id).copied()
    }

    /// La barre d'onglets d'une pile (en haut, `--header-height`).
    pub fn barre(&self, pile: ItemId, entete: f64) -> Option<Cadre> {
        self.cadre(pile)
            .map(|c| Cadre::new(c.x, c.y, c.largeur, entete.min(c.hauteur)))
    }

    /// Le corps d'une pile (sous la barre) : la place de la feuille visible.
    pub fn corps(&self, pile: ItemId, entete: f64) -> Option<Cadre> {
        self.cadre(pile).map(|c| {
            let h = entete.min(c.hauteur);
            Cadre::new(c.x, c.y + h, c.largeur, c.hauteur - h)
        })
    }

    /// Le dock visible d'un côté.
    pub fn dock(&self, cote: Cote) -> Option<&CadreDock> {
        self.docks.iter().find(|d| d.cote == cote)
    }
}

/// Un pixel d'écran partiellement ou entièrement couvert par un trait : sa position et sa longueur
/// (px logiques) et la part couverte (0 à 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Couvert {
    pub debut: f64,
    pub longueur: f64,
    pub part: f32,
}

/// ★ Les pixels couverts par le segment exact [`debut`, `fin`) (px logiques), comme les peint
/// Chromium quand le trait n'est PAS calé : le trait d'une poignée est un pseudo-élément centré par
/// `transform: translate(-50%)`, et une transformation échappe à l'arrondi des boîtes. À 621,5 px, le
/// trait de 1 px couvre deux pixels à moitié (mesuré sur `t1-split-vertical` : deux pixels au mélange
/// 50 / 50). Rend les pixels de bord (part fractionnaire) et le plein du milieu (part 1), dans l'ordre ;
/// le rendu les peint en quads ENTIERS, la couleur pondérée par la part : même mélange que l'anticrénelage
/// analytique de Skia, sans dépendre de celui de GPUI.
pub fn couverture(debut: f64, fin: f64, zoom: f64) -> Vec<Couvert> {
    let (a, b) = (debut * zoom, fin * zoom);
    if b <= a {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (pa, pb) = (a.floor(), b.ceil());
    let mut p = pa;
    let eps = 1e-6;
    while p < pb - eps {
        let part = ((p + 1.0).min(b) - p.max(a)).clamp(0.0, 1.0);
        if part > eps {
            match out.last_mut() {
                // Les pixels pleins consécutifs se fondent en un seul quad.
                Some(Couvert {
                    debut,
                    longueur,
                    part: dernier,
                }) if part > 1.0 - eps
                    && *dernier >= 1.0
                    && (*debut + *longueur - p / zoom).abs() < eps =>
                {
                    *longueur += 1.0 / zoom;
                }
                _ => out.push(Couvert {
                    debut: p / zoom,
                    longueur: 1.0 / zoom,
                    part: if part > 1.0 - eps { 1.0 } else { part as f32 },
                }),
            }
        }
        p += 1.0;
    }
    out
}

/// ★ Un canal de `teinte` posé sur `fond` à la couverture `part` (0 à 1), comme le mélange
/// d'anticrénelage de Chromium : `fond + ⌊(teinte − fond) × échelle / 256⌋`, l'échelle étant la part
/// en 256es. Mesuré sur les traits de poignée à cheval sur deux pixels (`t1-split-vertical`) : 34
/// sur 15 et 37 sur 21 en sombre (teinte 54), 222 sur 242 et 214 sur 225 en clair ; l'alpha de GPUI
/// (flottant arrondi au plus proche) tombait un niveau à côté sur chaque différence impaire.
pub fn melange_couverture(teinte: u8, fond: u8, part: f32) -> u8 {
    let echelle = (part.clamp(0.0, 1.0) * 256.0).round() as i32;
    let d = (i32::from(teinte) - i32::from(fond)) * echelle;
    (i32::from(fond) + (d >> 8)).clamp(0, 255) as u8
}

/// Une arête exacte calée comme Chromium : au pixel de l'écran le plus proche, l'égalité vers le
/// HAUT (`floor(v + 0,5)`), puis ramenée en px logiques.
pub fn arrondi(v: f64, zoom: f64) -> f64 {
    // Marge contre l'erreur de flottant : une égalité exacte (x,5) doit s'arrondir vers le haut.
    (v * zoom + 0.5 + 1e-6).floor() / zoom
}

/// Un rectangle exact calé par ses ARÊTES (pas par sa largeur) : deux voisins bout à bout restent
/// bout à bout, sans trou ni recouvrement.
pub fn caler(c: Cadre, zoom: f64) -> Cadre {
    let (x0, y0) = (arrondi(c.x, zoom), arrondi(c.y, zoom));
    let (x1, y1) = (arrondi(c.droite(), zoom), arrondi(c.bas(), zoom));
    Cadre::new(x0, y0, x1 - x0, y1 - y0)
}

/// Le dock est-il à l'écran ? Déplié ET porteur d'au moins une pile : un dock vide fait 0 px
/// (`.workspace-sidedock:not(:has(.workspace-tabs))`, workspace.css:86), bordure et poignée comprises.
pub fn dock_visible<F>(arbre: &Arbre<F>, dock: ItemId) -> bool {
    arbre.dock_ouvert(dock) && !arbre.piles_sous(dock).is_empty()
}

/// La mise en page de l'arbre dans une fenêtre de `largeur` × `hauteur` px logiques.
///
/// De gauche à droite (`.workspace`, une ligne flex) : le ruban (`--header-height` de large), le
/// dock gauche (largeur propre + bordure droite), sa poignée (largeur nulle), le centre (le reste),
/// la poignée du dock droit, le dock droit (bordure gauche + largeur propre).
///
/// ★ La largeur d'un dock est sa largeur de CONTENU : la bordure s'y ajoute (le `box-sizing` de
///   Fragment est celui par défaut, `content-box`).
pub fn disposer<F>(arbre: &Arbre<F>, largeur: f64, hauteur: f64, m: Mesures) -> Disposition {
    let mut d = Disposition {
        fenetre: Cadre::new(0.0, 0.0, largeur, hauteur),
        ..Disposition::default()
    };
    let z = m.zoom;
    d.ruban = caler(Cadre::new(0.0, 0.0, m.entete.min(largeur), hauteur), z);

    let largeur_dock = |dock: ItemId| {
        arbre
            .sidedock(dock)
            .and_then(|s| s.width)
            .filter(|w| w.is_finite() && *w >= 0.0)
            .unwrap_or(LARGEUR_DOCK_DEFAUT)
    };

    let mut gauche = m.entete.min(largeur);
    let mut droite = largeur;
    let mut docks = Vec::new();
    let (g, dr) = (arbre.left(), arbre.right());
    if dock_visible(arbre, g) {
        let w = largeur_dock(g);
        // ★ Le pied (`.workspace-drawer-vault-switcher`, `order: 1; margin-top: auto`) est le dernier
        //   enfant en flux de la colonne du dock : les piles (`flex: 1`) prennent le reste, au-dessus.
        let pied = m.pied_gauche.clamp(0.0, hauteur);
        let contenu = Cadre::new(gauche, 0.0, w, hauteur - pied);
        if pied > 0.0 {
            d.pied_gauche = Some(caler(Cadre::new(gauche, hauteur - pied, w, pied), z));
        }
        let bordure = Cadre::new(gauche + w, 0.0, m.bordure, hauteur);
        let boite = Cadre::new(gauche, 0.0, w + m.bordure, hauteur);
        gauche += w + m.bordure;
        docks.push((g, Cote::Gauche, boite, contenu, bordure));
    }
    if dock_visible(arbre, dr) {
        let w = largeur_dock(dr);
        let boite = Cadre::new(droite - w - m.bordure, 0.0, w + m.bordure, hauteur);
        let bordure = Cadre::new(boite.x, 0.0, m.bordure, hauteur);
        let contenu = Cadre::new(boite.x + m.bordure, 0.0, w, hauteur);
        droite = boite.x;
        docks.push((dr, Cote::Droite, boite, contenu, bordure));
    }

    // Le centre d'abord : c'est l'ordre du hit-test (`get_tabs_at_point`).
    let centre = Cadre::new(gauche, 0.0, (droite - gauche).max(0.0), hauteur);
    poser(
        arbre,
        arbre.root(),
        centre,
        Some(SplitDirection::Vertical),
        &mut d,
        z,
    );
    for (dock, cote, boite, contenu, bordure) in docks {
        poser(
            arbre,
            dock,
            contenu,
            Some(SplitDirection::Horizontal),
            &mut d,
            z,
        );
        // Le cadre du dock est sa boîte entière (bordure comprise), pas la place de ses piles.
        d.cadres.insert(dock, caler(boite, z));
        let x = match cote {
            Cote::Gauche => boite.droite(),
            Cote::Droite => boite.x,
        };
        d.poignees_docks.push(PoigneeDock {
            dock,
            cote,
            x: arrondi(x, z),
            y: 0.0,
            hauteur: arrondi(hauteur, z),
        });
        d.docks.push(CadreDock {
            dock,
            cote,
            boite: caler(boite, z),
            contenu: caler(contenu, z),
            bordure: caler(bordure, z),
        });
    }
    d
}

/// Pose `id` dans le rectangle EXACT `r`, puis ses enfants. `axe` force l'axe d'AFFICHAGE d'un split
/// (`Vertical` = une ligne) : le centre est une ligne, un dock une colonne (workspace.css:61, 169).
fn poser<F>(
    arbre: &Arbre<F>,
    id: ItemId,
    r: Cadre,
    axe: Option<SplitDirection>,
    d: &mut Disposition,
    z: f64,
) {
    d.cadres.insert(id, caler(r, z));
    if arbre.est_pile(id) {
        d.piles.push(id);
        return;
    }
    let Some(split) = arbre.split(id) else {
        return;
    };
    let enfants = arbre.children(id).to_vec();
    if enfants.is_empty() {
        return;
    }
    let n = enfants.len();
    // ★ L'axe d'affichage n'est pas toujours la direction du split. Le centre (`.workspace-root`) est
    //   TOUJOURS une ligne et un dock TOUJOURS une colonne : leur classe `mod-*` ne règle que leurs
    //   poignées. Quand les deux ne s'accordent pas (le centre en `horizontal`), la poignée n'a pas
    //   d'épaisseur sur l'axe d'affichage : 0 px, ni trait ni prise (vu sur Fragment, 11/11).
    let ligne = match axe {
        Some(a) => a == SplitDirection::Vertical,
        None => split.direction == SplitDirection::Vertical,
    };
    let direction = split.direction;
    let avec_poignees = (direction == SplitDirection::Vertical) == ligne;
    let epaisseur = if avec_poignees {
        EPAISSEUR_POIGNEE
    } else {
        0.0
    };
    let longueur = if ligne { r.largeur } else { r.hauteur };
    let poignees = epaisseur * (n - 1) as f64;
    let disponible = (longueur - poignees).max(0.0);
    let grows: Vec<f64> = enfants.iter().map(|&c| arbre.grow(c).max(0.0)).collect();
    let somme: f64 = grows.iter().sum();
    // ★ La règle de flexbox (CSS Flexbox §9.7), reproduite telle quelle : si la somme des
    //   flex-grow est sous 1, le conteneur ne distribue que CETTE FRACTION de la place libre, et un
    //   trou reste au bout. Le cœur normalise pour l'éviter (`normalize_grows`) ; le rendu montre ce
    //   que le modèle dit, comme Chromium le ferait.
    let part = |g: f64| {
        if somme <= 0.0 {
            disponible / n as f64
        } else if somme < 1.0 {
            disponible * g
        } else {
            disponible * g / somme
        }
    };
    let mut pos = if ligne { r.x } else { r.y };
    for (i, (&enfant, &g)) in enfants.iter().zip(&grows).enumerate() {
        let taille = part(g);
        let sous = if ligne {
            Cadre::new(pos, r.y, taille, r.hauteur)
        } else {
            Cadre::new(r.x, pos, r.largeur, taille)
        };
        poser(arbre, enfant, sous, None, d, z);
        pos += taille;
        if let Some(&apres) = enfants.get(i + 1)
            && avec_poignees
        {
            let t = if ligne {
                Cadre::new(pos, r.y, EPAISSEUR_POIGNEE, r.hauteur)
            } else {
                Cadre::new(r.x, pos, r.largeur, EPAISSEUR_POIGNEE)
            };
            d.poignees.push(Poignee {
                split: id,
                avant: enfant,
                apres,
                direction,
                trait_: caler(t, z),
                exact: t,
            });
            pos += EPAISSEUR_POIGNEE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbre_deux_piles(direction: SplitDirection) -> (Arbre<()>, ItemId, ItemId) {
        let mut a: Arbre<()> = Arbre::new();
        let root = a.root();
        a.set_direction(root, direction);
        let (p1, p2) = (a.creer_pile(), a.creer_pile());
        a.add_child(root, p1, None).unwrap();
        a.add_child(root, p2, None).unwrap();
        (a, p1, p2)
    }

    #[test]
    fn le_melange_de_couverture_tombe_comme_chromium() {
        // Mesures de t1-split-vertical (canal vert sombre, canal bleu clair).
        assert_eq!(melange_couverture(54, 15, 0.5), 34);
        assert_eq!(melange_couverture(54, 21, 0.5), 37);
        assert_eq!(melange_couverture(203, 242, 0.5), 222);
        assert_eq!(melange_couverture(203, 225, 0.5), 214);
        assert_eq!(melange_couverture(54, 21, 1.0), 54);
        assert_eq!(melange_couverture(54, 21, 0.0), 21);
    }

    #[test]
    fn l_arrondi_va_au_plus_proche_et_l_egalite_vers_le_haut() {
        assert_eq!(arrondi(657.5, 1.0), 658.0);
        assert_eq!(arrondi(657.49, 1.0), 657.0);
        assert_eq!(arrondi(10.25, 2.0), 10.5);
    }

    #[test]
    fn deux_piles_cote_a_cote_se_partagent_le_centre_autour_d_une_poignee_de_1_px() {
        let (a, p1, p2) = arbre_deux_piles(SplitDirection::Vertical);
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        // 1280 − 36 (ruban) = 1244, moins 1 de poignée : 621,5 chacun, arêtes calées.
        assert_eq!(d.cadre(p1), Some(Cadre::new(36.0, 0.0, 622.0, 800.0)));
        assert_eq!(d.poignees.len(), 1);
        assert_eq!(d.poignees[0].trait_, Cadre::new(658.0, 0.0, 1.0, 800.0));
        assert_eq!(d.cadre(p2), Some(Cadre::new(659.0, 0.0, 621.0, 800.0)));
    }

    #[test]
    fn une_somme_de_grows_sous_1_laisse_un_trou_comme_flexbox() {
        let (mut a, p1, p2) = arbre_deux_piles(SplitDirection::Vertical);
        a.poser_grow(p1, 0.3);
        a.poser_grow(p2, 0.3);
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        let fin = d.cadre(p2).unwrap().droite();
        assert!(fin < 1280.0 - 100.0, "il reste un trou : {fin}");
    }

    #[test]
    fn le_centre_reste_une_ligne_meme_en_horizontal_et_ses_poignees_n_ont_pas_d_epaisseur() {
        let (a, p1, p2) = arbre_deux_piles(SplitDirection::Horizontal);
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        assert_eq!(d.cadre(p1), Some(Cadre::new(36.0, 0.0, 622.0, 800.0)));
        assert_eq!(d.cadre(p2), Some(Cadre::new(658.0, 0.0, 622.0, 800.0)));
        assert!(d.poignees.is_empty());
    }

    #[test]
    fn un_split_horizontal_imbrique_empile_ses_piles() {
        let mut a: Arbre<()> = Arbre::new();
        let root = a.root();
        let s = a.creer_split(SplitDirection::Horizontal);
        a.add_child(root, s, None).unwrap();
        let (p1, p2) = (a.creer_pile(), a.creer_pile());
        a.add_child(s, p1, None).unwrap();
        a.add_child(s, p2, None).unwrap();
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        assert_eq!(d.cadre(p1), Some(Cadre::new(36.0, 0.0, 1244.0, 400.0)));
        assert_eq!(d.poignees[0].exact, Cadre::new(36.0, 399.5, 1244.0, 1.0));
        assert_eq!(d.cadre(p2), Some(Cadre::new(36.0, 401.0, 1244.0, 399.0)));
    }

    #[test]
    fn un_trait_a_cheval_sur_deux_pixels_les_couvre_a_moitie() {
        let c = couverture(657.5, 658.5, 1.0);
        assert_eq!(
            c,
            vec![
                Couvert {
                    debut: 657.0,
                    longueur: 1.0,
                    part: 0.5
                },
                Couvert {
                    debut: 658.0,
                    longueur: 1.0,
                    part: 0.5
                },
            ]
        );
        // Élargi à 3 px autour du même centre : deux demi-pixels, deux pleins fondus en un.
        let c = couverture(656.5, 659.5, 1.0);
        assert_eq!(c.len(), 3);
        assert_eq!(
            c[1],
            Couvert {
                debut: 657.0,
                longueur: 2.0,
                part: 1.0
            }
        );
        // Aligné : un seul pixel plein.
        assert_eq!(
            couverture(658.0, 659.0, 1.0),
            vec![Couvert {
                debut: 658.0,
                longueur: 1.0,
                part: 1.0
            }]
        );
    }

    #[test]
    fn un_dock_vide_ou_replie_ne_prend_aucune_place() {
        let (mut a, p1, _) = arbre_deux_piles(SplitDirection::Vertical);
        let droit = a.right();
        a.expand(droit);
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        assert!(d.docks.is_empty(), "déplié mais vide : 0 px");
        let pile = a.creer_pile();
        a.add_child(droit, pile, None).unwrap();
        a.collapse(droit);
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        assert!(d.docks.is_empty(), "replié : 0 px");
        assert_eq!(d.cadre(pile), None);
        assert_eq!(d.cadre(p1).unwrap().x, 36.0);
    }

    #[test]
    fn un_dock_ouvert_prend_sa_largeur_plus_sa_bordure() {
        let (mut a, _, p2) = arbre_deux_piles(SplitDirection::Vertical);
        let droit = a.right();
        let pile = a.creer_pile();
        a.add_child(droit, pile, None).unwrap();
        a.expand(droit);
        let d = disposer(&a, 1280.0, 800.0, Mesures::default());
        let dock = d.dock(Cote::Droite).unwrap();
        assert_eq!(dock.boite, Cadre::new(1019.0, 0.0, 261.0, 800.0));
        assert_eq!(dock.bordure, Cadre::new(1019.0, 0.0, 1.0, 800.0));
        assert_eq!(d.cadre(pile), Some(Cadre::new(1020.0, 0.0, 260.0, 800.0)));
        assert_eq!(d.cadre(p2).unwrap().droite(), 1019.0);
        assert_eq!(d.poignees_docks[0].x, 1019.0);
    }
}
