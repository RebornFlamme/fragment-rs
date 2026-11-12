//! ★ Les exceptions NOMMÉES TEMPORAIRES de T1 (ADR-009, amendement du 11/11/2026 à 12:00). **À
//! RETIRER PAR `fr-p4.1`** (mélange alpha de GPUI contre Chromium), dont le critère de fin est cette
//! liste vide et ses cliquets ramenés à 0.
//!
//! Résidus sous le pixel d'une seule cause, le mélange alpha (même cause que `--shadow-l`) :
//! étalement des ombres de l'infobulle et des notices claires ; position des arcs
//! clairs ; arcs de la palette et de la modale claires.
//!
//! - Chaque exception nomme son PNG (scène ET thème) et son élément : la surface ombrée de jeton
//!   `jeton` (déclarée par le candidat) et, pour [`Nature::Arcs`], la boîte arrondie de même boîte.
//!   Aucune tolérance générique, ni par thème, ni par catégorie.
//! - Ce qu'elle lève : [`Nature::Ombre`], les trois garde-fous de l'ombre (le delta ≤ 2 du halo
//!   reste exigé) ; [`Nature::Arcs`], le garde-fou de position de ses arcs et le delta ≤ 6 de ses
//!   bandes.
//! - Ce qu'elle mesure fait cliquet (clé `exceptions_t1` de `pixels/cliquet.json`) : ombre,
//!   `etalement`, `encre`, `glissement` en valeur absolue ; arcs, `deplacement` (pire déplacement
//!   radial moyen, px), `delta_max` et `forts` (pixels de delta > 6 de ses bandes).

use super::boites::{ArrondiDeclare, BoiteEncre, OmbreDeclaree};

/// Ce qu'une exception lève.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nature {
    /// Les garde-fous (étalement, encre, glissement) de la surface ombrée.
    Ombre,
    /// Le garde-fou de position et le delta ≤ 6 des arcs de la boîte arrondie.
    Arcs,
    /// Le SEUL garde-fou de teinte d'une boîte de texte détectée, désignée par sa position
    /// ([`Exception::boite`]) : encre, forme, bords et barycentres restent exigés.
    Teinte,
    /// Les pixels d'une boîte (en px physiques, [`Exception::boite`]) hors de toute boîte de texte,
    /// au delta ≤ [`DELTA_DEMI_COUVERTURE`], quand la RÉFÉRENCE y est une demi-couverture exacte
    /// entre le pixel du dessus et celui du dessous ([`demi_couverture`]).
    DemiCouverture,
}

/// Une exception nommée : un élément d'un PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    /// Le PNG (scène et thème).
    pub png: &'static str,
    /// Le nom de l'élément : clé du cliquet et du rapport.
    pub element: &'static str,
    /// Le jeton d'ombre de la surface qui porte l'élément.
    pub jeton: &'static str,
    pub nature: Nature,
    /// [`Nature::Teinte`] seulement : la boîte de texte visée (x, y, largeur, hauteur, px
    /// physiques), à [`TOLERANCE_BOITE`] px près sur chaque nombre.
    pub boite: Option<[u32; 4]>,
}

const fn ombre(png: &'static str, element: &'static str, jeton: &'static str) -> Exception {
    Exception {
        png,
        element,
        jeton,
        nature: Nature::Ombre,
        boite: None,
    }
}

const fn arcs(png: &'static str, element: &'static str, jeton: &'static str) -> Exception {
    Exception {
        png,
        element,
        jeton,
        nature: Nature::Arcs,
        boite: None,
    }
}

/// L'élément du « N » coupé de `t1-boutons-fenetre` sombre (décision du chef, fr-t1.5, 12/11 18:30
/// UTC, (a)) : quand « … » ne tient pas, Chromium coupe le titre (overflow) et peint ce reste de
/// glyphe en ClearType (franges de couleur), GPUI en niveaux de gris ; teinte 0,9950 pour une borne
/// de 0,995. Son nom EST la clé du cliquet (`teinte` = 1 − cosinus). Remontée à l'orchestrateur avec
/// la livraison de T1.
pub const N_COUPE: &str = "N coupé ClearType";

/// Écart (px) toléré sur chaque nombre de la boîte d'une exception de teinte : la détection des
/// boîtes peut gagner ou perdre une colonne de frange.
pub const TOLERANCE_BOITE: u32 = 1;

const fn teinte(png: &'static str, element: &'static str, boite: [u32; 4]) -> Exception {
    Exception {
        png,
        element,
        jeton: "--text-normal",
        nature: Nature::Teinte,
        boite: Some(boite),
    }
}

/// ★ L'élément de la ligne 52 de `t1-zoom-150` clair (décision du chef, fr-t1.5, 12/11) : à 150 %,
/// la ligne de base de la barre (1,5 px) est centrée à 53,25 ; la ligne 52 est couverte à 0,5
/// EXACTEMENT, entre la barre (232) et le trait (211) : 221,5. Le raster GPU de Chromium arrondit
/// tantôt 221, tantôt 222 (motif irrégulier sur le premier segment, bascule au milieu du second :
/// les triangles du quad), du bruit qu'aucune règle ne reproduit. Le sombre (33 exact) passe.
/// Son nom EST la clé du cliquet (`forts` = pixels levés, `delta_max`). Remontée à l'orchestrateur
/// avec la livraison de T1.
pub const DEMI_COUVERTURE: &str = "demi-couverture de la ligne de base";

/// Le seul delta levé par une exception [`Nature::DemiCouverture`].
pub const DELTA_DEMI_COUVERTURE: u8 = 1;

const fn demi(png: &'static str, boite: [u32; 4]) -> Exception {
    Exception {
        png,
        element: DEMI_COUVERTURE,
        jeton: "--tab-outline-color",
        nature: Nature::DemiCouverture,
        boite: Some(boite),
    }
}

/// L'élément nommé de demi-couverture qui lève le pixel (`x`, `y`) de delta `delta`, parmi
/// `liste` : dans la boîte de l'exception, delta ≤ [`DELTA_DEMI_COUVERTURE`], et la référence y
/// vaut, canal par canal, la moyenne de ses voisins du dessus et du dessous à ½ niveau près.
pub fn demi_couverture(
    liste: &[&'static Exception],
    reference: &image::RgbaImage,
    (x, y): (u32, u32),
    delta: u8,
) -> Option<&'static str> {
    if delta > DELTA_DEMI_COUVERTURE || y == 0 || y + 1 >= reference.height() {
        return None;
    }
    let (r, haut, bas) = (
        reference.get_pixel(x, y).0,
        reference.get_pixel(x, y - 1).0,
        reference.get_pixel(x, y + 1).0,
    );
    let moitie =
        (0..3).all(|c| (2 * i32::from(r[c]) - i32::from(haut[c]) - i32::from(bas[c])).abs() <= 1);
    if !moitie {
        return None;
    }
    liste
        .iter()
        .filter(|e| e.nature == Nature::DemiCouverture)
        .find(|e| {
            e.boite
                .is_some_and(|[bx, by, bl, bh]| x >= bx && x < bx + bl && y >= by && y < by + bh)
        })
        .map(|e| e.element)
}

/// L'élément des arcs du contour de l'onglet actif (haut convexe, pieds concaves, raccords
/// compris), déclarés sans ombre par `fragment_ui::onglets` : son nom EST la clé du cliquet.
pub const CONTOUR_ONGLET: &str = "contour d'onglet actif";

const fn contour(png: &'static str) -> Exception {
    Exception {
        png,
        element: CONTOUR_ONGLET,
        jeton: "--tab-outline-color",
        nature: Nature::Arcs,
        boite: None,
    }
}

/// L'élément de l'arc du coin haut gauche de la barre d'état (quad GPUI de rayon `--radius-sm`,
/// bordure haut et gauche), déclaré sans ombre par `fragment_ui::shell::barre_etat` : son nom EST la
/// clé du cliquet.
pub const COIN_BARRE_ETAT: &str = "coin de la barre d'état";

const fn coin_barre(png: &'static str) -> Exception {
    Exception {
        png,
        element: COIN_BARRE_ETAT,
        jeton: "--tab-outline-color",
        nature: Nature::Arcs,
        boite: None,
    }
}

/// L'élément des traits de poignée (repos et accent), déclarés sans coin par
/// `fragment_ui::splits` (boîte exacte du trait peint) : son nom EST la clé du cliquet.
pub const TRAIT_POIGNEE: &str = "trait de poignée";

const fn poignee(png: &'static str) -> Exception {
    Exception {
        png,
        element: TRAIT_POIGNEE,
        jeton: "--tab-outline-color",
        nature: Nature::Arcs,
        boite: None,
    }
}

/// La liste, À RETIRER PAR `fr-p4.1`. Valeurs mesurées : `pixels/cliquet.json`, `exceptions_t1`.
pub const EXCEPTIONS: &[Exception] = &[
    // Ombres : étalement de +0,22 à +0,24 px (infobulle, petite boîte : 4 échantillons du shader),
    // −0,15 à −0,16 (notices claires) ; borne 0,08. Le menu clair (−0,19 px au shader) est retiré
    // (chef, 12/11) : ombré sur le CPU au calque émulé, étalement 0,020.
    ombre(
        "t1-infobulle-dark.png",
        "ombre de l'infobulle",
        "--menu-shadow",
    ),
    ombre(
        "t1-infobulle-light.png",
        "ombre de l'infobulle",
        "--menu-shadow",
    ),
    ombre("t1-notice-light.png", "ombre de la notice", "--shadow-s"),
    ombre(
        "t1-notice-empilees-light.png",
        "ombre des notices",
        "--shadow-s",
    ),
    // Palette et modale claires (bordure opaque, bord unique de fr-p4.1) : 20 à 22 px de delta 7 à 8
    // à la jonction de l'arc et du côté droit, l'anticrénelage propre à Skia (mire de S8 : 8 au pire,
    // ADR-013). Les arcs du menu, des notices et de l'infobulle ne sont plus exceptés.
    arcs("t1-modale-light.png", "arcs de la modale", "--shadow-l"),
    arcs("t1-palette-light.png", "arcs de la palette", "--shadow-l"),
    arcs(
        "t1-palette-aucun-resultat-light.png",
        "arcs de la palette",
        "--shadow-l",
    ),
    arcs(
        "t1-palette-selection-light.png",
        "arcs de la palette",
        "--shadow-l",
    ),
    // Contour de l'onglet actif (décision du chef, fr-t1.5) : le chemin de GPUI est rastérisé en
    // MSAA 4x (couverture quantifiée, asymétrique d'un côté à l'autre) et Chromium met environ 40 %
    // d'encre de plus aux raccords ligne/arc : delta jusqu'à 11 en sombre. Les côtés droits et la
    // ligne de base, pré-composés, restent stricts ; seule la bande des arcs déclarés est levée.
    contour("t1-activation-fermeture-dark.png"),
    contour("t1-activation-fermeture-light.png"),
    contour("t1-boutons-fenetre-dark.png"),
    contour("t1-boutons-fenetre-light.png"),
    contour("t1-dock-replie-dark.png"),
    contour("t1-dock-replie-light.png"),
    contour("t1-docks-ouverts-dark.png"),
    contour("t1-docks-ouverts-light.png"),
    contour("t1-entete-vue-dark.png"),
    contour("t1-entete-vue-light.png"),
    contour("t1-fenetre-vide-dark.png"),
    contour("t1-fenetre-vide-light.png"),
    contour("t1-glisser-depot-dark.png"),
    contour("t1-glisser-depot-light.png"),
    contour("t1-glisser-zone-dark.png"),
    contour("t1-glisser-zone-light.png"),
    contour("t1-menu-onglet-dark.png"),
    contour("t1-menu-onglet-light.png"),
    contour("t1-onglet-epingle-dark.png"),
    contour("t1-onglet-epingle-light.png"),
    contour("t1-onglet-survol-dark.png"),
    contour("t1-onglet-survol-light.png"),
    contour("t1-onglets-dark.png"),
    contour("t1-onglets-light.png"),
    contour("t1-pile-active-inactive-dark.png"),
    contour("t1-pile-active-inactive-light.png"),
    contour("t1-poignee-survol-dark.png"),
    contour("t1-poignee-survol-light.png"),
    contour("t1-split-horizontal-dark.png"),
    contour("t1-split-horizontal-light.png"),
    contour("t1-split-proportions-dark.png"),
    contour("t1-split-proportions-light.png"),
    contour("t1-split-vertical-dark.png"),
    contour("t1-split-vertical-light.png"),
    contour("t1-zoom-150-dark.png"),
    contour("t1-zoom-150-light.png"),
    contour("t1-zoom-80-dark.png"),
    contour("t1-zoom-80-light.png"),
    // Coin haut gauche de la barre d'état (décision du chef, fr-t1.5) : la diagonale de l'arc
    // INTÉRIEUR du quad GPUI (r = --radius-sm, bordure 1) a 1 px de delta 9 à 11 en sombre, dans
    // la bande de l'arc ; même cause (mélange alpha). Seule la bande de l'arc déclaré est levée.
    coin_barre("t1-activation-fermeture-dark.png"),
    coin_barre("t1-boutons-fenetre-dark.png"),
    coin_barre("t1-dock-replie-dark.png"),
    coin_barre("t1-docks-ouverts-dark.png"),
    coin_barre("t1-entete-vue-dark.png"),
    coin_barre("t1-fenetre-vide-dark.png"),
    coin_barre("t1-glisser-depot-dark.png"),
    coin_barre("t1-glisser-zone-dark.png"),
    coin_barre("t1-menu-onglet-dark.png"),
    coin_barre("t1-onglet-epingle-dark.png"),
    coin_barre("t1-onglet-survol-dark.png"),
    coin_barre("t1-onglets-dark.png"),
    coin_barre("t1-pile-active-inactive-dark.png"),
    coin_barre("t1-poignee-survol-dark.png"),
    coin_barre("t1-split-horizontal-dark.png"),
    coin_barre("t1-split-proportions-dark.png"),
    coin_barre("t1-split-vertical-dark.png"),
    coin_barre("t1-zoom-150-dark.png"),
    coin_barre("t1-zoom-80-dark.png"),
    // Bords des traits de poignée, en CLAIR (décision du chef, fr-t1.5) : Skia arrondit d'un niveau
    // (canal bleu surtout) les cellules de bord d'un trait à position fractionnaire, sans règle
    // constante (plancher en 657,5, autre sens en 1166,5, alternance d'une ligne à l'autre). Même
    // famille que le 221/222 du côté de l'onglet. Delta 1 seulement, cellules de bord seulement.
    poignee("t1-boutons-fenetre-light.png"),
    poignee("t1-glisser-depot-light.png"),
    poignee("t1-pile-active-inactive-light.png"),
    poignee("t1-poignee-survol-light.png"),
    poignee("t1-split-horizontal-light.png"),
    poignee("t1-split-proportions-light.png"),
    poignee("t1-split-vertical-light.png"),
    // Le « N » coupé du titre étroit, sombre seulement (le clair passe dans la borne).
    teinte("t1-boutons-fenetre-dark.png", N_COUPE, [1183, 14, 7, 11]),
    // La ligne 52 de la barre à 150 %, clair seulement : la ligne de base y couvre 0,5 exactement.
    demi("t1-zoom-150-light.png", [54, 52, 1226, 1]),
];

/// Les exceptions du PNG `png`.
pub fn du_png(png: &str) -> Vec<&'static Exception> {
    EXCEPTIONS.iter().filter(|e| e.png == png).collect()
}

/// L'élément nommé de teinte qui vise la boîte de texte `b`, parmi `liste`.
pub fn pour_boite(liste: &[&'static Exception], b: &BoiteEncre) -> Option<&'static str> {
    liste
        .iter()
        .filter(|e| e.nature == Nature::Teinte)
        .find(|e| {
            e.boite.is_some_and(|v| {
                [b.x, b.y, b.largeur, b.hauteur]
                    .iter()
                    .zip(v)
                    .all(|(a, w)| a.abs_diff(w) <= TOLERANCE_BOITE)
            })
        })
        .map(|e| e.element)
}

/// L'élément nommé d'ombre qui couvre `o`, parmi `liste`.
pub fn pour_ombre(liste: &[&'static Exception], o: &OmbreDeclaree) -> Option<&'static str> {
    liste
        .iter()
        .find(|e| e.nature == Nature::Ombre && e.jeton == o.jeton)
        .map(|e| e.element)
}

/// Même boîte, au demi-pixel près.
fn meme_boite(a: &ArrondiDeclare, o: &OmbreDeclaree) -> bool {
    [
        (a.x, o.x),
        (a.y, o.y),
        (a.largeur, o.largeur),
        (a.hauteur, o.hauteur),
    ]
    .iter()
    .all(|(u, v)| (u - v).abs() < 0.5)
}

/// L'élément nommé d'arcs qui couvre `a`, parmi `liste` : la boîte arrondie d'une surface ombrée
/// de son jeton.
pub fn pour_arrondi(
    liste: &[&'static Exception],
    a: &ArrondiDeclare,
    ombres: &[OmbreDeclaree],
) -> Option<&'static str> {
    liste
        .iter()
        .filter(|e| e.nature == Nature::Arcs)
        .find(|e| {
            // Un arc sans ombre (contour d'onglet) se nomme lui-même ; les autres sont la boîte
            // arrondie d'une surface ombrée de leur jeton.
            (!a.element.is_empty() && a.element == e.element)
                || ombres
                    .iter()
                    .any(|o| o.jeton == e.jeton && meme_boite(a, o))
        })
        .map(|e| e.element)
}

/// Le delta levé sur les CÔTÉS du contour de l'onglet actif (décision du chef, fr-t1.5) : en clair,
/// Chromium passe lui-même de 221 à 222 au milieu du côté (artefact de Skia sur le chemin
/// `updateTabOutline`) ; nos quads pré-composés restent à 221. Delta 1 SEULEMENT, en cliquet.
pub const DELTA_COTES: u8 = 1;

/// Les pixels des deux côtés droits du contour de l'onglet actif, déduits de la DÉCLARATION de son
/// haut (boîte de bordure aux deux coins hauts, `element` = [`CONTOUR_ONGLET`]) : les colonnes que
/// coupe le trait de gauche et celui de droite, des bas des arcs hauts (`y + r`) au bas de la boîte
/// (`y + h`, les pieds ont leur propre rayon et leur bande passe avant). Vide pour toute autre
/// déclaration.
pub fn pixels_cotes(a: &ArrondiDeclare, largeur: u32, hauteur: u32) -> Vec<(u32, u32)> {
    if a.element == TRAIT_POIGNEE {
        return pixels_bords_trait(a, largeur, hauteur);
    }
    if a.element != CONTOUR_ONGLET || a.coins != [true, true, false, false] || a.bordure <= 0.0 {
        return Vec::new();
    }
    let colonnes = |x0: f64| {
        let (a0, a1) = (x0.floor().max(0.0), (x0 + a.bordure).ceil());
        (a0 as u32)..(a1.max(a0) as u32).min(largeur)
    };
    let (y0, y1) = (
        (a.y + a.rayon).ceil().max(0.0) as u32,
        ((a.y + a.hauteur).floor().max(0.0) as u32).min(hauteur),
    );
    let mut v = Vec::new();
    for x0 in [a.x, a.x + a.largeur - a.bordure] {
        for x in colonnes(x0) {
            v.extend((y0..y1).map(|y| (x, y)));
        }
    }
    v
}

/// Les cellules de BORD d'un trait de poignée déclaré (boîte exacte, px physiques) : en travers du
/// trait (vertical si plus haut que large), les colonnes (ou lignes) que ses bords coupent à une
/// couverture fractionnaire, sur toute sa longueur. Les cellules pleines restent strictes.
fn pixels_bords_trait(a: &ArrondiDeclare, largeur: u32, hauteur: u32) -> Vec<(u32, u32)> {
    let vertical = a.hauteur >= a.largeur;
    let (debut, fin) = if vertical {
        (a.x, a.x + a.largeur)
    } else {
        (a.y, a.y + a.hauteur)
    };
    let eps = 1e-3;
    let mut bords: Vec<i64> = Vec::new();
    for bord in [debut, fin] {
        if (bord - bord.round()).abs() > eps {
            bords.push(bord.floor() as i64);
        }
    }
    bords.dedup();
    let (l0, l1) = if vertical {
        (a.y, a.y + a.hauteur)
    } else {
        (a.x, a.x + a.largeur)
    };
    let limite = if vertical { hauteur } else { largeur };
    let le_long = (l0.floor().max(0.0) as u32)..(l1.ceil().max(0.0) as u32).min(limite);
    let mut v = Vec::new();
    for b in bords {
        let Ok(b) = u32::try_from(b) else { continue };
        if b >= if vertical { largeur } else { hauteur } {
            continue;
        }
        for t in le_long.clone() {
            v.push(if vertical { (b, t) } else { (t, b) });
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trait_de_poignee(x: f64, y: f64, l: f64, h: f64) -> ArrondiDeclare {
        ArrondiDeclare {
            x,
            y,
            largeur: l,
            hauteur: h,
            rayon: 0.0,
            bordure: 0.0,
            coins: [false; 4],
            element: TRAIT_POIGNEE.to_owned(),
        }
    }

    #[test]
    fn les_bords_d_un_trait_de_poignee_sont_ses_cellules_fractionnaires() {
        // Trait vertical de 1 px à cheval sur 657 et 658, sur 10 lignes.
        let p = pixels_cotes(&trait_de_poignee(657.5, 0.0, 1.0, 10.0), 1280, 800);
        assert_eq!(p.len(), 20);
        assert!(p.contains(&(657, 0)) && p.contains(&(658, 9)) && !p.contains(&(659, 0)));
        // Accent de 3 px de 656,5 à 659,5 : les deux bords, pas le plein du milieu.
        let p = pixels_cotes(&trait_de_poignee(656.5, 0.0, 3.0, 4.0), 1280, 800);
        assert!(p.contains(&(656, 0)) && p.contains(&(659, 3)));
        assert!(!p.contains(&(657, 0)) && !p.contains(&(658, 0)));
        // Trait horizontal calé au pixel : aucune cellule de bord.
        assert!(pixels_cotes(&trait_de_poignee(36.0, 400.0, 100.0, 1.0), 1280, 800).is_empty());
        let p = pixels_cotes(&trait_de_poignee(36.0, 399.5, 4.0, 1.0), 1280, 800);
        assert_eq!(
            p,
            vec![
                (36, 399),
                (37, 399),
                (38, 399),
                (39, 399),
                (36, 400),
                (37, 400),
                (38, 400),
                (39, 400)
            ]
        );
    }

    /// Contre-épreuve du trait de poignée : delta 1 levé en clair, delta 2 ou trait décalé d'1 px
    /// échouent ; le sombre reste strict.
    #[test]
    fn un_trait_de_poignee_decale_ou_a_delta_deux_echoue() {
        let t = trait_de_poignee(10.5, 0.0, 1.0, 40.0);
        // Fond 247 à gauche et à droite, trait de teinte 211 à moitié sur 10 et 11.
        let peindre = |x0: u32, bleu: u8| {
            let mut img = image::RgbaImage::from_pixel(40, 40, image::Rgba([247, 246, 242, 255]));
            for y in 0..40 {
                for x in [x0, x0 + 1] {
                    img.put_pixel(x, y, image::Rgba([229, 228, bleu, 255]));
                }
            }
            img
        };
        let reference = peindre(10, 223);
        let juger = |png: &str, c: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(
                &reference,
                c,
                &[],
                &[],
                std::slice::from_ref(&t),
                &du_png(png),
            )
            .unwrap()
            .differents_hors
        };
        let clair = "t1-split-vertical-light.png";
        assert_eq!(juger(clair, &peindre(10, 222)), 0);
        assert!(juger(clair, &peindre(10, 221)) > 0, "delta 2");
        assert!(juger(clair, &peindre(11, 223)) > 0, "trait décalé d'1 px");
        assert!(
            juger("t1-split-vertical-dark.png", &peindre(10, 222)) > 0,
            "sombre strict"
        );
    }

    #[test]
    fn les_cotes_du_contour_sont_ses_colonnes_entre_les_arcs() {
        let haut = ArrondiDeclare {
            x: 35.5,
            y: 5.5,
            largeur: 101.0,
            hauteur: 26.0,
            rayon: 6.5,
            bordure: 1.0,
            coins: [true, true, false, false],
            element: CONTOUR_ONGLET.to_owned(),
        };
        let p = pixels_cotes(&haut, 400, 40);
        // Deux colonnes par côté (trait à cheval sur deux pixels), lignes 12 à 30.
        assert!(p.contains(&(35, 12)) && p.contains(&(36, 30)));
        assert!(p.contains(&(135, 12)) && p.contains(&(136, 30)));
        assert!(!p.contains(&(37, 15)) && !p.contains(&(35, 11)) && !p.contains(&(35, 31)));
        assert_eq!(p.len(), 4 * 19);
        // Un pied, un arc anonyme ou d'un autre élément n'a pas de côté.
        let pied = ArrondiDeclare {
            coins: [false, false, false, true],
            ..haut.clone()
        };
        assert!(pixels_cotes(&pied, 400, 40).is_empty());
        let anonyme = ArrondiDeclare {
            element: String::new(),
            ..haut
        };
        assert!(pixels_cotes(&anonyme, 400, 40).is_empty());
    }

    /// Contre-épreuve des côtés : delta 1 levé (et compté), delta 2 ou trait décalé d'1 px échouent.
    #[test]
    fn un_cote_du_contour_a_delta_deux_echoue() {
        let haut = ArrondiDeclare {
            x: 9.5,
            y: 1.5,
            largeur: 21.0,
            hauteur: 30.0,
            rayon: 4.5,
            bordure: 1.0,
            coins: [true, true, false, false],
            element: CONTOUR_ONGLET.to_owned(),
        };
        let peindre = |colonne: u32, v: u8| {
            let mut img = image::RgbaImage::from_pixel(40, 40, image::Rgba([232, 232, 232, 255]));
            for y in 10..20 {
                img.put_pixel(colonne, y, image::Rgba([v, v, v, 255]));
            }
            img
        };
        let reference = peindre(10, 221);
        let nommees = du_png("t1-onglets-light.png");
        let juger = |c: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(
                &reference,
                c,
                &[],
                &[],
                std::slice::from_ref(&haut),
                &nommees,
            )
            .unwrap()
            .differents_hors
        };
        assert_eq!(juger(&peindre(10, 222)), 0);
        assert!(juger(&peindre(10, 223)) > 0, "delta 2");
        assert!(
            juger(&peindre(11, 221)) > 0,
            "côté décalé d'1 px hors des colonnes"
        );
    }

    /// Un « N » simplifié (deux fûts et une diagonale) de teinte `couleur`, posé à `x0` dans la
    /// boîte du « N » coupé, sur le fond sombre.
    fn glyphe_n(x0: u32, couleur: [u8; 3]) -> image::RgbaImage {
        let mut img = image::RgbaImage::from_pixel(1200, 30, image::Rgba([14, 15, 17, 255]));
        let [r, g, b] = couleur;
        for y in 15..24 {
            for x in [x0, x0 + 5, x0 + (y - 15) * 5 / 9] {
                img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
            }
        }
        img
    }

    /// Contre-épreuve du « N » coupé : une autre teinte est levée (et mesurée) dans SA boîte de SON
    /// PNG seulement ; le même glyphe décalé d'1 px échoue (barycentre), la même teinte dans une
    /// autre boîte ou un autre PNG échoue.
    #[test]
    fn le_n_coupe_ne_leve_que_la_teinte_de_sa_boite() {
        let boite = |x: u32| BoiteEncre {
            categorie: super::super::boites::Categorie::Texte,
            x,
            y: 14,
            largeur: 7,
            hauteur: 11,
        };
        let reference = glyphe_n(1184, [100, 100, 100]);
        // Cosinus 0,9926 (< 0,995), encre totale à 1 % près.
        let franges = [114, 100, 88];
        let juger = |png: &str, x: u32, candidat: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(
                &reference,
                candidat,
                &[boite(x)],
                &[],
                &[],
                &du_png(png),
            )
            .unwrap()
        };
        let png = "t1-boutons-fenetre-dark.png";
        let s = juger(png, 1183, &glyphe_n(1184, franges));
        assert!(s.boites.iter().all(|b| b.ok()), "{:?}", s.boites);
        let mesure = s.nommees[N_COUPE]["teinte"];
        assert!(mesure > 0.005 && mesure < 0.01, "{mesure}");
        // Teinte identique : rien à mesurer.
        let s = juger(png, 1183, &glyphe_n(1184, [100, 100, 100]));
        assert!(s.nommees[N_COUPE]["teinte"].abs() < 1e-9);
        // Le glyphe décalé d'1 px : barycentre hors borne, toujours un échec.
        let s = juger(png, 1183, &glyphe_n(1185, franges));
        assert!(s.boites.iter().any(|b| !b.ok()), "décalé d'1 px");
        // Le même écart de teinte dans le clair, ou dans une autre boîte, reste un échec.
        let s = juger(
            "t1-boutons-fenetre-light.png",
            1183,
            &glyphe_n(1184, franges),
        );
        assert!(s.boites.iter().any(|b| !b.ok()), "autre PNG");
        assert!(
            pour_boite(&du_png(png), &boite(1170)).is_none(),
            "autre boîte"
        );
        assert_eq!(pour_boite(&du_png(png), &boite(1184)), Some(N_COUPE));
    }

    #[test]
    fn la_demi_couverture_ne_leve_que_le_delta_1_de_sa_ligne_et_de_son_png() {
        // Barre 232 (lignes 0 à 51), ligne 52 à 221,5 arrondi en 222 (référence) ou 221, trait 211.
        let image_ = |l52: u8, l51: u8| {
            image::RgbaImage::from_fn(1280, 56, |_, y| {
                let v = match y {
                    52 => l52,
                    51 => l51,
                    y if y < 52 => 232,
                    _ => 211,
                };
                image::Rgba([v, v, v, 255])
            })
        };
        let reference = image_(222, 232);
        let juger = |png: &str, candidat: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(&reference, candidat, &[], &[], &[], &du_png(png))
                .unwrap()
        };
        let png = "t1-zoom-150-light.png";
        // Les 54 px à gauche de la boîte (x < 54) restent stricts.
        let s = juger(png, &image_(221, 232));
        assert_eq!(s.differents_hors, 54);
        assert_eq!(s.nommees[DEMI_COUVERTURE]["forts"], 1226.0);
        assert_eq!(s.nommees[DEMI_COUVERTURE]["delta_max"], 1.0);
        // Contre-épreuves (toute la ligne diverge : 1 280 px) : un delta 2, les lignes 51 et 53, le
        // sombre, un autre PNG.
        let hors = |png: &str, c: &image::RgbaImage| juger(png, c).differents_hors;
        assert_eq!(hors(png, &image_(220, 232)), 1280, "delta 2");
        assert_eq!(hors(png, &image_(222, 231)), 1280, "ligne 51");
        let mut l53 = reference.clone();
        for x in 0..1280 {
            l53.put_pixel(x, 53, image::Rgba([212, 212, 212, 255]));
        }
        assert_eq!(hors(png, &l53), 1280, "ligne 53");
        assert_eq!(hors("t1-zoom-150-dark.png", &image_(221, 232)), 1280);
        assert_eq!(hors("t1-zoom-80-light.png", &image_(221, 232)), 1280);
    }

    #[test]
    fn chaque_exception_nomme_un_png_de_t1_et_un_element() {
        for e in EXCEPTIONS {
            assert!(e.png.starts_with("t1-") && e.png.ends_with(".png"), "{e:?}");
            assert!(!e.element.is_empty() && e.jeton.starts_with("--"), "{e:?}");
        }
        // Pas de doublon (png, élément) ; deux arcs nommés d'un même PNG sont des éléments distincts
        // (contour d'onglet et coin de la barre d'état), une seule ombre par PNG.
        for (i, a) in EXCEPTIONS.iter().enumerate() {
            for b in &EXCEPTIONS[i + 1..] {
                assert!(!(a.png == b.png && a.element == b.element), "{a:?} {b:?}");
                assert!(
                    !(a.png == b.png && a.nature == Nature::Ombre && b.nature == Nature::Ombre),
                    "{a:?} {b:?}"
                );
            }
        }
    }

    #[test]
    fn une_exception_ne_vaut_que_pour_son_png_et_son_element() {
        let o = OmbreDeclaree {
            x: 10.0,
            y: 10.0,
            largeur: 50.0,
            hauteur: 20.0,
            rayon: 6.0,
            couches: Vec::new(),
            jeton: "--menu-shadow".to_owned(),
        };
        let a = ArrondiDeclare {
            x: 10.0,
            y: 10.0,
            largeur: 50.0,
            hauteur: 20.0,
            rayon: 6.0,
            bordure: 1.0,
            ..ArrondiDeclare::default()
        };
        let autre = ArrondiDeclare {
            x: 30.0,
            ..a.clone()
        };
        let liste = du_png("t1-infobulle-light.png");
        assert_eq!(pour_ombre(&liste, &o), Some("ombre de l'infobulle"));
        // Ses arcs ne sont plus exceptés (fr-p4.1, bord unique).
        assert_eq!(pour_arrondi(&liste, &a, std::slice::from_ref(&o)), None);
        let modale = OmbreDeclaree {
            jeton: "--shadow-l".to_owned(),
            ..o.clone()
        };
        let claire = du_png("t1-modale-light.png");
        assert_eq!(
            pour_arrondi(&claire, &a, std::slice::from_ref(&modale)),
            Some("arcs de la modale")
        );
        assert_eq!(
            pour_arrondi(&claire, &autre, std::slice::from_ref(&modale)),
            None
        );
        // Le thème sombre de la modale n'excepte rien.
        let sombre = du_png("t1-modale-dark.png");
        assert_eq!(
            pour_arrondi(&sombre, &a, std::slice::from_ref(&modale)),
            None
        );
        // Une scène sans exception n'en a aucune.
        assert!(du_png("t1-menu-dark.png").is_empty());
        let autre_jeton = OmbreDeclaree {
            jeton: "--shadow-s".to_owned(),
            ..o
        };
        assert_eq!(pour_ombre(&liste, &autre_jeton), None);
    }

    /// Le coin haut gauche du contour de l'onglet : un trait de 1 px centré sur un quart de cercle
    /// de centre (`cx`, `cy`) et de rayon `r`, prolongé par le côté (vers le bas) et le haut (vers
    /// la droite), sur-échantillonné 16 × 16 ; `biais` relève la couverture vers 0 ou 1 (un autre
    /// anticrénelage, même aire en moyenne).
    fn coin_de_contour(cx: f64, cy: f64, r: f64, biais: f64) -> image::RgbaImage {
        let trait_ = |x: f64, y: f64| {
            if x <= cx && y <= cy {
                ((x - cx).hypot(y - cy) - r).abs() <= 0.5
            } else if y > cy {
                (x - (cx - r)).abs() <= 0.5
            } else {
                (y - (cy - r)).abs() <= 0.5
            }
        };
        let mut img = image::RgbaImage::new(40, 40);
        const N: u32 = 16;
        for (x, y, p) in img.enumerate_pixels_mut() {
            let mut n = 0u32;
            for i in 0..N {
                for j in 0..N {
                    let sx = f64::from(x) + (f64::from(i) + 0.5) / f64::from(N);
                    let sy = f64::from(y) + (f64::from(j) + 0.5) / f64::from(N);
                    n += u32::from(trait_(sx, sy));
                }
            }
            let c = f64::from(n) / f64::from(N * N);
            let c = if c > 0.0 && c < 1.0 {
                (c + biais * (c - 0.5) * (1.0 - (2.0 * c - 1.0).abs())).clamp(0.0, 1.0)
            } else {
                c
            };
            let v = (14.0 + c * 38.0).round() as u8;
            *p = image::Rgba([v, v, v, 255]);
        }
        img
    }

    /// Contre-épreuve de l'exception du contour : un autre anticrénelage du même arc est levé
    /// (mesuré par le cliquet), un rayon ou un centre décalé d'1 px sort de la bande et échoue.
    #[test]
    fn le_contour_d_onglet_decale_d_un_pixel_echoue() {
        let (cx, cy, r) = (20.0, 15.5, 5.0);
        // Le trait de 1 px : boîte « à bordure » de rayon extérieur r + 0,5, au demi-pixel.
        let declare = ArrondiDeclare {
            x: cx - r - 0.5,
            y: cy - r - 0.5,
            largeur: 30.0,
            hauteur: 30.0,
            rayon: r + 0.5,
            bordure: 1.0,
            coins: [true, false, false, false],
            element: CONTOUR_ONGLET.to_owned(),
        };
        let nommees = du_png("t1-onglets-dark.png");
        let reference = coin_de_contour(cx, cy, r, 0.0);
        let juger = |candidat: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(
                &reference,
                candidat,
                &[],
                &[],
                std::slice::from_ref(&declare),
                &nommees,
            )
            .unwrap()
            .differents_hors
        };
        assert_eq!(juger(&coin_de_contour(cx, cy, r, 0.8)), 0);
        assert!(
            juger(&coin_de_contour(cx, cy, r + 1.0, 0.0)) > 0,
            "rayon + 1"
        );
        assert!(
            juger(&coin_de_contour(cx + 1.0, cy, r, 0.0)) > 0,
            "centre + 1 en x"
        );
        assert!(
            juger(&coin_de_contour(cx, cy + 1.0, r, 0.0)) > 0,
            "centre + 1 en y"
        );
    }

    /// Le coin haut gauche de la barre d'état : une boîte d'origine (`x0`, `y0`), bordée de 1 px en
    /// haut et à gauche, de rayon extérieur `r` (intérieur `r − 1`), sur-échantillonnée 16 × 16 ;
    /// `biais` comme pour [`coin_de_contour`].
    fn coin_de_barre(x0: f64, y0: f64, r: f64, biais: f64) -> image::RgbaImage {
        let dans_arrondi = |x: f64, y: f64, ox: f64, oy: f64, r: f64| {
            if x < ox || y < oy {
                return false;
            }
            let (cx, cy) = (ox + r, oy + r);
            x >= cx || y >= cy || (x - cx).hypot(y - cy) <= r
        };
        let mut img = image::RgbaImage::new(40, 40);
        const N: u32 = 16;
        for (x, y, p) in img.enumerate_pixels_mut() {
            let (mut bord, mut fond) = (0u32, 0u32);
            for i in 0..N {
                for j in 0..N {
                    let sx = f64::from(x) + (f64::from(i) + 0.5) / f64::from(N);
                    let sy = f64::from(y) + (f64::from(j) + 0.5) / f64::from(N);
                    if dans_arrondi(sx, sy, x0 + 1.0, y0 + 1.0, r - 1.0) {
                        fond += 1;
                    } else if dans_arrondi(sx, sy, x0, y0, r) {
                        bord += 1;
                    }
                }
            }
            let biaiser = |c: f64| {
                if c > 0.0 && c < 1.0 {
                    (c + biais * (c - 0.5) * (1.0 - (2.0 * c - 1.0).abs())).clamp(0.0, 1.0)
                } else {
                    c
                }
            };
            let total = f64::from(N * N);
            let (b, f) = (biaiser(f64::from(bord) / total), f64::from(fond) / total);
            let v = (14.0 + b * 38.0 + f * 16.0).round().min(255.0) as u8;
            *p = image::Rgba([v, v, v, 255]);
        }
        img
    }

    /// Contre-épreuve de l'exception du coin de la barre d'état : un autre anticrénelage du même
    /// arc est levé, un rayon ou une boîte décalés d'1 px échouent.
    #[test]
    fn le_coin_de_la_barre_d_etat_decale_d_un_pixel_echoue() {
        let (x0, y0, r) = (10.0, 10.0, 6.0);
        let declare = ArrondiDeclare {
            x: x0,
            y: y0,
            largeur: 30.0,
            hauteur: 30.0,
            rayon: r,
            bordure: 1.0,
            coins: [true, false, false, false],
            element: COIN_BARRE_ETAT.to_owned(),
        };
        let nommees = du_png("t1-onglets-dark.png");
        let reference = coin_de_barre(x0, y0, r, 0.0);
        let juger = |candidat: &image::RgbaImage| {
            crate::pixels::sas::comparer_tout(
                &reference,
                candidat,
                &[],
                &[],
                std::slice::from_ref(&declare),
                &nommees,
            )
            .unwrap()
            .differents_hors
        };
        assert_eq!(juger(&coin_de_barre(x0, y0, r, 0.8)), 0);
        assert!(juger(&coin_de_barre(x0, y0, r + 1.0, 0.0)) > 0, "rayon + 1");
        assert!(
            juger(&coin_de_barre(x0 + 1.0, y0, r, 0.0)) > 0,
            "boîte + 1 en x"
        );
        assert!(
            juger(&coin_de_barre(x0, y0 + 1.0, r, 0.0)) > 0,
            "boîte + 1 en y"
        );
        // Sans nom, le même coin reste dans la seule catégorie bord d'arrondi (delta ≤ 6).
        assert_eq!(
            pour_arrondi(
                &nommees,
                &ArrondiDeclare {
                    element: String::new(),
                    ..declare.clone()
                },
                &[]
            ),
            None
        );
        assert_eq!(pour_arrondi(&nommees, &declare, &[]), Some(COIN_BARRE_ETAT));
    }

    #[test]
    fn le_contour_d_onglet_ne_vaut_que_pour_ses_png_et_son_element() {
        let pied = ArrondiDeclare {
            x: 30.0,
            y: 26.0,
            largeur: 10.0,
            hauteur: 10.0,
            rayon: 4.5,
            bordure: 1.0,
            coins: [false, false, false, true],
            element: CONTOUR_ONGLET.to_owned(),
        };
        let onglets = du_png("t1-onglets-dark.png");
        assert_eq!(pour_arrondi(&onglets, &pied, &[]), Some(CONTOUR_ONGLET));
        // Sans nom, la même boîte reste stricte ; la fenêtre de réglages n'a pas de contour.
        let anonyme = ArrondiDeclare {
            element: String::new(),
            ..pied.clone()
        };
        assert_eq!(pour_arrondi(&onglets, &anonyme, &[]), None);
        assert_eq!(
            pour_arrondi(&du_png("t1-reglages-zoom-dark.png"), &pied, &[]),
            None
        );
    }
}
