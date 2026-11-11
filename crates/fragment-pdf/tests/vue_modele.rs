// Port de app/e2e/pdf-toolbar.spec.ts (tests 2 à 7) et app/e2e/pdf-geometry.spec.ts, en
// tests du MODÈLE de la vue PDF (fragment_pdf::vue_modele) : sans navigateur, sans pdfium.
// Même nom que le test e2e, en snake_case ; les cas AJOUTÉS (virtualisation, geste,
// état éphémère, pixels) suivent. Données réécrites : pages 300 x 420 pt, etc.

#![allow(clippy::expect_used, clippy::unwrap_used)] // fichier de tests

use fragment_pdf::pdf_geometry::{crans, depuis_crans};
use fragment_pdf::pdf_zoom::{Sens, Taille, ZoomMode};
use fragment_pdf::vue_modele::{
    DELAI_FIN_DE_GESTE, EtatPage, MODES_ZOOM, Pixels, Point, PositionLecture, Raccourci,
    SENSIBILITE_MOLETTE, VueModele, analyser_entier, mode_depuis_texte, mode_vers_texte,
    raccourci_pour, taille_physique,
};
use serde_json::json;

const GAP: f64 = 16.0;
const PAD: f64 = 16.0;

/// Un document de `n` pages `largeur` x `hauteur` (points), zone `zl` x `zh`, densité 1.
fn ouverte(n: usize, largeur: f64, hauteur: f64, zl: f64, zh: f64) -> VueModele {
    let mut v = VueModele::new();
    v.definir_zone(zl, 0.0, zh);
    v.ouvrir(n, Taille { largeur, hauteur }, GAP, PAD, 1.0);
    v
}

/// Le document de la barre d'outils : 40 pages 300 x 420, zone 800 x 600.
fn barre() -> VueModele {
    ouverte(40, 300.0, 420.0, 800.0, 600.0)
}

fn largeur_page(v: &VueModele) -> f64 {
    v.geometrie().largeur_px(0)
}

/// Marque comme prêtes toutes les pages en cours de rendu (la vue a fini de peindre).
fn tout_peindre(v: &mut VueModele) {
    for p in v.pages_peintes() {
        let t = v.pixels_attendus(p).expect("taille physique");
        v.page_prete(p, t);
    }
}

/// Le haut de chaque page recalculé à part, en crans entiers : padding, puis
/// (hauteur en crans + gap) cumulés. Les hauteurs sont `hauteurs` à l'échelle 1.
fn tops_attendus(hauteurs: &[f64], echelle: f64, densite: f64, gap: f64, pad: f64) -> Vec<f64> {
    let mut y = pad;
    hauteurs
        .iter()
        .map(|h| {
            let haut = y;
            y += depuis_crans(crans(h * echelle, densite), densite) + gap;
            haut
        })
        .collect()
}

fn ecart_max(v: &VueModele, attendus: &[f64]) -> f64 {
    attendus
        .iter()
        .enumerate()
        .map(|(i, t)| (v.geometrie().top_px(i) - t).abs())
        .fold(0.0, f64::max)
}

// ───────────────────────── pdf-toolbar.spec.ts ─────────────────────────

#[test]
fn of_n_et_le_numero_de_page_suit_le_defilement() {
    let mut v = barre();
    assert_eq!(v.total_texte(), "of 40");
    assert_eq!(v.champ().valeur, "1");
    let haut = v.geometrie().top_px(9);
    v.defiler(haut, 0.0);
    assert_eq!(v.page_vue(), 10);
    assert_eq!(v.champ().valeur, "10");
}

#[test]
fn saisir_un_numero_entree_y_va_borne_echap_restaure() {
    let mut v = barre();
    v.champ_focaliser();
    assert!(v.champ().tout_selectionne);
    v.champ_saisir("25");
    v.champ_valider();
    assert_eq!(v.page_vue(), 25);
    assert_eq!(v.champ().valeur, "25");
    // La page 25 est en haut de la zone.
    assert!((v.scroll_top() - v.geometrie().top_px(24)).abs() < 2.0);

    // Trop grand : borné au nombre de pages (la dernière ne peut pas monter en haut).
    v.champ_saisir("999");
    v.champ_valider();
    assert_eq!(v.page_vue(), 40);
    assert_eq!(v.champ().valeur, "40");

    // Échap : la saisie est abandonnée, le champ garde la page courante.
    v.champ_saisir("3");
    v.champ_echap();
    assert_eq!(v.champ().valeur, "40");
    assert!(!v.champ().focus);
    assert_eq!(v.page_vue(), 40);
}

#[test]
fn zoom_pm_change_la_taille_des_pages_les_modes_du_menu_aussi() {
    let mut v = barre();
    let avant = largeur_page(&v);
    v.zoomer_pas(Sens::Plus);
    assert!(largeur_page(&v) > avant + 5.0);
    v.zoomer_pas(Sens::Moins);
    v.zoomer_pas(Sens::Moins);
    assert!(largeur_page(&v) < avant - 5.0);

    // « Page width » : la page remplit la zone moins les 32 px de marge.
    v.choisir_mode(ZoomMode::PageWidth);
    let reste = v.zone().largeur_utile - largeur_page(&v);
    assert!(reste > 30.0 && reste < 34.0, "reste = {reste}");

    v.choisir_mode(ZoomMode::PageActual);
    assert!((largeur_page(&v) - 300.0).abs() < 0.5);

    v.choisir_pourcentage(2.0);
    assert!((largeur_page(&v) - 600.0).abs() < 0.5);
    assert_eq!(v.mode(), None);
    assert!(v.pourcentage_coche(2.0));

    v.choisir_mode(ZoomMode::PageFit);
    assert!(v.geometrie().hauteur_px(0) <= v.zone().hauteur);

    // Le mode coché dans le menu, c'est le mode en vigueur.
    let coches: Vec<&str> = MODES_ZOOM
        .iter()
        .filter(|(m, _)| v.mode_coche(*m))
        .map(|(_, titre)| *titre)
        .collect();
    assert_eq!(coches, ["Page fit"]);
    assert!(!v.pourcentage_coche(2.0));
}

#[test]
fn pinch_ctrl_molette_zoom_continu_ancre_sous_le_curseur_un_repeint_final() {
    // 200 % : la page (600 px) déborde la zone (500 px), l'ancrage horizontal a de la marge ;
    // loin du plafond de 400 %, que 12 crans de pinch atteindraient.
    let mut v = ouverte(40, 300.0, 420.0, 500.0, 700.0);
    v.choisir_pourcentage(2.0);
    tout_peindre(&mut v);
    v.champ_saisir("4");
    v.champ_valider();
    tout_peindre(&mut v);

    // Un point bien DANS la page 4, décalé du centre : à 30 % en largeur, 40 % en hauteur.
    let (w0, h0) = (v.geometrie().largeur_px(3), v.geometrie().hauteur_px(3));
    let cible = Point {
        x: v.geometrie().left_px(3) - v.scroll_left() + 0.3 * w0,
        y: v.geometrie().top_px(3) - v.scroll_top() + 0.4 * h0,
    };
    let fraction = |v: &VueModele| {
        let g = v.geometrie();
        (
            (cible.x + v.scroll_left() - g.left_px(3)) / g.largeur_px(3),
            (cible.y + v.scroll_top() - g.top_px(3)) / g.hauteur_px(3),
            g.largeur_px(3),
            g.hauteur_px(3),
        )
    };
    let (fx0, fy0, _, _) = fraction(&v);

    // Rafale de petits deltas, comme un pinch de trackpad.
    let mut t = 1_000;
    for _ in 0..12 {
        v.avancer(t);
        let r = v.zoomer_molette(-8.0, 0, true, cible);
        assert!(r.empeche_defaut);
        // ★ Aucun repeint, aucune page peinte ou libérée pendant le geste.
        assert!(r.rendu.peindre.is_empty() && r.rendu.liberer.is_empty());
        assert!(r.rendu.en_zoom);
        t += 5;
    }
    assert!(v.en_geste());
    let (fx, fy, w, h) = fraction(&v);
    assert!(w > w0 * 1.05);
    // Le point sous le curseur n'a pas bougé (5 px).
    assert!(((fx - fx0) * w).abs() < 5.0, "dx = {}", (fx - fx0) * w);
    assert!(((fy - fy0) * h).abs() < 5.0, "dy = {}", (fy - fy0) * h);

    // Fin du geste : UN repeint, à DELAI_FIN_DE_GESTE ms du dernier événement.
    let dernier = t - 5;
    assert!(v.avancer(dernier + DELAI_FIN_DE_GESTE - 1).est_vide());
    assert!(v.en_geste());
    let fin = v.avancer(dernier + DELAI_FIN_DE_GESTE);
    assert!(!v.en_geste());
    assert!(!fin.en_zoom);
    // La page 4 est repeinte, l'ancienne image gardée dessous (is-stale côté vue).
    let page4 = fin.peindre.iter().find(|p| p.page == 4).expect("page 4");
    assert!(page4.garder_ancienne);
    // Plus rien à repeindre ensuite.
    assert!(v.avancer(dernier + 10 * DELAI_FIN_DE_GESTE).est_vide());
    assert_eq!(v.prochaine_echeance(), None);

    // Ctrl + molette vers le bas : dézoome.
    let pendant = w;
    v.avancer(5_000);
    v.zoomer_molette(300.0, 0, true, cible);
    assert!(v.geometrie().largeur_px(3) < pendant * 0.9);
}

#[test]
fn ctrl_plus_ctrl_moins_ctrl_0_zooment_le_pdf_pas_la_fenetre() {
    let mut v = barre();
    v.choisir_mode(ZoomMode::PageActual);
    let w0 = largeur_page(&v);

    let touches = |v: &mut VueModele, code: &str, maj: bool| {
        let r = raccourci_pour(code, maj).unwrap_or_else(|| panic!("{code} sans raccourci"));
        v.executer_raccourci(r);
    };
    touches(&mut v, "Equal", false);
    assert!(largeur_page(&v) > w0 + 5.0);
    let w1 = largeur_page(&v);
    touches(&mut v, "Equal", true); // « + » en AZERTY : Maj + « = »
    assert!(largeur_page(&v) > w1 + 5.0);
    touches(&mut v, "Minus", false);
    touches(&mut v, "Minus", false);
    touches(&mut v, "NumpadSubtract", false);
    assert!(largeur_page(&v) < w0 - 5.0);

    // Ctrl + 0 : retour au mode automatique (touche physique : « à » en AZERTY).
    touches(&mut v, "Digit0", false);
    assert!(v.mode_coche(ZoomMode::Auto));
    assert_eq!(v.mode(), Some(ZoomMode::Auto));

    // Les autres touches ne sont pas des raccourcis de zoom ; Ctrl + Maj + « - » non plus.
    assert_eq!(raccourci_pour("KeyA", false), None);
    assert_eq!(raccourci_pour("Minus", true), None);
    assert_eq!(raccourci_pour("Numpad0", false), Some(Raccourci::ModeAuto));
    assert_eq!(
        raccourci_pour("NumpadAdd", false),
        Some(Raccourci::ZoomPlus)
    );
}

#[test]
fn page_width_suit_la_largeur_de_la_zone_barre_laterale_ouverte() {
    let mut v = barre();
    v.choisir_mode(ZoomMode::PageWidth);
    let fermee = largeur_page(&v);
    // La barre latérale s'ouvre : la zone perd 250 px.
    v.zone_redimensionnee(550.0, 600.0);
    assert!(largeur_page(&v) < fermee - 50.0);
    assert!(v.en_geste(), "le suivi de la zone est un geste différé");
    v.avancer(500);
    assert!(!v.en_geste());
    // Un mode libre (pourcentage) ne suit pas la zone.
    v.choisir_pourcentage(1.0);
    v.zone_redimensionnee(900.0, 600.0);
    assert!((largeur_page(&v) - 300.0).abs() < 0.5);
}

// ───────────────────────── pdf-geometry.spec.ts ─────────────────────────

#[test]
fn pages_egales_a_l_ouverture_aux_echelles_au_pinch_marge_de_lecture_fenetre_redimensionnee() {
    let hauteurs = vec![420.0; 30];
    let mut v = ouverte(30, 300.0, 420.0, 1000.0, 700.0);
    let controle = |v: &VueModele, quoi: &str| {
        let e = v.echelle();
        let attendus = tops_attendus(&hauteurs, e, 1.0, GAP, PAD);
        assert!(ecart_max(v, &attendus) < 0.5, "{quoi}");
        // Chaque page commence où la précédente finit, plus l'espacement.
        for i in 1..30 {
            let g = v.geometrie();
            let d = g.top_px(i) - g.top_px(i - 1) - g.hauteur_px(i - 1) - GAP;
            assert!(d.abs() < 1e-9, "{quoi} : page {i}");
        }
    };
    controle(&v, "ouverture");
    for e in [0.5, 1.0, 1.37, 2.0, 3.0] {
        v.appliquer_echelle(e, None, false);
        controle(&v, &format!("échelle {e}"));
    }
    // Pinch : canvas étirés, repeint différé, la géométrie est déjà juste.
    v.appliquer_echelle(1.6, None, true);
    controle(&v, "pinch");

    // Marge de lecture : 40 % de la zone en padding, les pages se recentrent.
    let avant = v.geometrie().left_px(0);
    v.definir_zone(600.0, 400.0, 700.0);
    controle(&v, "marge de lecture");
    let l = v.geometrie().largeur_px(0);
    assert!((v.geometrie().left_px(0) - ((600.0 - l) / 2.0).max(0.0)).abs() < 1e-9);
    assert!(v.geometrie().left_px(0) < avant);

    // Fenêtre redimensionnée, puis retour.
    v.zone_redimensionnee(760.0, 600.0);
    controle(&v, "fenêtre redimensionnée");
    v.zone_redimensionnee(1000.0, 700.0);
    controle(&v, "retour");
}

#[test]
fn zoom_de_l_app_densite_changee_la_grille_change_la_geometrie_suit() {
    let hauteurs = vec![420.0; 30];
    let mut v = ouverte(30, 300.0, 420.0, 1000.0, 700.0);
    v.appliquer_echelle(1.37, None, false);
    for d in [1.25, 0.9, 1.0] {
        let plan = v.set_densite(d);
        assert!(!plan.peindre.is_empty(), "la densité change : on repeint");
        let attendus = tops_attendus(&hauteurs, 1.37, d, GAP, PAD);
        assert!(ecart_max(&v, &attendus) < 0.5, "densité {d}");
        // Une page mesure un nombre ENTIER de crans de la nouvelle grille.
        let crans = v.geometrie().hauteur_px(0) * 64.0 * d;
        assert!((crans - crans.round()).abs() < 1e-6, "densité {d}");
    }
}

#[test]
fn la_page_la_plus_visible_et_la_position_de_lecture_suivent_le_modele() {
    let mut v = ouverte(30, 300.0, 420.0, 1000.0, 700.0);
    v.appliquer_echelle(1.0, None, false);
    let haut = v.geometrie().top_px(11) + 40.0;
    v.defiler(haut, 0.0);
    assert_eq!(v.page_courante(), 12);
    let lecture = v.lire_position().expect("position");
    assert_eq!(lecture.page, 12);
    assert!(lecture.fy > 0.0);
    // 40 px dans une page de 420 : fy = 40 / 420 arrondi à 1e-4.
    assert!((lecture.fy - 0.0952).abs() < 1e-9);
}

#[test]
fn contre_epreuve_un_espacement_change_sans_recalibrage_est_detecte() {
    let hauteurs = vec![420.0; 30];
    let mut v = ouverte(30, 300.0, 420.0, 1000.0, 700.0);
    // Le « CSS » passe à 20 px d'espacement : le modèle, non recalibré, s'écarte.
    let reel = tops_attendus(&hauteurs, v.echelle(), 1.0, 20.0, PAD);
    assert!(ecart_max(&v, &reel) > 0.5);
    // Recalibré, il retombe juste.
    v.recalibrer_espacements(20.0, PAD);
    assert!(ecart_max(&v, &reel) < 0.5);
}

#[test]
fn pages_de_tailles_differentes_corrigees_au_rendu_gardees_au_zoom() {
    let formats = [(420.0, 600.0), (600.0, 420.0), (300.0, 800.0)];
    let mut v = ouverte(12, 420.0, 600.0, 1000.0, 700.0);
    // Au rendu, chaque page déclare sa vraie taille.
    for i in 0..12 {
        let (l, h) = formats[i % 3];
        v.taille_page_connue(i + 1, l, h);
    }
    let hauteurs: Vec<f64> = (0..12).map(|i| formats[i % 3].1).collect();
    let largeurs: Vec<f64> = (0..3).map(|i| v.geometrie().taille(i).largeur).collect();
    assert_eq!(largeurs, [420.0, 600.0, 300.0]);
    assert!(ecart_max(&v, &tops_attendus(&hauteurs, v.echelle(), 1.0, GAP, PAD)) < 0.5);
    // Une page déjà connue ne bouge pas : pas de plan.
    assert!(v.taille_page_connue(1, 420.0, 600.0).est_vide());
    for e in [0.6, 1.8] {
        v.appliquer_echelle(e, None, false);
        let l: Vec<f64> = (0..3).map(|i| v.geometrie().taille(i).largeur).collect();
        assert_eq!(l, [420.0, 600.0, 300.0], "tailles gardées à {e}");
        assert!(ecart_max(&v, &tops_attendus(&hauteurs, e, 1.0, GAP, PAD)) < 0.5);
    }
}

#[test]
fn long_document_pas_de_derive_des_sommes_cumulees_300_pages_echelle_non_ronde() {
    let hauteurs = vec![841.89; 300];
    for densite in [1.0, 1.5] {
        let mut v = VueModele::new();
        v.definir_zone(1000.0, 0.0, 700.0);
        v.ouvrir(
            300,
            Taille {
                largeur: 595.276,
                hauteur: 841.89,
            },
            GAP,
            PAD,
            densite,
        );
        for e in [1.37, 0.73] {
            v.appliquer_echelle(e, None, false);
            let attendus = tops_attendus(&hauteurs, e, densite, GAP, PAD);
            assert!(
                ecart_max(&v, &attendus) < 1e-6,
                "300 pages à {e}, densité {densite}"
            );
        }
        // La virtualisation ne garde que la bande : 300 pages, une poignée de rendus.
        assert!(v.pages_peintes().len() < 15);
    }
}

// ───────────────────── Cas AJOUTÉS : virtualisation, gestes ─────────────────────

#[test]
fn le_defilement_peint_seulement_les_pages_qui_entrent() {
    let mut v = ouverte(100, 300.0, 420.0, 800.0, 600.0);
    v.choisir_pourcentage(1.0);
    tout_peindre(&mut v);
    let (a, b) = v.pages_visibles().expect("bande");
    assert_eq!(a, 1);
    assert!(b < 10, "la bande ne couvre qu'un écran de marge : {b}");
    let avant = v.pages_peintes();
    assert_eq!(avant, (a..=b).collect::<Vec<_>>());

    // On défile d'une page et demie : quelques pages entrent, aucune n'est repeinte.
    let plan = v.defiler(v.scroll_top() + 650.0, 0.0);
    let (a2, b2) = v.pages_visibles().expect("bande");
    assert!(b2 > b);
    let attendues: Vec<usize> = ((b + 1)..=b2).collect();
    assert_eq!(plan.pages_a_peindre(), attendues);
    assert!(plan.peindre.iter().all(|p| !p.garder_ancienne));
    assert_eq!(plan.liberer, (a..a2).collect::<Vec<_>>());
    // Les pages déjà prêtes restent prêtes.
    assert_eq!(v.etat_page(b), EtatPage::Prete);
    // Rejouer le même défilement ne refait rien.
    assert!(v.defiler(v.scroll_top(), 0.0).est_vide());
}

#[test]
fn aucun_repeint_pendant_un_geste_meme_en_defilant_la_fin_du_geste_range_tout() {
    let mut v = ouverte(100, 300.0, 420.0, 800.0, 600.0);
    v.choisir_pourcentage(1.0);
    tout_peindre(&mut v);
    v.avancer(0);
    v.appliquer_echelle(2.0, None, true);
    // Défilement PENDANT le geste : on note la bande, on ne peint rien.
    let plan = v.defiler(v.scroll_top() + 5000.0, 0.0);
    assert!(plan.peindre.is_empty() && plan.liberer.is_empty());
    // Chaque événement repousse la fin (resetTimer).
    v.avancer(60);
    v.appliquer_echelle(2.2, None, true);
    assert!(v.avancer(150).est_vide());
    let fin = v.avancer(160);
    let (a, b) = v.pages_visibles().expect("bande");
    assert_eq!(fin.pages_a_peindre(), (a..=b).collect::<Vec<_>>());
    // Les pages d'avant le défilement sont libérées, celles de la bande n'avaient pas d'image.
    assert!(!fin.liberer.is_empty());
    assert!(fin.peindre.iter().all(|p| !p.garder_ancienne));
    assert_eq!(v.pages_peintes(), (a..=b).collect::<Vec<_>>());
}

#[test]
fn un_changement_d_echelle_non_differe_repeint_tout_de_suite() {
    let mut v = barre();
    tout_peindre(&mut v);
    let vus = v.pages_peintes();
    let plan = v.zoomer_pas(Sens::Plus);
    assert!(!plan.en_zoom);
    assert!(!v.en_geste());
    for p in plan.peindre {
        assert!(p.garder_ancienne, "page {} : image gardée dessous", p.page);
    }
    assert!(!vus.is_empty());
    // Échelle inchangée (déjà au palier) : aucun plan.
    let mut v = barre();
    v.choisir_pourcentage(4.0);
    assert!(v.appliquer_echelle(4.0, None, false).is_none());
    assert!(
        v.appliquer_echelle(9.0, None, false).is_none(),
        "bornée à 4"
    );
}

#[test]
fn la_molette_sans_ctrl_n_est_pas_pour_nous_les_lignes_valent_16_pixels() {
    let mut v = barre();
    let milieu = Point { x: 400.0, y: 300.0 };
    let r = v.zoomer_molette(100.0, 0, false, milieu);
    assert!(!r.empeche_defaut && r.rendu.est_vide());
    assert_eq!(v.mode(), Some(ZoomMode::Auto));

    let mut a = barre();
    let mut b = barre();
    a.zoomer_molette(-3.0, 1, true, milieu);
    b.zoomer_molette(-48.0, 0, true, milieu);
    assert!((a.echelle() - b.echelle()).abs() < 1e-12);
    // Un cran de souris (100) vaut x1,6.
    assert!((SENSIBILITE_MOLETTE * 100.0 - 1.6f64.ln()).abs() < 1e-12);
    let mut c = ouverte(10, 300.0, 420.0, 800.0, 600.0);
    c.choisir_pourcentage(1.0);
    c.zoomer_molette(-100.0, 0, true, milieu);
    assert!((c.echelle() - 1.6).abs() < 1e-9);
    assert_eq!(c.mode(), None, "le pinch quitte le mode");

    // Sans document : on retient le défaut du navigateur (le zoom de la page), rien d'autre.
    let mut vide = VueModele::new();
    let r = vide.zoomer_molette(-100.0, 0, true, milieu);
    assert!(r.empeche_defaut && r.rendu.est_vide());
}

#[test]
fn largeur_zero_ignoree_l_echelle_et_le_defilement_survivent_a_un_dock_replie() {
    let mut v = barre();
    v.choisir_mode(ZoomMode::PageWidth);
    v.avancer(0);
    v.defiler(2_000.0, 0.0);
    let (echelle, haut, zone) = (v.echelle(), v.scroll_top(), v.zone());
    let plan = v.zone_redimensionnee(0.0, 0.0);
    assert!(plan.est_vide());
    assert_eq!(
        (v.echelle(), v.scroll_top(), v.zone()),
        (echelle, haut, zone)
    );
    assert!(!v.en_geste());
    // Au retour, rien n'a bougé : pas de zoom ancré sur des pages minuscules.
    v.zone_redimensionnee(800.0, 600.0);
    assert!((v.echelle() - echelle).abs() < 1e-9);
}

#[test]
fn zone_cachee_de_hauteur_nulle_aucune_page_a_peindre() {
    let mut v = barre();
    tout_peindre(&mut v);
    let plan = v.zone_redimensionnee(800.0, 0.0);
    assert!(plan.peindre.is_empty());
    assert!(v.pages_peintes().is_empty(), "tout est libéré");
    assert_eq!(v.pages_visibles(), None);
}

#[test]
fn le_zoom_se_recale_sur_le_haut_de_la_zone_sans_point() {
    // Sans curseur : le point fixe est le milieu du HAUT de la zone.
    let mut v = barre();
    v.choisir_pourcentage(1.0);
    v.defiler(2_000.0, 0.0);
    let avant = v.lire_position().expect("position");
    v.choisir_pourcentage(2.0);
    let apres = v.lire_position().expect("position");
    assert_eq!(avant.page, apres.page);
    assert!((avant.fy - apres.fy).abs() < 1e-3, "{avant:?} {apres:?}");
    assert_eq!(v.scroll_left(), 0.0);
}

#[test]
fn le_defilement_est_borne_au_contenu() {
    let mut v = barre();
    v.defiler(1e9, 1e9);
    let max = v.geometrie().hauteur_totale_px() - v.zone().hauteur;
    assert!((v.scroll_top() - max).abs() < 1e-9);
    assert_eq!(v.scroll_left(), 0.0, "page plus étroite que la zone");
    v.defiler(-50.0, 0.0);
    assert_eq!(v.scroll_top(), 0.0);
}

// ───────────────────── Cas AJOUTÉS : champ de page ─────────────────────

#[test]
fn le_champ_ne_s_ecrase_pas_pendant_la_saisie_et_refuse_l_illisible() {
    let mut v = barre();
    v.champ_focaliser();
    v.champ_saisir("2");
    v.defiler(v.geometrie().top_px(5), 0.0);
    assert_eq!(v.page_vue(), 6);
    assert_eq!(
        v.champ().valeur,
        "2",
        "le défilement n'écrase pas la saisie"
    );
    v.champ_saisir("abc");
    v.champ_valider();
    assert_eq!(v.page_vue(), 6, "saisie illisible : on n'y va pas");
    assert_eq!(v.champ().valeur, "6");
    v.champ_saisir(" 7 pages");
    v.champ_valider();
    assert_eq!(v.page_vue(), 7);
    v.champ_saisir("-4");
    v.champ_valider();
    assert_eq!(v.page_vue(), 1, "borné à 1");
    v.champ_perdre_focus();
    v.defiler(v.geometrie().top_px(2), 0.0);
    assert_eq!(v.champ().valeur, "3");
}

#[test]
fn analyser_entier_suit_parse_int() {
    assert_eq!(analyser_entier("25"), Some(25));
    assert_eq!(analyser_entier("  25abc"), Some(25));
    assert_eq!(analyser_entier("+8"), Some(8));
    assert_eq!(analyser_entier("-3"), Some(-3));
    assert_eq!(analyser_entier(""), None);
    assert_eq!(analyser_entier("abc"), None);
    assert_eq!(analyser_entier("99999999999999999999999"), Some(i64::MAX));
}

#[test]
fn aller_a_une_destination_vise_la_hauteur_pdf_origine_en_bas() {
    let mut v = ouverte(10, 595.0, 842.0, 1000.0, 600.0);
    v.choisir_pourcentage(1.0);
    v.aller_a_la_page(3, Some(700.0));
    let attendu = v.geometrie().top_px(2) + (842.0 - 700.0);
    assert!((v.scroll_top() - attendu).abs() < 1e-6);
    assert_eq!(v.page_vue(), 3);
    // Page hors bornes : on ne bouge pas.
    let avant = v.scroll_top();
    assert!(v.aller_a_la_page(11, None).est_vide());
    assert!(v.aller_a_la_page(0, None).est_vide());
    assert_eq!(v.scroll_top(), avant);
}

// ───────────────────── Cas AJOUTÉS : position de lecture, état éphémère ─────────────────────

#[test]
fn la_lecture_en_attente_est_rejouee_a_l_ouverture() {
    let mut v = VueModele::new();
    v.definir_zone(800.0, 0.0, 600.0);
    let (reprise, plan) = v.aller_a_position(PositionLecture {
        page: 12,
        fy: 0.25,
        fx: 0.0,
    });
    assert!(reprise.is_none() && plan.est_vide());
    // Pas encore de pages : la position demandée est celle qu'on rend.
    assert_eq!(v.lire_position().map(|p| p.page), Some(12));
    v.ouvrir(
        40,
        Taille {
            largeur: 300.0,
            hauteur: 420.0,
        },
        GAP,
        PAD,
        1.0,
    );
    let lue = v.lire_position().expect("position");
    assert_eq!(lue.page, 12);
    assert!((lue.fy - 0.25).abs() < 1e-3);
    assert_eq!(v.page_vue(), 12);
}

#[test]
fn la_reprise_de_position_attend_deux_essais_calmes() {
    let mut v = barre();
    let (reprise, _) = v.aller_a_position(PositionLecture {
        page: 20,
        fy: 0.5,
        fx: 0.0,
    });
    // Le premier essai a déplacé le défilement (écart important) : pas fini.
    let mut reprise = reprise.expect("encore en cours");
    // Quelque chose déplace la page entre deux essais (barre latérale, zoom restauré).
    v.defiler(0.0, 0.0);
    let (fini, _) = reprise.pas(&mut v);
    assert!(!fini, "écart important : on recommence");
    let (fini, _) = reprise.pas(&mut v);
    assert!(!fini, "un seul essai calme");
    let (fini, _) = reprise.pas(&mut v);
    assert!(fini);
    let lue = v.lire_position().expect("position");
    assert_eq!(lue.page, 20);
    assert!((lue.fy - 0.5).abs() < 1e-3);
}

#[test]
fn la_position_de_lecture_ne_depend_pas_du_zoom() {
    let mut v = barre();
    v.defiler(v.geometrie().top_px(14) + 100.0, 0.0);
    let avant = v.lire_position().expect("position");
    v.choisir_pourcentage(1.5);
    let (reprise, _) = v.aller_a_position(avant);
    assert!(reprise.is_some() || v.lire_position().is_some());
    let apres = v.lire_position().expect("position");
    assert_eq!(apres.page, avant.page);
    assert!((apres.fy - avant.fy).abs() < 1e-3);
    assert_eq!(
        PositionLecture::depuis_json(&apres.vers_json()),
        Some(apres)
    );
    assert_eq!(PositionLecture::depuis_json(&json!({ "fy": 0.3 })), None);
}

#[test]
fn l_etat_ephemere_se_restaure_un_mode_prime_sur_l_echelle() {
    let mut v = barre();
    v.choisir_pourcentage(1.5);
    v.defiler(v.geometrie().top_px(9), 0.0);
    let etat = v.etat_ephemere();
    assert_eq!(etat.page, 10);
    assert_eq!(etat.zoom_mode, None);
    assert_eq!(
        etat.vers_json(),
        json!({ "page": 10, "scale": 1.5, "zoomMode": null })
    );

    // Dans une vue neuve : échelle libre restaurée avant l'ouverture, page rejouée après.
    let mut n = VueModele::new();
    n.definir_zone(800.0, 0.0, 600.0);
    n.restaurer_etat_ephemere(&etat.vers_json());
    assert_eq!(n.etat_ephemere().page, 10, "page en attente");
    n.ouvrir(
        40,
        Taille {
            largeur: 300.0,
            hauteur: 420.0,
        },
        GAP,
        PAD,
        1.0,
    );
    assert_eq!(n.mode(), None);
    assert!((n.echelle() - 1.5).abs() < 1e-9);
    assert_eq!(n.page_vue(), 10);

    // Un mode prime sur l'échelle ; une échelle hors bornes est ignorée.
    let plan = n.restaurer_etat_ephemere(&json!({ "zoomMode": "page-width", "scale": 3.0 }));
    assert!(!plan.peindre.is_empty());
    assert_eq!(n.mode(), Some(ZoomMode::PageWidth));
    assert!(n.echelle() < 3.0);
    let avant = n.echelle();
    n.restaurer_etat_ephemere(&json!({ "scale": 99.0 }));
    assert_eq!(n.echelle(), avant);
    // Un mode inconnu retombe sur l'échelle.
    n.restaurer_etat_ephemere(&json!({ "zoomMode": "bizarre", "scale": 2.0 }));
    assert_eq!((n.mode(), n.echelle()), (None, 2.0));
    // Une position `scroll` (plus précise) prime sur la seule page.
    n.restaurer_etat_ephemere(&json!({ "page": 3, "scroll": { "page": 30 } }));
    assert_eq!(n.page_vue(), 10, "la page seule est ignorée");
}

#[test]
fn les_modes_se_serialisent_comme_pdf_js() {
    for (mode, _) in MODES_ZOOM {
        assert_eq!(mode_depuis_texte(mode_vers_texte(mode)), Some(mode));
    }
    assert_eq!(mode_vers_texte(ZoomMode::PageActual), "page-actual");
    assert_eq!(mode_depuis_texte("zoom"), None);
}

// ───────────────────── Cas AJOUTÉS : un pixel de raster par pixel physique ─────────────────────

#[test]
fn la_taille_physique_accroche_les_bords_a_la_grille() {
    // 100,2 px CSS de large posés à 10,3 sur un écran 150 % : 15,45 -> 15 et 165,75 -> 166.
    let p = taille_physique(10.3, 0.0, 100.2, 50.0, 1.5).expect("taille");
    assert_eq!(p, Pixels { w: 151, h: 75 });
    // Ce n'est pas l'arrondi de la largeur seule (150).
    assert_ne!(p.w, (100.2f64 * 1.5).round() as u32);
    // Boîte vide ou cachée : rien à peindre.
    assert_eq!(taille_physique(0.0, 0.0, 0.0, 10.0, 1.0), None);
    assert_eq!(taille_physique(0.0, 0.0, 10.0, 10.0, 0.0), None);
}

#[test]
fn le_canvas_a_la_taille_physique_attendue_un_ecart_se_recale_au_calme() {
    let mut v = ouverte(10, 300.0, 420.0, 800.0, 600.0);
    v.choisir_pourcentage(1.37);
    v.avancer(1_000);
    tout_peindre(&mut v);
    // Une page peinte à la taille calculée : rien à recaler.
    assert_eq!(v.prochaine_echeance(), None);

    // La vue mesure une autre taille (la page a glissé d'un pixel) : recalage à 100 ms.
    let t = v.pixels_attendus(2).expect("taille");
    v.pixels_livres(2, Some(Pixels { w: t.w + 1, h: t.h }));
    assert_eq!(v.prochaine_echeance(), Some(1_000 + DELAI_FIN_DE_GESTE));
    assert!(v.avancer(1_000 + DELAI_FIN_DE_GESTE - 1).est_vide());
    let plan = v.avancer(1_000 + DELAI_FIN_DE_GESTE);
    assert_eq!(plan.pages_a_peindre(), vec![2]);
    assert!(plan.peindre[0].garder_ancienne);
    assert_eq!(v.etat_page(2), EtatPage::EnRendu { ancienne: true });
    assert_eq!(v.etat_page(1), EtatPage::Prete);

    // Un canvas à la taille mesurée ne demande plus rien.
    v.page_prete(2, Pixels { w: t.w + 1, h: t.h });
    assert_eq!(v.prochaine_echeance(), None);
    // Cachée (`None`) : ignoré.
    v.pixels_livres(2, None);
    assert_eq!(v.prochaine_echeance(), None);
}

#[test]
fn pas_de_recalage_pendant_un_geste() {
    let mut v = barre();
    v.avancer(0);
    tout_peindre(&mut v);
    let t = v.pixels_attendus(1).expect("taille");
    v.pixels_livres(1, Some(Pixels { w: t.w + 2, h: t.h }));
    // Un pinch démarre : sa fin repeint tout, le recalage est annulé.
    v.appliquer_echelle(2.0, None, true);
    assert!(v.en_geste());
    let plan = v.avancer(DELAI_FIN_DE_GESTE);
    assert!(!plan.peindre.is_empty());
    assert_eq!(v.prochaine_echeance(), None);
}

#[test]
fn fermer_oublie_le_document_mais_garde_le_zoom() {
    let mut v = barre();
    v.choisir_pourcentage(2.0);
    v.defiler(3_000.0, 0.0);
    v.fermer();
    assert_eq!(v.nombre_pages(), 0);
    assert_eq!(v.page_vue(), 0);
    assert_eq!(v.total_texte(), "");
    assert_eq!(v.champ().valeur, "");
    assert!(v.pages_peintes().is_empty());
    assert_eq!(v.scroll_top(), 0.0);
    assert_eq!(v.echelle(), 2.0);
    assert!(v.lire_position().is_none());
}
