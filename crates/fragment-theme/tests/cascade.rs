// Port de app/e2e/theme-plugin.spec.ts (test 2 : l'accent CHOISI l'emporte, ses dérivés suivent,
// vide = celui du thème) et de l'esprit de app/src/ui/index.css (alias, color-mix, calc).
//
//! La cascade des jetons : chaque forme d'expression, l'accent choisi et ses dérivés, l'accent
//! vide, un `theme.json` partiel, un jeton inconnu, et l'égalité bit à bit sans surcharge.

use fragment_theme::{Couleur, ErreurCascade, Graphe, Mode, Theme, Valeur, surcharges_depuis_json};

/// Résout `expression` dans un graphe qui contient aussi `base` (paires nom, CSS).
fn resoudre(base: &[(&str, &str)], expression: &str) -> Result<Valeur, ErreurCascade> {
    let mut g = Graphe::default();
    for (nom, css) in base {
        g.definir_css(nom, css)?;
    }
    g.definir_css("--cible", expression)?;
    g.resoudre("--cible")
}

fn couleur_de(base: &[(&str, &str)], expression: &str) -> Couleur {
    match resoudre(base, expression) {
        Ok(Valeur::Couleur(c)) => c,
        autre => panic!("« {expression} » devait donner une couleur : {autre:?}"),
    }
}

fn longueur_de(base: &[(&str, &str)], expression: &str) -> f32 {
    match resoudre(base, expression) {
        Ok(Valeur::Longueur(x) | Valeur::Nombre(x)) => x,
        autre => panic!("« {expression} » devait donner une longueur : {autre:?}"),
    }
}

// ── Égalité bit à bit ─────────────────────────────────────────────────────────

/// L'instantané figé de `Theme::sombre()/clair()` AVANT le passage au graphe : `{:#?}` écrit
/// chaque flottant avec le plus court texte qui le relit identique, donc l'égalité du texte est
/// l'égalité des bits.
fn fige(nom: &str) -> String {
    let chemin = format!("{}/tests/figes/{nom}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(chemin)
        .unwrap_or_default()
        .replace("\r\n", "\n")
}

#[test]
fn egalite_bit_a_bit_sans_surcharge() {
    for (mode, nom) in [(Mode::Sombre, "sombre"), (Mode::Clair, "clair")] {
        let attendu = fige(nom);
        assert!(attendu.lines().count() > 500, "instantané {nom} lisible");
        let theme = Theme::pour(mode);
        assert_eq!(
            format!("{theme:#?}\n"),
            attendu,
            "thème {nom} = instantané figé"
        );
        let (sans, avertissements) = Theme::avec_surcharges::<&str, &str>(mode, &[]);
        assert_eq!(sans, theme, "aucune surcharge = le thème de référence");
        assert!(avertissements.is_empty());
    }
}

#[test]
fn chaque_jeton_du_theme_est_dans_le_graphe() {
    for mode in [Mode::Sombre, Mode::Clair] {
        let graphe = Graphe::par_defaut(mode).expect("table lisible");
        let noms: Vec<&str> = graphe.noms().collect();
        for jeton in Theme::noms_des_jetons() {
            assert!(noms.contains(&jeton.as_str()), "{jeton} absent du graphe");
        }
        for nom in &noms {
            assert!(graphe.resoudre(nom).is_ok(), "{nom} ne se résout pas");
        }
    }
}

// ── Chaque forme d'expression ─────────────────────────────────────────────────

#[test]
fn litteraux_de_couleur() {
    assert_eq!(
        couleur_de(&[], "#abc"),
        Couleur::octets(0xaa, 0xbb, 0xcc, 255)
    );
    assert_eq!(
        couleur_de(&[], "#abcd"),
        Couleur::octets(0xaa, 0xbb, 0xcc, 0xdd)
    );
    assert_eq!(couleur_de(&[], "#646cff"), Couleur::hex(0x646cff));
    assert_eq!(
        couleur_de(&[], "#646cff80"),
        Couleur::octets(0x64, 0x6c, 0xff, 0x80)
    );
    assert_eq!(
        couleur_de(&[], "rgb(1, 2, 3)"),
        Couleur::octets(1, 2, 3, 255)
    );
    assert_eq!(
        couleur_de(&[], "rgba(0, 0, 0, 0.55)"),
        Couleur::css(0, 0, 0, 0.55)
    );
    assert_eq!(
        couleur_de(&[], "hsl(0, 100%, 50%)"),
        Couleur::octets(255, 0, 0, 255)
    );
}

#[test]
fn alias_var_et_repli() {
    let base = [("--a", "#102030"), ("--b", "var(--a)"), ("--c", "var(--b)")];
    assert_eq!(
        couleur_de(&base, "var(--c)"),
        Couleur::hex(0x102030),
        "alias en chaîne"
    );
    assert_eq!(
        couleur_de(&base, "var(--absent, #fff)"),
        Couleur::BLANC,
        "repli"
    );
    assert_eq!(
        resoudre(&base, "var(--absent)"),
        Err(ErreurCascade::Inconnu {
            nom: "--absent".to_owned()
        })
    );
}

#[test]
fn color_mix_pourcentages() {
    let base = [("--t", "#dfded9"), ("--acc", "#646cff")];
    // Mêmes appels que les palettes écrites à la main.
    let texte = Couleur::hex(0xdfded9);
    assert_eq!(
        couleur_de(&base, "color-mix(in srgb, var(--t) 9%, transparent)"),
        texte.melange(9.0, Couleur::TRANSPARENT)
    );
    assert_eq!(
        couleur_de(&base, "color-mix(in srgb, var(--acc), white 25%)"),
        Couleur::hex(0x646cff).melange(75.0, Couleur::BLANC),
        "pourcentage sur la seconde couleur"
    );
    assert_eq!(
        couleur_de(&base, "color-mix(in srgb, 30% white, var(--acc))"),
        Couleur::BLANC.melange(30.0, Couleur::hex(0x646cff)),
        "pourcentage AVANT la couleur"
    );
    assert_eq!(
        couleur_de(&base, "color-mix(in srgb, black, white)"),
        Couleur::NOIR.melange(50.0, Couleur::BLANC),
        "sans pourcentage : moitié-moitié"
    );
    assert_eq!(
        couleur_de(&base, "color-mix(in srgb, black 30%, white 30%)"),
        Couleur::NOIR.melange(50.0, Couleur::BLANC),
        "deux pourcentages ramenés à 100"
    );
}

#[test]
fn color_mix_imbrique_et_litteraux_internes() {
    let interieur = Couleur::BLANC.melange(50.0, Couleur::NOIR);
    let attendu = Couleur::css(255, 0, 0, 0.5).melange(40.0, interieur);
    assert_eq!(
        couleur_de(
            &[],
            "color-mix(in srgb, rgba(255, 0, 0, 0.5) 40%, color-mix(in srgb, white, black))"
        ),
        attendu
    );
}

#[test]
fn calc_en_pixels() {
    let base = [("--w", "46px"), ("--n", "3")];
    assert_eq!(longueur_de(&base, "calc(3 * var(--w))"), 138.0);
    assert_eq!(
        longueur_de(&base, "calc(var(--n) * var(--w))"),
        138.0,
        "nombre via var()"
    );
    assert_eq!(longueur_de(&base, "calc(var(--w) + 4px)"), 50.0);
    assert_eq!(longueur_de(&base, "calc(var(--w) - 6px)"), 40.0);
    assert_eq!(longueur_de(&base, "calc(var(--w) / 2)"), 23.0);
    assert_eq!(
        longueur_de(&base, "calc((1px + 2px) * 4)"),
        12.0,
        "parenthèses"
    );
    assert_eq!(
        longueur_de(&base, "calc(2px + 3px * 4)"),
        14.0,
        "priorité du produit"
    );
    assert!(
        matches!(
            resoudre(&base, "calc(var(--w) * var(--w))"),
            Err(ErreurCascade::Type { .. })
        ),
        "longueur × longueur refusé"
    );
    assert!(
        matches!(
            resoudre(&base, "calc(1px + 2)"),
            Err(ErreurCascade::Type { .. })
        ),
        "longueur + nombre refusé"
    );
    assert!(matches!(
        resoudre(&base, "calc(1px +)"),
        Err(ErreurCascade::Syntaxe { .. })
    ));
}

#[test]
fn autres_valeurs_litterales() {
    assert_eq!(resoudre(&[], "36px"), Ok(Valeur::Longueur(36.0)));
    assert_eq!(resoudre(&[], "1.5"), Ok(Valeur::Nombre(1.5)));
    assert_eq!(resoudre(&[], "120ms"), Ok(Valeur::Duree(120.0)));
    assert_eq!(
        resoudre(&[], "cubic-bezier(0.2, 0, 0, 1)"),
        Ok(Valeur::Bezier([0.2, 0.0, 0.0, 1.0]))
    );
    assert!(
        matches!(resoudre(&[], "0 8px 24px rgba(0, 0, 0, 0.55)"), Ok(Valeur::Ombres(o)) if o.len() == 1)
    );
    assert!(matches!(
        resoudre(&[], "'DM Sans', sans-serif"),
        Ok(Valeur::Texte(_))
    ));
}

#[test]
fn cycle_et_syntaxe_sont_des_erreurs_nommees() {
    let base = [("--a", "var(--b)"), ("--b", "var(--a)")];
    assert!(matches!(
        resoudre(&base, "var(--a)"),
        Err(ErreurCascade::Cycle { chemin }) if chemin.contains(&"--a".to_owned())
    ));
    assert!(matches!(
        resoudre(&[], "attr(x)"),
        Err(ErreurCascade::Syntaxe { .. })
    ));
    assert!(matches!(
        resoudre(&[], "color-mix(in oklab, red, blue)"),
        Err(ErreurCascade::Syntaxe { .. })
    ));
    assert!(matches!(
        resoudre(&[], ""),
        Err(ErreurCascade::Syntaxe { .. })
    ));
}

// ── Accent choisi et dérivés ──────────────────────────────────────────────────

#[test]
fn accent_choisi_recalcule_les_derives() {
    let (theme, avertissements) =
        Theme::avec_surcharges(Mode::Sombre, &[("--color-accent", "#00aa55")]);
    assert!(avertissements.is_empty());
    let p = &theme.palette;
    assert_eq!(p.color_accent, Couleur::hex(0x00aa55));
    assert_eq!(p.interactive_accent, p.color_accent, "alias");
    // Valeur du test e2e : accent + 25 % de blanc = (64, 191, 128).
    assert_eq!(p.text_accent, Couleur::octets(64, 191, 128, 255));
    assert_eq!(
        p.color_accent_1,
        Couleur::hex(0x00aa55).melange(70.0, Couleur::BLANC)
    );
    assert_eq!(
        p.color_accent_2,
        Couleur::hex(0x00aa55).melange(75.0, Couleur::NOIR)
    );
    assert_eq!(
        p.interactive_accent_hover,
        Couleur::hex(0x00aa55).melange(85.0, p.text_normal)
    );
    // Ce qui ne dérive pas de l'accent ne bouge pas.
    let base = Theme::sombre();
    assert_eq!(p.background_primary, base.palette.background_primary);
    assert_eq!(
        p.background_modifier_border,
        base.palette.background_modifier_border
    );
    assert_eq!(theme.forme, base.forme);
}

#[test]
fn accent_choisi_en_clair_text_accent_est_l_accent() {
    let (theme, _) = Theme::avec_surcharges(Mode::Clair, &[("--color-accent", "#e8559a")]);
    assert_eq!(
        theme.palette.text_accent,
        Couleur::hex(0xe8559a),
        "alias dans le thème clair"
    );
}

#[test]
fn accent_vide_ou_blancs_laisse_celui_du_theme() {
    for vide in ["", "   "] {
        let (theme, avertissements) =
            Theme::avec_surcharges(Mode::Sombre, &[("--color-accent", vide)]);
        assert_eq!(theme, Theme::sombre());
        assert!(avertissements.is_empty(), "vide : pas d'avertissement");
    }
}

#[test]
fn l_accent_choisi_pose_en_dernier_l_emporte_sur_le_theme_json() {
    let (theme, _) = Theme::avec_surcharges(
        Mode::Sombre,
        &[("--color-accent", "#e8559a"), ("--color-accent", "#00aa55")],
    );
    assert_eq!(theme.palette.color_accent, Couleur::hex(0x00aa55));
}

#[test]
fn le_nom_se_passe_avec_ou_sans_tirets() {
    let (a, _) = Theme::avec_surcharges(Mode::Sombre, &[("color-accent", "#00aa55")]);
    let (b, _) = Theme::avec_surcharges(Mode::Sombre, &[("--color-accent", "#00aa55")]);
    assert_eq!(a, b);
}

// ── theme.json ────────────────────────────────────────────────────────────────

#[test]
fn theme_json_partiel_et_derives_du_texte() {
    let json = r##"{
        "--background-primary": "#201020",
        "--text-normal": "rgb(255, 200, 220)",
        "--radius-m": "12px",
        "--frame-button-width": "50px"
    }"##;
    let (paires, avertissements) = surcharges_depuis_json(json);
    assert!(avertissements.is_empty());
    assert_eq!(paires.len(), 4);
    let (theme, avertissements) = Theme::avec_surcharges(Mode::Sombre, &paires);
    assert!(avertissements.is_empty());
    let base = Theme::sombre();
    let p = &theme.palette;
    assert_eq!(p.background_primary, Couleur::hex(0x201020));
    assert_eq!(p.workspace_bg, p.background_primary, "alias suit");
    assert_eq!(p.text_normal, Couleur::octets(255, 200, 220, 255));
    assert_eq!(
        p.background_modifier_border,
        Couleur::octets(255, 200, 220, 255).melange(9.0, Couleur::TRANSPARENT),
        "les gris dérivés du texte suivent le texte"
    );
    assert_eq!(theme.forme.radius_m, 12.0);
    assert_eq!(theme.forme.menu_radius, 12.0, "alias de radius-m");
    assert_eq!(theme.forme.frame_right_space, 150.0, "calc recalculé");
    // Le reste est celui de Fragment.
    assert_eq!(p.background_secondary, base.palette.background_secondary);
    assert_eq!(theme.typo, base.typo);
}

#[test]
fn les_opacites_suivent_la_surcharge_d_un_gris() {
    let (theme, _) = Theme::avec_surcharges(
        Mode::Sombre,
        &[(
            "--background-modifier-border",
            "color-mix(in srgb, var(--text-normal) 15%, transparent)",
        )],
    );
    assert_eq!(theme.palette.opacites.bordure, 15.0);
    assert_eq!(
        theme.palette.opacites.survol, 6.0,
        "les autres ne bougent pas"
    );
}

#[test]
fn theme_json_valeurs_non_textuelles_et_forme_generale() {
    let (paires, avertissements) =
        surcharges_depuis_json(r#"{"--icon-stroke": 2, "--color-accent": ["x"], "--a": null}"#);
    assert_eq!(paires, vec![("--icon-stroke".to_owned(), "2".to_owned())]);
    assert_eq!(avertissements.len(), 2);
    let (vide, av) = surcharges_depuis_json("[1, 2]");
    assert!(vide.is_empty() && av.len() == 1, "pas un objet");
    let (vide, av) = surcharges_depuis_json("{ pas du json");
    assert!(vide.is_empty() && av.len() == 1, "JSON illisible");
}

// ── Ce qui est refusé : avertissement, jamais d'erreur ────────────────────────

#[test]
fn jeton_inconnu_avertit_et_ne_change_rien() {
    let (theme, avertissements) = Theme::avec_surcharges(
        Mode::Sombre,
        &[
            ("--background-modifier-form-field", "#fff"),
            ("--n-importe-quoi", "1px"),
        ],
    );
    assert_eq!(theme, Theme::sombre());
    assert_eq!(avertissements.len(), 2);
    assert!(avertissements[0].contains("--background-modifier-form-field"));
}

#[test]
fn valeur_illisible_circulaire_ou_mal_typee_est_ignoree() {
    let (theme, avertissements) = Theme::avec_surcharges(
        Mode::Sombre,
        &[
            ("--color-accent", "pas-une-couleur"),
            ("--text-normal", "var(--text-muted)"), // valide
            ("--text-muted", "var(--text-normal)"), // ferait un cycle
            ("--radius-m", "#fff"),                 // une couleur dans une longueur
            ("--background-primary", "calc(1px +"), // syntaxe
        ],
    );
    let base = Theme::sombre();
    assert_eq!(avertissements.len(), 4, "{avertissements:?}");
    assert_eq!(theme.palette.color_accent, base.palette.color_accent);
    assert_eq!(
        theme.palette.text_normal, base.palette.text_muted,
        "la valide s'applique"
    );
    assert_eq!(
        theme.palette.text_muted, base.palette.text_muted,
        "le cycle est refusé"
    );
    assert_eq!(theme.forme.radius_m, 8.0);
    assert_eq!(
        theme.palette.background_primary,
        base.palette.background_primary
    );
}

#[test]
fn les_piles_de_polices_avertissent() {
    let (theme, avertissements) =
        Theme::avec_surcharges(Mode::Sombre, &[("--font-interface", "Arial, sans-serif")]);
    assert_eq!(theme, Theme::sombre());
    assert_eq!(avertissements.len(), 1);
}

#[test]
fn fond_de_page_pdf_blanc_dans_les_deux_themes() {
    // `--pdf-page-bg: #ffffff` (pdf-view.css) : le papier ne suit pas le thème.
    for mode in [Mode::Sombre, Mode::Clair] {
        assert_eq!(
            Theme::pour(mode).palette.pdf_page_bg,
            Couleur::hex(0xffffff)
        );
    }
}

#[test]
fn surlignages_recherche_pdf_communs_aux_deux_themes() {
    // `--pdf-find-bg` / `--pdf-find-bg-current` (pdf-view.css) : jaune 45 %, orange 70 %.
    for mode in [Mode::Sombre, Mode::Clair] {
        let p = Theme::pour(mode).palette;
        assert_eq!(p.pdf_find_bg, Couleur::css(255, 210, 0, 0.45));
        assert_eq!(p.pdf_find_bg_current, Couleur::css(255, 140, 0, 0.7));
    }
}
