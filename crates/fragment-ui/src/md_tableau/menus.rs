// Port d'app/src/ui/views/markdown/table/tableMenu.ts.
//! Les menus contextuels du tableau, entrée pour entrée comme Obsidian :
//!
//! - poignée de COLONNE : tris, ajouts, déplacements possibles, alignements, dupliquer, supprimer ;
//! - poignée de LIGNE : ajouts, déplacements possibles, dupliquer, supprimer (l'en-tête ne se
//!   supprime ni ne se déplace, rien ne s'ajoute au-dessus de lui) ;
//! - le tableau ou une cellule : `Row ▸` et `Column ▸` (ces mêmes menus, sans les tris de colonne)
//!   puis les tris.
//!
//! Chaque entrée remet une [`Operation`] au tableau, qui l'écrit en UNE transaction (un undo).

use std::rc::Rc;

use fragment_markdown::commandes::menu::{
    ActionMenu, ContexteMenu, EntreeMenu, ItemMenu, SECTIONS_MENU_EDITEUR, modele_menu_editeur,
};
use fragment_markdown::table_model::{Align, SortDir, TableModel, col_count, row_count};

use super::controle::Operation;
use crate::menu::Menu;

/// Ce qu'une entrée appelle : le tableau applique l'opération.
pub type Executer = Rc<dyn Fn(Operation)>;

/// Les sections d'un menu de poignée, dans l'ordre.
pub const SECTIONS_POIGNEE: [&str; 5] = ["sort", "add", "move", "align", "edit"];
/// Les sections ajoutées au menu du tableau et au menu d'édition d'une cellule.
pub const SECTIONS_TABLEAU: [&str; 2] = ["table", "table-sort"];

fn entree(menu: &mut Menu, titre: &str, icone: &str, section: &str, ex: &Executer, op: Operation) {
    let ex = ex.clone();
    menu.add_item(|i| {
        i.set_title(titre)
            .set_icon(icone)
            .set_section(section)
            .on_click(move || ex(op));
    });
}

fn tris(menu: &mut Menu, ex: &Executer, c: usize, section: &str) {
    entree(
        menu,
        "Sort by column (A to Z)",
        "arrow-down-a-z",
        section,
        ex,
        Operation::Trier(c, SortDir::Asc),
    );
    entree(
        menu,
        "Sort by column (Z to A)",
        "arrow-up-z-a",
        section,
        ex,
        Operation::Trier(c, SortDir::Desc),
    );
}

/// Le menu de la colonne `c` ; `avec_tri` : en tête (poignée) ; sans (sous-menu `Column ▸`).
pub fn remplir_menu_colonne(
    menu: &mut Menu,
    m: &TableModel,
    ex: &Executer,
    c: usize,
    avec_tri: bool,
) {
    let n = col_count(m);
    if avec_tri {
        tris(menu, ex, c, "sort");
    }
    entree(
        menu,
        "Add column to the left",
        "between-vertical-start",
        "add",
        ex,
        Operation::AjouterColonne(c),
    );
    entree(
        menu,
        "Add column to the right",
        "between-vertical-end",
        "add",
        ex,
        Operation::AjouterColonne(c + 1),
    );
    // Un déplacement impossible n'est pas proposé.
    if c > 0 {
        let op = Operation::DeplacerColonne { de: c, vers: c - 1 };
        entree(menu, "Move column left", "arrow-left", "move", ex, op);
    }
    if c + 1 < n {
        let op = Operation::DeplacerColonne { de: c, vers: c + 1 };
        entree(menu, "Move column right", "arrow-right", "move", ex, op);
    }
    let actuel = m.aligns.get(c).copied().flatten();
    for (titre, icone, a) in [
        ("Align left", "text-align-start", Align::Left),
        ("Align center", "text-align-center", Align::Center),
        ("Align right", "text-align-end", Align::Right),
    ] {
        let ex = ex.clone();
        menu.add_item(|i| {
            i.set_title(titre)
                .set_icon(icone)
                .set_section("align")
                .set_checked(actuel == Some(a))
                .on_click(move || ex(Operation::Aligner(c, a)));
        });
    }
    entree(
        menu,
        "Duplicate column",
        "copy",
        "edit",
        ex,
        Operation::DupliquerColonne(c),
    );
    let ex = ex.clone();
    menu.add_item(|i| {
        i.set_title("Delete column")
            .set_icon("trash-2")
            .set_section("edit")
            .set_disabled(n <= 1)
            .on_click(move || ex(Operation::SupprimerColonne(c)));
    });
}

/// Le menu de la ligne `r` (0 = l'en-tête).
pub fn remplir_menu_ligne(menu: &mut Menu, m: &TableModel, ex: &Executer, r: usize) {
    let n = row_count(m);
    let entete = r == 0;
    let item = |menu: &mut Menu, titre: &str, icone: &str, section: &str, grise: bool, op| {
        let ex = ex.clone();
        menu.add_item(|i| {
            i.set_title(titre)
                .set_icon(icone)
                .set_section(section)
                .set_disabled(grise)
                .on_click(move || ex(op));
        });
    };
    item(
        menu,
        "Add row above",
        "between-horizontal-start",
        "add",
        entete,
        Operation::AjouterLigne(r),
    );
    item(
        menu,
        "Add row below",
        "between-horizontal-end",
        "add",
        false,
        Operation::AjouterLigne(r + 1),
    );
    if r > 1 {
        let op = Operation::DeplacerLigne { de: r, vers: r - 1 };
        item(menu, "Move row up", "arrow-up", "move", false, op);
    }
    if !entete && r + 1 < n {
        let op = Operation::DeplacerLigne { de: r, vers: r + 1 };
        item(menu, "Move row down", "arrow-down", "move", false, op);
    }
    item(
        menu,
        "Duplicate row",
        "copy",
        "edit",
        entete,
        Operation::DupliquerLigne(r),
    );
    item(
        menu,
        "Delete row",
        "trash-2",
        "edit",
        entete,
        Operation::SupprimerLigne(r),
    );
}

/// `Row ▸` / `Column ▸` (section `table`) puis les tris (section `table-sort`).
pub fn ajouter_sections_tableau(
    menu: &mut Menu,
    m: &TableModel,
    ex: &Executer,
    r: usize,
    c: usize,
) {
    menu.add_item(|i| {
        i.set_title("Row").set_icon("table").set_section("table");
        remplir_menu_ligne(i.set_submenu(), m, ex, r);
    });
    menu.add_item(|i| {
        i.set_title("Column").set_icon("table").set_section("table");
        remplir_menu_colonne(i.set_submenu(), m, ex, c, false);
    });
    tris(menu, ex, c, "table-sort");
}

/// Le menu de la poignée (colonne ou ligne) `index`.
#[must_use]
pub fn menu_poignee(m: &TableModel, ex: &Executer, colonne: bool, index: usize) -> Menu {
    let mut menu = Menu::new();
    menu.add_sections(&SECTIONS_POIGNEE);
    if colonne {
        remplir_menu_colonne(&mut menu, m, ex, index, true);
    } else {
        remplir_menu_ligne(&mut menu, m, ex, index);
    }
    menu
}

/// Le menu du tableau (clic droit hors du texte d'une cellule).
#[must_use]
pub fn menu_tableau(m: &TableModel, ex: &Executer, r: usize, c: usize) -> Menu {
    let mut menu = Menu::new();
    menu.add_sections(&SECTIONS_TABLEAU);
    ajouter_sections_tableau(&mut menu, m, ex, r, c);
    menu
}

/// Ce que fait une entrée du menu d'édition d'une cellule (la vue Markdown l'applique à l'éditeur
/// de la cellule : `executer_action`).
pub type ExecuterEdition = Rc<dyn Fn(ActionMenu)>;

/// Le menu d'un clic droit DANS le texte d'une cellule : le menu d'édition de la note (éditeur
/// imbriqué : blocs grisés), puis les sections du tableau.
#[must_use]
pub fn menu_cellule(
    m: &TableModel,
    ex: &Executer,
    edition: &ExecuterEdition,
    ctx: ContexteMenu,
    r: usize,
    c: usize,
) -> Menu {
    let mut menu = Menu::new();
    menu.add_sections(&SECTIONS_MENU_EDITEUR);
    let poser = |i: &mut crate::menu::MenuItem, e: &EntreeMenu, edition: &ExecuterEdition| {
        let (ed, action) = (edition.clone(), e.action);
        i.set_title(e.titre)
            .set_icon(e.icone)
            .set_section(e.section)
            .set_disabled(e.desactivee)
            .on_click(move || ed(action));
    };
    for item in modele_menu_editeur(ContexteMenu {
        imbrique: true,
        ..ctx
    }) {
        match item {
            ItemMenu::Entree(e) => {
                menu.add_item(|i| poser(i, &e, edition));
            }
            ItemMenu::SousMenu(s) => {
                menu.add_item(|i| {
                    i.set_title(s.titre)
                        .set_icon(s.icone)
                        .set_section(s.section)
                        .set_disabled(s.desactive);
                    let sous = i.set_submenu();
                    for e in &s.entrees {
                        sous.add_item(|j| poser(j, e, edition));
                    }
                });
            }
        }
    }
    ajouter_sections_tableau(&mut menu, m, ex, r, c);
    menu
}
