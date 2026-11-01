// Port de app/src/shared/htmlBridge.test.ts (les cas purs) — plus les cas de
// l'aplatissement, que le TS ne teste qu'en e2e (html-text.spec.ts : la page
// `texte.html`, reprise ici comme arbre de nœuds).

use fragment_text::{
    FlatText, HtmlNode, TextSegment, flatten_page, inject_html_bridge, page_servable,
};

/// Le script du pont n'est pas porté : un tenant-lieu, la règle d'insertion ne lit pas son contenu.
const SCRIPT: &str = "pont()";

fn balise() -> String {
    format!("<script>{SCRIPT}</script>")
}

// describe('injectHtmlBridge — le pont en tête de page')

#[test]
fn juste_apres_head_avant_tout_script_de_la_page() {
    let page = "<!doctype html><html lang=\"fr\"><head><title>x</title><script>page()</script></head><body>a</body></html>";
    let out = inject_html_bridge(page, SCRIPT);
    assert_eq!(
        out,
        format!(
            "<!doctype html><html lang=\"fr\"><head>{}<title>x</title><script>page()</script></head><body>a</body></html>",
            balise()
        )
    );
}

#[test]
fn head_avec_attributs_sans_head_apres_html_fragment_nu_en_tete() {
    assert_eq!(
        inject_html_bridge("<HEAD data-x=\"1\"><p>t</p>", SCRIPT),
        format!("<HEAD data-x=\"1\">{}<p>t</p>", balise())
    );
    assert_eq!(
        inject_html_bridge("<html><body>b</body></html>", SCRIPT),
        format!("<html>{}<body>b</body></html>", balise())
    );
    assert_eq!(
        inject_html_bridge("<p>seul</p>", SCRIPT),
        format!("{}<p>seul</p>", balise())
    );
}

#[test]
fn ne_confond_pas_header_avec_head() {
    assert_eq!(
        inject_html_bridge("<header>h</header>", SCRIPT),
        format!("{}<header>h</header>", balise())
    );
}

// describe('pageServable — ce qu’une page vivante peut charger du coffre')

#[test]
fn ses_ressources_web_a_toute_profondeur() {
    for p in [
        "anim.html",
        "img/pixel.png",
        "lib/three.module.js",
        "a/b/style.css",
        "data.json",
        "m/scene.glb",
        "f/Font.WOFF2",
    ] {
        assert!(page_servable(p), "{p}");
    }
}

#[test]
fn jamais_une_note_un_pdf_un_fichier_sans_extension_ni_un_dossier_cache() {
    for p in [
        "note.md",
        "docs/rapport.pdf",
        "README",
        ".fragment/app.json",
        "x/.git/config",
        "a/.secret.js",
        "",
    ] {
        assert!(!page_servable(p), "{p}");
    }
}

// flatten_page — la fonction `aplatir` du pont

#[test]
fn flatten_non_ascii_segments_en_unites_utf16() {
    let body = vec![
        HtmlNode::element("p", vec![HtmlNode::Text("é😀".to_owned())]),
        HtmlNode::element("p", vec![HtmlNode::Text("fin".to_owned())]),
    ];
    let flat = flatten_page(&body);
    assert_eq!(flat.text, "é😀\nfin");
    // « é😀 » : 3 unités ; le \n : 1 ; « fin » commence à l'unité 4.
    assert_eq!(
        flat.segs,
        vec![
            TextSegment {
                node: 0,
                start: 0,
                len: 3
            },
            TextSegment {
                node: 1,
                start: 4,
                len: 3
            },
        ]
    );
}

fn t(s: &str) -> HtmlNode {
    HtmlNode::text(s)
}

fn el(tag: &str, children: Vec<HtmlNode>) -> HtmlNode {
    HtmlNode::element(tag, children)
}

#[test]
fn le_texte_aplati_les_blocs_sur_leurs_lignes_ni_script_ni_style() {
    // Le `<body>` de `texte.html` (e2e/html-text.spec.ts).
    let body = vec![
        t("\n"),
        el("div", vec![]),
        t("\n"),
        el("h1", vec![t("Le cycle de Krebs")]),
        t("\n"),
        el(
            "p",
            vec![t("La mitochondrie produit l'énergie de la cellule.")],
        ),
        t("\n"),
        el(
            "p",
            vec![
                t("Le "),
                el("b", vec![t("pyruvate")]),
                t(" entre dans le cycle et devient acétyl-CoA."),
            ],
        ),
        t("\n"),
        el("div", vec![]),
        t("\n"),
        el("p", vec![t("Fin de page.")]),
        t("\n"),
        el(
            "script",
            vec![t(
                "\nwindow.touches = 0;\nwindow.addEventListener('keydown', () => {});\n",
            )],
        ),
        t("\n"),
    ];
    let flat = flatten_page(&body);
    assert_eq!(
        flat.text.split('\n').collect::<Vec<_>>(),
        vec![
            "Le cycle de Krebs",
            "La mitochondrie produit l'énergie de la cellule.",
            "Le pyruvate entre dans le cycle et devient acétyl-CoA.",
            "Fin de page.",
        ]
    );
}

#[test]
fn les_segments_noeud_par_noeud_avec_leur_ordinal() {
    let body = vec![
        t("\n"),
        el("p", vec![t("ab")]),
        el("p", vec![t("cd"), el("i", vec![t("e")])]),
    ];
    assert_eq!(
        flatten_page(&body),
        FlatText {
            text: "ab\ncde".to_owned(),
            segs: vec![
                TextSegment {
                    node: 1,
                    start: 0,
                    len: 2
                },
                TextSegment {
                    node: 2,
                    start: 3,
                    len: 2
                },
                TextSegment {
                    node: 3,
                    start: 5,
                    len: 1
                },
            ],
        }
    );
}

#[test]
fn un_seul_saut_entre_deux_blocs_meme_imbriques_ou_vides_jamais_a_la_fin() {
    assert_eq!(
        flatten_page(&[
            el("div", vec![el("p", vec![t("a")])]),
            el("div", vec![]),
            el("p", vec![t("b")])
        ])
        .text,
        "a\nb"
    );
    assert_eq!(
        flatten_page(&[el("p", vec![t("a\n")]), el("p", vec![t("b")])]).text,
        "a\nb"
    );
    assert_eq!(flatten_page(&[el("p", vec![t("fin")])]).text, "fin");
    assert_eq!(flatten_page(&[]).text, "");
}

#[test]
fn les_elements_en_ligne_ne_coupent_pas_br_coupe() {
    assert_eq!(
        flatten_page(&[el("span", vec![t("a")]), el("SPAN", vec![t("b")])]).text,
        "ab"
    );
    assert_eq!(
        flatten_page(&[t("a"), el("br", vec![]), t("b")]).text,
        "a\nb"
    );
}

#[test]
fn dans_pre_les_blancs_de_mise_en_forme_sont_du_texte() {
    assert_eq!(
        flatten_page(&[el("pre", vec![t("x\n"), t("  \n"), el("b", vec![t("y")])])]).text,
        "x\n  \ny"
    );
    assert_eq!(
        flatten_page(&[el("p", vec![t("x"), t("  \n  "), t("y")])]).text,
        "xy"
    );
    // Un blanc sans retour reste du texte (un espace entre deux mots).
    assert_eq!(
        flatten_page(&[el("p", vec![t("x"), t(" "), t("y")])]).text,
        "x y"
    );
}
