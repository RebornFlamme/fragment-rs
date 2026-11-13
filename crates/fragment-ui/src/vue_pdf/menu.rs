// Port de app/src/ui/views/pdf/PdfView.ts (onPaneMenu : la section « view » du menu de la vue)
//! Le menu de la vue PDF : « Zoom avant » et « Zoom arrière », dans la section `view`. Le même
//! remplissage sert le menu contextuel de la zone des pages et, une fois le hook de menu de pane
//! ajouté au cœur (dette notée dans `parite.d`), le menu « ⋯ » de l'en-tête.
//!
//! ★ Fragment ne déclare QUE sa section : l'ordre des groupes appartient à l'en-tête, qui ouvre la
//!   chaîne. Ici, `remplir_menu_vue` est un mutateur de même nature.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use fragment_pdf::vue_modele::Raccourci;

use super::vue::Touche;
use crate::menu::Menu;

/// La section du menu où la vue PDF range ses entrées.
pub const SECTION_VUE: &str = "view";

/// Ajoute « Zoom avant » / « Zoom arrière » ; un choix dépose le raccourci équivalent dans la file
/// de la vue (jouée à l'image suivante, comme Ctrl + − +).
pub fn remplir_menu_vue(menu: &mut Menu, touches: &Rc<RefCell<VecDeque<Touche>>>) {
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
    let mut menu = Menu::new();
    menu.add_sections(&[SECTION_VUE]);
    remplir_menu_vue(&mut menu, touches);
    menu
}
