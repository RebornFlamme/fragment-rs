//! Tests de la barre de recherche PDF (tâche `fr-t4-recherche-ui`), dans une fenêtre de test GPUI
//! (sans GPU), sur un vrai pdfium et un PDF écrit ici octet par octet (aucun PDF de Fragment).
//!
//! Les noms suivent les cas 2 à 6 de `pdf-find.spec.ts` (Ctrl+F ouvre la barre, la frappe cherche
//! et Entrée navigue, Aa et Whole words, surlignage juste à 200 %, Échap ferme et efface), en
//! snake_case et sans accents. Les autres cas sont AJOUTÉS. Le cas 1 du spec (le lien interne) est
//! porté par `pdf_texte.rs`.
//!
//! Écart de geste : les boutons (Aa, ⚙) sont des éléments dessinés dont le gestionnaire appelle la
//! même méthode que les tests ; les touches (Ctrl+F, Entrée, Maj+Entrée, Échap, la frappe) passent
//! par le vrai pont clavier (Keymap, portées empilées, écouteur du champ).
//!
//! pdfium est installé en CI (scripts/pdfium.ps1) ; en local, sans `PDFIUM_LIB_DIR`, les tests
//! sont sautés.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::time::Duration;

use fragment_core::Keymap;
use fragment_text::{DocumentSurface, Rect, TextSurface};
use fragment_ui::clavier;
use fragment_ui::menu;
use fragment_ui::pdf_pages::{FilDeRendu, PagesPdf, RasteriseurPdfium};
use fragment_ui::pdf_recherche::RecherchePdf;
use fragment_ui::pdf_texte::{FilDeTexte, SourceTextePdfium, TextePdf, pos_de_page};
use gpui_kit::{AppContext as _, Entity, TestAppContext, VisualTestContext, px, size};

// ─── La fixture ───

/// Un PDF : une ligne de texte en Helvetica 20 pt par page, en haut à gauche.
fn pdf(textes: &[String], largeur: f64, hauteur: f64) -> Vec<u8> {
    let n = textes.len();
    let obj_page = |i: usize| 4 + 2 * i;
    let mut objets: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {n} >>",
            (0..n)
                .map(|i| format!("{} 0 R", obj_page(i)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ];
    for (i, texte) in textes.iter().enumerate() {
        objets.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {largeur} {hauteur}] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            obj_page(i) + 1
        ));
        let flux = format!("BT /F1 20 Tf 20 {} Td ({texte}) Tj ET", hauteur - 40.0);
        objets.push(format!(
            "<< /Length {} >>\nstream\n{flux}\nendstream",
            flux.len()
        ));
    }
    let mut sortie = String::from("%PDF-1.4\n");
    let mut positions = Vec::new();
    for (i, o) in objets.iter().enumerate() {
        positions.push(sortie.len());
        sortie += &format!("{} 0 obj\n{o}\nendobj\n", i + 1);
    }
    let xref = sortie.len();
    sortie += &format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1);
    for p in positions {
        sortie += &format!("{p:010} 00000 n \n");
    }
    sortie += &format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objets.len() + 1
    );
    sortie.into_bytes()
}

const N: usize = 30;

/// 30 pages. Quatre occurrences de « spin » (casse ignorée) : deux page 3, une page 10, une
/// page 20 ; « seul » n'est que page 20.
fn document() -> Vec<u8> {
    let textes: Vec<String> = (1..=N)
        .map(|n| match n {
            3 => "Spin spins".to_string(),
            10 => "Le SPIN total".to_string(),
            20 => "un spin seul".to_string(),
            n => format!("Page {n}"),
        })
        .collect();
    pdf(&textes, 300.0, 420.0)
}

/// pdfium est-il là ? En CI il DOIT l'être.
fn pdfium() -> bool {
    match fragment_pdf::document::initialiser(None) {
        Ok(()) => true,
        Err(e) => {
            assert!(
                std::env::var_os("CI").is_none(),
                "pdfium absent en CI : {e}"
            );
            eprintln!("pdfium absent, test sauté : {e}");
            false
        }
    }
}

// ─── Le banc ───

struct Banc {
    vue: Entity<RecherchePdf>,
    pages: Entity<PagesPdf>,
    texte: Entity<TextePdf>,
    keymap: Keymap,
}

fn banc(cx: &mut TestAppContext) -> (Banc, &mut VisualTestContext) {
    let octets = document();
    let rendu = RasteriseurPdfium::ouvrir(octets.clone()).unwrap();
    let couche = SourceTextePdfium::ouvrir(octets.clone()).unwrap();
    let recherche = SourceTextePdfium::ouvrir(octets).unwrap();
    let mut sources = Some((rendu, couche, recherche));
    let keymap = Keymap::new();
    let km = keymap.clone();
    let (vue, vcx) = cx.add_window_view(move |_, cx| {
        let (rendu, couche, recherche) = sources.take().unwrap();
        let pages = cx.new(|cx| PagesPdf::new(FilDeRendu::en_ligne(Box::new(rendu)), cx));
        let texte = cx.new(|cx| {
            TextePdf::new(pages, FilDeTexte::en_ligne(Box::new(couche)), cx)
        });
        RecherchePdf::new(
            texte,
            FilDeTexte::en_ligne(Box::new(recherche)),
            km.clone(),
            cx,
        )
    });
    vcx.simulate_resize(size(px(1000.0), px(800.0)));
    // Le pont clavier de la fenêtre, et la portée de la vue (Ctrl+F) que le shell empile.
    let km = keymap.clone();
    vcx.update(|w, cx| clavier::brancher(w, cx, km, None));
    keymap.push_scope(&vue.read_with(vcx, |v, _| v.portee_vue()));
    let texte = vue.read_with(vcx, |v, _| v.texte().clone());
    let pages = texte.read_with(vcx, |t, _| t.pages().clone());
    (
        Banc {
            vue,
            pages,
            texte,
            keymap,
        },
        vcx,
    )
}

fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}

fn stabiliser(vcx: &mut VisualTestContext) {
    for _ in 0..80 {
        vcx.executor().advance_clock(Duration::from_millis(5));
        vcx.run_until_parked();
        image(vcx);
    }
}

fn compteur(b: &Banc, vcx: &mut VisualTestContext) -> String {
    b.vue.read_with(vcx, |v, _| v.barre().compteur())
}

fn page(b: &Banc, vcx: &mut VisualTestContext) -> usize {
    b.pages.read_with(vcx, |p, _| p.modele().page_vue())
}

fn aller_a(b: &Banc, vcx: &mut VisualTestContext, numero: usize) {
    b.pages
        .update(vcx, |p, cx| p.agir(cx, |m| m.aller_a_la_page(numero, None)));
    stabiliser(vcx);
}

/// Ctrl+F puis une requête : le geste d'un utilisateur.
fn chercher(b: &Banc, vcx: &mut VisualTestContext, requete: &str) {
    vcx.simulate_keystrokes("ctrl-f");
    stabiliser(vcx);
    vcx.simulate_input(requete);
    stabiliser(vcx);
}

/// Les rects du résultat courant, en px de la fenêtre : (gauche, haut, droite, bas).
fn courants_en_fenetre(b: &Banc, vcx: &mut VisualTestContext) -> Vec<Rect> {
    let rects = b.vue.read_with(vcx, |v, cx| v.rects_surlignage(cx));
    let surface = b.texte.read_with(vcx, |t, _| t.surface());
    rects
        .into_iter()
        .filter(|r| r.courant)
        .map(|r| {
            let (g, h) = surface.vers_fenetre(r.rect.left, r.rect.top);
            let (d, bas) = surface.vers_fenetre(r.rect.right, r.rect.bottom);
            Rect {
                left: g,
                top: h,
                right: d,
                bottom: bas,
            }
        })
        .collect()
}

/// Ouvre le menu des réglages et clique l'entrée `titre`, puis laisse le rendu rejouer le choix.
fn choisir(b: &Banc, vcx: &mut VisualTestContext, titre: &str) {
    let m = b.vue.read_with(vcx, |v, _| v.barre().menu_options());
    let idx = m
        .forme()
        .iter()
        .position(|t| t == titre)
        .unwrap_or_else(|| panic!("pas d'entrée « {titre} »"));
    m.show_at_position(10.0, 10.0, &b.keymap);
    assert!(menu::cliquer(0, idx));
    stabiliser(vcx);
}

// ───────────────────────── pdf-find.spec.ts ─────────────────────────

#[test]
fn ctrl_f_ouvre_la_barre_le_champ_a_le_focus() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    stabiliser(vcx);
    assert!(!b.vue.read_with(vcx, |v, _| v.barre().ouverte()));

    vcx.simulate_keystrokes("ctrl-f");
    stabiliser(vcx);
    assert!(b.vue.read_with(vcx, |v, _| v.barre().ouverte()));
    let focus = b.vue.read_with(vcx, |v, _| v.focus().clone());
    assert!(vcx.update(|w, _| focus.is_focused(w)), "le champ a le focus");
}

#[test]
fn la_frappe_cherche_dans_tout_le_document_entree_et_maj_entree_naviguent() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    // On part de la page 15 : le premier résultat après elle est page 20.
    aller_a(&b, vcx, 15);
    chercher(&b, vcx, "spin");
    assert_eq!(compteur(&b, vcx), "4 of 4");
    assert_eq!(page(&b, vcx), 20);
    assert!(
        !courants_en_fenetre(&b, vcx).is_empty(),
        "le résultat courant est surligné"
    );

    // Entrée boucle : le premier.
    vcx.simulate_keystrokes("enter");
    stabiliser(vcx);
    assert_eq!(compteur(&b, vcx), "1 of 4");
    assert_eq!(page(&b, vcx), 3);

    // Maj+Entrée boucle en arrière : le dernier.
    vcx.simulate_keystrokes("shift-enter");
    stabiliser(vcx);
    assert_eq!(compteur(&b, vcx), "4 of 4");
}

#[test]
fn aa_respecte_la_casse_whole_words_menu_ne_garde_que_les_mots() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    aller_a(&b, vcx, 15);
    chercher(&b, vcx, "spin");
    assert_eq!(compteur(&b, vcx), "4 of 4");

    // Aa : « spins » (page 3) et « spin » (page 20).
    b.vue.update(vcx, |v, cx| v.basculer_casse(cx));
    stabiliser(vcx);
    assert!(compteur(&b, vcx).ends_with(" of 2"), "{}", compteur(&b, vcx));

    // Whole words (menu) : « spin » seul.
    choisir(&b, vcx, "Whole words");
    assert_eq!(compteur(&b, vcx), "1 of 1");

    // Ctrl+F dans le champ le sélectionne : la frappe suivante le remplace.
    vcx.simulate_keystrokes("ctrl-f");
    vcx.simulate_input("introuvable");
    stabiliser(vcx);
    assert_eq!(compteur(&b, vcx), "No results");
    assert!(b.vue.read_with(vcx, |v, _| v.barre().aucun_resultat()));
}

#[test]
fn a_200_pour_cent_la_couche_de_texte_est_a_l_echelle_le_surlignage_couvre_le_bon_mot() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    b.pages
        .update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(2.0)));
    stabiliser(vcx);
    let echelle = b.pages.read_with(vcx, |p, _| p.modele().echelle());
    assert!((echelle - 2.0).abs() < 0.01, "échelle {echelle}");

    // « un spin seul », page 20 : « seul » finit la ligne.
    chercher(&b, vcx, "seul");
    assert_eq!(compteur(&b, vcx), "1 of 1");
    assert_eq!(page(&b, vcx), 20);
    let hl = courants_en_fenetre(&b, vcx);
    assert_eq!(hl.len(), 1, "une rangée surlignée");
    let hl = hl[0];

    // Le texte de la page 20 : « un spin seul » (12 caractères) ; « seul » est le dernier mot.
    let surface = b.texte.read_with(vcx, |t, _| t.surface());
    let glyphe = |i: usize| surface.coords_at_pos(pos_de_page(20) + 1 + i).unwrap();
    let (debut, fin) = (glyphe(0), glyphe(11));
    let (ligne_g, _) = surface.vers_fenetre(debut.left, debut.top);
    let (ligne_d, _) = surface.vers_fenetre(fin.right, fin.top);

    // Largeur de « seul » en Helvetica 20 pt (s 500, e 556, u 556, l 222) à 200 % : 73,4 px.
    let attendu = (500.0 + 556.0 + 556.0 + 222.0) / 1000.0 * 20.0 * 2.0;
    let reel = hl.right - hl.left;
    assert!(
        (reel - attendu).abs() / attendu < 0.10,
        "largeur {reel} px, attendu {attendu} px"
    );
    // « seul » termine la ligne : le surlignage finit avec elle, et commence bien après son début.
    assert!((hl.right - ligne_d).abs() < 4.0, "fin {} / {}", hl.right, ligne_d);
    assert!(hl.left - ligne_g > (ligne_d - ligne_g) * 0.5);
}

#[test]
fn echap_ferme_la_barre_et_efface_les_surlignages() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    chercher(&b, vcx, "spin");
    let c = compteur(&b, vcx);
    assert!(
        c.split(" of ").count() == 2 && c.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "compteur « {c} »"
    );
    assert!(!courants_en_fenetre(&b, vcx).is_empty());

    vcx.simulate_keystrokes("escape");
    stabiliser(vcx);
    assert!(!b.vue.read_with(vcx, |v, _| v.barre().ouverte()));
    assert!(b.vue.read_with(vcx, |v, cx| v.rects_surlignage(cx)).is_empty());
    for p in 1..=N {
        assert!(
            b.vue
                .read_with(vcx, |v, _| v.barre().surlignages_page(p).is_empty()),
            "page {p}"
        );
    }
}

// ───────────────────────── Cas ajoutés ─────────────────────────

#[test]
fn highlight_all_surligne_toutes_les_occurrences_visibles() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    // La page 3 : « Spin spins », deux occurrences sur une rangée.
    chercher(&b, vcx, "spin");
    assert_eq!(compteur(&b, vcx), "1 of 4");
    assert_eq!(page(&b, vcx), 3);
    let sur_page_3 = |b: &Banc, vcx: &mut VisualTestContext| {
        let surface = b.texte.read_with(vcx, |t, _| t.surface());
        let rects = b.vue.read_with(vcx, |v, cx| v.rects_surlignage(cx));
        let coin = surface.coords_at_pos(pos_de_page(3)).unwrap();
        let bas = surface.coords_at_pos(pos_de_page(4)).unwrap();
        rects
            .into_iter()
            .filter(|r| r.rect.top >= coin.top && r.rect.top < bas.top)
            .count()
    };
    assert_eq!(sur_page_3(&b, vcx), 1, "le courant seul");

    choisir(&b, vcx, "Highlight all");
    assert_eq!(sur_page_3(&b, vcx), 2, "les deux occurrences de la page");
    // Highlight all ne déplace rien.
    assert_eq!(compteur(&b, vcx), "1 of 4");
    assert_eq!(page(&b, vcx), 3);
}

#[test]
fn la_frappe_est_temporisee_puis_cherche() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    // Le texte du document est lu à l'ouverture.
    vcx.simulate_keystrokes("ctrl-f");
    stabiliser(vcx);
    assert!(b.vue.read_with(vcx, |v, _| v.prete()));
    vcx.simulate_input("seul");
    // Avant le délai de la frappe, rien n'a été cherché.
    vcx.executor().advance_clock(Duration::from_millis(50));
    vcx.run_until_parked();
    image(vcx);
    assert_eq!(compteur(&b, vcx), "");
    stabiliser(vcx);
    assert_eq!(compteur(&b, vcx), "1 of 1");

    // Un champ vidé remet le compteur à zéro, aussitôt.
    vcx.simulate_keystrokes("backspace backspace backspace backspace");
    image(vcx);
    assert_eq!(compteur(&b, vcx), "");
}

#[test]
fn rouvrir_la_barre_relance_la_requete_gardee() {
    let mut cx = TestAppContext::single();
    let cx = &mut cx;
    if !pdfium() {
        return;
    }
    let (b, vcx) = banc(cx);
    chercher(&b, vcx, "seul");
    assert_eq!(compteur(&b, vcx), "1 of 1");
    vcx.simulate_keystrokes("escape");
    stabiliser(vcx);
    assert!(b.vue.read_with(vcx, |v, cx| v.rects_surlignage(cx)).is_empty());

    vcx.simulate_keystrokes("ctrl-f");
    stabiliser(vcx);
    assert_eq!(compteur(&b, vcx), "1 of 1");
    assert!(!courants_en_fenetre(&b, vcx).is_empty());
}
