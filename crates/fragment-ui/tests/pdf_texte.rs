//! Tests de la couche texte PDF (tâche `fr-t4-texte`), dans une fenêtre de test GPUI (sans GPU),
//! sur un vrai pdfium et des PDF écrits ici octet par octet (aucun PDF de Fragment).
//!
//! Les noms suivent les cas e2e de Fragment, en snake_case et sans accents : `pdf-layers.spec.ts`
//! (surface et plan, texte sélectionnable, sélection en offsets et rects, pointeur capté seulement
//! si un outil est armé) et `pdf-find.spec.ts` (un lien interne mène à sa page). Les autres cas
//! sont AJOUTÉS (coordonnées à toutes les échelles, lien externe, page libérée).
//!
//! pdfium est installé en CI (scripts/pdfium.ps1) ; en local, sans `PDFIUM_LIB_DIR`, les tests
//! sont sautés.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use fragment_text::{DocumentSurface, EditorRange, Rect, TextSurface, has_text};
use fragment_ui::echelle::Echelle;
use fragment_ui::pdf_pages::{FilDeRendu, PagesPdf, RasteriseurPdfium};
use fragment_ui::pdf_texte::{
    FilDeTexte, PhasePointeur, PointeurPdf, SourceTextePdfium, SurfacePdf, TextePdf, pos_de_page,
};
use gpui_kit::{
    AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px,
    size,
};

// ─── La fixture ───

enum Cible {
    Page(usize),
    Uri(&'static str),
}

/// Un PDF : une ligne de texte en Helvetica 20 pt par page (en haut à gauche), et au plus un
/// lien par page, posé sur le mot « Lien » en bas à gauche (rect PDF [15 50 100 80]).
fn pdf(textes: &[&str], largeur: f64, hauteur: f64, liens: &[(usize, Cible)]) -> Vec<u8> {
    let n = textes.len();
    let obj_page = |i: usize| 4 + 3 * i;
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
        let lien = liens.iter().find(|(p, _)| *p == i + 1).map(|(_, c)| c);
        let annots = if lien.is_some() {
            format!(" /Annots [{} 0 R]", obj_page(i) + 2)
        } else {
            String::new()
        };
        objets.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {largeur} {hauteur}] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R{annots} >>",
            obj_page(i) + 1
        ));
        let mut flux = format!("BT /F1 20 Tf 20 {} Td ({texte}) Tj ET", hauteur - 40.0);
        if lien.is_some() {
            flux += " BT /F1 14 Tf 20 60 Td (Lien) Tj ET";
        }
        objets.push(format!(
            "<< /Length {} >>\nstream\n{flux}\nendstream",
            flux.len()
        ));
        objets.push(match lien {
            None => "null".into(),
            Some(c) => {
                let action = match c {
                    Cible::Page(p) => format!("/Dest [{} 0 R /XYZ 0 {hauteur} 0]", obj_page(p - 1)),
                    Cible::Uri(u) => format!("/A << /S /URI /URI ({u}) >>"),
                };
                format!(
                    "<< /Type /Annot /Subtype /Link /Rect [15 50 100 80] /Border [0 0 0] {action} >>"
                )
            }
        });
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

/// Les textes de pdf-layers : assez de pages pour que la virtualisation opère.
fn mots() -> Vec<String> {
    std::iter::once("Alpha Un".to_string())
        .chain((2..=30).map(|i| format!("Page {i} Mot")))
        .collect()
}

fn pdf_layers() -> Vec<u8> {
    let m = mots();
    let t: Vec<&str> = m.iter().map(String::as_str).collect();
    pdf(&t, 200.0, 280.0, &[])
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

fn fenetre(cx: &mut TestAppContext, octets: Vec<u8>) -> (Entity<TextePdf>, &mut VisualTestContext) {
    let rendu = RasteriseurPdfium::ouvrir(octets.clone()).unwrap();
    let texte = SourceTextePdfium::ouvrir(octets).unwrap();
    let mut sources = Some((rendu, texte));
    let (vue, vcx) = cx.add_window_view(move |_, cx| {
        let (rendu, texte) = sources.take().unwrap();
        let pages = cx.new(|cx| PagesPdf::new(FilDeRendu::en_ligne(Box::new(rendu)), cx));
        TextePdf::new(pages, FilDeTexte::en_ligne(Box::new(texte)), cx)
    });
    vcx.simulate_resize(size(px(1000.0), px(800.0)));
    (vue, vcx)
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

fn surface(vue: &Entity<TextePdf>, vcx: &mut VisualTestContext) -> SurfacePdf {
    vue.read_with(vcx, |v, _| v.surface())
}

fn pages(vue: &Entity<TextePdf>, vcx: &mut VisualTestContext) -> Entity<PagesPdf> {
    vue.read_with(vcx, |v, _| v.pages().clone())
}

/// Un point d'un rect du plan, en fraction de sa largeur et de sa hauteur, dans la fenêtre.
fn dans(s: &SurfacePdf, r: Rect, fx: f64, fy: f64) -> (f32, f32) {
    let (x, y) = s.vers_fenetre(
        r.left + (r.right - r.left) * fx,
        r.top + (r.bottom - r.top) * fy,
    );
    (x as f32, y as f32)
}

/// Un glisser du bouton gauche de `a` à `b`.
fn glisser(vcx: &mut VisualTestContext, a: (f32, f32), b: (f32, f32)) {
    let m = Modifiers::default();
    vcx.simulate_mouse_down(point(px(a.0), px(a.1)), MouseButton::Left, m);
    vcx.simulate_mouse_move(
        point(px((a.0 + b.0) / 2.0), px((a.1 + b.1) / 2.0)),
        MouseButton::Left,
        m,
    );
    vcx.simulate_mouse_move(point(px(b.0), px(b.1)), MouseButton::Left, m);
    vcx.simulate_mouse_up(point(px(b.0), px(b.1)), MouseButton::Left, m);
    image(vcx);
}

fn cliquer(vcx: &mut VisualTestContext, a: (f32, f32)) {
    vcx.simulate_click(point(px(a.0), px(a.1)), Modifiers::default());
    image(vcx);
}

/// Le glyphe du caractère `i` de la page `page` (unités du plan).
fn glyphe(s: &SurfacePdf, page: usize, i: usize) -> Rect {
    s.coords_at_pos(pos_de_page(page) + 1 + i).unwrap()
}

// ─── pdf-layers ───

#[test]
fn la_vue_pdf_fournit_une_surface_de_document_et_son_plan() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf_layers());
    stabiliser(vcx);
    let s = surface(&vue, vcx);
    // La surface PDF a du texte : le garde has_text est vrai, et une « ligne » est une page.
    assert!(has_text(&s));
    assert_eq!(s.line_count(), 30);
    // Le plan : la position d'une page est son coin, lu dans la géométrie du modèle.
    let p = pages(&vue, vcx);
    p.read_with(vcx, |p, _| {
        let g = p.modele().geometrie();
        for page in [1, 2, 30] {
            let coin = g.coin(page - 1);
            let r = s.coords_at_pos(pos_de_page(page)).unwrap();
            assert_eq!((r.left, r.top), (coin.left, coin.top), "page {page}");
        }
        // Le plan, ramené dans la fenêtre, tombe sur la boîte peinte de la page.
        let r = s.coords_at_pos(pos_de_page(2)).unwrap();
        let (x, y) = s.vers_fenetre(r.left, r.top);
        let m = p.modele();
        assert!((x - (g.left_px(1) - m.scroll_left())).abs() < 1e-9);
        assert!((y - (g.top_px(1) - m.scroll_top())).abs() < 1e-9);
    });
}

#[test]
fn la_couche_de_texte_rend_le_pdf_selectionnable() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf_layers());
    stabiliser(vcx);
    assert!(vue.read_with(vcx, |v, _| v.texte_pret(1)));
    let s = surface(&vue, vcx);
    assert!(s.get_line(0).contains("Alpha"), "{:?}", s.get_line(0));
    assert!(s.get_line(1).contains("Page 2 Mot"), "{:?}", s.get_line(1));
    // Le premier caractère a une vraie boîte, DANS sa page.
    let coin = s.coords_at_pos(0).unwrap();
    let a = glyphe(&s, 1, 0);
    assert!(a.right - a.left > 1.0 && a.bottom - a.top > 1.0);
    assert!(a.left >= coin.left && a.right <= coin.right && a.top >= coin.top);
    // Le point du glyphe rend sa position.
    let (x, y) = dans(&s, a, 0.25, 0.5);
    assert_eq!(s.pos_at_coords(f64::from(x), f64::from(y)), Some(1));
}

#[test]
fn une_selection_de_texte_se_lit_en_offsets_et_rend_des_rects_de_surlignage() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf_layers());
    stabiliser(vcx);
    let s = surface(&vue, vcx);
    // Du début du premier caractère à la fin du cinquième : « Alpha ».
    let debut = dans(&s, glyphe(&s, 1, 0), 0.2, 0.5);
    let fin = dans(&s, glyphe(&s, 1, 4), 0.8, 0.5);
    glisser(vcx, debut, fin);
    let plage = s.get_selection();
    // Page 1, cinq premiers caractères : offsets 1..6 dans le bloc de la page.
    assert_eq!(plage, EditorRange { from: 1, to: 6 });
    assert_eq!(s.texte_selectionne(), "Alpha");
    let rects = s.coords_for_range(plage.from, plage.to);
    assert!(!rects.is_empty());
    // Un rect réel, pas un point.
    assert!(rects[0].right - rects[0].left > 1.0);
    assert!(rects[0].bottom - rects[0].top > 1.0);
    // Le rect couvre les glyphes sélectionnés.
    assert!(rects[0].left <= glyphe(&s, 1, 0).left + 1e-9);
    assert!(rects[0].right >= glyphe(&s, 1, 4).right - 1e-9);
    // Un clic hors du texte efface la sélection.
    let coin = s.coords_at_pos(0).unwrap();
    cliquer(vcx, dans(&s, coin, 0.5, 0.95));
    assert_eq!(s.get_selection(), EditorRange::default());
}

#[test]
fn la_surface_ne_capte_le_pointeur_que_si_un_outil_est_arme() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf_layers());
    stabiliser(vcx);
    let s = surface(&vue, vcx);
    let recus: Rc<RefCell<Vec<PointeurPdf>>> = Rc::default();
    {
        let recus = Rc::clone(&recus);
        vcx.update(|_, cx| {
            cx.subscribe(&vue, move |_, ev: &PointeurPdf, _| {
                recus.borrow_mut().push(*ev)
            })
            .detach();
        });
    }
    let debut = dans(&s, glyphe(&s, 2, 0), 0.2, 0.5);
    let fin = dans(&s, glyphe(&s, 2, 3), 0.8, 0.5);

    // Au repos : le pointeur va au texte (une sélection), rien n'est émis.
    assert!(!vue.read_with(vcx, |v, _| v.capte_pointeur()));
    glisser(vcx, debut, fin);
    assert_eq!(s.get_selection().from, pos_de_page(2) + 1);
    assert!(recus.borrow().is_empty());

    // Outil armé : la sélection disparaît, le geste va à l'outil.
    vue.update(vcx, |v, cx| v.armer_outil(true, cx));
    assert!(vue.read_with(vcx, |v, _| v.capte_pointeur()));
    glisser(vcx, debut, fin);
    assert_eq!(s.get_selection(), EditorRange::default());
    let r = recus.borrow();
    assert_eq!(r.first().map(|e| e.phase), Some(PhasePointeur::Bas));
    assert_eq!(r.last().map(|e| e.phase), Some(PhasePointeur::Haut));
    assert!(r.iter().any(|e| e.phase == PhasePointeur::Deplace));
    // L'ancre tombe dans le bloc d'offsets de la page 2.
    for e in r.iter() {
        let pos = e.pos.unwrap();
        assert!((1_000_000..2_000_000).contains(&pos), "{pos}");
    }
    drop(r);

    // Désarmé : le texte reprend la main.
    vue.update(vcx, |v, cx| v.armer_outil(false, cx));
    assert!(!vue.read_with(vcx, |v, _| v.capte_pointeur()));
    glisser(vcx, debut, fin);
    assert_eq!(s.get_selection().from, pos_de_page(2) + 1);
}

// ─── pdf-find ───

#[test]
fn un_lien_interne_mene_a_sa_page() {
    if !pdfium() {
        return;
    }
    let textes: Vec<String> = (1..=30).map(|n| format!("Page {n}")).collect();
    let t: Vec<&str> = textes.iter().map(String::as_str).collect();
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf(&t, 300.0, 420.0, &[(1, Cible::Page(15))]));
    stabiliser(vcx);
    let s = surface(&vue, vcx);
    // Le lien est sur le mot « Lien » : le début du texte après « Page 1 ».
    let coin = s.coords_at_pos(0).unwrap();
    let centre = Rect {
        left: coin.left + 15.0,
        top: coin.top + 420.0 - 80.0,
        right: coin.left + 100.0,
        bottom: coin.top + 420.0 - 50.0,
    };
    cliquer(vcx, dans(&s, centre, 0.5, 0.5));
    stabiliser(vcx);
    let p = pages(&vue, vcx);
    let champ = p.read_with(vcx, |p, _| p.modele().champ().valeur.clone());
    assert_eq!(champ, "15");
    // Un lien n'est pas du texte : le clic n'a rien sélectionné.
    assert_eq!(s.get_selection(), EditorRange::default());
}

// ─── Ajoutés ───

#[test]
fn coords_at_pos_et_pos_at_coords_a_toutes_les_echelles() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf_layers());
    vcx.simulate_scale_factor_change(1.5);
    vcx.update(|_, cx| Echelle::installer(cx, 0.85));
    let p = pages(&vue, vcx);
    let mut local: Option<Rect> = None;
    for e in [0.5, 1.0, 1.37, 2.0] {
        p.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(e)));
        stabiliser(vcx);
        let s = surface(&vue, vcx);
        let coin = s.coords_at_pos(0).unwrap();
        let g = glyphe(&s, 1, 2);
        // Le glyphe, relatif à sa page, ne bouge pas au zoom.
        let rel = Rect {
            left: g.left - coin.left,
            top: g.top - coin.top,
            right: g.right - coin.left,
            bottom: g.bottom - coin.top,
        };
        match local {
            None => local = Some(rel),
            Some(l) => assert!(
                (l.left - rel.left).abs() < 1e-9 && (l.top - rel.top).abs() < 1e-9,
                "échelle {e} : {l:?} ≠ {rel:?}"
            ),
        }
        // Aller-retour : le quart gauche du glyphe rend sa position, le quart droit la suivante.
        let (x, y) = dans(&s, g, 0.25, 0.5);
        assert_eq!(
            s.pos_at_coords(f64::from(x), f64::from(y)),
            Some(3),
            "échelle {e}"
        );
        let (x, y) = dans(&s, g, 0.75, 0.5);
        assert_eq!(
            s.pos_at_coords(f64::from(x), f64::from(y)),
            Some(4),
            "échelle {e}"
        );
        // Un blanc de la page : l'ancre de la page.
        let (x, y) = dans(&s, coin, 0.9, 0.9);
        assert_eq!(
            s.pos_at_coords(f64::from(x), f64::from(y)),
            Some(0),
            "échelle {e}"
        );
    }
}

#[test]
fn un_lien_externe_ouvre_le_navigateur_du_systeme() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let octets = pdf(
        &["Accueil", "Suite"],
        300.0,
        420.0,
        &[(1, Cible::Uri("https://example.org/doc"))],
    );
    let (vue, vcx) = fenetre(&mut cx, octets);
    stabiliser(vcx);
    let s = surface(&vue, vcx);
    let coin = s.coords_at_pos(0).unwrap();
    let lien = Rect {
        left: coin.left + 15.0,
        top: coin.top + 340.0,
        right: coin.left + 100.0,
        bottom: coin.top + 370.0,
    };
    cliquer(vcx, dans(&s, lien, 0.5, 0.5));
    assert_eq!(vcx.opened_url().as_deref(), Some("https://example.org/doc"));
    // La vue n'a pas bougé.
    let p = pages(&vue, vcx);
    assert_eq!(p.read_with(vcx, |p, _| p.modele().scroll_top()), 0.0);
}

#[test]
fn une_page_liberee_perd_son_texte_mais_garde_ses_glyphes() {
    if !pdfium() {
        return;
    }
    let mut cx = TestAppContext::single();
    let (vue, vcx) = fenetre(&mut cx, pdf_layers());
    stabiliser(vcx);
    let s = surface(&vue, vcx);
    let avant = glyphe(&s, 1, 0);
    assert_ne!(avant, s.coords_at_pos(0).unwrap());
    // Loin en bas : la page 1 est libérée.
    let p = pages(&vue, vcx);
    p.update(vcx, |p, cx| p.agir(cx, |m| m.defiler(1.0e6, 0.0)));
    stabiliser(vcx);
    assert!(!p.read_with(vcx, |p, _| p.pages_occupees().contains(&1)));
    assert_eq!(s.get_line(0), "");
    assert!(!s.rendered_ranges().iter().any(|r| r.from == 0));
    // Le glyphe mémorisé reste : un trait ancré sur un mot ne saute pas au coin de la page.
    assert_eq!(glyphe(&s, 1, 0), avant);
    assert!(s.get_line(29).contains("Page 30"));
}
