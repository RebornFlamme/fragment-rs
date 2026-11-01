// Port de app/src/ui/core/editor/viewport.test.ts

use fragment_text::{Bande, ViewportBand, Vp, bande, bande_appropriee, biais};

// describe('viewport — le port de CodeMirror 6 (bande de pixels)')

const VISIBLE: Bande = Bande {
    top: 10_000.0,
    bottom: 10_800.0,
};

fn b(top: f64, bottom: f64) -> Bande {
    Bande { top, bottom }
}

#[test]
fn biais_le_sens_du_defilement_cm_measure() {
    assert_eq!(biais(40.0, 40.0), 40.0);
    assert_eq!(biais(10.0, 30.0), 30.0);
    assert_eq!(biais(-10.0, -30.0), -30.0);
    assert_eq!(biais(10.0, -30.0), 0.0);
    assert_eq!(biais(0.0, 0.0), 0.0);
}

#[test]
fn sans_biais_le_visible_plus_ou_moins_margin_sur_2_de_chaque_cote() {
    assert_eq!(
        bande(VISIBLE, 0.0),
        b(10_000.0 - Vp::MARGIN / 2.0, 10_800.0 + Vp::MARGIN / 2.0)
    );
}

#[test]
fn en_descendant_la_marge_part_en_bas_en_remontant_en_haut_bornee_a_toute_la_marge() {
    assert_eq!(bande(VISIBLE, 2000.0), b(10_000.0, 10_800.0 + Vp::MARGIN));
    assert_eq!(bande(VISIBLE, -2000.0), b(10_000.0 - Vp::MARGIN, 10_800.0));
    // Un petit biais penche à peine (40 px → 2 %).
    let penche = bande(VISIBLE, 40.0).bottom - 10_800.0;
    assert!((penche - Vp::MARGIN * 0.52).abs() < 0.005, "{penche}");
}

#[test]
fn hysteresis_un_petit_defilement_ne_change_rien_approcher_du_bord_recalcule() {
    let bd = bande(VISIBLE, 0.0);
    assert!(bande_appropriee(bd, b(10_100.0, 10_900.0), 0.0));
    assert!(bande_appropriee(bd, b(10_489.0, 11_289.0), 0.0));
    assert!(!bande_appropriee(bd, b(10_495.0, 11_295.0), 0.0)); // à 5 px du bord
    // Un biais agrandit la couverture exigée (jusqu'à MaxCoverMargin).
    assert!(bande_appropriee(bd, b(10_400.0, 11_200.0), 0.0));
    assert!(!bande_appropriee(bd, b(10_400.0, 11_200.0), 200.0));
}

#[test]
fn trop_grande_visible_bien_au_dela_de_2_x_margin_recalcul() {
    assert!(!bande_appropriee(b(0.0, 50_000.0), VISIBLE, 0.0));
}

#[test]
fn viewport_band_n_annonce_que_les_changements_de_bande() {
    let mut v = ViewportBand::new();
    assert!(v.measure(Some(VISIBLE))); // première mesure
    let bd = v.courante;
    let mut changements = 0;
    // 10 px par image pendant 40 images : la bande ne bouge qu'en approchant du bord.
    for i in 1..=40 {
        let d = 10.0 * f64::from(i);
        if v.measure(Some(b(10_000.0 + d, 10_800.0 + d))) {
            changements += 1;
        }
    }
    assert_eq!(changements, 0);
    assert_eq!(v.courante, bd);
    for i in 41..=60 {
        let d = 10.0 * f64::from(i);
        if v.measure(Some(b(10_000.0 + d, 10_800.0 + d))) {
            changements += 1;
        }
    }
    assert_eq!(changements, 1);
    // Hors vue : rien ne change.
    assert!(!v.measure(None));
    assert!(!v.measure(Some(b(5.0, 5.0))));
}
