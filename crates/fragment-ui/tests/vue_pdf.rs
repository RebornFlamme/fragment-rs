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

// ─── Le champ de page : caret et frappe routée (pdf-toolbar.spec.ts, `.pdf-page-input`) ───

/// Monte la vue avec le pont clavier de la fenêtre (Entrée et Échap passent par le Keymap).
fn monter_clavier<'a>(
    cx: &'a mut TestAppContext,
    b: &Banc,
    vue: &VuePdf,
) -> (Entity<MontagePdf>, &'a mut VisualTestContext) {
    let (m, vcx) = monter(cx, vue, 1000.0);
    let km = b.keymap.clone();
    vcx.update(|w, cx| fragment_ui::clavier::brancher(w, cx, km, None));
    (m, vcx)
}

fn champ(m: &Entity<MontagePdf>, vcx: &mut VisualTestContext) -> (String, bool, bool) {
    m.read_with(vcx, |m, cx| {
        let c = m.pages().unwrap().read(cx).modele().champ().clone();
        (c.valeur, c.focus, c.tout_selectionne)
    })
}

fn focaliser_champ(m: &Entity<MontagePdf>, vcx: &mut VisualTestContext) {
    let f = m.read_with(vcx, |m, _| m.focus_champ().clone());
    vcx.update(|w, cx| w.focus(&f, cx));
    stabiliser(vcx, m);
}

/// `fill('4')` puis `Enter` : la vue va à la page 4 (le champ rend la page courante).
#[gpui_kit::test]
fn saisir_un_numero_puis_entree_va_a_la_page(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter_clavier(cx, &b, &vue);
    focaliser_champ(&m, vcx);
    // Au focus, tout est sélectionné : la frappe remplace, puis le caret suit le texte.
    assert_eq!(champ(&m, vcx), ("1".to_string(), true, true));
    vcx.simulate_input("4");
    assert_eq!(champ(&m, vcx), ("4".to_string(), true, false));
    vcx.simulate_keystrokes("enter");
    stabiliser(vcx, &m);
    assert_eq!(page_vue(&m, vcx), 4);
    assert_eq!(champ(&m, vcx).0, "4");
}

/// Un numéro hors bornes est ramené à `1..=N` ; retour arrière efface.
#[gpui_kit::test]
fn un_numero_hors_bornes_est_borne_et_retour_arriere_efface(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter_clavier(cx, &b, &vue);
    focaliser_champ(&m, vcx);
    vcx.simulate_input("99");
    vcx.simulate_keystrokes("backspace");
    assert_eq!(champ(&m, vcx).0, "9");
    vcx.simulate_input("9");
    vcx.simulate_keystrokes("enter");
    stabiliser(vcx, &m);
    assert_eq!(page_vue(&m, vcx), N);
}

/// Échap restaure la page courante et rend le focus à la vue.
#[gpui_kit::test]
fn echap_restaure_la_page_et_rend_le_focus(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter_clavier(cx, &b, &vue);
    focaliser_champ(&m, vcx);
    vcx.simulate_input("7");
    vcx.simulate_keystrokes("escape");
    stabiliser(vcx, &m);
    assert_eq!(champ(&m, vcx), ("1".to_string(), false, false));
    let a_le_focus = m.update_in(vcx, |m, w, _| m.focus_champ().is_focused(w));
    assert!(!a_le_focus);
    assert_eq!(page_vue(&m, vcx), 1);
}

/// Perdre le focus après une saisie vaut validation (l'événement `change` du navigateur).
#[gpui_kit::test]
fn perdre_le_focus_apres_une_saisie_valide(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter_clavier(cx, &b, &vue);
    focaliser_champ(&m, vcx);
    vcx.simulate_input("3");
    vcx.update(|w, cx| w.blur(cx));
    stabiliser(vcx, &m);
    assert_eq!(page_vue(&m, vcx), 3);
    assert!(!champ(&m, vcx).1);
}

// ─── Le menu contextuel de la vue (PdfView.onPaneMenu : Zoom avant / Zoom arrière) ───

/// Clic droit sur les pages : « Zoom avant » et « Zoom arrière » ; chacun zoome d'un palier.
#[gpui_kit::test]
fn le_menu_de_la_vue_propose_zoom_avant_et_arriere(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    let b = banc();
    let vue = b.vue();
    let (m, vcx) = monter_clavier(cx, &b, &vue);
    let avant = echelle(&m, vcx);
    vcx.simulate_mouse_down(
        gpui_kit::point(px(500.0), px(400.0)),
        gpui_kit::MouseButton::Right,
        gpui_kit::Modifiers::default(),
    );
    assert!(fragment_ui::menu::menu_ouvert());
    assert_eq!(
        fragment_ui::menu::forme_ouverte(0),
        ["Zoom avant", "Zoom arrière"]
    );
    let i = fragment_ui::menu::index_ouvert(0, "Zoom avant").unwrap();
    assert!(fragment_ui::menu::cliquer(0, i));
    stabiliser(vcx, &m);
    let plus = echelle(&m, vcx);
    assert!(plus > avant, "zoom avant ({avant} → {plus})");
    vcx.simulate_mouse_down(
        gpui_kit::point(px(500.0), px(400.0)),
        gpui_kit::MouseButton::Right,
        gpui_kit::Modifiers::default(),
    );
    let i = fragment_ui::menu::index_ouvert(0, "Zoom arrière").unwrap();
    assert!(fragment_ui::menu::cliquer(0, i));
    stabiliser(vcx, &m);
    assert!(echelle(&m, vcx) < plus);
    fragment_ui::menu::fermer_menu();
}

// ─── La mesure mémoire (fr-7va) : un Document pdfium par vue, ouvert sur son fil ───

/// Le jeu de travail du processus, en Mo (PowerShell sous Windows, `/proc` ailleurs).
fn memoire_mo() -> f64 {
    #[cfg(windows)]
    {
        let sortie = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {}).WorkingSet64", std::process::id()),
            ])
            .output()
            .expect("powershell");
        let o: f64 = String::from_utf8_lossy(&sortie.stdout)
            .trim()
            .parse()
            .expect("octets");
        o / 1_048_576.0
    }
    #[cfg(not(windows))]
    {
        let s = std::fs::read_to_string("/proc/self/status").expect("proc");
        let ko: f64 = s
            .lines()
            .find_map(|l| l.strip_prefix("VmRSS:"))
            .and_then(|l| l.trim().trim_end_matches("kB").trim().parse().ok())
            .expect("VmRSS");
        ko / 1024.0
    }
}

/// Un PDF de 300 pages : mémoire avant l'ouverture, après l'ouverture, après un défilement
/// complet. Lancé à la main (`-- --ignored --nocapture`) ; les chiffres vont dans
/// `docs/mesures/memoire-pdf.md`. Le seuil large n'attrape qu'une fuite grossière.
#[gpui_kit::test]
#[ignore = "mesure, pas un test : cargo test -p fragment-ui --test vue_pdf mesure -- --ignored --nocapture"]
fn mesure_memoire_pdf_300_pages(cx: &mut TestAppContext) {
    if !pdfium() {
        return;
    }
    const PAGES: usize = 300;
    let disque = Rc::new(MemoryAdapter::new());
    let contenu = pdf(PAGES, 420.0, 600.0);
    let taille_pdf = contenu.len() as f64 / 1_048_576.0;
    disque.put("long.pdf", &contenu);
    let a: Rc<dyn DataAdapter> = disque;
    let vault = Vault::new(a);
    vault.load().unwrap();
    let b = Banc {
        vault,
        app: AppCoeur::new(),
        keymap: Keymap::new(),
    };
    let leaf = b
        .app
        .workspace()
        .borrow_mut()
        .get_leaf(NouvelleFeuille::Onglet)
        .unwrap();
    let mut vue = VuePdf::new(
        b.keymap.root_scope(),
        ContexteFichier {
            leaf,
            vault: b.vault.clone(),
            workspace_events: Events::new(),
        },
        b.keymap.clone(),
    );
    vue.on_open();
    let avant = memoire_mo();
    vue.load_file(Some(b.vault.get_file_by_path("long.pdf").unwrap()));
    let liens = vue.liens().clone();
    let (m, vcx) = cx.add_window_view(move |_, cx| MontagePdf::new(liens.clone(), cx));
    vcx.simulate_resize(size(px(1000.0), px(800.0)));
    for _ in 0..200 {
        std::thread::sleep(Duration::from_millis(2));
        vcx.executor().advance_clock(Duration::from_millis(5));
        vcx.run_until_parked();
        image(vcx);
    }
    let n = m.read_with(vcx, |m, cx| {
        m.pages().map_or(0, |p| p.read(cx).modele().nombre_pages())
    });
    assert_eq!(n, PAGES);
    let ouvert = memoire_mo();
    // Un défilement complet : on saute de page en page jusqu'à la dernière.
    let mut pic = ouvert;
    for p in (1..=PAGES).step_by(5) {
        m.update(vcx, |m, cx| m.agir(cx, |v| v.aller_a_la_page(p, None)));
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(2));
            vcx.executor().advance_clock(Duration::from_millis(10));
            vcx.run_until_parked();
            image(vcx);
        }
        if p % 50 == 1 {
            pic = pic.max(memoire_mo());
        }
    }
    let apres = memoire_mo();
    eprintln!(
        "MESURE pdf {taille_pdf:.2} Mo, {PAGES} pages : avant {avant:.1} Mo, ouvert {ouvert:.1} Mo, \
         pic échantillonné {pic:.1} Mo, après défilement complet {apres:.1} Mo"
    );
    assert!(
        apres - avant < 2000.0,
        "fuite grossière : {} Mo",
        apres - avant
    );
}

// ─── Le menu « ⋯ » de l'en-tête (PdfView.onPaneMenu) : le hook de menu de pane du cœur ───

/// La vue PDF contribue « Zoom avant » / « Zoom arrière » dans la section `view`, après celles de
/// l'ouvreur ; un choix dépose le raccourci de zoom dans la file de la vue.
#[test]
fn le_menu_plus_d_options_contient_zoom_avant_et_arriere_dans_la_section_view() {
    use fragment_core::{MenuPane, menu_de_pane};
    let b = banc();
    let vue = b.vue();
    let modele = menu_de_pane(&vue, "more-options", |m: &mut MenuPane| {
        m.add_item(|i| {
            i.set_title("Diviser à droite").set_section("pane");
        });
    });
    let mut menu = fragment_ui::menu::Menu::new();
    fragment_ui::entete::remplir_depuis_pane(&mut menu, &modele);
    assert_eq!(
        menu.forme(),
        ["Diviser à droite", "|", "Zoom avant", "Zoom arrière"]
    );
    let e = &modele.entrees()[1];
    assert_eq!((e.section.as_str(), e.icone.as_str()), ("view", "zoom-in"));
    (e.action.clone().unwrap())();
    assert_eq!(
        vue.liens().touches.borrow().back().copied(),
        Some(fragment_ui::vue_pdf::Touche::Zoom(
            fragment_pdf::vue_modele::Raccourci::ZoomPlus
        ))
    );
}
