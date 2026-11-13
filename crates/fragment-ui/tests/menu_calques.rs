//! Tests du menu « Calques » rendu (tâche `fr-t5-menu-calques`), nommés d'après les cas de
//! `app/e2e/artefacts-menu.spec.ts` et `app/e2e/agent-bar.spec.ts` (snake_case, sans accents) : un
//! arbre (calque, présets, lignes contribuées), l'œil par ligne, le clic de ligne, les boutons des
//! contributeurs. Les modèles ne touchent pas à GPUI ; le dernier test dessine la couche dans une
//! fenêtre de test sans GPU.

// Fichier de test : les assistants hors `#[test]` déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{App as _, AppCoeur, KeyEvent, Keymap};
use fragment_layers::menu_calques::{
    CatalogLayer, LayerMenuChild, LayerMenuContributor, MenuState, ViewLayers,
};
use fragment_layers::types::{LayerMenuSection, LayerPreset};
use fragment_ui::menu_calques::{
    self, Genre, HoteMenu, LigneVue, cliquer_action, cliquer_enfant, cliquer_ligne, cliquer_oeil,
    lignes_visibles, menu_calques_ouvert, ouvrir_menu_calques,
};
use gpui_kit::{
    Context, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px, size,
};
use serde_json::json;

fn calque(id: &str, nom: &str, applique: bool) -> CatalogLayer {
    CatalogLayer {
        id: id.to_owned(),
        name: nom.to_owned(),
        icon: None,
        default_enabled: false,
        in_switcher: true,
        applies: applique,
        presets: Vec::new(),
    }
}

/// Ce que le contributeur de test fait, et ce que le test en relève.
#[derive(Default)]
struct Traces {
    corbeille: Cell<u32>,
    clics_enfant: RefCell<Vec<(String, String)>>,
    reconciliations: Cell<u32>,
}

struct Contributeur {
    traces: Rc<Traces>,
}

impl LayerMenuContributor for Contributeur {
    fn contribute(&self, layer_id: &str, section: &mut LayerMenuSection) -> Vec<LayerMenuChild> {
        match layer_id {
            "agent" => vec![
                LayerMenuChild {
                    id: "barre".into(),
                    title: "Barre de l'agent".into(),
                    icon: Some("sparkles".into()),
                },
                LayerMenuChild {
                    id: "c1".into(),
                    title: "Conversation".into(),
                    icon: None,
                },
            ],
            "artefacts" => {
                let t = Rc::clone(&self.traces);
                section.add_action(
                    "trash-2",
                    "Tout supprimer",
                    move || t.corbeille.set(t.corbeille.get() + 1),
                    true,
                );
                vec![LayerMenuChild {
                    id: "a1".into(),
                    title: "Tableau".into(),
                    icon: None,
                }]
            }
            _ => Vec::new(),
        }
    }
}

struct Banc {
    hote: HoteMenu,
    traces: Rc<Traces>,
    keymap: Keymap,
}

fn banc_avec(catalogue: Vec<CatalogLayer>) -> Banc {
    let traces = Rc::new(Traces::default());
    let (t1, t2) = (Rc::clone(&traces), Rc::clone(&traces));
    let hote = HoteMenu {
        catalogue,
        calques: Rc::new(RefCell::new(ViewLayers::new())),
        etat: Rc::new(RefCell::new(MenuState::new())),
        contributeur: Rc::new(Contributeur {
            traces: Rc::clone(&traces),
        }),
        apres_bascule: Rc::new(move || t1.reconciliations.set(t1.reconciliations.get() + 1)),
        sur_enfant: Rc::new(move |calque, id| {
            t2.clics_enfant
                .borrow_mut()
                .push((calque.to_owned(), id.to_owned()));
        }),
    };
    let keymap = Keymap::new();
    ouvrir_menu_calques(hote.clone(), 100.0, 50.0, &keymap);
    Banc {
        hote,
        traces,
        keymap,
    }
}

fn banc() -> Banc {
    banc_avec(vec![
        calque("agent", "Agent", true),
        calque("artefacts", "Artefacts", true),
        calque("notes", "Notes", true),
    ])
}

fn titres() -> Vec<String> {
    lignes_visibles().into_iter().map(|l| l.titre).collect()
}

fn ligne(cle: &str) -> LigneVue {
    lignes_visibles()
        .into_iter()
        .find(|l| l.cle == cle && l.genre == Genre::Calque)
        .expect("ligne de calque")
}

#[test]
fn le_menu_est_un_arbre_une_ligne_par_calque_repliee_par_defaut() {
    let _b = banc();
    assert!(menu_calques_ouvert());
    // Replié : seuls les calques, jamais les lignes que les plugins rangent dessous.
    assert_eq!(titres(), ["Agent", "Artefacts", "Notes"]);
    assert!(ligne("agent").a_enfants && !ligne("agent").depliee);
    // Sans enfants : pas de chevron, et la ligne EST l'œil.
    assert!(!ligne("notes").a_enfants);
}

#[test]
fn un_calque_inapplicable_ou_hors_menu_n_a_pas_de_ligne() {
    let mut infra = calque("infra", "Infra", true);
    infra.in_switcher = false;
    let _b = banc_avec(vec![
        calque("agent", "Agent", true),
        calque("pdf", "PDF seul", false),
        infra,
    ]);
    assert_eq!(titres(), ["Agent"]);
}

#[test]
fn aucun_calque_disponible_une_ligne_desactivee() {
    let _b = banc_avec(vec![calque("pdf", "PDF seul", false)]);
    let l = lignes_visibles();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].genre, Genre::Vide);
    assert_eq!(l[0].titre, "Aucun calque disponible");
    // Rien à cliquer.
    assert!(!cliquer_oeil(""));
}

#[test]
fn une_ligne_par_preset_sous_son_calque() {
    let mut surlignages = calque("surlignages", "Surlignages", true);
    surlignages.presets = vec![LayerPreset {
        id: "jaune".into(),
        name: "Jaune".into(),
        icon: Some("highlighter".into()),
    }];
    let _b = banc_avec(vec![surlignages, calque("agent", "Agent", true)]);
    assert_eq!(titres(), ["Surlignages", "Surlignages — Jaune", "Agent"]);
    let preset = ligne("surlignages:jaune");
    assert_eq!(preset.icone.as_deref(), Some("highlighter"));
    assert!(!preset.a_enfants);
}

#[test]
fn un_clic_sur_la_ligne_deplie_et_replie_sans_toucher_l_oeil() {
    let b = banc();
    assert!(cliquer_ligne("agent"));
    assert_eq!(
        titres(),
        [
            "Agent",
            "Barre de l'agent",
            "Conversation",
            "Artefacts",
            "Notes"
        ]
    );
    assert!(ligne("agent").depliee);
    assert!(!ligne("agent").oeil, "déplier n'allume pas le calque");
    assert_eq!(b.traces.reconciliations.get(), 0);
    cliquer_ligne("agent");
    assert_eq!(titres(), ["Agent", "Artefacts", "Notes"]);
}

#[test]
fn le_depliage_est_retenu_tant_que_la_vue_vit() {
    let b = banc();
    cliquer_ligne("artefacts");
    menu_calques::fermer_menu_calques();
    assert!(!menu_calques_ouvert());
    assert!(lignes_visibles().is_empty());
    ouvrir_menu_calques(b.hote.clone(), 0.0, 0.0, &b.keymap);
    assert!(ligne("artefacts").depliee);
}

#[test]
fn sans_enfants_un_clic_sur_la_ligne_bascule_le_calque() {
    let b = banc();
    assert!(!ligne("notes").oeil);
    cliquer_ligne("notes");
    assert!(ligne("notes").oeil);
    assert_eq!(b.traces.reconciliations.get(), 1);
    assert!(b.hote.calques.borrow().is_layer_active("notes", None));
}

#[test]
fn l_oeil_affiche_puis_masque_le_calque_et_le_menu_reste_ouvert() {
    let b = banc();
    assert_eq!(ligne("agent").info_bulle_oeil, "Afficher Agent");
    cliquer_oeil("agent");
    assert!(ligne("agent").oeil);
    assert_eq!(ligne("agent").info_bulle_oeil, "Masquer Agent");
    assert!(menu_calques_ouvert(), "on enchaîne : le menu reste ouvert");
    assert!(!ligne("agent").depliee, "l'œil n'est pas un clic de ligne");
    cliquer_oeil("agent");
    assert!(!ligne("agent").oeil);
    assert_eq!(b.traces.reconciliations.get(), 2);
    assert!(b.hote.calques.borrow().instances().is_empty());
}

#[test]
fn l_oeil_d_un_preset_bascule_son_instance() {
    let mut s = calque("surlignages", "Surlignages", true);
    s.presets = vec![LayerPreset {
        id: "jaune".into(),
        name: "Jaune".into(),
        icon: None,
    }];
    let b = banc_avec(vec![s]);
    cliquer_oeil("surlignages:jaune");
    assert!(
        b.hote
            .calques
            .borrow()
            .is_layer_active("surlignages", Some("jaune"))
    );
    assert!(!b.hote.calques.borrow().is_layer_active("surlignages", None));
}

#[test]
fn le_bouton_d_un_contributeur_est_a_droite_de_l_oeil_et_ne_deplie_pas() {
    let b = banc();
    let l = ligne("artefacts");
    assert_eq!(l.boutons.len(), 1);
    assert_eq!(l.boutons[0].icone, "trash-2");
    assert_eq!(l.boutons[0].titre, "Tout supprimer");
    assert!(l.boutons[0].avertissement);
    cliquer_action("artefacts", 0);
    assert_eq!(b.traces.corbeille.get(), 1);
    assert!(!ligne("artefacts").depliee, "le clic ne déplie pas");
    assert!(menu_calques_ouvert(), "le menu reste ouvert");
    assert!(ligne("agent").boutons.is_empty());
}

#[test]
fn le_clic_d_une_ligne_contribuee_ferme_le_menu_puis_agit() {
    let b = banc();
    cliquer_ligne("agent");
    // L'enfant 0 : la barre de l'agent, première ligne sous « Agent ».
    let sous_agent = lignes_visibles();
    assert_eq!(sous_agent[1].genre, Genre::Enfant);
    assert_eq!(sous_agent[1].titre, "Barre de l'agent");
    assert!(cliquer_enfant("agent", 0));
    assert!(!menu_calques_ouvert());
    assert_eq!(
        *b.traces.clics_enfant.borrow(),
        [("agent".to_owned(), "barre".to_owned())]
    );
}

#[test]
fn echap_ferme_le_menu_et_depile_sa_portee() {
    let b = banc();
    assert!(b.keymap.scope() != b.keymap.get_root_scope());
    assert_eq!(
        b.keymap.handle_key_event(&KeyEvent::new("Escape")),
        Some(false)
    );
    assert!(!menu_calques_ouvert());
    assert!(b.keymap.scope() == b.keymap.get_root_scope());
}

#[test]
fn rouvrir_remplace_le_menu_et_ne_garde_qu_une_portee() {
    let b = banc();
    ouvrir_menu_calques(b.hote.clone(), 5.0, 5.0, &b.keymap);
    b.keymap.handle_key_event(&KeyEvent::new("Escape"));
    assert!(b.keymap.scope() == b.keymap.get_root_scope());
}

#[test]
fn la_commande_de_scene_ouvre_le_menu_d_une_fausse_vue() {
    let app = AppCoeur::new();
    let commandes = app.commands();
    menu_calques::commandes_de_scene(&commandes);
    commandes.events().trigger(
        "pixels:args",
        &json!({ "deplie": ["agent"], "masques": ["tags"] }),
    );
    assert!(commandes.execute_command_by_id("pixels:menu_calques", None));
    assert!(menu_calques_ouvert());
    let t = titres();
    assert_eq!(t[0], "Agent");
    assert!(t.contains(&"Barre de l'agent".to_owned()));
    assert!(!ligne("tags").oeil, "masqué par l'argument");
    assert!(ligne("agent").oeil);
}

struct VueMenu;

impl Render for VueMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(menu_calques::couche(cx))
    }
}

#[test]
fn la_couche_se_dessine_ouverte_depliee_et_fermee() {
    let _b = banc();
    cliquer_ligne("agent");
    let mut cx = TestAppContext::single();
    let (_vue, vcx) = cx.add_window_view(|_, _| VueMenu);
    vcx.simulate_resize(size(px(600.0), px(400.0)));
    let image = |vcx: &mut VisualTestContext| {
        vcx.update(|w, cx| {
            w.draw(cx).clear(cx);
        });
        vcx.run_until_parked();
    };
    image(vcx);
    menu_calques::fermer_menu_calques();
    image(vcx);
}
