//! Spike S6 : le HTML dans le Markdown. Que fait le rendu NATIF de gpui-kit 0.8.1 (`gpui-base`, analyseur
//! html5ever + markdown) de chaque fixture ? Et qu'est-ce qui se rattrape par un SVG peint par le
//! rastériseur de GPUI, avec un interpréteur SMIL minimal ? Les lignes `S6|…` (`--nocapture`) alimentent
//! `docs/spikes/S6-html-markdown.md`.
//!
//! Tests HEADLESS, sans fenêtre : l'analyse produit un arbre de blocs (`BlockNode`) qu'on lit par son
//! `Debug` (les champs sont privés à la crate). Les fixtures sont réécrites pour ce dépôt (ADR-007) ; les
//! références (`reference/`) sortent de Chromium par `capture/capture.mjs`.
//!
//! Windows seulement : les crates GPUI ne se construisent pas sous Linux sans bibliothèques système.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "spike_s6/smil.rs"]
mod smil;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use fragment_markdown::sanitize_html::{filter_element, is_tag_allowed};
use gpui_kit::SvgRenderer;
use gpui_kit::base::text::MarkdownExtensions;
use gpui_kit::base::text::editor_host::parse_markdown;

const FIXTURES: [(&str, &str); 12] = [
    (
        "f01-div-stylee",
        include_str!("spike_s6/fixtures/f01-div-stylee.md"),
    ),
    (
        "f02-svg-statique",
        include_str!("spike_s6/fixtures/f02-svg-statique.md"),
    ),
    (
        "f03-svg-smil",
        include_str!("spike_s6/fixtures/f03-svg-smil.md"),
    ),
    (
        "f04-details",
        include_str!("spike_s6/fixtures/f04-details.md"),
    ),
    (
        "f05-en-ligne",
        include_str!("spike_s6/fixtures/f05-en-ligne.md"),
    ),
    (
        "f06-commentaire",
        include_str!("spike_s6/fixtures/f06-commentaire.md"),
    ),
    (
        "f07-tableau-html",
        include_str!("spike_s6/fixtures/f07-tableau-html.md"),
    ),
    (
        "f08-iframe",
        include_str!("spike_s6/fixtures/f08-iframe.md"),
    ),
    (
        "f09-pieges",
        include_str!("spike_s6/fixtures/f09-pieges.md"),
    ),
    ("f10-image", include_str!("spike_s6/fixtures/f10-image.md")),
    (
        "f11-animation-css",
        include_str!("spike_s6/fixtures/f11-animation-css.md"),
    ),
    (
        "f12-mise-en-page",
        include_str!("spike_s6/fixtures/f12-mise-en-page.md"),
    ),
];

fn source(nom: &str) -> &'static str {
    FIXTURES.iter().find(|(n, _)| *n == nom).unwrap().1
}

/// L'arbre natif d'une source Markdown, en texte (`Debug`).
fn arbre(md: &str) -> String {
    let blocs = parse_markdown(md, Arc::new(MarkdownExtensions::default())).expect("analyse");
    format!("{blocs:?}")
}

/// Le texte des nœuds en ligne : tout ce qui suit `text: "` dans le `Debug`, jusqu'au guillemet fermant.
fn textes(dump: &str) -> Vec<String> {
    let mut sortie = Vec::new();
    let mut reste = dump;
    while let Some(p) = reste.find("text: \"") {
        let corps = &reste[p + 7..];
        let mut fin = corps.len();
        let mut echappe = false;
        for (i, c) in corps.char_indices() {
            if echappe {
                echappe = false;
            } else if c == '\\' {
                echappe = true;
            } else if c == '"' {
                fin = i;
                break;
            }
        }
        sortie.push(corps[..fin].to_owned());
        reste = &corps[fin..];
    }
    sortie
}

fn visible(dump: &str) -> String {
    textes(dump).join("|")
}

/// La source SANS ses déclarations `style="…"` (remplacées par des blancs de même longueur : les
/// positions de l'arbre ne bougent pas). Si l'arbre est identique, le rendu ignore tout le CSS en ligne.
fn sans_style(md: &str) -> String {
    let mut s = md.to_owned();
    let mut depart = 0;
    while let Some(p) = s[depart..].find("style=\"").map(|p| p + depart) {
        let debut = p + 7;
        let Some(fin) = s[debut..].find('"').map(|q| q + debut) else {
            break;
        };
        let blancs = " ".repeat(fin - debut);
        s.replace_range(debut..fin, &blancs);
        depart = fin;
    }
    s
}

/// Nombre de déclarations CSS en ligne de la fixture (celles qu'un navigateur honorerait).
fn declarations(md: &str) -> usize {
    let mut n = 0;
    let mut reste = md;
    while let Some(p) = reste.find("style=\"") {
        let corps = &reste[p + 7..];
        let fin = corps.find('"').unwrap_or(corps.len());
        n += corps[..fin].split(';').filter(|d| d.contains(':')).count();
        reste = &corps[fin..];
    }
    n
}

/// Un constat : (fixture, trait, vrai si le rendu natif le tient).
struct Constat {
    fixture: &'static str,
    trait_: &'static str,
    natif: bool,
}

fn constats() -> Vec<Constat> {
    let mut c = Vec::new();
    let mut note = |fixture: &'static str, trait_: &'static str, natif: bool| {
        c.push(Constat {
            fixture,
            trait_,
            natif,
        });
    };
    let d = |nom: &str| arbre(source(nom));
    // Le CSS en ligne est-il honoré ? (arbre différent avec et sans `style`)
    let style_honore = |nom: &str| d(nom) != arbre(&sans_style(source(nom)));

    let f = "f01-div-stylee";
    let a = d(f);
    note(
        f,
        "texte de la boîte",
        visible(&a).contains("Une boîte verte"),
    );
    note(f, "gras", a.contains("bold: true"));
    note(
        f,
        "couleur, fond, bordure, padding, rayon (CSS en ligne)",
        style_honore(f),
    );

    let f = "f02-svg-statique";
    let a = d(f);
    note(
        f,
        "SVG dessiné (nœud image)",
        a.contains("ImageNode") || a.contains("image: Some"),
    );
    note(
        f,
        "aucun texte du SVG ne fuit dans la note",
        !visible(&a).contains("fin"),
    );

    let f = "f03-svg-smil";
    let a = d(f);
    note(
        f,
        "SVG dessiné (nœud image)",
        a.contains("ImageNode") || a.contains("image: Some"),
    );
    note(f, "animation SMIL qui avance", false);

    let f = "f04-details";
    let a = d(f);
    note(f, "résumé visible", visible(&a).contains("Un détail"));
    note(
        f,
        "contenu masqué tant que fermé",
        !visible(&a).contains("doit rester masqué"),
    );
    note(f, "ouvrable au clic", false);

    let f = "f05-en-ligne";
    let a = d(f);
    note(f, "texte rouge (span style)", style_honore(f));
    note(f, "mark surligné", a.contains("highlight: Some"));
    note(f, "u souligné", a.contains("underline: true"));
    note(f, "sub abaissé", false);
    note(f, "sup relevé", false);
    note(f, "kbd en touche", a.contains("code: true"));
    note(f, "br en retour à la ligne", visible(&a).contains("\\n"));

    let f = "f06-commentaire";
    let a = d(f);
    note(f, "commentaire caché", !visible(&a).contains("remarque"));
    note(
        f,
        "texte autour conservé",
        visible(&a).contains("Avant") && visible(&a).contains("Après"),
    );

    let f = "f07-tableau-html";
    let a = d(f);
    note(f, "table rendue en tableau", a.contains("Table"));
    note(
        f,
        "cellules (texte)",
        ["alpha", "beta", "22"]
            .iter()
            .all(|t| visible(&a).contains(t)),
    );
    note(
        f,
        "fond de cellule, alignement (CSS en ligne)",
        style_honore(f),
    );

    let f = "f08-iframe";
    let a = d(f);
    note(
        f,
        "iframe affichée",
        a.contains("iframe") || a.contains("example.org"),
    );

    let f = "f09-pieges";
    let a = d(f);
    note(
        f,
        "script absent du texte",
        !visible(&a).contains("window.y"),
    );
    note(f, "lien javascript: neutralisé", !a.contains("javascript:"));
    note(
        f,
        "paragraphe conservé",
        visible(&a).contains("Paragraphe piégé"),
    );

    let f = "f10-image";
    let a = d(f);
    note(
        f,
        "image (data:) décodable",
        a.contains("data:image/svg+xml"),
    );
    note(f, "largeur 80 honorée", a.contains("width: Some"));

    let f = "f11-animation-css";
    let a = d(f);
    note(
        f,
        "texte conservé",
        visible(&a).contains("Un texte qui pulse"),
    );
    note(
        f,
        "balise style absente du texte",
        !visible(&a).contains("keyframes"),
    );
    note(f, "animation CSS qui joue", style_honore(f));

    let f = "f12-mise-en-page";
    let a = d(f);
    note(f, "deux colonnes côte à côte (flex)", false);
    note(f, "fonds colorés (CSS en ligne)", style_honore(f));
    note(
        f,
        "texte des deux cases",
        visible(&a).contains("Gauche") && visible(&a).contains("Droite"),
    );
    c
}

/// Tableau fixture × trait : ce que le rendu natif tient. Les `false` écrits en dur (SMIL, ouverture
/// d'un `details`, `sub`/`sup`, flex) sont des limites de l'API (aucun état ni champ pour le dire : voir
/// `TextMark`, `BlockNode`), non d'une lecture de l'arbre.
#[test]
fn rendu_natif_fixture_par_fixture() {
    let mut par_fixture: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for c in constats() {
        println!(
            "S6|natif|{}|{}|{}",
            c.fixture,
            c.trait_,
            if c.natif { "tenu" } else { "perdu" }
        );
        let e = par_fixture.entry(c.fixture).or_default();
        e.1 += 1;
        if c.natif {
            e.0 += 1;
        }
    }
    let (tenus, total) = par_fixture
        .values()
        .fold((0, 0), |a, v| (a.0 + v.0, a.1 + v.1));
    for (f, (t, n)) in &par_fixture {
        println!("S6|natif-score|{f}|{t}/{n}");
    }
    println!("S6|natif-score|TOTAL|{tenus}/{total}");
    for (nom, md) in FIXTURES {
        println!("S6|arbre|{nom}|{}", arbre(md).replace('\n', " "));
    }
}

/// Le CSS en ligne est-il lu par l'analyseur natif ? On compare l'arbre avec et sans `style`.
#[test]
fn le_css_en_ligne_dans_le_natif() {
    let mut total = 0;
    for nom in [
        "f01-div-stylee",
        "f07-tableau-html",
        "f11-animation-css",
        "f12-mise-en-page",
    ] {
        let md = source(nom);
        let n = declarations(md);
        total += n;
        println!(
            "S6|css|{nom}|{n} déclarations|arbre identique sans style : {}",
            arbre(md) == arbre(&sans_style(md))
        );
    }
    println!("S6|css|TOTAL|{total} déclarations");
    assert!(total > 0);
}

/// La politique d'assainissement (portée dans fragment-markdown) coupe les pièges de f09 : c'est elle qui
/// filtre le HTML AVANT le rendu, quel que soit le moteur (natif ou webview).
#[test]
fn la_politique_assainit_les_pieges() {
    assert!(!is_tag_allowed("script"));
    let p = filter_element("p", &[("onclick".into(), "window.x=1".into())]).unwrap();
    assert!(p.is_empty(), "onclick doit disparaître : {p:?}");
    let a = filter_element("a", &[("href".into(), "javascript:window.z=1".into())]).unwrap();
    assert!(
        !a.iter().any(|(k, _)| k == "href"),
        "javascript: doit disparaître : {a:?}"
    );
    assert!(is_tag_allowed("svg"));
}

// ── SVG peint par le rastériseur de GPUI ───────────────────────────────────────────────────────────

/// `SvgRenderer` rastérise au double de la taille logique pour un facteur 1,0 (suréchantillonnage de GPUI) :
/// 0,5 rend la taille déclarée du SVG, au pixel, comme la capture de Chromium à DPR 1.
const ECHELLE: f32 = 0.5;
const FOND: [u8; 3] = [0x1e, 0x1e, 0x1e];
const ACCENT: &str = "#7c83ff";
const TEXTE: &str = "#dcddde";

/// Les variables et `currentColor` du thème, posées dans le source (l'analyseur SVG ignore `var()`).
fn habiller(svg: &str) -> String {
    svg.replace("var(--color-accent)", ACCENT)
        .replace("currentColor", TEXTE)
}

/// Rastérise un SVG par `SvgRenderer` de GPUI (celui qui peint les `ImageFormat::Svg`), composé sur le
/// fond de la page. Rend (largeur, hauteur, RGB).
fn rasteriser(svg: &str) -> (u32, u32, Vec<u8>) {
    let rendu = SvgRenderer::new(Arc::new(()))
        .render_single_frame(habiller(svg).as_bytes(), ECHELLE)
        .expect("svg");
    let taille = rendu.size(0);
    let (l, h) = (taille.width.0 as u32, taille.height.0 as u32);
    let octets = rendu.as_bytes(0).expect("octets");
    let mut rgb = Vec::with_capacity((l * h * 3) as usize);
    for px in octets.as_chunks::<4>().0 {
        // BGRA prémultiplié : sortie = couleur + fond × (1 − alpha).
        let inv = 255 - u32::from(px[3]);
        for (canal, fond) in [(px[2], FOND[0]), (px[1], FOND[1]), (px[0], FOND[2])] {
            rgb.push((u32::from(canal) + u32::from(fond) * inv / 255).min(255) as u8);
        }
    }
    (l, h, rgb)
}

struct Ecart {
    differents: usize,
    au_dela_16: usize,
    total: usize,
    delta_max: u8,
}

fn comparer(rgb: &[u8], reference: &str) -> Ecart {
    let chemin = format!(
        "{}/tests/spike_s6/reference/{reference}",
        env!("CARGO_MANIFEST_DIR")
    );
    let png = image::open(&chemin)
        .unwrap_or_else(|e| panic!("{chemin} : {e}"))
        .to_rgb8();
    assert_eq!(
        png.as_raw().len(),
        rgb.len(),
        "même taille que la référence"
    );
    // Le candidat et la différence (rouge = écart > 16) sont écrits pour être regardés.
    let dossier = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("spike-s6");
    let _ = std::fs::create_dir_all(&dossier);
    let (l, h) = png.dimensions();
    if let Some(cand) = image::RgbImage::from_raw(l, h, rgb.to_vec()) {
        let _ = cand.save(dossier.join(reference));
        let mut diff = png.clone();
        for (d, (a, b)) in diff.pixels_mut().zip(png.pixels().zip(cand.pixels())) {
            let m =
                a.0.iter()
                    .zip(b.0)
                    .map(|(x, y)| x.abs_diff(y))
                    .max()
                    .unwrap_or(0);
            *d = if m > 16 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([
                    m.saturating_mul(8),
                    m.saturating_mul(8),
                    m.saturating_mul(8),
                ])
            };
        }
        let _ = diff.save(dossier.join(format!("diff-{reference}")));
    }
    let mut e = Ecart {
        differents: 0,
        au_dela_16: 0,
        total: rgb.len() / 3,
        delta_max: 0,
    };
    for (a, b) in png
        .as_raw()
        .as_chunks::<3>()
        .0
        .iter()
        .zip(rgb.as_chunks::<3>().0)
    {
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        if d > 0 {
            e.differents += 1;
        }
        if d > 16 {
            e.au_dela_16 += 1;
        }
        e.delta_max = e.delta_max.max(d);
    }
    e
}

fn pourcent(n: usize, total: usize) -> f64 {
    100.0 * n as f64 / total as f64
}

fn svg_de(nom: &str) -> String {
    source(nom).trim().to_owned()
}

#[test]
fn svg_statique_par_le_rasteriseur_de_gpui_contre_chromium() {
    let (l, h, rgb) = rasteriser(&svg_de("f02-svg-statique"));
    assert_eq!((l, h), (160, 60));
    let e = comparer(&rgb, "f02-svg-statique.png");
    println!(
        "S6|svg-statique|{l}x{h}|différents {}/{} ({:.2} %)|au-delà de 16 : {} ({:.2} %)|delta max {}",
        e.differents,
        e.total,
        pourcent(e.differents, e.total),
        e.au_dela_16,
        pourcent(e.au_dela_16, e.total),
        e.delta_max
    );
}

#[test]
fn smil_interprete_contre_chromium() {
    let svg = svg_de("f03-svg-smil");
    let mesures: serde_json::Value =
        serde_json::from_str(include_str!("spike_s6/reference/mesures.json"))
            .expect("mesures.json");
    let anims = smil::lire_animations(&svg);
    assert_eq!(anims.len(), 2, "animate + animateTransform : {anims:?}");
    for (t, cle, cle_json) in [
        (0.5_f32, "0500", "0.5"),
        (1.0, "1000", "1"),
        (1.5, "1500", "1.5"),
    ] {
        let ref_t = &mesures["f03-svg-smil"]["temps"][cle_json];
        let (cx_ref, tx_ref) = (
            ref_t["cx"].as_f64().unwrap() as f32,
            ref_t["tx"].as_f64().unwrap() as f32,
        );
        let cx = smil::evaluer(&anims[0], t)[0];
        let tx = smil::evaluer(&anims[1], t)[0];
        assert!(
            (cx - cx_ref).abs() < 0.01,
            "cx à {t} s : {cx} contre {cx_ref}"
        );
        assert!(
            (tx - tx_ref).abs() < 0.01,
            "tx à {t} s : {tx} contre {tx_ref}"
        );
        let (l, h, rgb) = rasteriser(&smil::figer(&svg, t));
        assert_eq!((l, h), (120, 40));
        let e = comparer(&rgb, &format!("f03-t{cle}.png"));
        println!(
            "S6|smil|t={t}|cx {cx} (Chromium {cx_ref})|tx {tx} (Chromium {tx_ref})|pixels différents {:.2} %|au-delà de 16 : {:.2} %|delta max {}",
            pourcent(e.differents, e.total),
            pourcent(e.au_dela_16, e.total),
            e.delta_max
        );
    }
}

/// Le coût d'une image d'animation SMIL peinte nativement : figer + analyser + rastériser. Budget : 16,7 ms (60 Hz).
#[test]
fn smil_cout_par_image() {
    let svg = svg_de("f03-svg-smil");
    let rendu = SvgRenderer::new(Arc::new(()));
    // Mise en chauffe (polices système, premier parcours).
    let _ = rendu.render_single_frame(habiller(&smil::figer(&svg, 0.0)).as_bytes(), ECHELLE);
    let n = 200;
    let debut = Instant::now();
    for i in 0..n {
        let s = habiller(&smil::figer(&svg, i as f32 * 0.016));
        rendu
            .render_single_frame(s.as_bytes(), ECHELLE)
            .expect("svg");
    }
    let ms = debut.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
    println!(
        "S6|smil-cout|{ms:.3} ms par image (figer + analyser + rastériser, profil {})",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
}
