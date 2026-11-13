// Port de app/src/ui/core/workspace/ItemView.ts (menu « Calques », toggleLayer) — pas de test TS :
// cas écrits d'après le contrat et les specs e2e gutter-arbiter (« décocher puis recocher »).

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use fragment_layers::menu_calques::{
    CatalogLayer, LayerMenuChild, MenuState, ViewLayers, build_menu,
};
use fragment_layers::overlay_host::LayerSpec;
use fragment_layers::types::{LayerMenuSection, LayerPreset};

fn calque(id: &str, name: &str) -> CatalogLayer {
    CatalogLayer {
        id: id.into(),
        name: name.into(),
        icon: None,
        default_enabled: false,
        in_switcher: true,
        applies: true,
        presets: vec![],
    }
}

fn enfant(id: &str) -> LayerMenuChild {
    LayerMenuChild {
        id: id.into(),
        title: id.to_uppercase(),
        icon: None,
    }
}

fn aucun(_: &str, _: &mut LayerMenuSection) -> Vec<LayerMenuChild> {
    vec![]
}

#[test]
fn basculer_une_instance_la_veut_puis_ne_la_veut_plus() {
    let mut v = ViewLayers::new();
    assert!(!v.is_layer_active("globe", None));
    assert!(v.toggle_layer("globe", None));
    assert!(v.is_layer_active("globe", None));
    assert!(!v.toggle_layer("globe", None));
    assert!(!v.is_layer_active("globe", None));
}

#[test]
fn un_preset_est_une_instance_distincte_de_l_instance_par_defaut() {
    let mut v = ViewLayers::new();
    v.toggle_layer("annotation", Some("relecture"));
    assert!(v.is_layer_active("annotation", Some("relecture")));
    assert!(!v.is_layer_active("annotation", None));
    assert_eq!(v.active_keys(), vec!["annotation:relecture"]);
}

#[test]
fn decocher_puis_recocher_un_calque_remonte_l_instance() {
    let catalog = vec![calque("e2e-globe", "Globe")];
    let mut v = ViewLayers::new();
    v.toggle_layer("e2e-globe", None);
    let mut montes: HashSet<String> = HashSet::new();
    assert_eq!(v.reconcile(&catalog, &montes).to_mount.len(), 1);
    montes.insert("e2e-globe".into());
    v.toggle_layer("e2e-globe", None); // décocher : démonté
    let plan = v.reconcile(&catalog, &montes);
    assert_eq!(plan.to_unmount, vec!["e2e-globe"]);
    assert!(plan.to_mount.is_empty());
    montes.clear();
    v.toggle_layer("e2e-globe", None); // recocher : remonté, une seule fois
    assert_eq!(v.reconcile(&catalog, &montes).to_mount[0].key, "e2e-globe");
    montes.insert("e2e-globe".into());
    assert_eq!(v.reconcile(&catalog, &montes), Default::default());
}

#[test]
fn un_spec_qui_quitte_le_catalogue_est_demonte_mais_l_instance_reste_voulue() {
    let mut v = ViewLayers::new();
    v.toggle_layer("plugin", None);
    let montes: HashSet<String> = ["plugin".to_string()].into();
    let plan = v.reconcile(&[], &montes);
    assert_eq!(plan.to_unmount, vec!["plugin"]);
    assert!(v.is_layer_active("plugin", None));
    // le plugin revient : remontage sans rien recocher
    let plan = v.reconcile(&[calque("plugin", "Plugin")], &HashSet::new());
    assert_eq!(plan.to_mount.len(), 1);
}

#[test]
fn un_calque_inapplicable_n_est_pas_monte() {
    let mut c = calque("pdf", "PDF");
    c.applies = false;
    let mut v = ViewLayers::new();
    v.toggle_layer("pdf", None);
    assert!(v.reconcile(&[c], &HashSet::new()).to_mount.is_empty());
}

#[test]
fn les_calques_actives_d_office_sont_voulus_a_l_ouverture() {
    let mut a = calque("agent", "Agent");
    a.default_enabled = true;
    let mut b = calque("globe", "Globe");
    b.default_enabled = true;
    b.applies = false;
    let c = calque("annotation", "Annotation");
    let mut v = ViewLayers::new();
    v.seed_defaults(&[a, b, c]);
    assert_eq!(v.active_keys(), vec!["agent"]);
}

#[test]
fn close_est_idempotent() {
    let mut v = ViewLayers::new();
    v.toggle_layer("agent", None);
    assert!(v.close("agent"));
    assert!(!v.close("agent"));
    assert!(!v.is_layer_active("agent", None));
}

#[test]
fn l_etat_ephemere_survit_dans_l_ordre_et_se_lit_aussi_depuis_les_seules_cles() {
    let mut v = ViewLayers::new();
    v.toggle_layer("b", None);
    v.toggle_layer("a", Some("p"));
    let mut w = ViewLayers::new();
    w.restore(Some(v.instances().to_vec()), None);
    assert_eq!(w.active_keys(), vec!["b", "a:p"]);
    let mut x = ViewLayers::new();
    x.restore(None, Some(v.active_keys()));
    assert_eq!(x, v);
    x.restore(None, None); // rien à restaurer : l'état ne bouge pas
    assert_eq!(x, v);
}

#[test]
fn le_menu_suit_l_ordre_du_catalogue_et_saute_l_infrastructure_et_l_inapplicable() {
    let mut infra = calque("infra", "Infra");
    infra.in_switcher = false;
    let mut absent = calque("pdf", "PDF");
    absent.applies = false;
    let catalog = vec![calque("b", "B"), infra, calque("a", "A"), absent];
    let m = build_menu(&catalog, &ViewLayers::new(), &MenuState::new(), &aucun);
    let titres: Vec<_> = m.rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titres, vec!["B", "A"]);
}

#[test]
fn le_menu_vide_signale_aucun_calque_disponible() {
    let m = build_menu(&[], &ViewLayers::new(), &MenuState::new(), &aucun);
    assert!(m.is_empty());
}

#[test]
fn l_icone_par_defaut_est_layers_et_l_oeil_suit_l_instance() {
    let mut avec = calque("a", "A");
    avec.icon = Some("bot".into());
    let catalog = vec![avec, calque("b", "B")];
    let mut v = ViewLayers::new();
    v.toggle_layer("a", None);
    let m = build_menu(&catalog, &v, &MenuState::new(), &aucun);
    assert_eq!(m.rows[0].icon, "bot");
    assert!(m.rows[0].active);
    assert_eq!(m.rows[0].eye_tooltip(), "Masquer A");
    assert_eq!(m.rows[1].icon, "layers");
    assert!(!m.rows[1].active);
    assert_eq!(m.rows[1].eye_tooltip(), "Afficher B");
}

#[test]
fn une_ligne_par_preset_nommee_calque_tiret_preset_sans_enfants() {
    let mut c = calque("annotation", "Annotation");
    c.icon = Some("pen".into());
    c.presets = vec![
        LayerPreset {
            id: "relecture".into(),
            name: "Relecture".into(),
            icon: None,
        },
        LayerPreset {
            id: "idees".into(),
            name: "Idées".into(),
            icon: Some("lightbulb".into()),
        },
    ];
    let mut v = ViewLayers::new();
    v.toggle_layer("annotation", Some("idees"));
    let contrib = |_: &str, _: &mut LayerMenuSection| vec![enfant("x")];
    let m = build_menu(&[c], &v, &MenuState::new(), &contrib);
    assert_eq!(m.rows.len(), 3);
    assert_eq!(m.rows[1].title, "Annotation — Relecture");
    assert_eq!(m.rows[1].key, "annotation:relecture");
    assert_eq!(m.rows[1].icon, "pen"); // celle du calque
    assert_eq!(m.rows[2].icon, "lightbulb");
    assert!(m.rows[2].active && !m.rows[1].active);
    assert!(m.rows[1].children.is_empty() && m.rows[2].children.is_empty());
    assert!(m.rows[0].has_children());
}

#[test]
fn les_contributions_vont_sous_leur_calque_et_leurs_boutons_sur_la_ligne() {
    let appels = Rc::new(Cell::new(0));
    let a = appels.clone();
    let contrib = move |id: &str, s: &mut LayerMenuSection| {
        a.set(a.get() + 1);
        if id == "agent" {
            s.add_action("trash", "Tout supprimer", || {}, true);
            vec![enfant("barre")]
        } else {
            vec![]
        }
    };
    let catalog = vec![calque("annotation", "Annotation"), calque("agent", "Agent")];
    let m = build_menu(&catalog, &ViewLayers::new(), &MenuState::new(), &contrib);
    assert_eq!(appels.get(), 2);
    assert!(
        m.row("annotation")
            .is_some_and(|r| !r.has_children() && r.actions.is_empty())
    );
    let agent = m.row("agent").expect("ligne agent");
    assert_eq!(agent.children[0].id, "barre");
    assert_eq!(agent.actions.len(), 1);
    assert!(agent.actions[0].warning);
}

#[test]
fn un_clic_de_ligne_deplie_avec_enfants_et_bascule_l_oeil_sans() {
    let contrib = |id: &str, _: &mut LayerMenuSection| {
        if id == "a" { vec![enfant("o")] } else { vec![] }
    };
    let catalog = vec![calque("a", "A"), calque("b", "B")];
    let mut st = MenuState::new();
    let m = build_menu(&catalog, &ViewLayers::new(), &st, &contrib);
    assert!(m.rows[0].is_folded());
    assert!(!st.click_row(&m.rows[0])); // déplie, ne touche pas l'œil
    let m = build_menu(&catalog, &ViewLayers::new(), &st, &contrib);
    assert!(m.rows[0].expanded && !m.rows[0].is_folded());
    assert!(!st.click_row(&m.rows[0])); // replie
    assert!(!st.is_expanded("a"));
    assert!(st.click_row(&m.rows[1])); // rien à déplier : la ligne est l'œil
}

#[test]
fn from_spec_reprend_les_champs_du_spec_et_son_applicabilite() {
    let mut spec = LayerSpec::new("globe", "Globe", |_| Box::new(|| {}))
        .with_applies_to(|v| v.downcast_ref::<u8>() == Some(&1));
    spec.in_switcher = false;
    let oui = CatalogLayer::from_spec(&spec, &1u8);
    let non = CatalogLayer::from_spec(&spec, &2u8);
    assert!(oui.applies && !non.applies);
    assert!(!oui.in_switcher);
    assert_eq!(oui.name, "Globe");
}
