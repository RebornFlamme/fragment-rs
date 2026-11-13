// Port de app/src/ui/core/workspace/View.ts (onPaneMenu) et ItemView.ts : la chaîne du menu « ⋯ ».
// Cas : l'ouvreur déclare l'ordre des sections, la vue AJOUTE ses entrées sans rien ouvrir, une
// vue qui ne dit rien laisse le menu intact, la source est transmise, les clics s'exécutent.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{MenuPane, SECTIONS_PANE, SOURCE_PLUS_D_OPTIONS, View, menu_de_pane};

/// Une vue qui contribue deux entrées et note la source reçue.
struct VueQuiContribue {
    source: Rc<RefCell<String>>,
    clics: Rc<Cell<u32>>,
}

impl View for VueQuiContribue {
    fn get_view_type(&self) -> String {
        "test".to_owned()
    }
    fn get_display_text(&self) -> String {
        "test".to_owned()
    }
    fn on_pane_menu(&self, menu: &mut MenuPane, source: &str) {
        *self.source.borrow_mut() = source.to_owned();
        let c = self.clics.clone();
        menu.add_item(|i| {
            i.set_title("Zoom avant")
                .set_icon("zoom-in")
                .set_section("view")
                .on_click(move || c.set(c.get() + 1));
        });
        menu.add_item(|i| {
            i.set_title("Zoom arrière")
                .set_icon("zoom-out")
                .set_section("view")
                .set_disabled(true);
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn vue() -> (VueQuiContribue, Rc<RefCell<String>>, Rc<Cell<u32>>) {
    let s = Rc::new(RefCell::new(String::new()));
    let c = Rc::new(Cell::new(0));
    (
        VueQuiContribue {
            source: s.clone(),
            clics: c.clone(),
        },
        s,
        c,
    )
}

#[test]
fn l_ouvreur_declare_l_ordre_des_sections_obsidian() {
    let m = menu_de_pane(&VueMuette, SOURCE_PLUS_D_OPTIONS, |_| {});
    assert_eq!(m.sections(), SECTIONS_PANE);
    assert_eq!(
        SECTIONS_PANE,
        ["pane", "action", "view", "info", "system", "danger"]
    );
}

#[test]
fn une_vue_sans_entrees_laisse_le_menu_de_base_intact() {
    let m = menu_de_pane(&VueMuette, "more-options", |m| {
        m.add_item(|i| {
            i.set_title("Diviser à droite").set_section("pane");
        });
    });
    assert_eq!(m.titres(), ["Diviser à droite"]);
}

#[test]
fn la_vue_ajoute_ses_entrees_apres_celles_de_l_ouvreur() {
    let (v, _, _) = vue();
    let m = menu_de_pane(&v, "more-options", |m| {
        m.add_item(|i| {
            i.set_title("Diviser à droite").set_section("pane");
        });
    });
    assert_eq!(
        m.titres(),
        ["Diviser à droite", "Zoom avant", "Zoom arrière"]
    );
    let e = &m.entrees()[1];
    assert_eq!((e.section.as_str(), e.icone.as_str()), ("view", "zoom-in"));
    assert!(m.entrees()[2].desactivee);
}

#[test]
fn la_source_est_transmise_a_la_vue() {
    let (v, source, _) = vue();
    let _ = menu_de_pane(&v, "tab-header", |_| {});
    assert_eq!(*source.borrow(), "tab-header");
}

#[test]
fn le_clic_d_une_entree_contribuee_s_execute() {
    let (v, _, clics) = vue();
    let m = menu_de_pane(&v, "more-options", |_| {});
    (m.entrees()[0].action.clone().unwrap())();
    assert_eq!(clics.get(), 1);
    assert!(m.entrees()[1].action.is_none());
}

#[test]
fn un_sous_menu_est_un_menu_independant() {
    let mut m = MenuPane::new();
    m.add_item(|i| {
        i.set_title("Copier le chemin").set_section("info");
        i.set_submenu().add_item(|s| {
            s.set_title("Depuis la racine du vault");
        });
    });
    let sous = m.entrees()[0].sous_menu.as_ref().unwrap();
    assert_eq!(sous.titres(), ["Depuis la racine du vault"]);
}

/// Une vue qui ne surcharge pas le hook.
struct VueMuette;

impl View for VueMuette {
    fn get_view_type(&self) -> String {
        "muette".to_owned()
    }
    fn get_display_text(&self) -> String {
        "muette".to_owned()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
