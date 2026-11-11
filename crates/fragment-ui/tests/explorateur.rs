//! Tests de l'explorateur de fichiers (tâche `fr-t2-explorateur`).
//!
//! Le TS n'a qu'un test e2e de cette vue : « les dossiers OUVERTS se rouvrent au redémarrage (et
//! pas les autres) » (`scroll-persistence.spec.ts`), porté ici sous le même nom en snake_case.
//! Les autres sont des cas AJOUTÉS, d'après le comportement de `FileTree.tsx` : tri, ouvrir /
//! fermer, fichier actif, révéler, renommage extérieur, ouverture au clic. Fixtures réécrites.
//!
//! Le coffre est un vrai `Vault` sur adaptateur mémoire ; les clics passent par la vraie mise en
//! page (fenêtre de test GPUI sans GPU, bornes relevées par `debug_selector`).

// Fichier de test : les assistants hors `#[test]` déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::workspace::vue_fichier::{REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{
    ChangeType, DataAdapter, DocumentKey, Events, FILE_OPEN, ItemView as _, Keymap, MemoryAdapter,
    TFile, Vault, View as _,
};
use fragment_ui::explorateur::{
    ContexteExplorateur, FILE_EXPLORER_VIEW_TYPE, FileExplorerView, comparer_noms,
    rendre_explorateur,
};
use gpui_kit::{
    Context, Entity, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, px,
};
use serde_json::json;

// ─── La fenêtre de test : l'explorateur dans un panneau de 260 × 200 ───

struct Panneau {
    vue: FileExplorerView,
}

impl Render for Panneau {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(260.0))
            .h(px(200.0))
            .child(rendre_explorateur(&self.vue, cx))
    }
}

/// Tout ce qu'un test touche : le disque, le coffre, le flux du Workspace, ce que l'app a reçu.
struct Banc {
    disque: Rc<MemoryAdapter>,
    vault: Rc<Vault>,
    evenements: Events,
    ouverts: Rc<RefCell<Vec<String>>>,
    sauvegardes: Rc<RefCell<u32>>,
}

/// Deux dossiers (`Dossier A` > `Sous-dossier`, `Dossier B`) et des fichiers, comme le test e2e.
fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("Dossier A/Sous-dossier/profonde.md", "profonde");
    disque.put("Dossier A/a.md", "a");
    disque.put("Dossier B/b.md", "b");
    disque.put("racine.md", "r");
    disque.put("image.png", "png");
    let adaptateur: Rc<dyn DataAdapter> = disque.clone();
    let vault = Vault::new(adaptateur);
    vault.load().expect("chargement");
    Banc {
        disque,
        vault,
        evenements: Events::new(),
        ouverts: Rc::default(),
        sauvegardes: Rc::default(),
    }
}

impl Banc {
    fn vue(&self) -> FileExplorerView {
        let (ouverts, sauvegardes) = (self.ouverts.clone(), self.sauvegardes.clone());
        let vue = FileExplorerView::new(
            ContexteExplorateur {
                vault: self.vault.clone(),
                workspace_events: self.evenements.clone(),
                ouvrir: Rc::new(move |f: &TFile| ouverts.borrow_mut().push(f.path.clone())),
                sauvegarder: Rc::new(move || *sauvegardes.borrow_mut() += 1),
            },
            &Keymap::new().get_root_scope(),
        );
        vue.activer_sondes();
        vue
    }
}

fn peindre(vcx: &mut VisualTestContext) {
    for _ in 0..2 {
        vcx.update(|w, cx| {
            w.draw(cx).clear(cx);
        });
    }
}

fn monter(
    cx: &mut TestAppContext,
    vue: FileExplorerView,
) -> (Entity<Panneau>, &mut VisualTestContext) {
    let (racine, vcx) = cx.add_window_view(|_, _| Panneau { vue });
    peindre(vcx);
    (racine, vcx)
}

/// Agit sur la vue puis repeint (le Workspace réel notifie ; ici le test le fait).
fn agir<R>(
    racine: &Entity<Panneau>,
    vcx: &mut VisualTestContext,
    f: impl FnOnce(&mut FileExplorerView) -> R,
) -> R {
    let r = racine.update(vcx, |p, cx| {
        let r = f(&mut p.vue);
        cx.notify();
        r
    });
    peindre(vcx);
    r
}

fn lire<R>(
    racine: &Entity<Panneau>,
    vcx: &mut VisualTestContext,
    f: impl FnOnce(&FileExplorerView) -> R,
) -> R {
    racine.read_with(vcx, |p, _| f(&p.vue))
}

/// Clique au centre de la ligne `chemin`.
fn cliquer(vcx: &mut VisualTestContext, chemin: &'static str) {
    let sonde: &'static str = Box::leak(format!("{chemin}/ligne").into_boxed_str());
    let b = vcx
        .debug_bounds(sonde)
        .unwrap_or_else(|| panic!("ligne {chemin} introuvable"));
    vcx.simulate_click(b.center(), Modifiers::none());
    peindre(vcx);
}

fn visible(vcx: &mut VisualTestContext, chemin: &str) -> bool {
    let sonde: &'static str = Box::leak(format!("{chemin}/ligne").into_boxed_str());
    vcx.debug_bounds(sonde).is_some()
}

fn s(liste: &[&str]) -> Vec<String> {
    liste.iter().map(|x| (*x).to_owned()).collect()
}

// ─── Le tri ───

#[test]
fn tri_dossiers_d_abord_puis_noms_naturels() {
    let b = banc();
    b.disque.put("note 10.md", "");
    b.disque.put("note 2.md", "");
    b.disque.put("Zèbre.md", "");
    b.disque.put("école.md", "");
    b.disque.put("eau.md", "");
    b.disque.put("Alpha.md", "");
    b.disque.put("beta.md", "");
    let vault = b.vault.clone();
    vault.load().expect("rechargement");
    let vue = b.vue();
    let noms: Vec<String> = vue.lignes().into_iter().map(|l| l.chemin).collect();
    assert_eq!(
        noms,
        s(&[
            "Dossier A",
            "Dossier B",
            "Alpha.md",
            "beta.md",
            "eau.md",
            "école.md",
            "image.png",
            "note 2.md",
            "note 10.md",
            "racine.md",
            "Zèbre.md",
        ]),
        "dossiers, puis noms : casse et accents ignorés, nombres comparés comme des nombres"
    );
}

#[test]
fn tri_inverse_garde_les_dossiers_en_tete() {
    let b = banc();
    let vue = b.vue();
    vue.basculer_tri();
    let noms: Vec<String> = vue.lignes().into_iter().map(|l| l.chemin).collect();
    assert_eq!(
        noms,
        s(&["Dossier B", "Dossier A", "racine.md", "image.png"]),
        "seuls les noms s'inversent"
    );
}

#[test]
fn comparaison_naturelle_des_noms() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    assert_eq!(comparer_noms("a", "B"), Less);
    assert_eq!(
        comparer_noms("e", "é"),
        Less,
        "l'accent ne départage qu'à égalité"
    );
    assert_eq!(comparer_noms("é", "f"), Less);
    assert_eq!(
        comparer_noms("a", "A"),
        Less,
        "la minuscule d'abord à égalité"
    );
    assert_eq!(comparer_noms("note 9", "note 10"), Less);
    // Égalité numérique : l'ordre reste total (le plus long après), jamais « égal ».
    assert_eq!(comparer_noms("note 007", "note 7"), Greater);
    assert_eq!(comparer_noms("note 7", "note 7"), Equal);
    assert_eq!(comparer_noms("note", "note 1"), Less);
    assert_eq!(
        comparer_noms("1", "a"),
        Less,
        "les chiffres avant les lettres"
    );
    assert_eq!(comparer_noms("b", "a"), Greater);
}

#[test]
fn les_fichiers_s_affichent_sans_leur_extension() {
    let b = banc();
    let vue = b.vue();
    let titres: Vec<(String, String)> = vue
        .lignes()
        .into_iter()
        .map(|l| (l.chemin, l.titre))
        .collect();
    assert!(titres.contains(&("racine.md".to_owned(), "racine".to_owned())));
    assert!(titres.contains(&("image.png".to_owned(), "image".to_owned())));
    assert!(titres.contains(&("Dossier A".to_owned(), "Dossier A".to_owned())));
}

// ─── Ouvrir / fermer ───

#[test]
fn clic_sur_un_dossier_l_ouvre_puis_le_ferme() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    assert!(!visible(vcx, "Dossier A/a.md"), "replié au départ");

    cliquer(vcx, "Dossier A");
    assert!(visible(vcx, "Dossier A/a.md"));
    assert!(visible(vcx, "Dossier A/Sous-dossier"));
    assert!(!visible(vcx, "Dossier A/Sous-dossier/profonde.md"));
    assert_eq!(
        *b.sauvegardes.borrow(),
        1,
        "un dossier ouvert demande la sauvegarde du layout"
    );

    cliquer(vcx, "Dossier A/Sous-dossier");
    assert!(visible(vcx, "Dossier A/Sous-dossier/profonde.md"));

    cliquer(vcx, "Dossier A");
    assert!(!visible(vcx, "Dossier A/a.md"));
    assert_eq!(*b.sauvegardes.borrow(), 3);
    // Le sous-dossier reste « ouvert » : rouvrir le parent le retrouve déplié.
    assert!(lire(&racine, vcx, |v| v.ouverts()).contains(&"Dossier A/Sous-dossier".to_owned()));
    cliquer(vcx, "Dossier A");
    assert!(visible(vcx, "Dossier A/Sous-dossier/profonde.md"));
}

#[test]
fn clic_sur_un_fichier_l_ouvre_et_le_rend_actif() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    cliquer(vcx, "racine.md");
    assert_eq!(*b.ouverts.borrow(), s(&["racine.md"]));
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("racine.md".to_owned())
    );
    assert_eq!(
        *b.sauvegardes.borrow(),
        0,
        "ouvrir un fichier ne change pas les dossiers"
    );
}

// ─── Le fichier actif ───

#[test]
fn le_fichier_de_la_feuille_active_est_suivi() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    b.evenements
        .trigger(FILE_OPEN, &Some(DocumentKey::new("Dossier B/b.md")));
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("Dossier B/b.md".to_owned())
    );
    assert_eq!(
        lire(&racine, vcx, |v| v.ouverts()),
        s(&[""]),
        "suivre l'actif n'ouvre aucun dossier"
    );

    // Une feuille sans document (l'explorateur lui-même) ne retire pas l'actif.
    b.evenements.trigger(FILE_OPEN, &None::<DocumentKey>);
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("Dossier B/b.md".to_owned())
    );
    // Un document hors du coffre non plus.
    b.evenements
        .trigger(FILE_OPEN, &Some(DocumentKey::new("chat:42")));
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("Dossier B/b.md".to_owned())
    );
}

// ─── Révéler ───

#[test]
fn reveler_ouvre_les_ancetres_pas_le_dossier_revele() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    agir(&racine, vcx, |v| {
        v.reveal_in_folder("Dossier A/Sous-dossier/profonde.md");
    });
    assert!(visible(vcx, "Dossier A/Sous-dossier/profonde.md"));
    assert_eq!(
        lire(&racine, vcx, |v| v.ouverts()),
        s(&["", "Dossier A", "Dossier A/Sous-dossier"])
    );
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("Dossier A/Sous-dossier/profonde.md".to_owned())
    );

    // Un dossier révélé reste replié : on le montre, on ne l'ouvre pas.
    agir(&racine, vcx, |v| v.reveal_in_folder("Dossier B"));
    assert!(visible(vcx, "Dossier B"));
    assert!(!visible(vcx, "Dossier B/b.md"));
}

#[test]
fn reveal_in_folder_du_workspace_revele_le_dossier() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    let dossier = b
        .vault
        .get_folder_by_path("Dossier A/Sous-dossier")
        .unwrap();
    b.evenements
        .trigger(REVEAL_IN_FOLDER, &RevealInFolder { folder: dossier });
    peindre(vcx);
    assert_eq!(
        lire(&racine, vcx, |v| v.ouverts()),
        s(&["", "Dossier A"]),
        "les ancêtres de Sous-dossier, pas Sous-dossier"
    );
    assert!(visible(vcx, "Dossier A/Sous-dossier"));
    assert_eq!(*b.sauvegardes.borrow(), 1);
}

/// Une longue liste : 30 notes dans un dossier, dans un panneau de 200 px (7 lignes de 26).
fn banc_long() -> Banc {
    let b = banc();
    for n in 0..30 {
        b.disque.put(&format!("Long/note {n:02}.md"), "");
    }
    b.vault.load().expect("rechargement");
    b
}

#[test]
fn reveler_fait_defiler_jusqu_a_la_ligne() {
    let b = banc_long();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    assert_eq!(lire(&racine, vcx, |v| v.defilement().offset().y), px(0.0));
    agir(&racine, vcx, |v| v.reveal_in_folder("Long/note 25.md"));
    // Les lignes : Dossier A, Dossier B, Long, puis note 00… : note 25 est la 29e (rang 28).
    let liste = vcx.debug_bounds("explorateur/liste").expect("liste");
    let ligne = vcx.debug_bounds("Long/note 25.md/ligne").expect("ligne");
    assert!(
        ligne.bottom() <= liste.bottom() && ligne.top() >= liste.top(),
        "la ligne révélée est entièrement dans la liste : ligne {ligne:?}, liste {liste:?}"
    );
    assert!(lire(&racine, vcx, |v| v.defilement().offset().y) < px(0.0));
}

#[test]
fn reveler_avant_la_premiere_mise_en_page_est_rejoue() {
    let b = banc_long();
    let vue = b.vue();
    // Demandée dès la création, avant que la vue soit montée : elle attend la mise en page.
    vue.reveal_in_folder("Long/note 25.md");
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, vue);
    peindre(vcx);
    let liste = vcx.debug_bounds("explorateur/liste").expect("liste");
    let ligne = vcx.debug_bounds("Long/note 25.md/ligne").expect("ligne");
    assert!(ligne.bottom() <= liste.bottom() && ligne.top() >= liste.top());
    drop(racine);
}

// ─── Mise à jour incrémentale : le coffre bouge hors de l'explorateur ───

#[test]
fn creation_exterieure_reflete_sans_perdre_les_dossiers_ouverts() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    cliquer(vcx, "Dossier A");
    let avant = lire(&racine, vcx, |v| v.version());

    b.disque.put("Dossier A/nouvelle.md", "n");
    b.disque.emit(ChangeType::Create, "Dossier A/nouvelle.md");
    peindre(vcx);
    assert!(visible(vcx, "Dossier A/nouvelle.md"));
    assert!(visible(vcx, "Dossier A/a.md"), "Dossier A est resté ouvert");
    assert_eq!(lire(&racine, vcx, |v| v.version()), avant + 1);
}

#[test]
fn renommage_exterieur_reflete() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    cliquer(vcx, "Dossier A");
    cliquer(vcx, "Dossier A/a.md");
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("Dossier A/a.md".to_owned())
    );

    // Le disque renomme `Dossier A/a.md` en `Dossier A/z.md` : le watcher voit une suppression,
    // puis une création.
    b.disque.retirer("Dossier A/a.md");
    b.disque.emit(ChangeType::Delete, "Dossier A/a.md");
    b.disque.put("Dossier A/z.md", "a");
    b.disque.emit(ChangeType::Create, "Dossier A/z.md");
    peindre(vcx);
    assert!(!visible(vcx, "Dossier A/a.md"));
    assert!(visible(vcx, "Dossier A/z.md"));
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        None,
        "le fichier actif a disparu : plus rien n'est surligné"
    );
}

#[test]
fn renommage_du_coffre_suit_l_actif_et_replie_le_dossier_renomme() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    agir(&racine, vcx, |v| {
        v.reveal_in_folder("Dossier A/Sous-dossier/profonde.md")
    });
    assert!(visible(vcx, "Dossier A/Sous-dossier/profonde.md"));

    let dossier = b.vault.get_folder_by_path("Dossier A").unwrap();
    b.vault.rename(&dossier, "Dossier C").expect("renommage");
    peindre(vcx);
    assert!(!visible(vcx, "Dossier A"));
    assert!(visible(vcx, "Dossier C"));
    assert!(
        !visible(vcx, "Dossier C/a.md"),
        "le dossier renommé repart replié, comme dans Fragment"
    );
    assert_eq!(
        lire(&racine, vcx, |v| v.actif()),
        Some("Dossier C/Sous-dossier/profonde.md".to_owned()),
        "l'actif suit le renommage"
    );
    assert_eq!(
        lire(&racine, vcx, |v| v.ouverts()),
        s(&[""]),
        "plus de chemin périmé"
    );
}

#[test]
fn suppression_d_un_dossier_ouvert_l_oublie() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    cliquer(vcx, "Dossier B");
    assert_eq!(lire(&racine, vcx, |v| v.ouverts()), s(&["", "Dossier B"]));
    let dossier = b.vault.get_folder_by_path("Dossier B").unwrap();
    b.vault.delete(&dossier, true).expect("suppression");
    peindre(vcx);
    assert!(!visible(vcx, "Dossier B"));
    assert_eq!(lire(&racine, vcx, |v| v.ouverts()), s(&[""]));
}

// ─── L'état éphémère : dossiers ouverts + défilement ───

#[test]
fn explorateur_les_dossiers_ouverts_se_rouvrent_au_redemarrage_et_pas_les_autres() {
    let b = banc();
    let mut cx = TestAppContext::single();
    let (racine, vcx) = monter(&mut cx, b.vue());
    // Ouverts À LA MAIN, comme l'utilisateur.
    cliquer(vcx, "Dossier A");
    cliquer(vcx, "Dossier A/Sous-dossier");
    assert!(visible(vcx, "Dossier A/Sous-dossier/profonde.md"));
    assert!(!visible(vcx, "Dossier B/b.md"));

    let etat = lire(&racine, vcx, |v| v.get_ephemeral_state());
    assert_eq!(
        etat["expanded"],
        json!(["", "Dossier A", "Dossier A/Sous-dossier"]),
        "seuls les ouverts, la racine toujours"
    );
    // Le layout passe par le JSON de workspace.json.
    let etat = serde_json::to_string(&etat).unwrap();
    drop(racine);
    drop(cx);

    // « Redémarrage » : une vue neuve, rouverte par l'état.
    let mut cx = TestAppContext::single();
    let mut vue = b.vue();
    vue.set_ephemeral_state(&serde_json::from_str(&etat).unwrap());
    let (_racine, vcx) = monter(&mut cx, vue);
    assert!(visible(vcx, "Dossier A/Sous-dossier/profonde.md"));
    assert!(visible(vcx, "Dossier A/a.md"));
    assert!(!visible(vcx, "Dossier B/b.md"), "fermé, il reste fermé");
}

#[test]
fn etat_ephemere_aller_retour_defilement_apres_les_dossiers() {
    let b = banc_long();
    let mut cx = TestAppContext::single();
    // L'état à restaurer : « Long » ouvert, défilé de 130 px (5 lignes) : impossible sans le
    // dossier ouvert, c'est pourquoi les dossiers se rouvrent AVANT.
    let mut vue = b.vue();
    vue.set_ephemeral_state(&json!({ "scroll": 130.0, "expanded": ["", "Long"] }));
    assert_eq!(
        vue.get_ephemeral_state()["scroll"],
        json!(130.0),
        "tant que la liste n'est pas mesurée, la demande est rendue telle quelle"
    );
    let (racine, vcx) = monter(&mut cx, vue);
    assert_eq!(
        lire(&racine, vcx, |v| v.defilement().offset().y),
        px(-130.0)
    );
    let etat = lire(&racine, vcx, |v| v.get_ephemeral_state());
    assert_eq!(etat["scroll"], json!(130.0));
    assert_eq!(etat["expanded"], json!(["", "Long"]));
}

#[test]
fn etat_ephemere_sans_dossiers_ne_touche_pas_aux_dossiers() {
    let b = banc();
    let mut vue = b.vue();
    vue.basculer_dossier("Dossier B");
    vue.set_ephemeral_state(&json!({ "scroll": 0 }));
    assert_eq!(vue.ouverts(), s(&["", "Dossier B"]));
    // Une liste mal formée est filtrée : seules les chaînes comptent.
    vue.set_ephemeral_state(&json!({ "expanded": ["Dossier A", 3, null] }));
    assert_eq!(vue.ouverts(), s(&["", "Dossier A"]));
}

// ─── L'identité de la vue ───

#[test]
fn identite_de_la_vue() {
    let b = banc();
    let vue = b.vue();
    assert_eq!(vue.get_view_type(), FILE_EXPLORER_VIEW_TYPE);
    assert_eq!(FILE_EXPLORER_VIEW_TYPE, "file-explorer");
    assert_eq!(vue.get_icon(), "folder-closed");
    assert_eq!(vue.get_display_text(), b.vault.get_name());
    assert!(!vue.navigation(), "un panneau statique : on n'en part pas");
    assert!(vue.scope().is_some());
    let _ = vue.item_scope();
}

#[test]
fn la_vue_se_desabonne_quand_elle_disparait() {
    let b = banc();
    let avant = b.evenements.listener_count(FILE_OPEN);
    let vue = b.vue();
    assert_eq!(b.evenements.listener_count(FILE_OPEN), avant + 1);
    drop(vue);
    assert_eq!(b.evenements.listener_count(FILE_OPEN), avant);
    assert_eq!(b.evenements.listener_count(REVEAL_IN_FOLDER), 0);
}
