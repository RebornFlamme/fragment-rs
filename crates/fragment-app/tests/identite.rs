//! L'identité visuelle : l'icône (fichiers de marque, ressource de l'exécutable) et les polices
//! sans repli système (interface DM Sans embarquée, pile du code résolue).

use fragment_app::identite::{ICONE_ICO, ICONE_PNG, pile_resolue, replis};
use fragment_ui::polices::{
    FAMILLE_INTERFACE, GRAISSES_INTERFACE, enregistrer, se_resout_sans_repli,
};
use gpui_kit::TextSystem;
use gpui_kit::platform::current_platform;

/// Un `TextSystem` réel (DirectWrite sous Windows), sans fenêtre, polices embarquées enregistrées.
/// ★ `current_platform(true)` (sans tête) donne un système de texte VIDE : il faut `false`.
fn systeme() -> TextSystem {
    let systeme = TextSystem::new(current_platform(false).text_system());
    enregistrer(&systeme).unwrap_or_else(|e| panic!("polices non enregistrées : {e}"));
    systeme
}

/// Les entrées de l'en-tête ICO : (largeur, hauteur, taille des données, décalage). 0 = 256 px.
fn entrees_ico(ico: &[u8]) -> Vec<(u32, u32, u32, u32)> {
    let u16_a = |i: usize| u32::from(u16::from_le_bytes([ico[i], ico[i + 1]]));
    let u32_a = |i: usize| u32::from_le_bytes([ico[i], ico[i + 1], ico[i + 2], ico[i + 3]]);
    assert_eq!(u16_a(0), 0, "réservé");
    assert_eq!(u16_a(2), 1, "type : icône");
    (0..u16_a(4) as usize)
        .map(|n| {
            let e = 6 + 16 * n;
            let dim = |o: u8| match ico[e + usize::from(o)] {
                0 => 256,
                d => u32::from(d),
            };
            (dim(0), dim(1), u32_a(e + 8), u32_a(e + 12))
        })
        .collect()
}

#[test]
fn l_icone_de_l_exe_porte_les_tailles_de_la_marque() {
    let tailles: Vec<u32> = entrees_ico(ICONE_ICO).iter().map(|e| e.0).collect();
    for attendue in [16, 24, 32, 48, 64, 128, 256] {
        assert!(
            tailles.contains(&attendue),
            "taille absente : {attendue} ({tailles:?})"
        );
    }
}

#[test]
fn l_icone_png_est_carree() {
    let image = image::load_from_memory(ICONE_PNG).expect("PNG lisible");
    assert_eq!(image.width(), image.height());
    assert!(
        image.width() >= 256,
        "icône trop petite : {}",
        image.width()
    );
}

#[test]
fn la_ressource_windows_designe_l_icone_d_identifiant_1() {
    // ★ GPUI charge la ressource d'identifiant 1 : tout autre numéro laisse la fenêtre sans icône.
    let rc = include_str!("../assets/identite.rc");
    assert!(
        rc.lines()
            .any(|l| l.trim() == r#"1 ICON "brand/app-icon.ico""#),
        "ligne d'icône absente de identite.rc"
    );
}

#[test]
fn la_notice_reserve_les_droits_de_la_marque() {
    let notice = include_str!("../assets/brand/NOTICE");
    assert!(notice.contains("tous droits réservés, exclues de la licence MIT"));
}

/// L'exécutable contient bien les octets de l'icône (la ressource a été compilée et liée).
#[cfg(windows)]
#[test]
fn l_exe_embarque_l_icone() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_fragment")).expect("exe lisible");
    let entrees = entrees_ico(ICONE_ICO);
    let (_, _, taille, decalage) = *entrees.iter().max_by_key(|e| e.0).expect("une image");
    let echantillon =
        &ICONE_ICO[decalage as usize + 64..decalage as usize + 128.min(taille as usize)];
    assert!(
        exe.windows(echantillon.len()).any(|f| f == echantillon),
        "les octets de l'icône sont absents de l'exécutable"
    );
}

#[test]
fn dm_sans_se_resout_sans_repli_aux_trois_graisses() {
    let systeme = systeme();
    for graisse in GRAISSES_INTERFACE {
        assert!(
            se_resout_sans_repli(&systeme, FAMILLE_INTERFACE, graisse),
            "repli système à la graisse {graisse:?}"
        );
    }
}

#[test]
fn aucune_police_de_l_interface_ni_du_code_ne_tombe_sur_un_repli() {
    let systeme = systeme();
    let defauts = replis(&systeme);
    assert!(defauts.is_empty(), "replis : {defauts:#?}");
}

#[test]
fn la_pile_du_code_se_resout_sur_une_police_installee() {
    let systeme = systeme();
    let pile = fragment_theme::theme::Theme::sombre().typo.font_monospace;
    let famille = pile_resolue(&systeme, pile).expect("une famille de la pile existe");
    // Comme Chromium sur la machine de référence : JetBrains Mono n'est pas embarquée (ADR-005 §5),
    // la pile se termine sur `monospace`, qui donne Consolas sous Windows.
    #[cfg(windows)]
    assert_eq!(famille, "Consolas");
    assert!(!["ui-monospace", "monospace"].contains(&famille));
}

#[test]
fn une_pile_sans_famille_connue_est_un_repli() {
    // Contre-épreuve : le test sait dire « repli » (aucun nom générique à la fin de la pile).
    let systeme = systeme();
    assert_eq!(pile_resolue(&systeme, &["Famille Qui N'Existe Pas"]), None);
}

#[cfg(windows)]
#[test]
fn les_noms_generiques_se_resolvent_comme_chromium() {
    let systeme = systeme();
    assert_eq!(
        pile_resolue(&systeme, &["Inconnue", "monospace"]),
        Some("Consolas")
    );
    // `ui-monospace` n'est pas géré par Chromium : on passe à la suivante.
    assert_eq!(
        pile_resolue(&systeme, &["ui-monospace", "monospace"]),
        Some("Consolas")
    );
}
