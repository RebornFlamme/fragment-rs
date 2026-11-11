// Port de app/src/ui/views/markdown/language.ts (extensions Lezer de Fragment) : tests de l'arbre.
//! Tests de `fragment_markdown::syntaxe` (fr-t3-syntaxe, ADR-010).
//!
//! Deux familles :
//! 1. `lezer_*` : l'arbre de chaque document de `tests/syntaxe/*.md` est comparé, nœud pour nœud
//!    (nom, profondeur, plage UTF-16), au relevé de Lezer `tests/syntaxe/*.lezer`. Les documents sont
//!    RÉÉCRITS (mêmes constructions que les specs e2e `editor-hell`, `markdown-html`, `properties`,
//!    autres mots) ; les relevés sont produits HORS du dépôt par un script qui charge le parseur de
//!    Fragment (`markdownLanguage` + `markdownExtensions`) — rien n'est recopié de ses sources.
//! 2. un test par règle citée par le bead et par extension, sur de petits textes.
//!
//! ★ Le nœud `FrontMatter` est un AJOUT de fragment-rs (absent de Lezer) : la comparaison le retire
//!   et remonte ses enfants d'un niveau.

use fragment_markdown::syntaxe::{self, Arbre, Cote, Genre, Parcours};

// ─── Relevés de Lezer ────────────────────────────────────────────────────────

/// L'arbre au format des relevés : `<indentation>Nom from to`, `FrontMatter` retiré.
fn releve(arbre: &Arbre) -> Vec<String> {
    let mut out = Vec::new();
    for n in arbre.noeuds() {
        if n.genre == Genre::FrontMatter {
            continue;
        }
        let dans_fm = arbre.noeuds().iter().any(|f| f.genre == Genre::FrontMatter) && {
            let mut p = n.parent;
            let mut oui = false;
            while let Some(k) = p {
                if arbre.noeud(k).genre == Genre::FrontMatter {
                    oui = true;
                }
                p = arbre.noeud(k).parent;
            }
            oui
        };
        let prof = n.profondeur - usize::from(dans_fm);
        out.push(format!(
            "{}{} {} {}",
            "  ".repeat(prof),
            n.genre,
            n.from(),
            n.to()
        ));
    }
    out
}

fn comparer(nom: &str) {
    let dossier = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/syntaxe/");
    let lire = |f: String| std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{f} : {e}"));
    let texte = lire(format!("{dossier}{nom}.md"));
    let attendu: Vec<String> = lire(format!("{dossier}{nom}.lezer"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect();
    let obtenu = releve(&syntaxe::analyser(&texte));
    if obtenu != attendu {
        let mut diff = String::new();
        for i in 0..obtenu.len().max(attendu.len()) {
            let (a, o) = (
                attendu.get(i).map_or("∅", String::as_str),
                obtenu.get(i).map_or("∅", String::as_str),
            );
            if a != o {
                diff.push_str(&format!("{i:4} lezer : {a}\n     rust  : {o}\n"));
            }
        }
        panic!("{nom} : l'arbre diffère du relevé de Lezer\n{diff}");
    }
}

#[test]
fn lezer_listes() {
    comparer("listes");
}

#[test]
fn lezer_inline() {
    comparer("inline");
}

#[test]
fn lezer_blocs() {
    comparer("blocs");
}

#[test]
fn lezer_html() {
    comparer("html");
}

#[test]
fn lezer_proprietes() {
    comparer("proprietes");
}

#[test]
fn lezer_maths_ouvertes() {
    comparer("maths_ouvertes");
}

#[test]
fn lezer_proprietes_ouvertes() {
    comparer("proprietes_ouvertes");
}

// ─── Outils ──────────────────────────────────────────────────────────────────

/// Les nœuds d'un genre, avec leur texte.
fn textes(texte: &str, genre: Genre) -> Vec<&str> {
    let a = syntaxe::analyser(texte);
    a.noeuds()
        .iter()
        .filter(|n| n.genre == genre)
        .map(|n| n.plage.texte(texte))
        .collect()
}

/// Les genres des enfants directs de la racine.
fn blocs(texte: &str) -> Vec<Genre> {
    let a = syntaxe::analyser(texte);
    a.enfants(0).map(|i| a.noeud(i).genre).collect()
}

// ─── Règles de l'éditeur de Fragment (CLAUDE.md, section Éditeur Markdown) ──

/// editor-hell « `- [ ]` en fin de ligne (sans texte) se rend en case » : pour Lezer ce n'est PAS
/// une tâche (il faut un espace après `]`), c'est un paragraphe `[ ]` ; le rendu le traite à part.
#[test]
fn tache_sans_texte_n_est_pas_une_tache() {
    assert!(textes("- [ ]", Genre::Task).is_empty());
    assert_eq!(textes("- [ ]", Genre::Paragraph), ["[ ]"]);
    // Avec l'espace, si : la tâche couvre aussi l'espace final.
    assert_eq!(textes("- [ ] ", Genre::Task), ["[ ] "]);
    assert_eq!(textes("- [x] fait", Genre::TaskMarker), ["[x]"]);
    assert_eq!(textes("- [X] fait", Genre::TaskMarker), ["[X]"]);
    // Collé au texte, ou hors d'une liste : pas de tâche.
    assert!(textes("- [ ]collé", Genre::Task).is_empty());
    assert!(textes("[ ] hors liste", Genre::Task).is_empty());
}

/// « `$$` en début de ligne interrompt un paragraphe » (endLeaf de BlockMath).
#[test]
fn maths_en_bloc_interrompt_un_paragraphe() {
    let t = "texte\n$$\nx^2\n$$\nsuite";
    assert_eq!(
        blocs(t),
        [Genre::Paragraph, Genre::BlockMath, Genre::Paragraph]
    );
    assert_eq!(textes(t, Genre::BlockMath), ["$$\nx^2\n$$"]);
    assert_eq!(textes(t, Genre::Paragraph), ["texte", "suite"]);
}

#[test]
fn maths_en_bloc_sur_une_ligne_ignore_la_fin_de_ligne() {
    let t = "$$a+b$$ reste\nlà";
    assert_eq!(textes(t, Genre::BlockMath), ["$$a+b$$"]);
    assert_eq!(textes(t, Genre::Paragraph), ["là"]);
}

/// Un `$$` jamais fermé (en cours de frappe) prend tout jusqu'au bout : le nœud existe quand même.
#[test]
fn maths_en_bloc_sans_fermeture_va_jusqu_au_bout() {
    let t = "$$\nx\n\n# pas un titre";
    assert_eq!(blocs(t), [Genre::BlockMath]);
    assert_eq!(textes(t, Genre::BlockMath), [t]);
}

/// « Un tableau GFM ne s'arrête qu'à une ligne vide » : la ligne collée dessous est une rangée.
#[test]
fn tableau_ne_s_arrete_qu_a_une_ligne_vide() {
    let t = "| a | b |\n| - | - |\n| 1 | 2 |\ncollée\n\naprès";
    assert_eq!(blocs(t), [Genre::Table, Genre::Paragraph]);
    assert_eq!(textes(t, Genre::TableRow), ["| 1 | 2 |", "collée"]);
    assert_eq!(textes(t, Genre::TableCell), ["a", "b", "1", "2", "collée"]);
    assert_eq!(textes(t, Genre::TableHeader), ["| a | b |"]);
    assert_eq!(textes(t, Genre::TableDelimiter).len(), 3 + 1 + 3);
}

/// Un paragraphe suivi d'une ligne à barres ET d'une ligne de délimiteurs : le tableau l'interrompt.
#[test]
fn tableau_interrompt_un_paragraphe() {
    assert_eq!(
        blocs("texte\n| a |\n| - |"),
        [Genre::Paragraph, Genre::Table]
    );
}

/// « Blocs HTML CommonMark » : un `<div>` court jusqu'à la ligne vide ; un commentaire est un
/// `CommentBlock` ; `<?…?>` un `ProcessingInstructionBlock`.
#[test]
fn blocs_html_commonmark() {
    let t = "<div>\nun\n\ndeux";
    assert_eq!(blocs(t), [Genre::HTMLBlock, Genre::Paragraph]);
    assert_eq!(textes(t, Genre::HTMLBlock), ["<div>\nun"]);
    assert_eq!(
        blocs("<!-- note\nsur deux lignes -->\nsuite"),
        [Genre::CommentBlock, Genre::Paragraph]
    );
    assert_eq!(blocs("<?x y?>"), [Genre::ProcessingInstructionBlock]);
    // `<script>` (style 1) court jusqu'à sa balise fermante, même par-delà une ligne vide.
    assert_eq!(
        textes("<script>\na\n\nb</script>\nc", Genre::HTMLBlock),
        ["<script>\na\n\nb</script>"]
    );
}

/// « Un `<svg>` sur UNE ligne est du HTML EN LIGNE » : pas un bloc (le style 7 exige la balise seule).
#[test]
fn svg_sur_une_ligne_est_du_html_en_ligne() {
    let t = "<svg width=\"10\"><rect/></svg>";
    assert_eq!(blocs(t), [Genre::Paragraph]);
    assert_eq!(
        textes(t, Genre::HTMLTag),
        ["<svg width=\"10\">", "<rect/>", "</svg>"]
    );
    // Une balise seule sur sa ligne, elle, ouvre un bloc (style 7).
    assert_eq!(blocs("<svg>\n<rect/>\n</svg>"), [Genre::HTMLBlock]);
}

/// markdown-html « HTML en ligne » : balises paires, `<br>`, commentaire en ligne.
#[test]
fn html_en_ligne_balises_et_commentaire() {
    let t = "Du <mark>texte</mark>, H<sub>2</sub>O,<br>et <!-- caché --> fin.";
    assert_eq!(
        textes(t, Genre::HTMLTag),
        ["<mark>", "</mark>", "<sub>", "</sub>", "<br>"]
    );
    assert_eq!(textes(t, Genre::Comment), ["<!-- caché -->"]);
}

// ─── Extensions ──────────────────────────────────────────────────────────────

#[test]
fn surligne() {
    let t = "a ==b== c";
    assert_eq!(textes(t, Genre::Highlight), ["==b=="]);
    assert_eq!(textes(t, Genre::HighlightMark), ["==", "=="]);
    // Le surligné contient l'emphase ; un `==` seul reste du texte.
    assert_eq!(textes("==x **y** z==", Genre::StrongEmphasis), ["**y**"]);
    assert!(textes("==seul", Genre::Highlight).is_empty());
}

#[test]
fn liens_internes_et_embeds() {
    assert_eq!(textes("voir [[Note]] ici", Genre::WikiLink), ["[[Note]]"]);
    assert_eq!(
        textes("voir [[Note|alias]]", Genre::WikiLink),
        ["[[Note|alias]]"]
    );
    assert_eq!(
        textes("![[image.png|300]]", Genre::Embed),
        ["![[image.png|300]]"]
    );
    // Cible vide, `[[` imbriqué, saut de ligne : pas de lien interne.
    assert!(textes("[[]]", Genre::WikiLink).is_empty());
    assert_eq!(textes("[[a [[b]]", Genre::WikiLink), ["[[b]]"]);
    assert!(textes("[[a\nb]]", Genre::WikiLink).is_empty());
    // Un lien interne n'a pas d'enfant.
    let a = syntaxe::analyser("[[x]]");
    let i = a
        .noeuds()
        .iter()
        .position(|n| n.genre == Genre::WikiLink)
        .unwrap();
    assert_eq!(a.enfants(i).count(), 0);
}

/// editor-hell « math inline $…$ et $$…$$ dans la phrase : rendus hors curseur ».
#[test]
fn maths_en_ligne_et_display_dans_la_phrase() {
    let t = "a $x^2$ b $$y$$ c";
    assert_eq!(textes(t, Genre::InlineMath), ["$x^2$"]);
    assert_eq!(textes(t, Genre::InlineDisplayMath), ["$$y$$"]);
    assert_eq!(textes(t, Genre::InlineMathMark), ["$", "$", "$$", "$$"]);
}

/// La règle d'Obsidian contre les faux positifs : `$5 et $10`, « 5 $ et 10 $ ».
#[test]
fn maths_en_ligne_pas_de_faux_positif_sur_les_prix() {
    assert!(textes("ça coûte $5 et $10", Genre::InlineMath).is_empty());
    assert!(textes("5 $ et 10 $", Genre::InlineMath).is_empty());
    // Échappé, ou dans du code : pas de maths.
    assert!(textes(r"\$x$", Genre::InlineMath).is_empty());
    assert!(textes("`$x$`", Genre::InlineMath).is_empty());
}

#[test]
fn indice_exposant_et_barre() {
    let t = "H~2~O, x^2^ et ~~barré~~";
    assert_eq!(textes(t, Genre::Subscript), ["~2~"]);
    assert_eq!(textes(t, Genre::Superscript), ["^2^"]);
    assert_eq!(textes(t, Genre::Strikethrough), ["~~barré~~"]);
    // Pas d'espace dans un indice.
    assert!(textes("~a b~", Genre::Subscript).is_empty());
}

#[test]
fn url_nue_et_emoji_gfm() {
    let t = "va sur https://exemple.org/a. ou www.site.fr :smile:";
    assert_eq!(
        textes(t, Genre::URL),
        ["https://exemple.org/a", "www.site.fr"]
    );
    assert_eq!(textes(t, Genre::Emoji), [":smile:"]);
}

/// editor-hell, inline : chaque marque cachée hors curseur est un nœud distinct.
#[test]
fn marques_de_l_inline() {
    assert_eq!(textes("x **g** y", Genre::EmphasisMark), ["**", "**"]);
    assert_eq!(textes("x *i* y", Genre::EmphasisMark), ["*", "*"]);
    assert_eq!(textes("x `c` y", Genre::CodeMark), ["`", "`"]);
    assert_eq!(
        textes("[l](https://e.org)", Genre::LinkMark),
        ["[", "]", "(", ")"]
    );
    assert_eq!(textes("[l](https://e.org)", Genre::URL), ["https://e.org"]);
}

/// editor-hell « titres 1 à 6 » : un genre par niveau, la marque `#` à part.
#[test]
fn titres_un_a_six() {
    let t = "# T1\n## T2\n### T3\n#### T4\n##### T5\n###### T6\n####### non";
    let a = syntaxe::analyser(t);
    let niveaux: Vec<u8> = a
        .noeuds()
        .iter()
        .filter_map(|n| n.genre.niveau_titre())
        .collect();
    assert_eq!(niveaux, [1, 2, 3, 4, 5, 6]);
    assert_eq!(
        textes(t, Genre::HeaderMark),
        ["#", "##", "###", "####", "#####", "######"]
    );
    assert_eq!(textes("## fermé ##", Genre::HeaderMark), ["##", "##"]);
    assert_eq!(textes("titre\n===", Genre::SetextHeading1), ["titre\n==="]);
}

/// editor-hell « sous-puce : son niveau vient de la profondeur de liste ».
#[test]
fn listes_imbriquees_et_numerotees() {
    let t = "- a\n  - b\n    - c\n1. un\n2. deux";
    let a = syntaxe::analyser(t);
    let c = a
        .noeuds()
        .iter()
        .position(|n| n.genre == Genre::ListItem && n.plage.texte(t) == "- c")
        .unwrap();
    let listes = a
        .ancetres(c)
        .filter(|&k| a.noeud(k).genre == Genre::BulletList)
        .count();
    assert_eq!(listes, 3);
    assert_eq!(textes(t, Genre::OrderedList).len(), 1);
    assert_eq!(textes(t, Genre::ListMark), ["-", "-", "-", "1.", "2."]);
}

/// editor-hell « blocs, clics et alignement » : code clôturé (info, texte, marques), citation, règle.
#[test]
fn code_cloture_citation_et_regle() {
    let t = "```js\nconst a = 1;\n\nconst b = 2;\n```\n> cité\n***";
    assert_eq!(
        blocs(t),
        [Genre::FencedCode, Genre::Blockquote, Genre::HorizontalRule]
    );
    assert_eq!(textes(t, Genre::CodeInfo), ["js"]);
    assert_eq!(textes(t, Genre::CodeText), ["const a = 1;\n\nconst b = 2;"]);
    assert_eq!(textes(t, Genre::CodeMark), ["```", "```"]);
    assert_eq!(textes(t, Genre::QuoteMark), [">"]);
}

/// properties : le frontmatter de tête ENVELOPPE ce que Lezer y voit ; ailleurs, `---` est une règle.
#[test]
fn front_matter_enveloppe_la_tete() {
    let t = "---\ntags:\n  - a\n---\nCorps";
    let a = syntaxe::analyser(t);
    assert_eq!(blocs(t), [Genre::FrontMatter, Genre::Paragraph]);
    assert_eq!(
        a.front_matter().map(|p| p.texte(t)),
        Some("---\ntags:\n  - a\n---")
    );
    let fm = a.enfants(0).next().unwrap();
    let dedans: Vec<Genre> = a.enfants(fm).map(|i| a.noeud(i).genre).collect();
    assert_eq!(
        dedans,
        [
            Genre::HorizontalRule,
            Genre::Paragraph,
            Genre::BulletList,
            Genre::HorizontalRule
        ]
    );
    // Pas fermé, ou pas en ligne 1 : pas de frontmatter.
    assert!(syntaxe::analyser("---\na: 1").front_matter().is_none());
    assert!(syntaxe::analyser("x\n---\na\n---").front_matter().is_none());
    // Vide : `---\n---`.
    assert_eq!(blocs("---\n---\nx"), [Genre::FrontMatter, Genre::Paragraph]);
}

// ─── Positions, parcours ─────────────────────────────────────────────────────

/// ADR-004 : chaque nœud porte UTF-16 ET octets ; un émoji compte 2 unités et 4 octets.
#[test]
fn plages_utf16_et_octets() {
    let t = "é😀 **gras**";
    let a = syntaxe::analyser(t);
    let n = a
        .noeuds()
        .iter()
        .find(|n| n.genre == Genre::StrongEmphasis)
        .unwrap();
    assert_eq!((n.plage.utf16.from, n.plage.utf16.to), (4, 12));
    assert_eq!((n.plage.octets.from, n.plage.octets.to), (7, 15));
    assert_eq!(n.plage.texte(t), "**gras**");
    // Le Document couvre tout le texte.
    assert_eq!(a.noeud(0).to(), 12);
    assert_eq!(a.noeud(0).plage.octets.to, t.len());
}

/// Le parcours limité à une plage (viewport) ne visite que ce qui la touche, et sait sauter.
#[test]
fn parcours_limite_a_une_plage() {
    let t = "# A\n\npara **x**\n\n# B\n\nfin";
    let a = syntaxe::analyser(t);
    let mut vus = Vec::new();
    a.parcourir(6, 14, |_, n| {
        vus.push(n.genre);
        Parcours::Entrer
    });
    assert_eq!(
        vus,
        [
            Genre::Document,
            Genre::Paragraph,
            Genre::StrongEmphasis,
            Genre::EmphasisMark,
            Genre::EmphasisMark
        ]
    );
    let mut vus = Vec::new();
    a.parcourir(0, t.len(), |_, n| {
        vus.push(n.genre);
        if n.genre == Genre::Paragraph {
            Parcours::Sauter
        } else {
            Parcours::Entrer
        }
    });
    assert!(!vus.contains(&Genre::StrongEmphasis));
    assert_eq!(
        a.noeuds_dans(0, 0).len(),
        3,
        "Document, ATXHeading1, HeaderMark"
    );
}

/// `noeud_a` : le nœud le plus profond à une position, selon le côté (`resolveInner`).
#[test]
fn noeud_a_une_position() {
    let t = "a **b** c";
    let a = syntaxe::analyser(t);
    let g = |pos, cote| a.noeud(a.noeud_a(pos, cote)).genre;
    assert_eq!(g(4, Cote::Dedans), Genre::StrongEmphasis);
    assert_eq!(g(2, Cote::Dedans), Genre::Paragraph);
    assert_eq!(g(2, Cote::Apres), Genre::EmphasisMark);
    assert_eq!(g(7, Cote::Avant), Genre::EmphasisMark);
    let i = a.noeud_a(4, Cote::Dedans);
    assert_eq!(
        a.ancetre_de_genre(i, Genre::Paragraph)
            .map(|k| a.noeud(k).genre),
        Some(Genre::Paragraph)
    );
}

/// Une note vide, des débuts de syntaxe seuls : la racine, sans panique.
#[test]
fn documents_vides_et_debuts_seuls() {
    for t in [
        "", "\n", "\n\n  \n", "-", ">", "#", "$$", "[", "|", "```", "- [", "<", "`", "~", "\t- a",
    ] {
        let a = syntaxe::analyser(t);
        assert_eq!(a.noeud(0).genre, Genre::Document, "{t:?}");
        assert_eq!(a.noeud(0).plage.octets.to, t.len(), "{t:?}");
    }
}

// ─── Mesure (ADR-010) ────────────────────────────────────────────────────────

/// Le document de mesure : 10 000 lignes (titres, gras, liens internes, maths, surligné, tâches,
/// tableaux, code, citations), le même que celui des mesures de tree-sitter-md et pulldown-cmark.
fn dix_mille_lignes() -> String {
    let motif = [
        "## Section {i}",
        "Un paragraphe avec du **gras**, de l'*italique*, un [[lien {i}]] et $x_{i}^2$ en ligne, puis ==surligné== é.",
        "- [ ] tâche {i} avec `code`",
        "- [x] tâche faite {i}",
        "| a | b {i} |",
        "| --- | --- |",
        "| 1 | 2 |",
        "",
        "```js",
        "const v{i} = {i};",
        "```",
        "",
        "> citation {i}",
        "",
        "",
    ];
    let mut lignes = Vec::new();
    let mut i = 0;
    while lignes.len() < 10_000 {
        for l in motif {
            lignes.push(l.replace("{i}", &i.to_string()));
        }
        i += 1;
    }
    lignes.truncate(10_000);
    lignes.join("\n") + "\n"
}

/// Temps d'analyse de 10 000 lignes, puis d'une réanalyse après une frappe au milieu.
/// `cargo test -p fragment-markdown --release --test syntaxe -- --ignored --nocapture mesure`
#[test]
#[ignore = "mesure, à lancer en --release"]
fn mesure_dix_mille_lignes() {
    let doc = dix_mille_lignes();
    let mut temps = Vec::new();
    for _ in 0..5 {
        let t0 = std::time::Instant::now();
        let a = syntaxe::analyser(&doc);
        temps.push(t0.elapsed());
        assert!(a.noeuds().len() > 10_000);
    }
    let mut frappe = doc.clone();
    let milieu = (doc.len() / 2..doc.len())
        .find(|&i| doc.is_char_boundary(i))
        .unwrap();
    frappe.insert(milieu, 'x');
    let t1 = std::time::Instant::now();
    let a = syntaxe::analyser(&frappe);
    let d_frappe = t1.elapsed();
    temps.sort();
    println!(
        "{} octets, {} nœuds : analyse complète min {:?} / médiane {:?} ; après une frappe {:?}",
        doc.len(),
        a.noeuds().len(),
        temps[0],
        temps[2],
        d_frappe
    );
}

/// Sans mesurer le temps (la CI varie) : 10 000 lignes s'analysent, et le résultat a la forme attendue.
#[test]
fn dix_mille_lignes_s_analysent() {
    let doc = dix_mille_lignes();
    let a = syntaxe::analyser(&doc);
    let compte = |g| a.noeuds().iter().filter(|n| n.genre == g).count();
    assert_eq!(compte(Genre::ATXHeading2), 667);
    assert_eq!(compte(Genre::Task), 2 * 667);
    assert_eq!(compte(Genre::WikiLink), 667);
    assert_eq!(compte(Genre::InlineMath), 667);
    assert_eq!(a.noeud(0).plage.octets.to, doc.len());
}

/// Écrit le relevé de l'arbre de chaque fichier de `RELEVE_MD` (chemins séparés par `;`) dans
/// `<fichier>.rust`, au format des relevés de Lezer : pour comparer HORS du dépôt sur d'autres
/// documents (ceux des specs e2e, que ce dépôt ne recopie pas).
#[test]
#[ignore = "outil de relevé, hors CI"]
fn releve_rust() {
    let Ok(liste) = std::env::var("RELEVE_MD") else {
        return;
    };
    for f in liste.split(';').filter(|f| !f.is_empty()) {
        let texte = std::fs::read_to_string(f).unwrap();
        let lignes = releve(&syntaxe::analyser(&texte));
        std::fs::write(format!("{f}.rust"), lignes.join("\n") + "\n").unwrap();
    }
}
