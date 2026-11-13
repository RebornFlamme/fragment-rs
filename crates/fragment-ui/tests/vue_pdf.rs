//! Tests de la vue PDF montée (tâche `fr-t4-vue-pdf`), dans une fenêtre de test GPUI (sans GPU),
//! sur un vrai pdfium et un PDF écrit ici (aucun PDF de Fragment).
//!
//! Les noms suivent `pdf-toolbar.spec.ts` (test 1 : la barre d'outils sous l'en-tête, sans zoom
//! dans l'en-tête ; « Page width suit la largeur de la zone ») et le cas PDF de
//! `scroll-persistence.spec.ts` (autre onglet puis retour, rebuildView, recharger), en snake_case.
//! Les autres cas sont AJOUTÉS (état éphémère complet, garde d'extension, taille 0 ignorée).
//!
//! ★ Les fils sont de vrais fils (document partagé, rendu, texte) : la boucle d'attente fait
//!   avancer l'horloge de test ET laisse le temps réel aux fils.
//!
//! pdfium est installé en CI (scripts/pdfium.ps1) ; en local, sans `PDFIUM_LIB_DIR`, les tests
//! sont sautés.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::rc::Rc;
use std::time::Duration;

use fragment_core::workspace::vue_fichier::{ContexteFichier, FileView};
use fragment_core::{
    App as _, AppCoeur, DataAdapter, Events, Keymap, MemoryAdapter, NouvelleFeuille, Vault, View,
};
use fragment_pdf::pdf_zoom::ZoomMode;
use fragment_ui::pdf_barre::{Bouton, LIBELLES_BOUTONS};
use fragment_ui::vue_pdf::{MontagePdf, VuePdf};
use gpui_kit::{Entity, TestAppContext, VisualTestContext, px, size};
use serde_json::Value;

// ─── La fixture : un PDF de N pages, « Feuille n » en haut de chacune ───

const N: usize = 12;

fn pdf(n: usize, largeur: f64, hauteur: f64) -> String {
    let page = |i: usize| 4 + 2 * i;
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", page(i))).collect();
    let mut objets = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    for i in 0..n {
        objets.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {largeur} {hauteur}] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            page(i) + 1
        ));
        let flux = format!(
            "BT /F1 18 Tf 30 {} Td (Feuille {}) Tj ET",
            hauteur - 50.0,
            i + 1
        );
        objets.push(format!(
            "<< /Length {} >>\nstream\n{flux}\nendstream",
            flux.len()
        ));
    }
    let mut s = String::from("%PDF-1.4\n");
    let mut decalages = Vec::new();
    for (i, o) in objets.iter().enumerate() {
        decalages.push(s.len());
        s += &format!("{} 0 obj\n{o}\nendobj\n", i + 1);
    }
    let xref = s.len();
    s += &format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1);
    for d in decalages {
        s += &format!("{d:010} 00000 n \n");
    }
    s += &format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objets.len() + 1
    );
    s
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
    vault: Rc<Vault>,
    app: Rc<AppCoeur>,
    keymap: Keymap,
}

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("long.pdf", &pdf(N, 420.0, 600.0));
    disque.put("note.md", "# Note");
    let a: Rc<dyn DataAdapter> = disque;
    let vault = Vault::new(a);
    vault.load().unwrap();
    Banc {
        vault,
        app: AppCoeur::new(),
        keymap: Keymap::new(),
    }
}

impl Banc {
    fn vue(&self) -> VuePdf {
        let leaf = self
            .app
            .workspace()
            .borrow_mut()
            .get_leaf(NouvelleFeuille::Onglet)
            .unwrap();
        let mut v = VuePdf::new(
            self.keymap.root_scope(),
            ContexteFichier {
                leaf,
                vault: self.vault.clone(),
                workspace_events: Events::new(),
            },
            self.keymap.clone(),
        );
        v.on_open();
        v.load_file(Some(self.vault.get_file_by_path("long.pdf").unwrap()));
        v
    }
}

/// Monte la vue dans une fenêtre de `largeur` × 800.
fn monter<'a>(
    cx: &'a mut TestAppContext,
    vue: &VuePdf,
    largeur: f32,
) -> (Entity<MontagePdf>, &'a mut VisualTestContext) {
    let liens = vue.liens().clone();
    let (m, vcx) = cx.add_window_view(move |_, cx| MontagePdf::new(liens.clone(), cx));
    vcx.simulate_resize(size(px(largeur), px(800.0)));
    stabiliser(vcx, &m);
    (m, vcx)
}

fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}

/// Laisse tourner l'horloge de test et les vrais fils jusqu'à ce que les pages soient là.
fn stabiliser(vcx: &mut VisualTestContext, m: &Entity<MontagePdf>) {
    for _ in 0..200 {
        std::thread::sleep(Duration::from_millis(2));
        vcx.executor().advance_clock(Duration::from_millis(5));
        vcx.run_until_parked();
        image(vcx);
    }
    let n = m.read_with(vcx, |m, cx| {
        m.pages().map_or(0, |p| p.read(cx).modele().nombre_pages())
    });
    assert_eq!(n, N, "les pages du document sont ouvertes");
}

fn echelle(m: &Entity<MontagePdf>, vcx: &mut VisualTestContext) -> f64 {
    m.read_with(vcx, |m, cx| m.pages().unwrap().read(cx).modele().echelle())
}

fn page_vue(m: &Entity<MontagePdf>, vcx: &mut VisualTestContext) -> usize {
    m.read_with(vcx, |m, cx| m.pages().unwrap().read(cx).modele().page_vue())
}

// ─── pdf-toolbar.spec.ts ───

/// Test 1 : l'en-tête ne porte plus le zoom ; la barre d'outils (36 px) est sous lui, et la zone
/// des pages commence sous la barre.
#[gpui_kit::test]
fn le_header_ne_porte_plus_le_zoom_la_barre_d_outils_est_sous_lui(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter(cx, &vue, 1000.0);
    assert_eq!(
        LIBELLES_BOUTONS,
        [
            "Toggle sidebar",
            "Sidebar options",
            "Zoom out",
            "Zoom in",
            "Zoom options",
            "Page"
        ]
    );
    // La zone des pages = la fenêtre moins la barre d'outils.
    let hauteur = m.read_with(vcx, |m, cx| {
        m.pages().unwrap().read(cx).modele().zone().hauteur
    });
    assert!(
        (hauteur - (800.0 - 36.0)).abs() < 0.5,
        "la zone des pages commence sous la barre de 36 px (hauteur {hauteur})"
    );
}

/// « Page width » suit la largeur de la zone : ouvrir la barre latérale rétrécit la page.
#[gpui_kit::test]
fn page_width_suit_la_largeur_de_la_zone_barre_laterale_ouverte(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter(cx, &vue, 1000.0);
    m.update(vcx, |m, cx| {
        m.agir(cx, |v| v.choisir_mode(ZoomMode::PageWidth))
    });
    stabiliser(vcx, &m);
    let fermee = echelle(&m, vcx) * 420.0;
    m.update(vcx, |m, cx| m.cliquer(Bouton::BasculerBarre, 0.0, 0.0, cx));
    stabiliser(vcx, &m);
    assert!(m.read_with(vcx, |m, _| m.barre().barre_ouverte()));
    assert!(m.read_with(vcx, |m, _| m.laterale().is_some()));
    let ouverte = echelle(&m, vcx) * 420.0;
    assert!(
        ouverte < fermee - 50.0,
        "la page rétrécit avec la barre latérale ({fermee} → {ouverte})"
    );
}

// ─── scroll-persistence.spec.ts (cas PDF) ───

/// Autre onglet puis retour, rebuildView, recharger : la même page, la même hauteur dans la page,
/// par l'état éphémère rendu par une vue et appliqué à une vue neuve.
#[gpui_kit::test]
fn pdf_autre_onglet_puis_retour_rebuildview_recharger(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let premiere = b.vue();
    let etat = {
        let (m, vcx) = monter(cx, &premiere, 1000.0);
        m.update(vcx, |m, cx| {
            m.agir(cx, |v| v.aller_a_la_page(7, Some(300.0)))
        });
        stabiliser(vcx, &m);
        assert_eq!(page_vue(&m, vcx), 7);
        premiere.get_ephemeral_state()
    };
    let scroll = etat.get("scroll").cloned().expect("position de lecture");
    assert_eq!(scroll.get("page"), Some(&Value::from(7)));

    let mut seconde = b.vue();
    seconde.set_ephemeral_state(&Value::Object(etat.clone()));
    // Pas encore montée : la vue rend l'état reçu, sans le perdre.
    assert_eq!(seconde.get_ephemeral_state(), etat);
    let (m, vcx) = monter(cx, &seconde, 1000.0);
    assert_eq!(page_vue(&m, vcx), 7);
    assert_eq!(seconde.get_ephemeral_state().get("scroll"), Some(&scroll));
}

// ─── Ajoutés ───

/// L'état éphémère complet : page, zoom, barre latérale, recherche, position de lecture ; et
/// `getState` ne porte que le fichier.
#[gpui_kit::test]
fn etat_ephemere_complet_et_etat_persistant_reduit_au_fichier(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter(cx, &vue, 1000.0);
    m.update(vcx, |m, cx| {
        m.agir(cx, |v| v.choisir_mode(ZoomMode::PageFit))
    });
    stabiliser(vcx, &m);
    let e = vue.get_ephemeral_state();
    for cle in ["page", "scale", "zoomMode", "sidebar", "find", "scroll"] {
        assert!(e.contains_key(cle), "clé {cle} dans {e:?}");
    }
    assert_eq!(e["zoomMode"], "page-fit");
    assert_eq!(e["sidebar"]["open"], false);
    assert_eq!(e["find"]["caseSensitive"], false);
    let s = vue.get_state();
    assert_eq!(s.keys().collect::<Vec<_>>(), ["file"]);

    // Restauré sur une vue neuve : mode, barre latérale ouverte en table des matières, Aa.
    let mut etat = e.clone();
    etat["sidebar"] = serde_json::json!({ "open": true, "mode": "outline", "width": 240.0 });
    etat["find"]["caseSensitive"] = Value::Bool(true);
    let mut autre = b.vue();
    autre.set_ephemeral_state(&Value::Object(etat));
    let (m, vcx) = monter(cx, &autre, 1000.0);
    let r = autre.get_ephemeral_state();
    assert_eq!(r["zoomMode"], "page-fit");
    assert_eq!(r["sidebar"]["open"], true);
    assert_eq!(r["sidebar"]["mode"], "outline");
    assert_eq!(r["find"]["caseSensitive"], true);
    assert!(m.read_with(vcx, |m, _| m.laterale().is_some()));
}

/// Taille 0 ignorée quand la vue est cachée : le zoom « Page width » ne s'écrase pas.
#[gpui_kit::test]
fn taille_zero_ignoree_quand_la_vue_est_cachee(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter(cx, &vue, 1000.0);
    m.update(vcx, |m, cx| {
        m.agir(cx, |v| v.choisir_mode(ZoomMode::PageWidth))
    });
    stabiliser(vcx, &m);
    let avant = echelle(&m, vcx);
    m.update(vcx, |m, cx| m.agir(cx, |v| v.zone_redimensionnee(0.0, 0.0)));
    stabiliser(vcx, &m);
    assert!((echelle(&m, vcx) - avant).abs() < 1e-9);
}

/// Le garde d'extension : seule une restauration de PDF s'ouvre ici.
#[test]
fn n_accepte_que_l_extension_pdf() {
    let b = banc();
    let vue = b.vue();
    assert!(vue.can_accept_extension("pdf"));
    assert!(!vue.can_accept_extension("md"));
    assert_eq!(vue.get_view_type(), "pdf");
}
