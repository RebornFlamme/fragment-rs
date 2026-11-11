//! Tests du gabarit d'item d'arbre (tâche `fr-t2-tree-item`).
//!
//! Aucun test unitaire côté TS : ce sont des cas ajoutés, qui reprennent les valeurs des CSS de
//! Fragment (`docs/constantes-rendu.md`, `treeItem.css` et `explorer.css`) :
//! - ligne de 26 px (4 + 18 + 4 : 12 px sur un interligne de 1,5) ;
//! - pas de 17 px par niveau (marge 12 + guide 1 + rembourrage 4), titre à `24 + 17 × profondeur` ;
//! - triangle de 10 px, bord gauche exact `7,5 + 17 × profondeur` (arrondi à 8 au niveau 0), posé
//!   à 8 px du haut de la ligne ;
//! - couleurs des états lues dans les jetons de thème.
//!
//! La géométrie vient de la vraie mise en page (fenêtre de test GPUI sans GPU, bornes relevées par
//! `debug_selector`), les couleurs de fonctions pures.

// Fichier de test : les assistants hors `#[test]` déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use fragment_theme::{Aplat, Theme};
use fragment_ui::icones;
use fragment_ui::tree_item::geometrie::{
    PAS, arrondi_chromium, axe_guide_des_enfants, chevron_gauche, chevron_haut, hauteur_ligne,
    marge_gauche, retrait_titre,
};
use fragment_ui::tree_item::{Etat, Ligne, couleur_guide, couleurs_ligne, rendre_item};
use gpui_kit::{
    Bounds, Context, IntoElement, ParentElement as _, Pixels, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px,
};

// ─── La fenêtre de test : un panneau de largeur fixe contenant un arbre ───

struct Panneau {
    largeur: f32,
    titre_long: bool,
}

impl Render for Panneau {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let titre_c = if self.titre_long {
            "un titre de fichier beaucoup trop long pour tenir dans le panneau étroit"
        } else {
            "c"
        };
        // a (déplié) > b (déplié) > c (feuille) ; d (replié, racine) ; e (feuille, racine, active).
        let c = rendre_item(
            Ligne::new("c", titre_c).profondeur(2).sonde("c"),
            vec![],
            cx,
        );
        let b = rendre_item(
            Ligne::new("b", "b")
                .profondeur(1)
                .repliable(false)
                .sonde("b"),
            vec![c],
            cx,
        );
        let a = rendre_item(
            Ligne::new("a", "a").repliable(false).sonde("a"),
            vec![b],
            cx,
        );
        let d = rendre_item(Ligne::new("d", "d").repliable(true).sonde("d"), vec![], cx);
        let e = rendre_item(Ligne::new("e", "e").actif(true).sonde("e"), vec![], cx);
        div().w(px(self.largeur)).child(a).child(d).child(e)
    }
}

fn peindre(vcx: &mut VisualTestContext) {
    for _ in 0..2 {
        vcx.update(|w, cx| {
            w.draw(cx).clear(cx);
        });
    }
}

fn monter(cx: &mut TestAppContext, largeur: f32, titre_long: bool) -> &mut VisualTestContext {
    let (_racine, vcx) = cx.add_window_view(|_, _| Panneau {
        largeur,
        titre_long,
    });
    peindre(vcx);
    vcx
}

/// Les bornes `(x, y, largeur, hauteur)` d'une sonde.
fn bornes(vcx: &mut VisualTestContext, sonde: &'static str) -> (f32, f32, f32, f32) {
    let b: Bounds<Pixels> = vcx
        .debug_bounds(sonde)
        .unwrap_or_else(|| panic!("sonde {sonde} introuvable"));
    (
        f32::from(b.origin.x),
        f32::from(b.origin.y),
        f32::from(b.size.width),
        f32::from(b.size.height),
    )
}

// ─── Hauteur de ligne ───

#[test]
fn hauteur_de_ligne_26_px_et_empilement_sans_derive() {
    let t = Theme::sombre();
    assert_eq!(hauteur_ligne(t.forme.size_4_1, t.typo.font_ui_small), 26.0);

    let mut cx = TestAppContext::single();
    let vcx = monter(&mut cx, 260.0, false);
    // a, b, c, d, e s'empilent bout à bout : 5 lignes de 26, de 0 à 130.
    for (rang, nom) in [
        (0.0, "a/ligne"),
        (1.0, "b/ligne"),
        (2.0, "c/ligne"),
        (3.0, "d/ligne"),
        (4.0, "e/ligne"),
    ] {
        let (_, y, _, h) = bornes(vcx, nom);
        assert_eq!((y, h), (rang * 26.0, 26.0), "{nom}");
    }
}

// ─── Retrait par niveau ───

#[test]
fn retrait_par_niveau_17_px_et_boites_pleine_largeur() {
    assert_eq!(PAS, 17.0);
    assert_eq!(
        [0, 1, 2].map(retrait_titre),
        [24.0, 41.0, 58.0],
        "le rembourrage de la ligne"
    );
    assert_eq!([0, 1, 2].map(marge_gauche), [0.0, -17.0, -34.0]);

    let mut cx = TestAppContext::single();
    let vcx = monter(&mut cx, 260.0, false);
    // La boîte de chaque ligne part du bord du panneau et en remplit la largeur, à toute profondeur.
    for nom in ["a/ligne", "b/ligne", "c/ligne", "d/ligne"] {
        let (x, _, l, _) = bornes(vcx, nom);
        assert_eq!((x, l), (0.0, 260.0), "{nom}");
    }
    // Le titre est au bord du rembourrage : 24, 41 et 58.
    for (nom, x) in [("a/titre", 24.0), ("b/titre", 41.0), ("c/titre", 58.0)] {
        assert_eq!(bornes(vcx, nom).0, x, "{nom}");
    }
}

#[test]
fn conteneur_des_enfants_borde_du_guide_a_12_px_de_chaque_niveau() {
    let mut cx = TestAppContext::single();
    let vcx = monter(&mut cx, 260.0, false);
    // Les enfants de a : marge de 12 ; ceux de b : 12 + 17 + 12 = 29.
    let (x_a, y_a, l_a, h_a) = bornes(vcx, "a/enfants");
    assert_eq!((x_a, y_a, l_a), (12.0, 26.0, 248.0));
    // Il court exactement sur la hauteur de ses enfants (b, puis c) : 2 × 26.
    assert_eq!(h_a, 52.0);
    let (x_b, y_b, _, h_b) = bornes(vcx, "b/enfants");
    assert_eq!((x_b, y_b, h_b), (29.0, 52.0, 26.0));
    // Pas de conteneur (donc pas de guide) pour une feuille ou un dossier replié.
    assert!(vcx.debug_bounds("c/enfants").is_none());
    assert!(vcx.debug_bounds("d/enfants").is_none());
    // L'axe du guide : le centre du trait, ½ px après la marge.
    assert_eq!(axe_guide_des_enfants(0), 12.5);
    assert_eq!(axe_guide_des_enfants(1), 29.5);
}

// ─── Chevron et titre ───

#[test]
fn position_du_chevron_arrondie_depuis_sa_position_exacte() {
    // Exact : 7,5 + 17 × profondeur ; Chromium arrondit l'arête à l'entier le plus proche, la
    // demie vers le haut : 8, 25.
    assert_eq!(chevron_gauche(0), 7.5);
    assert_eq!(chevron_gauche(1), 24.5);
    assert_eq!(arrondi_chromium(chevron_gauche(0), 1.0), 8.0);
    assert_eq!(arrondi_chromium(chevron_gauche(1), 1.0), 25.0);
    let t = Theme::sombre();
    assert_eq!(chevron_haut(t.forme.size_4_1, t.typo.font_ui_small), 8.0);

    let mut cx = TestAppContext::single();
    let vcx = monter(&mut cx, 260.0, false);
    // a (profondeur 0, ligne en y = 0) : triangle de 10 × 10 en (8, 8).
    assert_eq!(bornes(vcx, "a/chevron"), (8.0, 8.0, 10.0, 10.0));
    // b (profondeur 1, ligne en y = 26) : en (25, 34).
    assert_eq!(bornes(vcx, "b/chevron"), (25.0, 34.0, 10.0, 10.0));
    // d (replié, ligne en y = 78) : même place, la pointe seule change.
    assert_eq!(bornes(vcx, "d/chevron"), (8.0, 86.0, 10.0, 10.0));
}

#[test]
fn le_chevron_n_occupe_aucune_place_le_titre_reste_au_bord_du_rembourrage() {
    let mut cx = TestAppContext::single();
    let vcx = monter(&mut cx, 260.0, false);
    // d a un chevron, e n'en a pas : leurs titres sont alignés (largeur extérieure nulle).
    assert_eq!(bornes(vcx, "d/titre").0, bornes(vcx, "e/titre").0);
    assert!(vcx.debug_bounds("e/chevron").is_none());
}

#[test]
fn un_titre_trop_long_est_tronque_au_rembourrage_de_fin() {
    let mut cx = TestAppContext::single();
    let vcx = monter(&mut cx, 120.0, true);
    // c : titre de 58 au bord droit moins le rembourrage de fin (6) : 114.
    let (x, _, l, _) = bornes(vcx, "c/titre");
    assert_eq!((x, x + l), (58.0, 114.0));
}

#[test]
fn les_deux_chevrons_sont_dans_le_jeu_d_icones() {
    for nom in ["right-triangle", "right-triangle-replie"] {
        let s = icones::svg(nom).expect("icône embarquée");
        assert!(s.contains("stroke-width=\"4\""), "{nom} : trait épaissi");
    }
    // Le replié est le triangle tourné de -90 degrés : sa pointe est à droite.
    assert!(
        icones::svg("right-triangle")
            .unwrap()
            .contains("M3 8L12 17L21 8")
    );
    assert!(
        icones::svg("right-triangle-replie")
            .unwrap()
            .contains("M8 21L17 12L8 3")
    );
}

// ─── Couleurs des états, par jetons ───

#[test]
fn couleurs_au_repos_texte_attenue_sans_fond() {
    for t in [Theme::sombre(), Theme::clair()] {
        let c = couleurs_ligne(
            &t.palette,
            Etat::default(),
            false,
            t.palette.background_secondary,
        );
        assert_eq!(c.texte, t.palette.text_muted);
        assert_eq!(c.fond, None);
    }
}

#[test]
fn couleurs_au_survol_texte_normal_et_fond_de_survol() {
    for t in [Theme::sombre(), Theme::clair()] {
        let panneau = t.palette.background_secondary;
        let c = couleurs_ligne(&t.palette, Etat::default(), true, panneau);
        assert_eq!(c.texte, t.palette.text_normal);
        assert_eq!(c.fond, Some(t.palette.aplat(Aplat::Survol, panneau)));
    }
}

#[test]
fn couleurs_de_l_actif_texte_normal_et_fond_actif() {
    for t in [Theme::sombre(), Theme::clair()] {
        let panneau = t.palette.background_secondary;
        let etat = Etat {
            actif: true,
            cible_depot: false,
        };
        let c = couleurs_ligne(&t.palette, etat, false, panneau);
        assert_eq!(c.texte, t.palette.text_normal);
        assert_eq!(c.fond, Some(t.palette.aplat(Aplat::SurvolActif, panneau)));
    }
}

#[test]
fn le_survol_l_emporte_sur_l_actif_comme_en_css() {
    let t = Theme::sombre();
    let panneau = t.palette.background_secondary;
    let etat = Etat {
        actif: true,
        cible_depot: false,
    };
    let c = couleurs_ligne(&t.palette, etat, true, panneau);
    assert_eq!(c.fond, Some(t.palette.aplat(Aplat::Survol, panneau)));
}

#[test]
fn la_cible_de_depot_colore_la_ligne_de_l_accent_a_32_pour_cent() {
    let t = Theme::sombre();
    let panneau = t.palette.background_secondary;
    let etat = Etat {
        actif: false,
        cible_depot: true,
    };
    let repos = couleurs_ligne(&t.palette, etat, false, panneau);
    let survol = couleurs_ligne(&t.palette, etat, true, panneau);
    assert!(repos.fond.is_some());
    assert_eq!(
        repos.fond, survol.fond,
        "le dépôt reste visible sous le pointeur"
    );
    assert_ne!(repos.fond, Some(t.palette.aplat(Aplat::Survol, panneau)));
}

#[test]
fn le_guide_prend_la_bordure_de_survol_sur_le_panneau() {
    for t in [Theme::sombre(), Theme::clair()] {
        let panneau = t.palette.background_secondary;
        let g = couleur_guide(&t.palette, panneau);
        assert_ne!(g, panneau);
        assert_eq!(g.a, 255, "un aplat opaque, composé comme Chromium");
    }
}
