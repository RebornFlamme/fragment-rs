// Port des cas d'app/e2e/editor-hell.spec.ts (listes, titres, inline, maths, note ouverte, lecture
// seule) et de la règle `revealOnFocusOnly` de decorationPlugin.ts, au niveau du MODÈLE.
//! Tests de `fragment_markdown::decorations` (fr-t3-decorations).
//!
//! Une spec e2e lit le `innerText` d'une ligne ; ici, [`Decorations::texte_visible`] en est
//! l'équivalent : la source moins les masques, chaque widget remplacé par un glyphe témoin (`☐`, `☑`,
//! `•`, `⟨tex⟩`, le texte d'un lien interne). Les documents sont réécrits.

use fragment_markdown::decorations::{
    self, Contexte, Curseur, Decorations, GenreLigne, GenreMarque, GenreWidget, Largeur,
    Revelation, WidgetEnLigne, styles,
};
use fragment_markdown::syntaxe::modele_rangees::lignes_de;
use fragment_markdown::syntaxe::{self, Etendue};

// ─── Outils ──────────────────────────────────────────────────────────────────

/// Les décorations de `texte` avec le curseur en `pos` (UTF-16), éditeur hôte (`Toujours`).
fn deco(texte: &str, pos: usize) -> Decorations {
    deco_avec(
        texte,
        Etendue::new(pos, pos),
        true,
        true,
        &Contexte::default(),
    )
}

fn deco_avec(
    texte: &str,
    sel: Etendue,
    focus: bool,
    a_eu_le_focus: bool,
    ctx: &Contexte,
) -> Decorations {
    let arbre = syntaxe::analyser(texte);
    let selections = [sel];
    let curseur = Curseur {
        selections: &selections,
        focus,
        a_eu_le_focus,
    };
    decorations::calculer(texte, &arbre, &curseur, ctx)
}

fn temoin(w: &WidgetEnLigne) -> String {
    match &w.genre {
        GenreWidget::Case { cochee: true, .. } => "☑".into(),
        GenreWidget::Case { cochee: false, .. } => "☐".into(),
        GenreWidget::Puce { .. } => "•".into(),
        GenreWidget::Indentation | GenreWidget::BadgeLangue { .. } => String::new(),
        GenreWidget::Maths { tex, .. } => format!("⟨{tex}⟩"),
        GenreWidget::LienInterne { texte, .. } => texte.clone(),
        GenreWidget::Image { chemin } => format!("🖼{chemin}"),
        GenreWidget::Separateur => "—".into(),
    }
}

/// Ce que l'utilisateur voit sur la ligne `n` (le `innerText` de la `.cm-line`).
fn visible(texte: &str, d: &Decorations, n: usize) -> String {
    let l = lignes_de(texte)[n].octets;
    d.texte_visible(texte, l.from, l.to, temoin)
}

fn widgets_de(d: &Decorations, f: impl Fn(&GenreWidget) -> bool) -> Vec<&WidgetEnLigne> {
    d.widgets.iter().filter(|w| f(&w.genre)).collect()
}

fn cases(d: &Decorations) -> Vec<&WidgetEnLigne> {
    widgets_de(d, |g| matches!(g, GenreWidget::Case { .. }))
}

fn puces(d: &Decorations) -> Vec<&WidgetEnLigne> {
    widgets_de(d, |g| matches!(g, GenreWidget::Puce { .. }))
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

// ─── editor-hell : listes et tâches ──────────────────────────────────────────

/// « `- [ ]` en fin de ligne (sans texte) se rend en case, pas en `[ ]` brut. »
#[test]
fn tache_sans_texte_en_fin_de_ligne_se_rend_en_case() {
    let t = "- [ ]\n- [x]\n\nsuite";
    let d = deco(t, 15);
    let c = cases(&d);
    assert_eq!(c.len(), 2);
    assert!(matches!(c[1].genre, GenreWidget::Case { cochee: true, .. }));
    assert_eq!(visible(t, &d, 0), "☐");
    assert_eq!(visible(t, &d, 1), "☑");
    assert!(!(0..2).any(|n| visible(t, &d, n).contains("[ ]")));
}

/// « Cocher / décocher au clic, sans déplacer le texte » : la case porte la position de son `[`,
/// le caractère à remplacer est juste derrière ; le texte qui suit ne bouge pas.
#[test]
fn cocher_decocher_au_clic_sans_deplacer_le_texte() {
    let t = "- [ ] tâche\nfin";
    let d = deco(t, utf16(t));
    let c = cases(&d);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].source.utf16, Etendue::new(2, 5));
    assert!(matches!(
        c[0].genre,
        GenreWidget::Case {
            cochee: false,
            marqueur: 2
        }
    ));
    assert_eq!(visible(t, &d, 0), "☐ tâche");
    // Coché : même géométrie (la case a une largeur fixe, le texte garde sa place).
    let t2 = "- [x] tâche\nfin";
    let d2 = deco(t2, utf16(t2));
    assert_eq!(cases(&d2)[0].source, c[0].source);
    assert_eq!(cases(&d2)[0].largeur, c[0].largeur);
    assert_eq!(visible(t2, &d2, 0), "☑ tâche");
}

/// « Alignement : sous-puce bien décalée, case alignée sur les puces, texte aligné. »
#[test]
fn alignement_sous_puce_case_et_texte() {
    let t = "- parent\n  - enfant\n- [ ] tache\n\nsuite";
    let d = deco(t, utf16(t));
    let p = puces(&d);
    assert_eq!(p.len(), 2);
    // La sous-puce est décalée d'UN vrai niveau : un espaceur d'indentation de la largeur d'une
    // cellule (1,6 em, soit 25,6 px à 16 px : plus que les 14 px de la spec).
    let barres = widgets_de(&d, |g| matches!(g, GenreWidget::Indentation));
    assert_eq!(barres.len(), 1);
    assert_eq!(barres[0].source.utf16, Etendue::new(9, 11));
    assert_eq!(
        barres[0].largeur,
        Largeur::Em(styles::LARGEUR_INDENTATION_EM)
    );
    const { assert!(styles::LARGEUR_INDENTATION_EM * styles::TAILLE_TEXTE_DEFAUT >= 14.0) };
    assert!(
        d.lignes
            .iter()
            .any(|l| l.ligne == 1 && l.genre == GenreLigne::Indentation { niveaux: 1 })
    );
    // La case a la même cellule que les puces : centres alignés.
    let c = cases(&d);
    assert_eq!(c[0].largeur, p[0].largeur);
    // Le texte suit la même cellule et la même espace : « parent » et « tache » alignés.
    assert_eq!(visible(t, &d, 0), "• parent");
    assert_eq!(visible(t, &d, 1), "• enfant");
    assert_eq!(visible(t, &d, 2), "☐ tache");
}

/// La case se révèle tant que le curseur est entre le début de la ligne et le `]` ; au-delà, rendue.
#[test]
fn tache_zone_active_jusqu_au_crochet() {
    let t = "- [ ] faire\n";
    assert_eq!(visible(t, &deco(t, 4), 0), "- [ ] faire");
    assert_eq!(visible(t, &deco(t, 5), 0), "- [ ] faire");
    assert_eq!(visible(t, &deco(t, 6), 0), "☐ faire");
}

/// Une liste numérotée garde son numéro ; une puce sous le curseur reste en source.
#[test]
fn puce_sous_le_curseur_et_liste_numerotee() {
    let t = "- a\n1. b\n";
    // La puce ne se révèle que curseur SUR la marque (pas ailleurs dans la ligne).
    assert_eq!(visible(t, &deco(t, 3), 0), "• a");
    let d = deco(t, 1);
    assert_eq!(visible(t, &d, 0), "- a");
    assert_eq!(visible(t, &d, 1), "1. b");
    let d = deco(t, utf16(t));
    assert_eq!(visible(t, &d, 0), "• a");
    assert!(matches!(
        puces(&d)[0].genre,
        GenreWidget::Puce { debut_ligne: 0 }
    ));
}

/// Les barres d'indentation hors liste comptent les colonnes (tabulation au taquet), jamais dans du
/// code ; les blancs sont répartis entre les espaceurs.
#[test]
fn barres_d_indentation_colonnes_et_code() {
    let ctx = Contexte {
        unite_indentation: 2,
        ..Default::default()
    };
    let t = "a\n    b\n\t\tc\n\n```\n    x\n```\n";
    let d = deco_avec(t, Etendue::new(0, 0), true, true, &ctx);
    let niveaux: Vec<(usize, usize)> = d
        .lignes
        .iter()
        .filter_map(|l| match l.genre {
            GenreLigne::Indentation { niveaux } => Some((l.ligne, niveaux)),
            _ => None,
        })
        .collect();
    // « b » : 4 colonnes / 2 = 2 niveaux ; « c » : 2 tabulations = 4 colonnes / 2 = 2, mais jamais
    // plus de niveaux que de blancs (2). Le code n'en a pas.
    assert_eq!(niveaux, [(1, 2), (2, 2)]);
    let b: Vec<Etendue> = widgets_de(&d, |g| matches!(g, GenreWidget::Indentation))
        .iter()
        .map(|w| w.source.utf16)
        .collect();
    assert_eq!(
        b,
        [
            Etendue::new(2, 4),
            Etendue::new(4, 6),
            Etendue::new(8, 9),
            Etendue::new(9, 10)
        ]
    );
}

// ─── editor-hell : note ouverte ──────────────────────────────────────────────

/// « Note ouverte : RIEN n'est révélé avant que l'utilisateur ait mis son curseur dedans (la 1re
/// ligne est rendue) ; puis comme avant. »
#[test]
fn note_ouverte_rien_revele_avant_le_premier_focus() {
    let ctx = Contexte {
        revelation: Revelation::ApresPremierFocus,
        ..Default::default()
    };
    let t = "# Un titre en **gras**\n\nun autre paragraphe";
    let vue = |pos: usize, focus: bool, a_eu: bool| {
        let d = deco_avec(t, Etendue::new(pos, pos), focus, a_eu, &ctx);
        visible(t, &d, 0)
    };
    // Aucun clic : la sélection par défaut (0) est sur la 1re ligne, qui reste RENDUE.
    assert_eq!(vue(0, false, false), "Un titre en gras");
    // Un clic dans le 2e paragraphe : la 1re ligne reste rendue.
    assert_eq!(vue(30, true, true), "Un titre en gras");
    // Un clic dans la 1re ligne (au bout) : sa source se révèle.
    let fin = utf16("# Un titre en **gras**");
    assert_eq!(vue(fin, true, true), "# Un titre en **gras**");
    // Le focus part : la ligne du curseur reste en source.
    assert_eq!(vue(fin, false, true), "# Un titre en **gras**");
    // Un autre document : la règle repart de zéro.
    let t2 = "## Encore un **titre**\n\nla suite";
    let d = deco_avec(t2, Etendue::new(0, 0), false, false, &ctx);
    assert_eq!(visible(t2, &d, 0), "Encore un titre");
}

/// `revealOnFocusOnly` : une cellule de tableau garde sa sélection (0) sans focus ; une formule en
/// tête de cellule n'est révélée que dans la cellule focalisée.
#[test]
fn cellule_revelee_seulement_au_focus() {
    let ctx = Contexte {
        revelation: Revelation::FocusSeulement,
        ..Default::default()
    };
    let t = "$k^2$ en tête";
    let sans = deco_avec(t, Etendue::new(0, 0), false, true, &ctx);
    assert_eq!(visible(t, &sans, 0), "⟨k^2⟩ en tête");
    let avec = deco_avec(t, Etendue::new(0, 0), true, true, &ctx);
    assert_eq!(visible(t, &avec, 0), "$k^2$ en tête");
}

#[test]
fn curseur_actif_selon_la_regle() {
    use decorations::curseur_actif;
    assert!(curseur_actif(Revelation::Toujours, false, false));
    assert!(!curseur_actif(Revelation::ApresPremierFocus, true, false));
    assert!(curseur_actif(Revelation::ApresPremierFocus, false, true));
    assert!(!curseur_actif(Revelation::FocusSeulement, false, true));
    assert!(curseur_actif(Revelation::FocusSeulement, true, false));
}

/// `touches` : bornes comprises, un ET des deux comparaisons.
#[test]
fn touches_bornes_comprises() {
    use decorations::touches;
    assert!(touches(Etendue::new(5, 5), 5, 9));
    assert!(touches(Etendue::new(9, 9), 5, 9));
    assert!(!touches(Etendue::new(10, 10), 5, 9));
    assert!(!touches(Etendue::new(4, 4), 5, 9));
    assert!(touches(Etendue::new(0, 20), 5, 9));
}

// ─── editor-hell : titres et inline ──────────────────────────────────────────

/// « Titres 1 à 6 : marques cachées hors curseur, révélées au curseur » ; tailles décroissantes.
#[test]
fn titres_un_a_six_marques_cachees_hors_curseur() {
    let t = "# T1\n## T2\n### T3\n#### T4\n##### T5\n###### T6\nfin";
    let d = deco(t, utf16(t));
    for k in 1..=6u8 {
        let n = usize::from(k) - 1;
        assert!(
            d.lignes
                .iter()
                .any(|l| l.ligne == n && l.genre == GenreLigne::Titre(k))
        );
        assert_eq!(visible(t, &d, n), format!("T{k}"));
    }
    // Le curseur dans le titre 3 : sa source se révèle, pas celle des autres.
    let d3 = deco(t, 14);
    assert_eq!(visible(t, &d3, 2), "### T3");
    assert_eq!(visible(t, &d3, 1), "T2");
    // Tailles (editor.css) et hauteurs de ligne (interligne 1,25), décroissantes.
    let tailles: Vec<f32> = d.rangees.polices.iter().map(|p| p.taille).collect();
    assert_eq!(tailles, [30.4, 25.6, 21.6, 18.88, 16.8, 16.0]);
    assert!(tailles.windows(2).all(|w| w[0] >= w[1]));
    let hauteurs: Vec<(usize, f32)> = d
        .rangees
        .hauteurs
        .iter()
        .map(|h| (h.ligne, h.hauteur))
        .collect();
    assert_eq!(hauteurs[0], (0, 30.4 * 1.25));
    assert_eq!(hauteurs[5], (5, 20.0));
    // Le H6 est à 80 %.
    assert_eq!(GenreLigne::Titre(6).style().opacite, Some(0.8));
    assert_eq!(GenreLigne::Titre(1).style().opacite, None);
}

/// La taille du texte de l'éditeur (Appearance) entraîne celle des titres.
#[test]
fn titre_suit_la_taille_du_texte() {
    let ctx = Contexte {
        taille_texte: 20.0,
        ..Default::default()
    };
    let d = deco_avec("## Deux\n", Etendue::new(8, 8), true, true, &ctx);
    assert_eq!(d.rangees.polices[0].taille, 32.0);
    assert_eq!(d.rangees.hauteurs[0].hauteur, 40.0);
}

/// Un `#` seul sur sa ligne reste visible (sinon la ligne paraîtrait vide).
#[test]
fn marque_de_titre_orpheline_reste_visible() {
    let t = "#\n> \nfin";
    let d = deco(t, utf16(t));
    assert_eq!(visible(t, &d, 0), "#");
}

/// Un cas `inline_*` de la spec : rendu hors curseur, source au curseur, rendu à nouveau.
fn inline(src: &str, rendu: &str, marque: Option<GenreMarque>) {
    let t = format!("avant\nx {src} y\naprès");
    let dehors = deco(&t, 0);
    assert_eq!(visible(&t, &dehors, 1), format!("x {rendu} y"), "{src}");
    let dedans = deco(&t, "avant\n".len() + 4);
    assert_eq!(visible(&t, &dedans, 1), format!("x {src} y"), "{src}");
    if let Some(m) = marque {
        // Le style vaut dans les deux cas, sur toute la plage (marques comprises).
        for d in [&dehors, &dedans] {
            let p = d
                .marques
                .iter()
                .find(|x| x.genre == m)
                .map(|x| x.plage.utf16);
            assert_eq!(p, Some(Etendue::new(8, 8 + utf16(src))), "{src}");
        }
    }
}

#[test]
fn inline_gras() {
    inline("**gras**", "gras", Some(GenreMarque::Gras));
}

#[test]
fn inline_italique() {
    inline("*italique*", "italique", Some(GenreMarque::Italique));
}

#[test]
fn inline_barre() {
    inline("~~barré~~", "barré", Some(GenreMarque::Barre));
}

#[test]
fn inline_surligne() {
    inline("==surligné==", "surligné", Some(GenreMarque::Surligne));
}

#[test]
fn inline_code() {
    inline("`code`", "code", Some(GenreMarque::Code));
}

#[test]
fn inline_wikilien() {
    inline("[[cible]]", "cible", None);
}

#[test]
fn inline_lien() {
    inline(
        "[lien](https://exemple.org)",
        "lien",
        Some(GenreMarque::Lien),
    );
}

/// L'alias d'un lien interne s'affiche ; sinon le chemin et son sous-chemin.
#[test]
fn lien_interne_alias_et_sous_chemin() {
    let t = "[[Note#Partie|vu]] et [[Note#Partie]]\n";
    let d = deco(t, utf16(t));
    assert_eq!(visible(t, &d, 0), "vu et Note#Partie");
    let liens = widgets_de(&d, |g| matches!(g, GenreWidget::LienInterne { .. }));
    assert!(
        matches!(&liens[0].genre, GenreWidget::LienInterne { interieur, .. } if interieur == "Note#Partie|vu")
    );
}

/// L'URL d'un lien automatique n'est jamais masquée (le lien disparaîtrait).
#[test]
fn url_d_un_lien_automatique_reste_visible() {
    let t = "voir <https://exemple.org> ici\n";
    let d = deco(t, utf16(t));
    assert!(visible(t, &d, 0).contains("https://exemple.org"));
}

/// « Math inline `$…$` et `$$…$$` dans la phrase : rendus hors curseur. »
#[test]
fn maths_en_ligne_rendues_hors_curseur() {
    let t = "avant\na $x^2$ b $$y$$ c\naprès";
    let d = deco(t, 0);
    let m = widgets_de(&d, |g| matches!(g, GenreWidget::Maths { .. }));
    assert_eq!(m.len(), 2);
    assert_eq!(
        widgets_de(&d, |g| matches!(
            g,
            GenreWidget::Maths {
                affichage: true,
                ..
            }
        ))
        .len(),
        1
    );
    assert_eq!(visible(t, &d, 1), "a ⟨x^2⟩ b ⟨y⟩ c");
    assert_eq!(m[0].largeur, Largeur::Mesuree);
    let dedans = deco(t, "avant\n".len() + 4);
    assert!(visible(t, &dedans, 1).contains("$x^2$"));
    // Une formule vide reste en source.
    let t = "a $ $ b\n";
    assert!(
        widgets_de(&deco(t, utf16(t)), |g| matches!(
            g,
            GenreWidget::Maths { .. }
        ))
        .is_empty()
    );
}

/// « Lecture seule : tout se rend » : même curseur dessus, rien n'est révélé.
#[test]
fn lecture_seule_tout_se_rend() {
    let ctx = Contexte {
        lecture_seule: true,
        ..Default::default()
    };
    let t = "# Titre\n- [ ] tâche **grasse**\n- puce\n$m$ et [[lien]]\n";
    for pos in [
        0,
        4,
        10,
        20,
        utf16("# Titre\n- [ ] tâche **grasse**\n- p"),
        40,
    ] {
        let d = deco_avec(t, Etendue::new(pos, pos), true, true, &ctx);
        assert_eq!(visible(t, &d, 0), "Titre");
        assert_eq!(visible(t, &d, 1), "☐ tâche grasse");
        assert_eq!(visible(t, &d, 2), "• puce");
        assert_eq!(visible(t, &d, 3), "⟨m⟩ et lien");
    }
}

// ─── Les autres branches du modèle ───────────────────────────────────────────

/// Bloc de code : fond sur chaque ligne, coins en haut et en bas, clôtures masquées et badge de
/// langue hors curseur ; tout visible dedans ; un bloc vide reste en source.
#[test]
fn bloc_de_code_hors_et_dans_le_curseur() {
    let t = "```rust\nlet x = 1;\n```\nfin";
    let d = deco(t, utf16(t));
    let genres = |d: &Decorations, n: usize| -> Vec<GenreLigne> {
        d.lignes
            .iter()
            .filter(|l| l.ligne == n)
            .map(|l| l.genre)
            .collect()
    };
    assert_eq!(genres(&d, 0), [GenreLigne::Code, GenreLigne::CodePremiere]);
    assert_eq!(genres(&d, 1), [GenreLigne::Code]);
    assert_eq!(genres(&d, 2), [GenreLigne::Code, GenreLigne::CodeDerniere]);
    let badge = widgets_de(&d, |g| matches!(g, GenreWidget::BadgeLangue { .. }));
    assert!(matches!(&badge[0].genre, GenreWidget::BadgeLangue { langue } if langue == "rust"));
    assert_eq!(badge[0].largeur, Largeur::Nulle);
    assert_eq!(visible(t, &d, 0), "");
    assert_eq!(visible(t, &d, 1), "let x = 1;");
    assert_eq!(visible(t, &d, 2), "");
    // La police du code (0,9 em) sur les trois lignes.
    assert_eq!(d.rangees.polices.len(), 3);
    assert!((d.rangees.polices[0].taille - 14.4).abs() < 1e-4);
    // Curseur dedans : tout se voit, fond compris.
    let dd = deco(t, 9);
    assert_eq!(visible(t, &dd, 0), "```rust");
    assert_eq!(visible(t, &dd, 2), "```");
    assert_eq!(genres(&dd, 0), [GenreLigne::Code, GenreLigne::CodePremiere]);
    // Sans langue : l'ouverture est simplement masquée.
    let t = "```\nx\n```\n";
    let d = deco(t, utf16(t));
    assert!(widgets_de(&d, |g| matches!(g, GenreWidget::BadgeLangue { .. })).is_empty());
    assert_eq!(visible(t, &d, 0), "");
}

/// Citation : chaque ligne stylée, `> ` masqué hors curseur, révélé dans toute la citation.
#[test]
fn citation_lignes_et_marques() {
    let t = "> un\n> deux\n\nfin";
    let d = deco(t, utf16(t));
    let l: Vec<usize> = d
        .lignes
        .iter()
        .filter(|l| l.genre == GenreLigne::Citation)
        .map(|l| l.ligne)
        .collect();
    assert_eq!(l, [0, 1]);
    assert_eq!(visible(t, &d, 0), "un");
    assert_eq!(visible(t, &d, 1), "deux");
    let d = deco(t, 3);
    assert_eq!(visible(t, &d, 0), "> un");
}

/// Règle horizontale : un séparateur sur toute la ligne, sauf curseur dessus.
#[test]
fn separateur_hors_curseur() {
    let t = "a\n\n---\n\nb";
    let d = deco(t, 0);
    let s = widgets_de(&d, |g| matches!(g, GenreWidget::Separateur));
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].source.utf16, Etendue::new(3, 6));
    assert_eq!(s[0].largeur, Largeur::Ligne);
    assert!(widgets_de(&deco(t, 4), |g| matches!(g, GenreWidget::Separateur)).is_empty());
}

/// Image du coffre : un widget au chemin décodé ; une image externe reste en source.
#[test]
fn image_du_coffre_et_image_externe() {
    let t = "![a](dossier/mon%20image.png) et ![b](https://ex.org/i.png)\n";
    let d = deco(t, utf16(t));
    let i = widgets_de(&d, |g| matches!(g, GenreWidget::Image { .. }));
    assert_eq!(i.len(), 1);
    assert!(
        matches!(&i[0].genre, GenreWidget::Image { chemin } if chemin == "dossier/mon image.png")
    );
    assert!(visible(t, &d, 0).contains("![b](https://ex.org/i.png)"));
}

/// Les positions sortantes sont en UTF-16 ET en octets, cohérentes (ADR-004).
#[test]
fn positions_utf16_et_octets() {
    let t = "é😀 **gr** ✓\n";
    let d = deco(t, utf16(t));
    let g = d
        .marques
        .iter()
        .find(|m| m.genre == GenreMarque::Gras)
        .map(|m| m.plage);
    let g = g.unwrap_or_default();
    assert_eq!(g.utf16, Etendue::new(4, 10));
    assert_eq!(g.texte(t), "**gr**");
    for m in &d.rangees.masques {
        assert_eq!(m.texte(t), "**");
    }
    // Un curseur donné en UTF-16 dans la marque la révèle.
    assert_eq!(visible(t, &deco(t, 6), 0), "é😀 **gr** ✓");
}

/// Seules les plages visibles sont décorées (le viewport) ; les barres aussi.
#[test]
fn calcul_limite_aux_plages_visibles() {
    let mut t = String::new();
    for i in 0..100 {
        t.push_str(&format!("**l{i}**\n  retrait\n"));
    }
    let lignes = lignes_de(&t);
    let vis = [Etendue::new(lignes[40].utf16.from, lignes[45].utf16.to)];
    let ctx = Contexte {
        visibles: &vis,
        unite_indentation: 2,
        ..Default::default()
    };
    let d = deco_avec(&t, Etendue::new(0, 0), true, true, &ctx);
    assert_eq!(d.marques.len(), 3);
    assert!(d.lignes.iter().all(|l| (40..=45).contains(&l.ligne)));
    assert_eq!(d.lignes.len(), 3);
    // Le tout = l'union des morceaux.
    let entier = deco(&t, 0);
    assert_eq!(entier.marques.len(), 100);
}

/// La première sélection est la principale (`selection.main`) : les autres ne révèlent rien.
#[test]
fn seule_la_selection_principale_revele() {
    let t = "**a** **b**\n";
    let arbre = syntaxe::analyser(t);
    let sels = [Etendue::new(12, 12), Etendue::new(1, 1)];
    let c = Curseur {
        selections: &sels,
        focus: true,
        a_eu_le_focus: true,
    };
    let d = decorations::calculer(t, &arbre, &c, &Contexte::default());
    assert_eq!(visible(t, &d, 0), "a b");
    // Une sélection qui couvre le gras le révèle.
    let d = deco_avec(t, Etendue::new(0, 3), true, true, &Contexte::default());
    assert_eq!(visible(t, &d, 0), "**a** b");
}

/// Chaque nom de jeton des styles existe dans `fragment-theme` (jamais une couleur en dur).
#[test]
fn jetons_des_styles_existent_dans_le_theme() {
    use fragment_theme::{Forme, Palette, Typo};
    let existe = |j: &str| {
        Palette::CHAMPS.contains(&j) || Forme::CHAMPS.contains(&j) || Typo::CHAMPS.contains(&j)
    };
    let marques = [
        GenreMarque::Gras,
        GenreMarque::Italique,
        GenreMarque::Barre,
        GenreMarque::Surligne,
        GenreMarque::Code,
        GenreMarque::Lien,
    ];
    for m in marques {
        let s = m.style();
        for j in [s.couleur, s.fond, s.graisse, s.police, s.rayon]
            .into_iter()
            .flatten()
        {
            assert!(existe(j), "{m:?} : {j}");
        }
    }
    let lignes = [
        GenreLigne::Titre(1),
        GenreLigne::Citation,
        GenreLigne::Code,
        GenreLigne::CodePremiere,
        GenreLigne::CodeDerniere,
        GenreLigne::Indentation { niveaux: 2 },
    ];
    for l in lignes {
        let s = l.style();
        let mut j: Vec<&str> = [
            s.couleur,
            s.fond,
            s.graisse,
            s.police,
            s.rayon_haut,
            s.rayon_bas,
        ]
        .into_iter()
        .flatten()
        .collect();
        j.extend(s.bordure_gauche.map(|b| b.1));
        j.extend(s.traits.map(|t| t.1));
        for x in j {
            assert!(existe(x), "{l:?} : {x}");
        }
    }
}

// ─── 10 000 lignes ───────────────────────────────────────────────────────────

fn dix_mille_lignes() -> String {
    let motif = [
        "# Titre de section",
        "Un paragraphe avec du **gras**, de l'*italique*, du `code` et un [[lien]].",
        "- une puce avec ==surligné==",
        "  - une sous-puce avec $x^2$",
        "- [ ] une tâche",
        "> une citation",
        "```js",
        "const a = 1;",
        "```",
        "",
    ];
    (0..10_000)
        .map(|i| motif[i % motif.len()])
        .collect::<Vec<_>>()
        .join("\n")
}

/// Sans mesurer le temps (la CI varie) : 10 000 lignes se décorent entièrement, et le viewport ne
/// décore que ses lignes.
#[test]
fn dix_mille_lignes_forme() {
    let t = dix_mille_lignes();
    let d = deco(&t, 0);
    assert_eq!(
        d.lignes
            .iter()
            .filter(|l| matches!(l.genre, GenreLigne::Titre(1)))
            .count(),
        1000
    );
    assert_eq!(cases(&d).len(), 1000);
}

/// Temps du calcul sur 10 000 lignes : document entier, et un viewport de 60 lignes au milieu.
/// `cargo test -p fragment-markdown --release --test decorations -- --ignored --nocapture mesure`
#[test]
#[ignore = "mesure, à lancer en --release"]
fn mesure_dix_mille_lignes() {
    let t = dix_mille_lignes();
    let arbre = syntaxe::analyser(&t);
    let lignes = lignes_de(&t);
    let sel = [Etendue::new(0, 0)];
    let c = Curseur {
        selections: &sel,
        focus: true,
        a_eu_le_focus: true,
    };
    let mesurer = |ctx: &Contexte| {
        let mut temps = Vec::new();
        for _ in 0..7 {
            let t0 = std::time::Instant::now();
            let d = decorations::calculer(&t, &arbre, &c, ctx);
            temps.push(t0.elapsed());
            assert!(!d.lignes.is_empty());
        }
        temps.sort();
        temps
    };
    let entier = mesurer(&Contexte::default());
    let vis = [Etendue::new(lignes[5000].utf16.from, lignes[5060].utf16.to)];
    let viewport = mesurer(&Contexte {
        visibles: &vis,
        ..Default::default()
    });
    println!(
        "{} octets, {} nœuds : document entier min {:?} / médiane {:?} ; viewport de 60 lignes min {:?} / médiane {:?}",
        t.len(),
        arbre.noeuds().len(),
        entier[0],
        entier[3],
        viewport[0],
        viewport[3]
    );
}
