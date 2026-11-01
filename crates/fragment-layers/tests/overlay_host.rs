// Port de app/src/ui/core/layers/types.ts (LayerSpec, LayerContext) — pas de
// fichier de tests TS : cas écrits d'après le contrat. OverlayHost est exercé par
// tests/widget_layer.rs (l'hôte simulé l'implémente).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_layers::gutter_arbiter::GutterSlot;
use fragment_layers::overlay_host::{Disposer, LayerContext, LayerSpec, OverlayHost};
use fragment_layers::types::{
    ElementId, GutterBand, GutterSide, LayerInstanceRef, LayerPreset, OverlayPlane, Point,
    ViewportBand,
};
use fragment_layers::widget_activation::ActivationEntry;
use fragment_text::Rect;

/// Un hôte sans plan document (une vue sans éditeur) : seul le viewport répond.
struct HoteMuet;

impl OverlayHost for HoteMuet {
    fn mount(&self, _el: ElementId, _plane: OverlayPlane) -> Disposer {
        Box::new(|| {})
    }
    fn client_to_viewport(&self, x: f64, y: f64) -> Option<Point> {
        Some(Point { x, y })
    }
    fn client_to_document(&self, _x: f64, _y: f64) -> Option<Point> {
        None
    }
    fn document_to_client(&self, _x: f64, _y: f64) -> Option<Point> {
        None
    }
    fn gutter_band(&self, _side: GutterSide) -> Option<GutterBand> {
        None
    }
    fn gutter_slot(&self, _el: ElementId, _side: GutterSide) -> GutterSlot {
        unreachable!("pas de marge sans plan document")
    }
    fn track_activation(&self, _el: ElementId) -> ActivationEntry {
        unreachable!("non exercé ici")
    }
    fn on_geometry_change(&self, _cb: Box<dyn Fn()>) -> Disposer {
        Box::new(|| {})
    }
    fn viewport(&self) -> Option<ViewportBand> {
        None
    }
    fn on_viewport_change(&self, _cb: Box<dyn Fn()>) -> Disposer {
        Box::new(|| {})
    }
    fn content_client_rect(&self) -> Option<Rect> {
        None
    }
}

fn contexte(doc: Rc<RefCell<Option<String>>>, fermetures: Rc<Cell<u32>>) -> LayerContext {
    LayerContext::new(
        None,
        move || doc.borrow().clone(),
        Rc::new(HoteMuet),
        LayerInstanceRef::new("annotation", Some("relecture".into())),
        move || fermetures.set(fermetures.get() + 1),
    )
}

#[test]
fn le_contexte_lit_la_cle_du_document_a_l_usage_et_ferme_son_instance() {
    let doc = Rc::new(RefCell::new(None));
    let fermetures = Rc::new(Cell::new(0));
    let ctx = contexte(Rc::clone(&doc), Rc::clone(&fermetures));
    // Les calques montent avant que la vue ait son document : la clé est LUE à l'usage.
    assert_eq!(ctx.doc(), None);
    *doc.borrow_mut() = Some("notes/a.md".into());
    assert_eq!(ctx.doc().as_deref(), Some("notes/a.md"));
    assert!(ctx.editor.is_none());
    assert_eq!(ctx.instance.key, "annotation:relecture");
    ctx.close();
    assert_eq!(fermetures.get(), 1);
    // Sans plan document, l'hôte le dit par des `None`.
    assert!(ctx.overlays.client_to_document(1.0, 1.0).is_none());
    assert!(ctx.overlays.viewport().is_none());
}

#[test]
fn un_spec_a_ses_valeurs_par_defaut_et_create_rend_le_disposer() {
    let demontes = Rc::new(Cell::new(0));
    let d = Rc::clone(&demontes);
    let spec = LayerSpec::new("globe", "Globe", move |ctx: &LayerContext| {
        let d = Rc::clone(&d);
        let cle = ctx.instance.key.clone();
        assert_eq!(cle, "globe");
        Box::new(move || d.set(d.get() + 1))
    });
    assert_eq!(spec.id, "globe");
    assert_eq!(spec.name, "Globe");
    assert!(spec.icon.is_none());
    assert!(!spec.default_enabled);
    assert!(spec.in_switcher);
    assert!(spec.presets().is_empty());
    // Sans condition, il concerne toute vue.
    assert!(spec.applies_to(&()));
    let ctx = LayerContext::new(
        None,
        || None,
        Rc::new(HoteMuet),
        LayerInstanceRef::new("globe", None),
        || {},
    );
    let off = spec.create(&ctx);
    assert_eq!(demontes.get(), 0);
    off();
    assert_eq!(demontes.get(), 1);
}

#[test]
fn applies_to_interroge_la_vue_opaque_et_presets_les_instances_en_plus() {
    struct VueNote;
    let spec = LayerSpec::new("annotation", "Annotations", |_ctx: &LayerContext| {
        Box::new(|| {})
    })
    .with_applies_to(|vue: &dyn Any| vue.is::<VueNote>())
    .with_presets(|| {
        vec![LayerPreset {
            id: "relecture".into(),
            name: "Relecture".into(),
            icon: Some("eye".into()),
        }]
    });
    assert!(spec.applies_to(&VueNote));
    assert!(!spec.applies_to(&42_u32));
    let presets = spec.presets();
    assert_eq!(presets.len(), 1);
    assert_eq!(
        LayerInstanceRef::new(&spec.id, Some(presets[0].id.clone())).key,
        "annotation:relecture"
    );
}
