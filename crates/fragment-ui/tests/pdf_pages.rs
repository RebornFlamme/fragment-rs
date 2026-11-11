//! Tests de la colonne des pages PDF (tâche `fr-t4-pages`), dans une fenêtre de test GPUI (sans GPU).
//!
//! Les noms suivent les cas e2e de Fragment, en snake_case et sans accents : `pdf-nettete.spec.ts`
//! (un pixel de raster par pixel physique) et le cas pinch de `pdf-toolbar.spec.ts` (aucun repeint
//! pendant le geste, un repeint final). Les autres cas sont AJOUTÉS (virtualisation, fil de rendu).
//!
//! Le rastériseur est un FAUX en ligne (horloge simulée, déterministe) qui journalise chaque rendu ;
//! deux tests passent par un vrai fil (dont un par pdfium, sur un PDF écrit ici octet par octet).

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

use fragment_pdf::document::{Raster, TaillePage};
use fragment_pdf::vue_modele::{DELAI_FIN_DE_GESTE, Pixels};
use fragment_ui::echelle::Echelle;
use fragment_ui::pdf_pages::{
    ESPACE_PAGES, FilDeRendu, MARGE_HAUTE, PagesPdf, Rasteriseur, RasteriseurPdfium,
};
use gpui_kit::{
    Entity, Modifiers, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px, size,
};

// ─── Un rastériseur factice ───

type Journal = Arc<Mutex<Vec<(usize, Pixels, ThreadId)>>>;

struct Faux {
    tailles: Vec<TaillePage>,
    journal: Journal,
}

impl Rasteriseur for Faux {
    fn tailles(&mut self) -> Result<Vec<TaillePage>, String> {
        Ok(self.tailles.clone())
    }

    fn rendre(&mut self, page: usize, pixels: Pixels) -> Result<Raster, String> {
        self.journal
            .lock()
            .unwrap()
            .push((page, pixels, thread::current().id()));
        Ok(Raster {
            largeur: pixels.w,
            hauteur: pixels.h,
            octets: vec![255; (pixels.w * pixels.h * 4) as usize],
        })
    }
}

fn faux(n: usize) -> (Faux, Journal) {
    let journal = Journal::default();
    (
        Faux {
            tailles: vec![
                TaillePage {
                    largeur: 612.0,
                    hauteur: 792.0
                };
                n
            ],
            journal: Arc::clone(&journal),
        },
        journal,
    )
}

// ─── Le banc ───

fn fenetre(
    cx: &mut TestAppContext,
    fil: FilDeRendu,
    largeur: f32,
) -> (Entity<PagesPdf>, &mut VisualTestContext) {
    let mut fil = Some(fil);
    let (pages, vcx) = cx.add_window_view(move |_, cx| PagesPdf::new(fil.take().unwrap(), cx));
    vcx.simulate_resize(size(px(largeur), px(800.0)));
    (pages, vcx)
}

/// Une image : dessin, puis ce qu'il a déclenché (mesures, demandes).
fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}

/// Le temps passe de `ms`, une image est dessinée.
fn attendre(vcx: &mut VisualTestContext, ms: u64) {
    vcx.executor().advance_clock(Duration::from_millis(ms));
    vcx.run_until_parked();
    image(vcx);
}

/// Laisse tout se poser : rendus récoltés, fins de geste et recalages passés.
fn stabiliser(vcx: &mut VisualTestContext) {
    for _ in 0..80 {
        attendre(vcx, 5);
    }
}

/// Toutes les pages peintes ont EXACTEMENT la taille physique de leur boîte. Rend le nombre de
/// pages vérifiées.
fn verifier_net(pages: &Entity<PagesPdf>, vcx: &mut VisualTestContext, quoi: &str) -> usize {
    pages.read_with(vcx, |p, _| {
        let occupees = p.pages_occupees();
        assert!(!occupees.is_empty(), "{quoi} : aucune page peinte");
        for page in &occupees {
            let e = p.etat_images(*page);
            assert!(!e.en_cours, "{quoi} : page {page} encore en rendu");
            assert_eq!(e.perimee, None, "{quoi} : page {page} encore périmée");
            assert_eq!(
                e.courante,
                p.mesure(*page),
                "{quoi} : page {page} étirée (image ≠ boîte physique)"
            );
        }
        occupees.len()
    })
}

fn rendus(journal: &Journal) -> usize {
    journal.lock().unwrap().len()
}

fn molette(vcx: &mut VisualTestContext, x: f32, y: f32, dy: f32, ctrl: bool) {
    vcx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(dy))),
        modifiers: Modifiers {
            control: ctrl,
            ..Modifiers::default()
        },
        ..ScrollEvent::default()
    });
}

// ─── pdf-nettete.spec.ts ───

#[test]
fn densites_zoom_de_l_app_x_echelles_du_pdf() {
    let mut cx = TestAppContext::single();
    let (r, _) = faux(12);
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::en_ligne(Box::new(r)), 1000.0);
    stabiliser(vcx);
    let mut ancienne_fausse = 0;
    for facteur in [2.0_f32, 1.5] {
        vcx.simulate_scale_factor_change(facteur);
        for z in [1.0, 0.85, 1.1] {
            vcx.update(|_, cx| Echelle::installer(cx, z));
            for e in [1.0, 1.37, 2.05] {
                pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(e)));
                stabiliser(vcx);
                let quoi = format!("facteur {facteur}, zoom de l'app {z}, échelle {e}");
                verifier_net(&pages, vcx, &quoi);
                // La densité livrée au modèle est bien facteur × zoom de l'app.
                let d = pages.read_with(vcx, |p, _| p.densite());
                assert!(
                    (d - f64::from(facteur) * z).abs() < 1e-6,
                    "{quoi} : densité {d}"
                );
                // Contre-épreuve : floor(largeur logique × facteur) se trompe parfois.
                ancienne_fausse += pages.read_with(vcx, |p, _| {
                    let g = p.modele().geometrie();
                    p.pages_occupees()
                        .into_iter()
                        .filter(|&page| {
                            let l = (g.largeur_px(page - 1) * z) as f32;
                            let h = (g.hauteur_px(page - 1) * z) as f32;
                            let ancienne = Pixels {
                                w: (l * facteur).floor() as u32,
                                h: (h * facteur).floor() as u32,
                            };
                            p.mesure(page) != Some(ancienne)
                        })
                        .count()
                });
            }
        }
    }
    assert!(
        ancienne_fausse > 0,
        "floor(largeur × facteur) aurait étiré au moins une image"
    );
}

#[test]
fn la_page_glisse_d_une_fraction_de_pixel_fenetre_recalee() {
    let mut cx = TestAppContext::single();
    let (r, journal) = faux(12);
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::en_ligne(Box::new(r)), 1000.0);
    vcx.simulate_scale_factor_change(1.5);
    vcx.update(|_, cx| Echelle::installer(cx, 0.85));
    pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(1.37)));
    stabiliser(vcx);
    verifier_net(&pages, vcx, "avant");
    // Les trois largeurs de l'e2e, puis deux fractions de pixel (le centrage glisse d'un demi-pixel).
    for largeur in [1001.0, 1203.0, 1377.0, 1377.5, 1378.25] {
        let (avant, mesures) = (
            rendus(&journal),
            pages.read_with(vcx, |p, _| {
                p.pages_occupees()
                    .into_iter()
                    .map(|page| p.mesure(page))
                    .collect::<Vec<_>>()
            }),
        );
        vcx.simulate_resize(size(px(largeur), px(800.0)));
        stabiliser(vcx);
        verifier_net(&pages, vcx, &format!("fenêtre {largeur} px"));
        let e = pages.read_with(vcx, |p, _| p.modele().echelle());
        assert!((e - 1.37).abs() < 1e-9, "l'échelle n'a pas bougé : {e}");
        // Une boîte qui a changé de taille physique a été repeinte (recalée), sinon rien.
        let apres = pages.read_with(vcx, |p, _| {
            p.pages_occupees()
                .into_iter()
                .map(|page| p.mesure(page))
                .collect::<Vec<_>>()
        });
        if apres != mesures {
            assert!(
                rendus(&journal) > avant,
                "fenêtre {largeur} px : aucune page recalée"
            );
        }
    }
}

// ─── pdf-toolbar.spec.ts ───

#[test]
fn pinch_ctrl_molette_zoom_continu_ancre_sous_le_curseur_un_repeint_final() {
    let mut cx = TestAppContext::single();
    let (r, journal) = faux(12);
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::en_ligne(Box::new(r)), 1000.0);
    pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(1.0)));
    stabiliser(vcx);
    verifier_net(&pages, vcx, "avant le geste");
    let (e0, avant) = (
        pages.read_with(vcx, |p, _| p.modele().echelle()),
        rendus(&journal),
    );

    // Rafale de petits deltas = un pinch de trackpad (GPUI : y positif = vers le haut = zoom).
    for _ in 0..12 {
        molette(vcx, 400.0, 300.0, 8.0, true);
        attendre(vcx, 10);
        // Pendant le geste : aucun rendu, et aucune page blanche (image gardée, étirée).
        assert_eq!(rendus(&journal), avant, "un rendu pendant le geste");
        pages.read_with(vcx, |p, _| {
            assert!(p.modele().en_geste());
            for page in p.pages_occupees() {
                let e = p.etat_images(page);
                assert!(
                    e.courante.is_some() || e.perimee.is_some(),
                    "page {page} blanche pendant le geste"
                );
            }
        });
    }
    let e1 = pages.read_with(vcx, |p, _| p.modele().echelle());
    assert!(e1 > e0 * 1.05, "le pinch zoome : {e0} → {e1}");

    // Fin du geste : chaque page repeinte, l'ancienne image dessous jusqu'à la nouvelle. On avance
    // PILE jusqu'à la fin du geste (10 ms sont déjà passées depuis le dernier pas) : la récolte
    // suivante n'est pas encore due.
    vcx.executor()
        .advance_clock(Duration::from_millis(DELAI_FIN_DE_GESTE - 10));
    vcx.run_until_parked();
    image(vcx);
    pages.read_with(vcx, |p, _| {
        assert!(!p.modele().en_geste());
        let occupees = p.pages_occupees();
        assert!(occupees.iter().any(|&page| {
            let e = p.etat_images(page);
            e.perimee.is_some() && e.courante.is_none() && e.en_cours
        }));
        // Une page en repeint compte comme dessinée.
        let plages = p.plages_dessinees();
        for page in occupees {
            let pos = (page - 1) * 1_000_000;
            assert!(plages.iter().any(|r| r.from <= pos && pos <= r.to));
        }
    });
    stabiliser(vcx);
    verifier_net(&pages, vcx, "après le geste");
    // UN repeint final : chaque page rendue une seule fois depuis le début du geste.
    let mut par_page: BTreeMap<usize, usize> = BTreeMap::new();
    for (page, _, _) in journal.lock().unwrap()[avant..].iter() {
        *par_page.entry(*page).or_default() += 1;
    }
    assert!(!par_page.is_empty());
    assert!(
        par_page.values().all(|&n| n == 1),
        "un seul repeint par page : {par_page:?}"
    );

    // Ctrl + molette vers le bas : dézoome.
    molette(vcx, 400.0, 300.0, -300.0, true);
    stabiliser(vcx);
    let e2 = pages.read_with(vcx, |p, _| p.modele().echelle());
    assert!(e2 < e1 * 0.9, "dézoom : {e1} → {e2}");
}

// ─── Ajoutés : virtualisation ───

#[test]
fn pages_liberees_hors_bande_et_rendus_annules() {
    let mut cx = TestAppContext::single();
    let (r, journal) = faux(40);
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::en_ligne(Box::new(r)), 1000.0);
    pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(1.0)));
    // La zone, l'ouverture (récoltée), puis les mesures : les demandes partent, pas encore rendues.
    image(vcx);
    attendre(vcx, 5);
    assert!(pages.read_with(vcx, |p, _| p.rendus_en_cours()));
    let premieres = pages.read_with(vcx, |p, _| p.pages_occupees());
    assert!(premieres.contains(&1));
    // Saut loin dans le document avant que les rendus ne reviennent.
    pages.update(vcx, |p, cx| {
        let haut = p.modele().geometrie().top_px(29);
        p.agir(cx, |m| m.defiler(haut, 0.0));
    });
    stabiliser(vcx);
    verifier_net(&pages, vcx, "après le saut");
    pages.read_with(vcx, |p, _| {
        let (a, b) = p.modele().pages_visibles().unwrap();
        for page in p.pages_occupees() {
            assert!((a..=b).contains(&page), "page {page} hors bande gardée");
        }
        for page in &premieres {
            assert_eq!(
                p.etat_images(*page),
                Default::default(),
                "page {page} libérée"
            );
        }
    });
    // Les rendus des premières pages ont été annulés avant d'être faits.
    assert!(
        journal
            .lock()
            .unwrap()
            .iter()
            .all(|(page, _, _)| !premieres.contains(page)),
        "un rendu annulé a été fait"
    );
}

#[test]
fn pages_qui_entrent_peintes_au_defilement_sans_attendre() {
    let mut cx = TestAppContext::single();
    let (r, _) = faux(40);
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::en_ligne(Box::new(r)), 1000.0);
    pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(1.0)));
    stabiliser(vcx);
    for _ in 0..15 {
        molette(vcx, 400.0, 300.0, -250.0, false);
        // Deux images et une récolte (moins que la fin d'un geste) : la zone visible est peinte.
        image(vcx);
        attendre(vcx, 5);
        pages.read_with(vcx, |p, _| {
            assert!(!p.modele().en_geste(), "le défilement n'est pas un geste");
            let m = p.modele();
            let g = m.geometrie();
            let (haut, bas) = (m.scroll_top(), m.scroll_top() + m.zone().hauteur);
            for i in 0..m.nombre_pages() {
                let (t, b) = (g.top_px(i), g.top_px(i) + g.hauteur_px(i));
                if b > haut && t < bas {
                    assert!(
                        p.etat_images(i + 1).courante.is_some(),
                        "page {} visible sans image à {haut}",
                        i + 1
                    );
                }
            }
        });
    }
    assert!(pages.read_with(vcx, |p, _| p.modele().scroll_top()) > 2000.0);
}

#[test]
fn espacements_du_css_entre_les_pages() {
    let mut cx = TestAppContext::single();
    let (r, _) = faux(3);
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::en_ligne(Box::new(r)), 1000.0);
    pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(1.0)));
    stabiliser(vcx);
    pages.read_with(vcx, |p, _| {
        let g = p.modele().geometrie();
        assert_eq!(g.top_px(0), MARGE_HAUTE);
        assert!((g.top_px(1) - (MARGE_HAUTE + g.hauteur_px(0) + ESPACE_PAGES)).abs() < 1.0 / 64.0);
    });
}

// ─── Ajoutés : le fil de rendu ───

/// Attend en temps RÉEL (le fil n'obéit pas à l'horloge simulée) que tout soit peint.
fn attendre_fil(pages: &Entity<PagesPdf>, vcx: &mut VisualTestContext) {
    for _ in 0..2000 {
        attendre(vcx, 5);
        let pret = pages.read_with(vcx, |p, _| {
            p.erreur().is_none()
                && !p.rendus_en_cours()
                && !p.pages_occupees().is_empty()
                && p.pages_occupees()
                    .iter()
                    .all(|&page| p.etat_images(page).courante.is_some())
        });
        if pret {
            return;
        }
        assert_eq!(
            pages.read_with(vcx, |p, _| p.erreur().map(str::to_owned)),
            None
        );
        thread::sleep(Duration::from_millis(2));
    }
    panic!("le fil de rendu n'a pas fini");
}

#[test]
fn rendu_hors_du_fil_de_l_interface() {
    let mut cx = TestAppContext::single();
    let (r, journal) = faux(6);
    let fil = FilDeRendu::demarrer(Box::new(move || Ok(Box::new(r) as Box<dyn Rasteriseur>)));
    let (pages, vcx) = fenetre(&mut cx, fil, 1000.0);
    attendre_fil(&pages, vcx);
    stabiliser(vcx);
    attendre_fil(&pages, vcx);
    verifier_net(&pages, vcx, "fil dédié");
    let ici = thread::current().id();
    let j = journal.lock().unwrap();
    assert!(!j.is_empty());
    assert!(
        j.iter().all(|(_, _, fil)| *fil != ici),
        "rendu sur le fil de l'interface"
    );
}

/// Un PDF minimal écrit ici : des pages nues de tailles données, un rectangle noir chacune.
fn pdf(tailles: &[(f64, f64)]) -> Vec<u8> {
    let n = tailles.len();
    let mut objets: Vec<String> = Vec::new();
    objets.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    let enfants: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    objets.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {n} >>",
        enfants.join(" ")
    ));
    for (i, (l, h)) in tailles.iter().enumerate() {
        objets.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {l} {h}] /Contents {} 0 R >>",
            4 + 2 * i
        ));
        let flux = "0 0 0 rg 20 20 60 40 re f";
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

#[test]
fn pdfium_rend_chaque_page_a_sa_taille_physique() {
    // Le binaire épinglé est installé en CI (scripts/pdfium.ps1) ; en local il peut manquer.
    if let Err(e) = fragment_pdf::document::initialiser(None) {
        assert!(
            std::env::var_os("CI").is_none(),
            "pdfium absent en CI : {e}"
        );
        eprintln!("pdfium absent, test sauté : {e}");
        return;
    }
    let octets = pdf(&[(300.0, 400.0), (300.0, 400.0), (420.0, 297.0)]);
    // Le document s'ouvre aussi sur ce fil : la fixture est lisible.
    assert!(RasteriseurPdfium::ouvrir(octets.clone()).is_ok());
    let mut cx = TestAppContext::single();
    let (pages, vcx) = fenetre(&mut cx, FilDeRendu::pdfium(octets), 1000.0);
    vcx.simulate_scale_factor_change(1.5);
    vcx.update(|_, cx| Echelle::installer(cx, 0.85));
    pages.update(vcx, |p, cx| p.agir(cx, |m| m.choisir_pourcentage(1.37)));
    attendre_fil(&pages, vcx);
    stabiliser(vcx);
    attendre_fil(&pages, vcx);
    let n = verifier_net(&pages, vcx, "pdfium");
    assert_eq!(n, 3);
    // La page 3 est en paysage : sa taille vient du document, pas de la page 1.
    pages.read_with(vcx, |p, _| {
        let g = p.modele().geometrie();
        assert!(g.largeur_px(2) > g.hauteur_px(2));
    });
}
