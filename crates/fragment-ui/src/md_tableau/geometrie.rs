// Port des règles de mise en page d'app/src/ui/views/markdown/table/table.css et de la géométrie des
// poignées de TableWidget.ts (placeGrip, boundaries, showDropBar).
//! La géométrie PURE du tableau : largeurs de colonnes (mise en page automatique d'un `<table>`),
//! frontières de la grille, cellule sous un point, cible d'un glisser, boîtes des bandes « + » et
//! des poignées. Aucune dépendance au rendu : les largeurs de texte arrivent mesurées.

use fragment_theme::Theme;

/// `.cm-table-add-col { width: 18px }`, `.cm-table-add-row { height: 18px }` (table.css:114, 121).
pub const EPAISSEUR_BANDE: f32 = 18.0;
/// `.cm-table-grip-col { height: 12px }`, `.cm-table-grip-row { width: 12px }` (table.css:175, 179).
pub const EPAISSEUR_POIGNEE: f32 = 12.0;
/// `.cm-table-grip-col.is-active { height: 14px }` (table.css:200).
pub const EPAISSEUR_POIGNEE_ACTIVE: f32 = 14.0;
/// `.cm-table-cell .cm-editor { min-width: 6em }` (table.css:75) : la largeur minimale d'une colonne.
pub const LARGEUR_MIN_EM: f32 = 6.0;
/// `.cm-table-cell .cm-editor .cm-content { min-height: 1.5em }` (table.css:88).
pub const HAUTEUR_MIN_EM: f32 = 1.5;
/// `.cm-table-widget::-webkit-scrollbar { height: 6px }` (table.css:27) : présente si le bloc défile.
pub const HAUTEUR_BARRE: f32 = 6.0;
/// La barre d'insertion d'un glisser (`showDropBar` : 4 px, centrée sur la frontière).
pub const EPAISSEUR_INSERTION: f32 = 4.0;
/// Seuil (px, distance de Manhattan) au-delà duquel un appui sur une poignée devient un glisser.
pub const SEUIL_GLISSER: f32 = 4.0;

/// Les réglages de mise en page, lus dans les jetons du thème et la taille du texte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reglages {
    /// `--font-text-size`.
    pub taille: f32,
    /// `--border-width` (bordure des cellules, `border-collapse: collapse`).
    pub bordure: f32,
    /// `.cm-table-cell .cm-editor .cm-content { padding: var(--size-4-1) var(--size-4-2) }`.
    pub marge_cellule_v: f32,
    pub marge_cellule_h: f32,
    /// `--line-height-normal` : l'interligne d'une cellule (multiple de la taille).
    pub interligne: f32,
    /// `.cm-table-widget { padding: --size-2-7 --size-2-11 --size-4-5 --size-4-3 }` (haut, droite,
    /// bas, gauche).
    pub marges_bloc: [f32; 4],
    /// `margin-left` / `margin-top` des bandes : `--size-2-1`.
    pub marge_bande: f32,
}

impl Reglages {
    /// Les réglages du thème pour un texte de `taille` px.
    #[must_use]
    pub fn du_theme(theme: &Theme, taille: f32) -> Reglages {
        let f = &theme.forme;
        Reglages {
            taille,
            bordure: f.border_width,
            marge_cellule_v: f.size_4_1,
            marge_cellule_h: f.size_4_2,
            interligne: theme.typo.line_height_normal,
            marges_bloc: [f.size_2_7, f.size_2_11, f.size_4_5, f.size_4_3],
            marge_bande: f.size_2_1,
        }
    }

    /// La hauteur d'une ligne de texte de cellule.
    #[must_use]
    pub fn ligne_texte(&self) -> f32 {
        self.taille * self.interligne
    }

    /// La hauteur d'une rangée dont le texte le plus haut mesure `contenu` px.
    #[must_use]
    pub fn hauteur_rangee(&self, contenu: f32) -> f32 {
        contenu.max(HAUTEUR_MIN_EM * self.taille) + 2.0 * self.marge_cellule_v
    }

    /// La largeur minimale (éditeur de cellule, marges comprises) d'une colonne.
    #[must_use]
    pub fn largeur_min(&self) -> f32 {
        LARGEUR_MIN_EM * self.taille
    }
}

/// La largeur min-content et max-content d'une cellule, marges comprises : `mots` = largeur du mot
/// le plus long, `ligne` = largeur du texte entier sans retour.
#[must_use]
pub fn largeurs_cellule(r: &Reglages, mots: f32, ligne: f32) -> (f32, f32) {
    let m = 2.0 * r.marge_cellule_h;
    let min = (mots + m).max(r.largeur_min());
    (min, (ligne + m).max(min))
}

/// La mise en page automatique d'un `<table>` qui RÉTRÉCIT pour tenir (`shrink-to-fit`) : les
/// colonnes prennent leur max-content si tout tient dans `dispo`, sinon la place est partagée entre
/// min et max au prorata de leur écart ; sous la somme des min, la grille déborde (le bloc défile).
///
/// `dispo` est la largeur offerte aux colonnes (bordures retirées).
#[must_use]
pub fn largeurs_colonnes(min: &[f32], max: &[f32], dispo: f32) -> Vec<f32> {
    let somme_min: f32 = min.iter().sum();
    let somme_max: f32 = max.iter().sum();
    if somme_max <= dispo {
        return max.to_vec();
    }
    if somme_min >= dispo || somme_max <= somme_min {
        return min.to_vec();
    }
    let k = (dispo - somme_min) / (somme_max - somme_min);
    min.iter()
        .zip(max)
        .map(|(&a, &b)| a + (b - a) * k)
        .collect()
}

/// La grille posée : les frontières (bord gauche des traits verticaux, haut des traits
/// horizontaux), relatives au coin haut-gauche de la grille. `N + 1` valeurs chacune.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grille {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub bordure: f32,
}

impl Grille {
    /// Les frontières de colonnes de `largeurs` et de rangées de `hauteurs`, traits de `bordure`
    /// partagés (`border-collapse`).
    #[must_use]
    pub fn new(largeurs: &[f32], hauteurs: &[f32], bordure: f32) -> Grille {
        let cumul = |v: &[f32]| {
            let mut out = Vec::with_capacity(v.len() + 1);
            let mut a = 0.0;
            out.push(a);
            for &w in v {
                a += bordure + w;
                out.push(a);
            }
            out
        };
        Grille {
            x: cumul(largeurs),
            y: cumul(hauteurs),
            bordure,
        }
    }

    #[must_use]
    pub fn largeur(&self) -> f32 {
        self.x.last().copied().unwrap_or(0.0) + self.bordure
    }

    #[must_use]
    pub fn hauteur(&self) -> f32 {
        self.y.last().copied().unwrap_or(0.0) + self.bordure
    }

    #[must_use]
    pub fn colonnes(&self) -> usize {
        self.x.len().saturating_sub(1)
    }

    #[must_use]
    pub fn rangees(&self) -> usize {
        self.y.len().saturating_sub(1)
    }

    /// La boîte (gauche, haut, largeur, hauteur) de la cellule (r, c), bordures comprises.
    #[must_use]
    pub fn cellule(&self, r: usize, c: usize) -> Option<(f32, f32, f32, f32)> {
        let (x0, x1) = (*self.x.get(c)?, *self.x.get(c + 1)?);
        let (y0, y1) = (*self.y.get(r)?, *self.y.get(r + 1)?);
        Some((x0, y0, x1 - x0 + self.bordure, y1 - y0 + self.bordure))
    }

    /// La cellule sous le point (x, y) de la grille, bordures comprises.
    #[must_use]
    pub fn cellule_a(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        if x < 0.0 || y < 0.0 || x >= self.largeur() || y >= self.hauteur() {
            return None;
        }
        let c = self.x.partition_point(|&b| b <= x).saturating_sub(1);
        let r = self.y.partition_point(|&b| b <= y).saturating_sub(1);
        Some((
            r.min(self.rangees().saturating_sub(1)),
            c.min(self.colonnes().saturating_sub(1)),
        ))
    }
}

/// L'index de la frontière la plus proche de `p` (`boundaries` + recherche du minimum ; à égalité,
/// la première).
#[must_use]
pub fn frontiere_proche(frontieres: &[f32], p: f32) -> usize {
    let mut meilleure = 0;
    for (i, b) in frontieres.iter().enumerate() {
        if (b - p).abs() < (frontieres[meilleure] - p).abs() {
            meilleure = i;
        }
    }
    meilleure
}

/// La destination d'un glisser de l'élément `index` lâché à la frontière `cible` : `None` s'il ne
/// bouge pas.
#[must_use]
pub fn destination(index: usize, cible: usize) -> Option<usize> {
    let to = if cible > index { cible - 1 } else { cible };
    (to != index).then_some(to)
}

/// Une boîte (gauche, haut, largeur, hauteur) relative au coin haut-gauche de la grille.
pub type Boite = (f32, f32, f32, f32);

/// La bande « + » de droite : à `margin-left` du bord droit, sur toute la hauteur.
#[must_use]
pub fn bande_colonne(g: &Grille, r: &Reglages) -> Boite {
    (
        g.largeur() + r.marge_bande,
        0.0,
        EPAISSEUR_BANDE,
        g.hauteur(),
    )
}

/// La bande « + » du bas : à `margin-top` du bord bas, sur toute la largeur.
#[must_use]
pub fn bande_rangee(g: &Grille, r: &Reglages) -> Boite {
    (
        0.0,
        g.hauteur() + r.marge_bande,
        g.largeur(),
        EPAISSEUR_BANDE,
    )
}

/// La poignée de la colonne `c` : au-dessus de la grille (`translateY(-100%)`), sur la largeur de
/// la cellule d'en-tête.
#[must_use]
pub fn poignee_colonne(g: &Grille, c: usize, active: bool) -> Option<Boite> {
    let (x, _, w, _) = g.cellule(0, c)?;
    let h = if active {
        EPAISSEUR_POIGNEE_ACTIVE
    } else {
        EPAISSEUR_POIGNEE
    };
    Some((x, -h, w, h))
}

/// La poignée de la rangée `r` : à gauche de la grille (`translateX(-100%)`).
#[must_use]
pub fn poignee_rangee(g: &Grille, r: usize) -> Option<Boite> {
    let (_, y, _, h) = g.cellule(r, 0)?;
    Some((-EPAISSEUR_POIGNEE, y, EPAISSEUR_POIGNEE, h))
}

/// La colonne ou la rangée soulevée (`.cm-table-selection`).
#[must_use]
pub fn soulevee(g: &Grille, colonne: bool, index: usize) -> Option<Boite> {
    if colonne {
        let (x, _, w, _) = g.cellule(0, index)?;
        Some((x, 0.0, w, g.hauteur()))
    } else {
        let (_, y, _, h) = g.cellule(index, 0)?;
        Some((0.0, y, g.largeur(), h))
    }
}

/// La barre d'insertion d'un glisser à la frontière d'abscisse (ou d'ordonnée) `a`.
#[must_use]
pub fn barre_insertion(g: &Grille, colonne: bool, a: f32) -> Boite {
    let e = EPAISSEUR_INSERTION;
    if colonne {
        (a - e / 2.0, 0.0, e, g.hauteur())
    } else {
        (0.0, a - e / 2.0, g.largeur(), e)
    }
}

/// La hauteur du bloc (`.cm-table-widget`) : ses marges, la grille, et la barre de défilement si la
/// grille déborde de la colonne de `largeur_bloc` px.
#[must_use]
pub fn hauteur_bloc(g: &Grille, r: &Reglages, largeur_bloc: f32) -> f32 {
    let [h, _, b, _] = r.marges_bloc;
    let barre = if deborde(g, r, largeur_bloc) {
        HAUTEUR_BARRE
    } else {
        0.0
    };
    h + g.hauteur() + b + barre
}

/// La grille (et sa bande de droite) dépasse-t-elle la boîte du bloc ?
#[must_use]
pub fn deborde(g: &Grille, r: &Reglages, largeur_bloc: f32) -> bool {
    let [_, d, _, gauche] = r.marges_bloc;
    gauche + g.largeur() + d > largeur_bloc + 0.5
}

/// La largeur offerte aux colonnes dans un bloc de `largeur_bloc` px pour `n` colonnes.
#[must_use]
pub fn place_des_colonnes(r: &Reglages, largeur_bloc: f32, n: usize) -> f32 {
    let [_, d, _, g] = r.marges_bloc;
    #[allow(clippy::cast_precision_loss)]
    let traits = (n + 1) as f32 * r.bordure;
    (largeur_bloc - g - d - traits).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reglages() -> Reglages {
        Reglages {
            taille: 16.0,
            bordure: 1.0,
            marge_cellule_v: 4.0,
            marge_cellule_h: 8.0,
            interligne: 1.5,
            marges_bloc: [14.0, 22.0, 20.0, 12.0],
            marge_bande: 2.0,
        }
    }

    #[test]
    fn une_cellule_ne_descend_pas_sous_6_em() {
        let r = reglages();
        assert_eq!(largeurs_cellule(&r, 10.0, 20.0), (96.0, 96.0));
        assert_eq!(largeurs_cellule(&r, 100.0, 300.0), (116.0, 316.0));
        assert!((r.hauteur_rangee(24.0) - 32.0).abs() < 1e-4);
    }

    #[test]
    fn les_colonnes_retrecissent_entre_min_et_max() {
        assert_eq!(
            largeurs_colonnes(&[10., 10.], &[20., 40.], 100.),
            [20., 40.]
        );
        assert_eq!(largeurs_colonnes(&[10., 10.], &[20., 40.], 10.), [10., 10.]);
        // 30 px à partager sur un écart total de 60 : la moitié de chaque écart.
        assert_eq!(largeurs_colonnes(&[10., 10.], &[30., 50.], 50.), [20., 30.]);
    }

    #[test]
    fn grille_frontieres_et_cellule_sous_un_point() {
        let g = Grille::new(&[100., 50.], &[30., 30.], 1.0);
        assert_eq!(g.x, [0., 101., 152.]);
        assert_eq!(g.y, [0., 31., 62.]);
        assert!((g.largeur() - 153.).abs() < 1e-4);
        assert_eq!(g.cellule_a(5., 5.), Some((0, 0)));
        assert_eq!(g.cellule_a(120., 40.), Some((1, 1)));
        assert_eq!(g.cellule_a(160., 5.), None);
        assert_eq!(g.cellule(1, 1), Some((101., 31., 52., 32.)));
    }

    #[test]
    fn glisser_vers_la_frontiere_la_plus_proche() {
        let b = [0., 101., 152., 200.];
        assert_eq!(frontiere_proche(&b, 2.0), 0);
        assert_eq!(frontiere_proche(&b, 140.0), 2);
        assert_eq!(destination(2, 0), Some(0));
        assert_eq!(destination(0, 3), Some(2));
        assert_eq!(destination(1, 1), None);
        assert_eq!(destination(1, 2), None);
    }

    #[test]
    fn hauteur_du_bloc_et_debordement() {
        let r = reglages();
        let g = Grille::new(&[100., 100.], &[32., 32.], 1.0);
        assert!(!deborde(&g, &r, 700.));
        assert!((hauteur_bloc(&g, &r, 700.) - (14. + 67. + 20.)).abs() < 1e-4);
        assert!(deborde(&g, &r, 200.));
        assert!((hauteur_bloc(&g, &r, 200.) - (14. + 67. + 20. + 6.)).abs() < 1e-4);
    }
}
