// Port de app/src/ui/core/layers/types.ts — pas de fichier de tests TS : cas
// écrits d'après le contrat (clef d'instance, regroupable par défaut, menu).

use std::cell::Cell;
use std::rc::Rc;

use fragment_layers::types::{
    CompactInfo, GutterMember, LayerInstanceRef, LayerMenuSection, layer_instance_key,
};

#[test]
fn la_cle_d_instance_est_l_id_du_spec_sans_preset_et_spec_deux_points_preset_avec() {
    assert_eq!(layer_instance_key("annotation", None), "annotation");
    assert_eq!(layer_instance_key("annotation", Some("")), "annotation");
    assert_eq!(
        layer_instance_key("annotation", Some("relecture")),
        "annotation:relecture"
    );
    let r = LayerInstanceRef::new("globe", Some("a".into()));
    assert_eq!(r.key, "globe:a");
    assert_eq!(r.spec, "globe");
    assert_eq!(r.preset.as_deref(), Some("a"));
}

#[test]
fn un_widget_reduit_est_regroupable_par_defaut_et_son_membre_lit_l_ancre_courante() {
    let revele = Rc::new(Cell::new(0));
    let r = Rc::clone(&revele);
    let info = Rc::new(CompactInfo::new("Fil", vec!["chat".into()], move || {
        r.set(r.get() + 1)
    }));
    assert!(info.groupable);
    // L'ancre est LUE à l'usage : elle suit les éditions, aucune copie périmée.
    let pos = Rc::new(Cell::new(10));
    let p = Rc::clone(&pos);
    let membre = GutterMember::new(Rc::clone(&info), move || p.get());
    assert_eq!(membre.pos(), 10);
    pos.set(42);
    assert_eq!(membre.pos(), 42);
    assert_eq!(membre.label(), "Fil");
    assert!(membre.groupable());
    (membre.info.reveal)();
    assert_eq!(revele.get(), 1);
}

#[test]
fn la_section_de_menu_collecte_les_actions_dans_l_ordre() {
    let clics = Rc::new(Cell::new(0));
    let c = Rc::clone(&clics);
    let mut section = LayerMenuSection::default();
    section.add_action("trash", "Tout supprimer", move || c.set(c.get() + 1), true);
    section.add_action("eye", "Voir", || {}, false);
    assert_eq!(section.actions.len(), 2);
    assert_eq!(section.actions[0].title, "Tout supprimer");
    assert!(section.actions[0].warning);
    (section.actions[0].on_click)();
    assert_eq!(clics.get(), 1);
    assert!(!section.actions[1].warning);
}
