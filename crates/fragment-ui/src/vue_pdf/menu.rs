// Port de app/src/ui/views/pdf/PdfView.ts (onPaneMenu : la section « view » du menu de la vue)
//! Le menu de la vue PDF : « Zoom avant » et « Zoom arrière », dans la section `view`. Le même
//! remplissage sert le menu « ⋯ » de l'en-tête (le hook `on_pane_menu` du cœur) et le menu
//! contextuel de la zone des pages.
//!
//! ★ Fragment ne déclare QUE sa section : l'ordre des groupes appartient à l'en-tête, qui ouvre la
//!   chaîne. Ici, `contribuer_au_menu_de_pane` est un mutateur de même nature.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use fragment_core::MenuPane;
use fragment_pdf::vue_modele::Raccourci;

use super::vue::Touche;
use crate::entete::remplir_depuis_pane;
use crate::menu::Menu;

/// La section du menu où la vue PDF range ses entrées.
pub const SECTION_VUE: &str = "view";

/// Ajoute « Zoom avant » / « Zoom arrière » ; un choix dépose le raccourci équivalent dans la file
/// de la vue (jouée à l'image suivante, comme Ctrl + − +).
pub fn contribuer_au_menu_de_pane(menu: &mut MenuPane, touches: &Rc<RefCell<VecDeque<Touche>>>) {
    for (titre, icone, r) in [
        ("Zoom avant", "zoom-in", Raccourci::ZoomPlus),
        ("Zoom arrière", "zoom-out", Raccourci::ZoomMoins),
    ] {
        let file = touches.clone();
        menu.add_item(|i| {
            i.set_title(titre)
                .set_icon(icone)
                .set_section(SECTION_VUE)
                .on_click(move || file.borrow_mut().push_back(Touche::Zoom(r)));
        });
    }
}

/// Le menu contextuel (clic droit) de la zone des pages.
pub fn menu_contextuel(touches: &Rc<RefCell<VecDeque<Touche>>>) -> Menu {
    let mut modele = MenuPane::new();
    modele.add_sections(&[SECTION_VUE]);
    contribuer_au_menu_de_pane(&mut modele, touches);
    let mut menu = Menu::new();
    remplir_depuis_pane(&mut menu, &modele);
    menu
}
