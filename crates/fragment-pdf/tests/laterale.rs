// Port de app/e2e/pdf-toolbar.spec.ts (tests 7 à 12 : barre latérale), en tests du MODÈLE
// de la barre (fragment_pdf::laterale) : sans navigateur, sans pdfium. Même nom que le
// test e2e, en snake_case ; les cas AJOUTÉS (virtualisation, poignée, état éphémère, ...)
// suivent. Données réécrites : 40 pages 300 x 420 pt, table à trois racines.

#![allow(clippy::expect_used, clippy::unwrap_used)] // fichier de tests

use fragment_pdf::laterale::{
    BarreLaterale, Destination, LARGEUR_DEFAUT, LARGEUR_MAX, LARGEUR_MIN, MESSAGE_SANS_TABLE,
    ModeBarre, PlanVignettes,
};
use fragment_pdf::pdf_outline::EntreeToc;
use fragment_pdf::pdf_zoom::{Taille, ZoomMode};
use fragment_pdf::vue_modele::VueModele;

const PAGES: usize = 40;
const RATIO: f64 = 420.0 / 300.0;
const HAUTEUR_BARRE: f64 = 500.0;

fn entree(titre: &str, page: u32, enfants: Vec<EntreeToc>) -> EntreeToc {
    EntreeToc {
        titre: titre.to_string(),
        page: Some(page),
        y: None,
        enfants,
    }
}

/// Introduction (1), Chapitre Un (5) { Section 1.1 (7), Section 1.2 (12) }, Chapitre Deux (20).
fn table() -> Vec<EntreeToc> {
    vec![
        entree("Introduction", 1, vec![]),
        entree(
            "Chapitre Un",
            5,
            vec![
                entree("Section 1.1", 7, vec![]),
                entree("Section 1.2", 12, vec![]),
            ],
        ),
        entree("Chapitre Deux", 20, vec![]),
    ]
}

/// Une barre ouverte sur le document de la barre d'outils, mesurée à 500 px de haut.
fn barre_ouverte(toc: Vec<EntreeToc>) -> BarreLaterale {
    let mut b = BarreLaterale::new();
    b.charger(PAGES, RATIO, toc);
    b.definir_hauteur(HAUTEUR_BARRE);
    b.set_ouverte(true);
    b
}

fn titres(b: &BarreLaterale) -> Vec<String> {
    b.toc().lignes().into_iter().map(|l| l.titre).collect()
}

fn chemin_de(b: &BarreLaterale, titre: &str) -> Vec<usize> {
    b.toc()
        .lignes()
        .into_iter()
        .find(|l| l.titre == titre)
        .unwrap_or_else(|| panic!("ligne « {titre} » non visible"))
        .chemin
}

// ───────────────────────── pdf-toolbar.spec.ts ─────────────────────────

#[test]
fn page_width_suit_la_largeur_de_la_zone_barre_laterale_ouverte() {
    let mut v = VueModele::new();
    v.definir_zone(800.0, 0.0, 600.0);
    v.ouvrir(
        PAGES,
        Taille {
            largeur: 300.0,
            hauteur: 420.0,
        },
        16.0,
        16.0,
        1.0,
    );
    v.choisir_mode(ZoomMode::PageWidth);
    let fermee = v.geometrie().largeur_px(0);

    // Fermée, la barre ne prend rien ; ouverte, elle retire sa largeur à la zone.
    let mut b = BarreLaterale::new();
    assert!(!b.est_ouverte());
    assert_eq!(b.zone_utile(800.0), 800.0);
    b.basculer();
    assert!(b.est_ouverte());
    assert_eq!(b.zone_utile(800.0), 800.0 - LARGEUR_DEFAUT);

    v.zone_redimensionnee(b.zone_utile(800.0), 600.0);
    assert!(v.geometrie().largeur_px(0) < fermee - 50.0);
}

#[test]
fn vignettes_peintes_cliquables_la_page_courante_est_active() {
    let mut b = barre_ouverte(table());
    b.set_mode(ModeBarre::Vignettes);
    assert_eq!(b.nombre_vignettes(), PAGES);
    // Peintes : au moins la première a un canvas demandé.
    assert!(b.vignettes_peintes().contains(&1));

    // Cliquer la vignette 3 mène à la page 3 (haut de page) ; la vue répond par sa page courante.
    assert_eq!(
        b.cliquer_vignette(3),
        Some(Destination { page: 3, y: None })
    );
    b.set_page_courante(3);
    assert_eq!(b.page_active(), 3);

    // Aller loin : la vignette active défile en vue.
    b.set_page_courante(35);
    assert_eq!(b.page_active(), 35);
    assert!(b.vignettes_peintes().contains(&35));
    assert!(b.scroll_top() > 0.0);
    // Le numéro de la vignette est son numéro de page : pas de page hors document.
    assert_eq!(b.cliquer_vignette(41), None);
    assert_eq!(b.cliquer_vignette(0), None);
}

#[test]
fn table_des_matieres_arbre_repli_clic_page_a_droite() {
    let mut b = barre_ouverte(table());
    b.set_mode(ModeBarre::TableDesMatieres);

    let lignes = b.toc().lignes();
    let racines: Vec<_> = lignes
        .iter()
        .filter(|l| l.profondeur == 0)
        .map(|l| l.titre.as_str())
        .collect();
    assert_eq!(racines, ["Introduction", "Chapitre Un", "Chapitre Deux"]);
    // Le flair de la première entrée est sa page.
    assert_eq!(lignes[0].page, Some(1));

    // Les enfants sont repliés ; le chevron les déplie.
    assert!(!titres(&b).contains(&"Section 1.2".to_string()));
    let un = chemin_de(&b, "Chapitre Un");
    assert!(b.toc().est_replie(&un));
    b.basculer_entree(&un);
    assert!(titres(&b).contains(&"Section 1.2".to_string()));

    // Cliquer l'entrée mène à sa page et la rend active.
    let section = chemin_de(&b, "Section 1.2");
    assert_eq!(
        b.cliquer_entree(&section),
        Some(Destination { page: 12, y: None })
    );
    let actives: Vec<_> = b
        .toc()
        .lignes()
        .into_iter()
        .filter(|l| l.active)
        .map(|l| l.titre)
        .collect();
    assert_eq!(actives, ["Section 1.2"]);
}

#[test]
fn reveal_page_in_table_of_contents_deplie_et_surligne_lentree_de_la_page() {
    let mut b = barre_ouverte(table());
    b.set_mode(ModeBarre::TableDesMatieres);
    // Déplier puis replier le chapitre, comme le test : il est replié.
    let un = chemin_de(&b, "Chapitre Un");
    b.basculer_entree(&un);
    b.basculer_entree(&un);
    assert!(b.toc().est_replie(&un));
    // La vue est ailleurs (vignettes), page 9 (dans Section 1.1).
    b.set_mode(ModeBarre::Vignettes);
    b.set_page_courante(9);

    let (_, centre) = b.reveler(9);
    assert_eq!(b.mode(), ModeBarre::TableDesMatieres);
    assert!(b.est_ouverte());
    // L'entrée de la page 9 est la dernière qui commence avant : Section 1.1 (page 7).
    assert_eq!(centre, Some(vec![1, 0]));
    let actives: Vec<_> = b
        .toc()
        .lignes()
        .into_iter()
        .filter(|l| l.active)
        .map(|l| l.titre)
        .collect();
    assert_eq!(actives, ["Section 1.1"]);
    // Les ancêtres sont dépliés : l'entrée est visible.
    assert!(!b.toc().est_replie(&un));
    assert!(titres(&b).contains(&"Section 1.1".to_string()));
}

#[test]
fn le_toggle_referme_la_barre_pas_de_debordement_a_600_px() {
    let mut b = barre_ouverte(table());
    b.basculer();
    assert!(!b.est_ouverte());
    assert_eq!(b.largeur_effective(1200.0), 0.0);
    assert!(
        b.vignettes_visibles().is_empty(),
        "barre fermée : rien ne se peint"
    );

    b.basculer();
    assert!(b.est_ouverte());
    // Fenêtre étroite : la barre ne dépasse jamais la moitié de la vue, le document garde sa part.
    b.set_largeur(LARGEUR_MAX);
    let vue = 600.0;
    assert_eq!(b.largeur_effective(vue), 300.0);
    assert!(b.zone_utile(vue) >= vue * 0.5);
    // Même une vue minuscule : jamais de largeur négative.
    assert_eq!(b.zone_utile(100.0), 50.0);
    assert!(b.largeur_effective(0.0) >= 0.0);
}

#[test]
fn sans_table_des_matieres_un_message_et_les_vignettes_restent_possibles() {
    let mut b = barre_ouverte(vec![]);
    assert!(!b.a_une_table_des_matieres());
    b.set_mode(ModeBarre::TableDesMatieres);
    assert_eq!(b.message_vide(), Some(MESSAGE_SANS_TABLE));
    assert_eq!(MESSAGE_SANS_TABLE, "No table of contents");
    assert!(b.toc().lignes().is_empty());

    // Les vignettes restent possibles.
    b.set_mode(ModeBarre::Vignettes);
    assert_eq!(b.message_vide(), None);
    assert!(b.vignettes_peintes().contains(&1));
}

// ───────────────────────── cas ajoutés ─────────────────────────

#[test]
fn virtualisation_seules_les_vignettes_proches_sont_peintes() {
    let b = barre_ouverte(table());
    let visibles = b.vignettes_visibles();
    assert!(
        visibles.len() < PAGES / 2,
        "pas les {PAGES} vignettes : {visibles:?}"
    );
    assert_eq!(visibles.first(), Some(&1));
    assert_eq!(b.vignettes_peintes(), visibles.as_slice());
}

#[test]
fn le_defilement_libere_les_vignettes_qui_sortent_et_peint_celles_qui_entrent() {
    let mut b = barre_ouverte(table());
    let avant = b.vignettes_visibles();
    let plan = b.defiler(3000.0);
    assert!(plan.a_liberer.contains(&1));
    assert!(!plan.a_peindre.is_empty());
    assert!(plan.a_peindre.iter().all(|p| !avant.contains(p)));
    // Un défilement sans effet ne demande rien.
    assert!(b.defiler(3000.0).est_vide());
}

#[test]
fn barre_fermee_ou_en_table_aucune_vignette_nest_peinte() {
    let mut b = barre_ouverte(table());
    let plan = b.set_mode(ModeBarre::TableDesMatieres);
    assert!(plan.a_peindre.is_empty());
    assert!(
        !plan.a_liberer.is_empty(),
        "le passage en table jette les canvas"
    );
    assert!(b.vignettes_peintes().is_empty());
    let plan = b.set_mode(ModeBarre::Vignettes);
    assert!(!plan.a_peindre.is_empty());
    let plan = b.set_ouverte(false);
    assert!(plan.a_peindre.is_empty());
    assert!(b.vignettes_peintes().is_empty());
}

#[test]
fn la_vignette_deja_visible_ne_fait_pas_defiler_la_barre() {
    let mut b = barre_ouverte(table());
    b.set_page_courante(1);
    assert_eq!(b.scroll_top(), 0.0);
    // Une page qui sort par le bas fait défiler juste assez, avec une marge.
    b.set_page_courante(4);
    assert!(b.scroll_top() > 0.0);
    let haut = b.scroll_top();
    b.set_page_courante(4);
    assert_eq!(b.scroll_top(), haut);
    // Revenir en haut remonte.
    b.set_page_courante(1);
    assert_eq!(b.scroll_top(), 0.0);
}

#[test]
fn la_largeur_est_bornee_et_arrondie() {
    let mut b = BarreLaterale::new();
    assert_eq!(b.largeur(), LARGEUR_DEFAUT);
    b.set_largeur(10.0);
    assert_eq!(b.largeur(), LARGEUR_MIN);
    b.set_largeur(5000.0);
    assert_eq!(b.largeur(), LARGEUR_MAX);
    b.set_largeur(250.6);
    assert_eq!(b.largeur(), 251.0);
}

#[test]
fn la_poignee_redimensionne_puis_repeint_toutes_les_vignettes_visibles() {
    let mut b = barre_ouverte(table());
    let avant: Vec<usize> = b.vignettes_peintes().to_vec();
    b.poignee_saisir(200.0);
    assert!(b.en_glissement());
    b.poignee_deplacer(260.0);
    assert_eq!(b.largeur(), LARGEUR_DEFAUT + 60.0);
    // La largeur suit le départ, pas le dernier déplacement : on revient en arrière.
    b.poignee_deplacer(150.0);
    assert_eq!(b.largeur(), LARGEUR_MIN.max(LARGEUR_DEFAUT - 50.0));
    b.poignee_deplacer(-1000.0);
    assert_eq!(b.largeur(), LARGEUR_MIN);

    let plan = b.poignee_relacher();
    assert!(!b.en_glissement());
    assert_eq!(plan.a_liberer, avant, "tout ce qui était peint est flou");
    assert_eq!(plan.a_peindre, b.vignettes_visibles());
    // Relâcher sans avoir saisi ne fait rien.
    assert_eq!(b.poignee_relacher(), PlanVignettes::default());
}

#[test]
fn deplacer_sans_saisir_la_poignee_ne_change_rien() {
    let mut b = barre_ouverte(table());
    b.poignee_deplacer(400.0);
    assert_eq!(b.largeur(), LARGEUR_DEFAUT);
}

#[test]
fn une_page_dun_autre_format_corrige_sa_vignette() {
    let mut b = barre_ouverte(table());
    let haut_avant = b.scroll_top();
    // La page 1 est en paysage : sa vignette raccourcit, les suivantes remontent.
    let nb = b.vignettes_visibles().len();
    b.format_page_connu(1, 420.0, 300.0);
    assert!(b.vignettes_visibles().len() > nb);
    assert_eq!(b.scroll_top(), haut_avant);
    // Une largeur nulle est ignorée.
    let plan = b.format_page_connu(2, 0.0, 300.0);
    assert!(plan.est_vide());
}

#[test]
fn le_chevron_ne_rend_pas_lentree_active_et_ignore_les_feuilles() {
    let mut b = barre_ouverte(table());
    b.set_mode(ModeBarre::TableDesMatieres);
    let un = chemin_de(&b, "Chapitre Un");
    b.basculer_entree(&un);
    assert!(b.toc().active().is_none());
    // Une feuille n'est pas repliable : basculer ne change rien.
    let intro = chemin_de(&b, "Introduction");
    b.basculer_entree(&intro);
    assert!(!b.toc().est_replie(&intro));
    assert_eq!(b.toc().lignes().len(), 5);
    // Un chemin inconnu ne plante pas.
    b.basculer_entree(&[9, 9]);
    assert_eq!(b.cliquer_entree(&[9]), None);
}

#[test]
fn une_entree_sans_page_est_cliquable_sans_destination() {
    let toc = vec![EntreeToc {
        titre: "Lien externe".into(),
        page: None,
        y: None,
        enfants: vec![],
    }];
    let mut b = barre_ouverte(toc);
    assert_eq!(b.cliquer_entree(&[0]), None);
    assert_eq!(b.toc().active(), Some(&vec![0]));
    assert_eq!(b.toc().lignes()[0].page, None);
}

#[test]
fn le_clic_transmet_le_haut_vise() {
    let toc = vec![EntreeToc {
        titre: "Annexe".into(),
        page: Some(30),
        y: Some(700.0),
        enfants: vec![],
    }];
    let mut b = barre_ouverte(toc);
    assert_eq!(
        b.cliquer_entree(&[0]),
        Some(Destination {
            page: 30,
            y: Some(700.0)
        })
    );
}

#[test]
fn reveler_avant_la_premiere_entree_ouvre_la_table_sans_rien_surligner() {
    let toc = vec![entree("Chapitre", 5, vec![])];
    let mut b = barre_ouverte(toc);
    b.set_ouverte(false);
    let (_, chemin) = b.reveler(2);
    assert_eq!(chemin, None);
    assert!(b.est_ouverte());
    assert_eq!(b.mode(), ModeBarre::TableDesMatieres);
    assert!(b.toc().active().is_none());
}

#[test]
fn recharger_un_document_vide_les_vignettes_et_la_table() {
    let mut b = barre_ouverte(table());
    b.set_mode(ModeBarre::TableDesMatieres);
    let un = chemin_de(&b, "Chapitre Un");
    b.cliquer_entree(&un);
    b.charger(3, RATIO, vec![entree("Seul", 1, vec![])]);
    assert_eq!(b.nombre_vignettes(), 3);
    assert_eq!(titres(&b), ["Seul"]);
    assert!(b.toc().active().is_none());
    b.vider();
    assert_eq!(b.nombre_vignettes(), 0);
    assert!(!b.a_une_table_des_matieres());
}

#[test]
fn letat_ephemere_se_restaure() {
    let mut b = BarreLaterale::new();
    b.restaurer(Some("outline"), Some(333.0));
    assert_eq!(b.etat().mode, ModeBarre::TableDesMatieres);
    assert_eq!(b.etat().largeur, 333.0);
    assert!(!b.etat().ouverte);
    // Valeurs invalides ignorées.
    b.restaurer(Some("nimporte"), Some(f64::NAN));
    assert_eq!(b.mode(), ModeBarre::TableDesMatieres);
    assert_eq!(b.largeur(), 333.0);
    assert_eq!(
        ModeBarre::depuis_texte("thumbnails"),
        Some(ModeBarre::Vignettes)
    );
    assert_eq!(ModeBarre::Vignettes.vers_texte(), "thumbnails");
    assert_eq!(ModeBarre::TableDesMatieres.vers_texte(), "outline");
}

#[test]
fn la_largeur_de_limage_suit_la_largeur_de_la_barre() {
    // 200 - bordure 1 - 2 x 7 (vue) - 2 x 4 (vignette) = 177.
    assert_eq!(BarreLaterale::largeur_image(200.0), 177.0);
    assert_eq!(BarreLaterale::largeur_image(0.0), 1.0);
}
