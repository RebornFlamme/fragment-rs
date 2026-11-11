// Port de app/e2e/pdf-toolbar.spec.ts (tests 1 à 6 et 7 pour le bouton de barre latérale),
// partie BARRE : la barre pilote le modèle de la vue (fragment_pdf::vue_modele) et construit ses
// menus avec `fragment_ui::menu`. Sans navigateur, sans fenêtre pour le modèle ; une fenêtre de
// test GPUI pour le rendu. Même nom que le test e2e, en snake_case ; les cas AJOUTÉS suivent.
// Données réécrites : 40 pages de 300 x 420 pt, zone de 800 x 600.

#![allow(clippy::expect_used, clippy::unwrap_used)] // fichier de tests

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::Keymap;
use fragment_pdf::pdf_zoom::{Taille, ZoomMode};
use fragment_pdf::vue_modele::{Point, VueModele};
use fragment_ui::menu::{self, Ligne, Menu};
use fragment_ui::pdf_barre::rendu::{SurBouton, rendre_barre};
use fragment_ui::pdf_barre::{BarrePdf, Bouton, LIBELLES_BOUTONS, ModeBarre};
use gpui_kit::TestAppContext;

const N: usize = 40;

/// Le document de la barre d'outils : `N` pages 300 x 420, zone 800 x 600, densité 1.
fn document() -> VueModele {
    let mut v = VueModele::new();
    v.definir_zone(800.0, 0.0, 600.0);
    v.ouvrir(
        N,
        Taille {
            largeur: 300.0,
            hauteur: 420.0,
        },
        16.0,
        16.0,
        1.0,
    );
    v
}

fn largeur_page(v: &VueModele) -> f64 {
    v.geometrie().largeur_px(0)
}

/// Les titres du menu et celui qui est coché.
fn titres(m: &Menu) -> (Vec<String>, Vec<String>) {
    let mut tous = Vec::new();
    let mut coches = Vec::new();
    for l in m.lignes() {
        if let Ligne::Item(i) = l {
            tous.push(i.title().to_string());
            if i.is_checked() {
                coches.push(i.title().to_string());
            }
        }
    }
    (tous, coches)
}

/// Ouvre un bouton de menu, clique l'entrée `titre`, rejoue la file : le geste d'un utilisateur.
fn choisir(barre: &mut BarrePdf, vue: &mut VueModele, bouton: Bouton, titre: &str) {
    barre.appui_sur(bouton);
    let (m, _) = barre.cliquer(bouton, vue);
    let m = m.expect("le menu s'ouvre");
    let idx = m
        .forme()
        .iter()
        .position(|t| t == titre)
        .unwrap_or_else(|| panic!("pas d'entrée « {titre} »"));
    m.show_at_position(10.0, 10.0, &Keymap::new());
    assert!(menu::cliquer(0, idx));
    barre.traiter(vue);
}

fn coche_du_menu_zoom(barre: &BarrePdf, vue: &VueModele) -> Vec<String> {
    titres(&barre.menu_zoom(vue)).1
}

// ───────────────────────── pdf-toolbar.spec.ts ─────────────────────────

#[test]
fn le_header_ne_porte_plus_le_zoom_la_barre_d_outils_est_sous_lui() {
    // Le header n'est pas à nous ; la barre, elle, porte les six contrôles dans l'ordre.
    let mut ordre: Vec<&str> = Bouton::TOUS.iter().map(|b| b.libelle()).collect();
    ordre.push("Page");
    assert_eq!(ordre, LIBELLES_BOUTONS);
    assert_eq!(
        LIBELLES_BOUTONS,
        [
            "Toggle sidebar",
            "Sidebar options",
            "Zoom out",
            "Zoom in",
            "Zoom options",
            "Page"
        ]
    );
    // Les icônes sont du registre.
    for b in Bouton::TOUS {
        assert!(
            fragment_ui::icones::svg(b.icone()).is_some(),
            "icône {} absente",
            b.icone()
        );
    }
    assert!(Bouton::OptionsBarre.est_menu() && Bouton::OptionsZoom.est_menu());
    assert!(!Bouton::BasculerBarre.est_menu());
}

#[test]
fn of_n_et_le_numero_de_page_suit_le_defilement() {
    let mut v = document();
    assert_eq!(v.total_texte(), "of 40");
    assert_eq!(v.champ().valeur, "1");
    let haut = v.geometrie().top_px(9);
    v.defiler(haut, 0.0);
    assert_eq!(v.champ().valeur, "10");
}

#[test]
fn saisir_un_numero_entree_y_va_borne_echap_restaure() {
    let barre = BarrePdf::new();
    let mut v = document();
    barre.champ_focus(&mut v);
    assert!(v.champ().tout_selectionne);
    for c in ["2", "5"] {
        barre.touche_champ(&mut v, c, Some(c));
    }
    assert_eq!(v.champ().valeur, "25", "la frappe remplace la sélection");
    barre.touche_champ(&mut v, "enter", None);
    assert_eq!(v.page_vue(), 25);
    assert_eq!(v.champ().valeur, "25");
    assert!((v.scroll_top() - v.geometrie().top_px(24)).abs() < 2.0);

    for c in ["9", "9", "9"] {
        barre.touche_champ(&mut v, c, Some(c));
    }
    // Le champ est resté sur « 25 » sans sélection : la frappe s'y ajoute.
    assert_eq!(v.champ().valeur, "25999");
    barre.touche_champ(&mut v, "enter", None);
    assert_eq!(v.page_vue(), N);
    assert_eq!(v.champ().valeur, "40");

    barre.touche_champ(&mut v, "backspace", None);
    barre.touche_champ(&mut v, "backspace", None);
    barre.touche_champ(&mut v, "3", Some("3"));
    assert_eq!(v.champ().valeur, "3");
    barre.touche_champ(&mut v, "escape", None);
    assert_eq!(v.champ().valeur, "40");
    assert!(!v.champ().focus);
}

#[test]
fn zoom_pm_change_la_taille_des_pages_les_modes_du_menu_aussi() {
    let mut barre = BarrePdf::new();
    let mut v = document();
    let avant = largeur_page(&v);
    barre.cliquer(Bouton::ZoomPlus, &mut v);
    assert!(largeur_page(&v) > avant + 5.0);
    barre.cliquer(Bouton::ZoomMoins, &mut v);
    barre.cliquer(Bouton::ZoomMoins, &mut v);
    assert!(largeur_page(&v) < avant - 5.0);

    choisir(&mut barre, &mut v, Bouton::OptionsZoom, "Page width");
    let reste = v.zone().largeur_utile - largeur_page(&v);
    assert!(reste > 30.0 && reste < 34.0, "reste = {reste}");

    choisir(&mut barre, &mut v, Bouton::OptionsZoom, "Actual size");
    assert!((largeur_page(&v) - 300.0).abs() < 0.5);

    choisir(&mut barre, &mut v, Bouton::OptionsZoom, "200%");
    assert!((largeur_page(&v) - 600.0).abs() < 0.5);
    assert_eq!(v.mode(), None);

    choisir(&mut barre, &mut v, Bouton::OptionsZoom, "Page fit");
    assert!(v.geometrie().hauteur_px(0) <= v.zone().hauteur);

    // Le mode coché dans le menu, c'est le mode en vigueur.
    assert_eq!(coche_du_menu_zoom(&barre, &v), ["Page fit"]);
    let (tous, _) = titres(&barre.menu_zoom(&v));
    assert_eq!(
        tous,
        [
            "Automatic",
            "Actual size",
            "Page fit",
            "Page width",
            "50%",
            "75%",
            "100%",
            "125%",
            "150%",
            "200%",
            "300%",
            "400%"
        ]
    );
    menu::fermer_menu();
}

#[test]
fn pinch_ctrl_molette_zoom_continu_ancre_sous_le_curseur_un_repeint_final() {
    let barre = BarrePdf::new();
    let mut v = document();
    v.choisir_pourcentage(3.0);
    v.champ_saisir("4");
    v.champ_valider();
    let cible = Point { x: 400.0, y: 250.0 };

    let largeur0 = largeur_page(&v);
    // Rafale de petits deltas : un pinch de trackpad.
    for _ in 0..12 {
        let r = barre.molette(&mut v, -8.0, 0, true, cible);
        assert!(r.empeche_defaut);
    }
    assert!(largeur_page(&v) > largeur0 * 1.05);
    assert!(v.en_geste());
    // Sans Ctrl, la molette n'est pas pour la barre : la vue défile.
    assert!(!barre.molette(&mut v, 100.0, 0, false, cible).empeche_defaut);

    // Fin du geste : un seul repeint, quand le silence est tenu.
    let echeance = v
        .prochaine_echeance()
        .expect("une échéance de fin de geste");
    v.avancer(echeance);
    assert!(!v.en_geste());

    // Ctrl + molette vers le bas : dézoome.
    let w = largeur_page(&v);
    barre.molette(&mut v, 300.0, 0, true, cible);
    assert!(largeur_page(&v) < w * 0.9);
}

#[test]
fn ctrl_plus_ctrl_moins_ctrl_0_zooment_le_pdf_pas_la_fenetre() {
    let mut barre = BarrePdf::new();
    let mut v = document();
    choisir(&mut barre, &mut v, Bouton::OptionsZoom, "Actual size");
    let w0 = largeur_page(&v);

    // Hors de la portée de la vue : rien n'est consommé, rien ne bouge.
    assert!(barre.touche(&mut v, "Equal", true, false).is_none());

    barre.definir_portee(true);
    assert!(barre.touche(&mut v, "Equal", true, false).is_some());
    assert!(largeur_page(&v) > w0 + 5.0);
    let w1 = largeur_page(&v);
    // « + » en AZERTY : Maj + « = ».
    assert!(barre.touche(&mut v, "Equal", true, true).is_some());
    assert!(largeur_page(&v) > w1 + 5.0);
    barre.touche(&mut v, "Minus", true, false);
    barre.touche(&mut v, "Minus", true, false);
    barre.touche(&mut v, "NumpadSubtract", true, false);
    assert!(largeur_page(&v) < w0 - 5.0);

    // Ctrl + 0 : retour au mode automatique, sur le code physique.
    assert!(barre.touche(&mut v, "Digit0", true, false).is_some());
    assert_eq!(coche_du_menu_zoom(&barre, &v), ["Automatic"]);

    // Sans Ctrl, ou une autre touche : pas pour nous (la fenêtre garde son zoom).
    assert!(barre.touche(&mut v, "Equal", false, false).is_none());
    assert!(barre.touche(&mut v, "KeyA", true, false).is_none());
    assert_eq!(v.mode(), Some(ZoomMode::Auto));
}

#[test]
fn page_width_suit_la_largeur_de_la_zone_barre_laterale_ouverte() {
    // Partie barre : le bouton de bascule est actif tant que la barre latérale est ouverte ;
    // la largeur de la zone, elle, est livrée au modèle par la vue montée.
    let mut barre = BarrePdf::new();
    let mut v = document();
    choisir(&mut barre, &mut v, Bouton::OptionsZoom, "Page width");
    let fermee = largeur_page(&v);
    assert!(!barre.bouton_actif(Bouton::BasculerBarre));
    barre.cliquer(Bouton::BasculerBarre, &mut v);
    assert!(barre.barre_ouverte());
    assert!(barre.bouton_actif(Bouton::BasculerBarre));
    v.zone_redimensionnee(550.0, 600.0);
    assert!(largeur_page(&v) < fermee - 50.0);

    barre.cliquer(Bouton::BasculerBarre, &mut v);
    assert!(!barre.barre_ouverte());
}

// ───────────────────────── Cas AJOUTÉS ─────────────────────────

#[test]
fn le_menu_de_la_barre_laterale_coche_son_contenu_et_ouvre_la_barre() {
    let mut barre = BarrePdf::new();
    let mut v = document();
    let (tous, coches) = titres(&barre.menu_barre());
    assert_eq!(
        tous,
        [
            "Thumbnails",
            "Table of contents",
            "Reveal page in table of contents"
        ]
    );
    assert_eq!(coches, ["Thumbnails"]);

    choisir(
        &mut barre,
        &mut v,
        Bouton::OptionsBarre,
        "Table of contents",
    );
    assert_eq!(barre.mode_barre(), ModeBarre::Plan);
    assert!(barre.barre_ouverte(), "choisir un contenu ouvre la barre");
    assert_eq!(titres(&barre.menu_barre()).1, ["Table of contents"]);
}

#[test]
fn reveler_la_page_ouvre_la_table_des_matieres_sur_la_page_courante() {
    let mut barre = BarrePdf::new();
    let mut v = document();
    v.aller_a_la_page(9, None);
    choisir(
        &mut barre,
        &mut v,
        Bouton::OptionsBarre,
        "Reveal page in table of contents",
    );
    assert_eq!(barre.mode_barre(), ModeBarre::Plan);
    assert!(barre.barre_ouverte());
    assert_eq!(barre.prendre_revele(), Some(9));
    assert_eq!(
        barre.prendre_revele(),
        None,
        "la demande ne sert qu'une fois"
    );
}

#[test]
fn recliquer_le_chevron_ferme_le_menu_au_lieu_de_le_rouvrir() {
    let mut barre = BarrePdf::new();
    let mut v = document();
    let (m, _) = barre.cliquer(Bouton::OptionsZoom, &mut v);
    let m = m.expect("premier clic : le menu s'ouvre");
    assert!(barre.bouton_actif(Bouton::OptionsZoom));
    m.show_at_position(0.0, 0.0, &Keymap::new());

    // L'appui ferme le menu (appui extérieur) puis le clic arrive : il ne doit rien rouvrir.
    barre.appui_sur(Bouton::OptionsZoom);
    menu::fermer_menu();
    assert!(
        !barre.bouton_actif(Bouton::OptionsZoom),
        "on_hide a rendu le bouton"
    );
    let (m, _) = barre.cliquer(Bouton::OptionsZoom, &mut v);
    assert!(m.is_none());

    // Le clic suivant rouvre.
    let (m, _) = barre.cliquer(Bouton::OptionsZoom, &mut v);
    assert!(m.is_some());
}

#[test]
fn perdre_le_focus_apres_une_saisie_vaut_validation_sans_saisie_rien() {
    let barre = BarrePdf::new();
    let mut v = document();
    barre.champ_focus(&mut v);
    barre.touche_champ(&mut v, "1", Some("1"));
    barre.touche_champ(&mut v, "2", Some("2"));
    barre.champ_perdre_focus(&mut v);
    assert_eq!(v.page_vue(), 12);
    assert!(!v.champ().focus);

    barre.champ_focus(&mut v);
    barre.champ_perdre_focus(&mut v);
    assert_eq!(v.page_vue(), 12, "rien de saisi, rien ne bouge");
    // Une saisie illisible n'y va pas et le champ retrouve la page courante.
    barre.champ_focus(&mut v);
    barre.touche_champ(&mut v, "x", Some("x"));
    barre.touche_champ(&mut v, "enter", None);
    assert_eq!(v.page_vue(), 12);
    assert_eq!(v.champ().valeur, "12");
}

#[test]
fn la_barre_se_dessine_et_un_appui_sur_un_bouton_est_rendu_a_la_vue() {
    let cx = TestAppContext::single();
    let barre = BarrePdf::new();
    let v = document();
    let recus: Rc<RefCell<Vec<Bouton>>> = Rc::default();
    let r = recus.clone();
    let sur: SurBouton = Rc::new(move |b, _x, _y, _cx| r.borrow_mut().push(b));
    cx.update(|cx| {
        // Un élément de plus par bouton : la construction ne doit pas échouer, ici sans fenêtre.
        let _ = rendre_barre(&barre, &v, cx, &sur);
    });
    assert!(recus.borrow().is_empty());
}
