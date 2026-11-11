// Port de app/src/ui/workspace.css (règles `.workspace-tabs`, `.workspace-tab-header*`,
// `.clickable-icon`) et de `updateTabOutline` de app/src/ui/core/workspace/WorkspaceTabs.ts.
//
//! Les mesures de la pile d'onglets et le tracé du contour de l'onglet actif.
//!
//! Les jetons de FORME (hauteur d'en-tête 36, rayons, icônes) viennent de `Theme::forme` ; ce
//! fichier ne garde que ce que le CSS écrit en littéral (`docs/constantes-rendu.md`, section
//! `ui/workspace.css`). Toutes les valeurs sont LOGIQUES : le rendu les passe par `u()`.
//!
//! Le tracé du contour est une fonction PURE (des segments, pas un chemin GPUI) : elle se teste
//! sans fenêtre, et le rendu n'a plus qu'à la peindre.

/// `--tab-max-width: 200px` (workspace.css:257), posé en `width` d'une boîte de CONTENU : le
/// rembourrage de l'en-tête s'y ajoute (200 + 2 × 3 = 206 de bord à bord, mesuré sur Fragment).
pub const LARGEUR_ONGLET_CONTENU: f32 = 200.0;

/// `.workspace-tab-header { padding: 3px }` (workspace.css:402) : la zone « tappable ».
pub const REMBOURRAGE_ENTETE: f32 = 3.0;

/// La largeur d'un en-tête d'onglet de bord à bord (206).
pub const LARGEUR_ONGLET: f32 = LARGEUR_ONGLET_CONTENU + 2.0 * REMBOURRAGE_ENTETE;

/// `.workspace-tab-header-inner { padding: 3px 3px 3px 9px }` (workspace.css:436) : seul le côté
/// du titre est généreux.
pub const INTERIEUR_COTE_TITRE: f32 = 9.0;
/// Les trois autres côtés du même rembourrage.
pub const INTERIEUR_AUTRES_COTES: f32 = 3.0;

/// Le trait vertical entre deux onglets inactifs : `top: 8px; bottom: 8px; width: 1px`
/// (workspace.css:517-520).
pub const SEPARATEUR_MARGE: f32 = 8.0;
/// Sa largeur.
pub const SEPARATEUR_LARGEUR: f32 = 1.0;

/// Opacité d'un onglet inactif, de la croix et d'une icône cliquable au repos (`opacity: 0.6`).
pub const OPACITE_REPOS: f32 = 0.6;
/// Opacité d'un bouton d'en-tête de vue désactivé (`.view-action:disabled`, workspace.css:860).
pub const OPACITE_DESACTIVE: f32 = 0.25;

/// `.workspace-tab-header-new-tab { margin-left: -1px }` (workspace.css:312) : la pilule du « + »
/// démarre pile au bord tappable du dernier onglet.
pub const NOUVEL_ONGLET_MARGE_GAUCHE: f32 = -1.0;

/// `.view-action { width: 26px; height: 26px }` (workspace.css:814-815).
pub const BOUTON_ENTETE_VUE: f32 = 26.0;

/// L'épaisseur du trait du contour (`stroke-width: 1px`, workspace.css:593).
pub const TRAIT_CONTOUR: f32 = 1.0;

/// Un segment du contour, en pixels de la barre (origine en haut à gauche de la barre).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// `M x y`.
    Debut(f32, f32),
    /// `L x y` (`H` et `V` du SVG, déjà résolus).
    Ligne(f32, f32),
    /// `A r r 0 0 sens x y` : un quart de cercle de rayon `rayon`, dans le sens horaire de l'écran
    /// (balayage 1 du SVG) si `horaire`.
    Arc {
        rayon: f32,
        vers_x: f32,
        vers_y: f32,
        horaire: bool,
    },
}

/// L'onglet actif, dans le repère de la barre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OngletActif {
    /// Le bord gauche.
    pub gauche: f32,
    /// Le bord droit.
    pub droite: f32,
    /// Le bord haut.
    pub haut: f32,
}

/// Le contour de la barre : la ligne de base, interrompue sous l'onglet actif pour épouser sa
/// silhouette (montée, arcs convexes en haut, arcs concaves au pied).
///
/// ★ Le trait de 1 px est CENTRÉ sur le chemin : la ligne de base vit à `hauteur - demi_trait` et
/// le haut à `haut + demi_trait`, pour que le trait tombe sur des pixels entiers (posée pile sur
/// le bord, elle serait à moitié rognée et floue). Les côtés, eux, tombent sur l'entier `gauche`
/// / `droite` : leur trait se répartit sur deux colonnes, comme chez Chromium.
/// Sans onglet actif (ou sorti de la barre), il ne reste que la ligne de base.
pub fn contour(
    largeur: f32,
    hauteur: f32,
    actif: Option<OngletActif>,
    rayon_haut: f32,
    rayon_bas: f32,
    demi_trait: f32,
) -> Vec<Segment> {
    let base = hauteur - demi_trait;
    let Some(a) = actif else {
        return vec![Segment::Debut(0.0, base), Segment::Ligne(largeur, base)];
    };
    let haut = a.haut + demi_trait;
    vec![
        Segment::Debut(0.0, base),
        Segment::Ligne(a.gauche - rayon_bas, base),
        // Coin concave du pied, côté gauche.
        Segment::Arc {
            rayon: rayon_bas,
            vers_x: a.gauche,
            vers_y: base - rayon_bas,
            horaire: false,
        },
        Segment::Ligne(a.gauche, haut + rayon_haut),
        // Coin convexe, en haut à gauche.
        Segment::Arc {
            rayon: rayon_haut,
            vers_x: a.gauche + rayon_haut,
            vers_y: haut,
            horaire: true,
        },
        Segment::Ligne(a.droite - rayon_haut, haut),
        // Coin convexe, en haut à droite.
        Segment::Arc {
            rayon: rayon_haut,
            vers_x: a.droite,
            vers_y: haut + rayon_haut,
            horaire: true,
        },
        Segment::Ligne(a.droite, base - rayon_bas),
        // Coin concave du pied, côté droit.
        Segment::Arc {
            rayon: rayon_bas,
            vers_x: a.droite + rayon_bas,
            vers_y: base,
            horaire: false,
        },
        Segment::Ligne(largeur, base),
    ]
}

/// Le petit triangle courbe au pied de l'onglet actif, que le CSS peint par le `box-shadow` d'un
/// pseudo-élément (`::before` à gauche, `::after` à droite, workspace.css:555-573) : le carré
/// `rayon × rayon` posé juste à l'extérieur du bord de l'onglet, moins le quart de disque centré
/// sur son coin haut éloigné. Rendu en segments fermés, peints dans la couleur du pane.
///
/// `cote_gauche` : le coin gauche (carré à gauche du bord `bord`) ou le coin droit.
pub fn coin_concave(bord: f32, bas: f32, rayon: f32, cote_gauche: bool) -> Vec<Segment> {
    if cote_gauche {
        vec![
            Segment::Debut(bord, bas - rayon),
            Segment::Ligne(bord, bas),
            Segment::Ligne(bord - rayon, bas),
            Segment::Arc {
                rayon,
                vers_x: bord,
                vers_y: bas - rayon,
                horaire: false,
            },
        ]
    } else {
        vec![
            Segment::Debut(bord, bas - rayon),
            Segment::Ligne(bord, bas),
            Segment::Ligne(bord + rayon, bas),
            Segment::Arc {
                rayon,
                vers_x: bord,
                vers_y: bas - rayon,
                horaire: true,
            },
        ]
    }
}

/// La part des `n` onglets de la barre quand la place `disponible` ne suffit plus (largeur
/// uniforme « façon Chrome » : `width` définie + rétrécissement autorisé, workspace.css:423) :
/// chaque onglet prend `min(206, place / n)`. Les onglets REMPLISSENT la barre : la croix du
/// dernier reste au même endroit tant qu'on en ferme.
pub fn largeur_d_un_onglet(n: usize, disponible: f32) -> f32 {
    if n == 0 {
        return LARGEUR_ONGLET;
    }
    LARGEUR_ONGLET.min(disponible / n as f32)
}

/// Les bords gauche et droit EXACTS (px logiques, depuis le bord gauche du conteneur des onglets)
/// de l'onglet de rang `rang` parmi `n`, quand le conteneur (rembourrage `marge` de chaque côté
/// compris) dispose de `place` px : chaque onglet prend `min(largeur_max, (place − 2 marge) / n)`.
///
/// ★ C'est la géométrie que lit le contour de Fragment (`getBoundingClientRect`, NON calé), alors
///   que GPUI cale chaque boîte au pixel : dans une pile de largeur fractionnaire (621,5 px d'un
///   split 50/50), les onglets font 195,25 px chez Chromium, 195 ici, et le trait du côté droit de
///   l'actif tombait 0,5 px à gauche (mesuré sur `t1-pile-active-inactive` : 858,25 et 1053,5 dans
///   la référence, 858 et 1053 dans le candidat).
pub fn bords_exacts(place: f32, marge: f32, n: usize, rang: usize, largeur_max: f32) -> (f32, f32) {
    let largeur = if n == 0 {
        largeur_max
    } else {
        largeur_max.min(((place - 2.0 * marge) / n as f32).max(0.0))
    };
    let gauche = marge + rang as f32 * largeur;
    (gauche, gauche + largeur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_bords_exacts_suivent_la_largeur_fractionnaire_de_la_pile() {
        // `t1-pile-active-inactive`, pile de droite : 398 px calés + 0,5 px de largeur exacte.
        assert_eq!(bords_exacts(398.5, 4.0, 2, 1, 206.0), (199.25, 394.5));
        // Assez de place : les onglets restent à 206, la fraction part dans l'espaceur.
        assert_eq!(bords_exacts(600.5, 4.0, 2, 1, 206.0), (210.0, 416.0));
        assert_eq!(bords_exacts(10.0, 4.0, 0, 0, 206.0), (4.0, 210.0));
    }

    #[test]
    fn un_onglet_mesure_206_de_bord_a_bord() {
        assert_eq!(LARGEUR_ONGLET, 206.0);
    }

    #[test]
    fn sans_onglet_actif_il_ne_reste_que_la_ligne_de_base() {
        let c = contour(1244.0, 36.0, None, 5.0, 4.0, 0.5);
        assert_eq!(
            c,
            vec![Segment::Debut(0.0, 35.5), Segment::Ligne(1244.0, 35.5)]
        );
    }

    #[test]
    fn le_contour_de_l_onglet_actif_epouse_sa_silhouette() {
        // Le 2e onglet de la scène `t1-onglets` : x 246..452 dans la fenêtre, donc 210..416 dans
        // la barre qui commence à 36 ; haut à 4.
        let a = OngletActif {
            gauche: 210.0,
            droite: 416.0,
            haut: 4.0,
        };
        let c = contour(1244.0, 36.0, Some(a), 5.0, 4.0, 0.5);
        assert_eq!(c.len(), 10);
        assert_eq!(c[0], Segment::Debut(0.0, 35.5));
        // La ligne de base s'arrête un rayon concave avant l'onglet.
        assert_eq!(c[1], Segment::Ligne(206.0, 35.5));
        assert_eq!(
            c[2],
            Segment::Arc {
                rayon: 4.0,
                vers_x: 210.0,
                vers_y: 31.5,
                horaire: false
            }
        );
        // Le bord monte jusqu'au rayon convexe sous le trait du haut (4 + 0,5).
        assert_eq!(c[3], Segment::Ligne(210.0, 9.5));
        assert_eq!(
            c[4],
            Segment::Arc {
                rayon: 5.0,
                vers_x: 215.0,
                vers_y: 4.5,
                horaire: true
            }
        );
        assert_eq!(c[5], Segment::Ligne(411.0, 4.5));
        assert_eq!(c[7], Segment::Ligne(416.0, 31.5));
        assert_eq!(c[9], Segment::Ligne(1244.0, 35.5));
    }

    #[test]
    fn le_coin_concave_est_un_carre_moins_un_quart_de_disque() {
        let g = coin_concave(210.0, 36.0, 4.0, true);
        assert_eq!(g[0], Segment::Debut(210.0, 32.0));
        assert_eq!(g[2], Segment::Ligne(206.0, 36.0));
        let d = coin_concave(416.0, 36.0, 4.0, false);
        assert_eq!(d[2], Segment::Ligne(420.0, 36.0));
        assert!(matches!(d[3], Segment::Arc { horaire: true, .. }));
    }

    #[test]
    fn les_onglets_retrecissent_a_egalite_pour_remplir_la_barre() {
        assert_eq!(largeur_d_un_onglet(4, 1244.0), 206.0);
        // 10 onglets dans 1000 px : 100 chacun.
        assert_eq!(largeur_d_un_onglet(10, 1000.0), 100.0);
        assert_eq!(largeur_d_un_onglet(0, 500.0), 206.0);
    }
}
