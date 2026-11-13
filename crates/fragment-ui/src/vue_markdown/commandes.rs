// Port de app/src/ui/views/markdown/EditorCommandsPlugin.ts (l'enregistrement : les commandes
// `editor:*` du modèle, plus `editor:save-file` qui a besoin de la vue) et de l'enregistrement de
// la vue `markdown` (registre des vues, extension `md`).
//! Le plugin cœur « Markdown » : la vue, l'extension `md`, les commandes d'éditeur.
//!
//! Les commandes `editor:*` du modèle passent par `CommandHost::active_editor` : la vue active
//! expose son éditeur (`View::editor_context`), absent en lecture seule (commande indisponible).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::{
    App as _, AppCoeur, Command, Disposer, Hotkey, Modifier, NouvelleFeuille, Workspace,
};
use fragment_markdown::commandes::actions::commandes_editeur;
use fragment_markdown::commandes::menu::ActionMenu;

use super::interaction::{fichier_du_lien, lien_a_suivre, poster};
use super::vue::{ContexteMarkdown, Liens, MARKDOWN_VIEW_TYPE, VueMarkdown, fabrique};

/// Le propriétaire des enregistrements (l'id du plugin cœur).
pub const PLUGIN_MARKDOWN: &str = "markdown";
/// L'extension que la vue ouvre.
pub const EXTENSION_MD: &str = "md";
/// `editor:save-file`.
pub const COMMANDE_ENREGISTRER: &str = "editor:save-file";
/// `editor:paste-plain`.
pub const COMMANDE_COLLER_BRUT: &str = "editor:paste-plain";
/// `editor:follow-link`.
pub const COMMANDE_SUIVRE_LIEN: &str = "editor:follow-link";

/// La vue Markdown de la feuille active, s'il y en a une.
fn avec_vue_active<R>(
    ws: &Weak<RefCell<Workspace>>,
    f: impl FnOnce(&VueMarkdown) -> Option<R>,
) -> Option<R> {
    let ws = ws.upgrade()?;
    let mut w = ws.try_borrow_mut().ok()?;
    let leaf = w.active_leaf()?;
    let vue = w.view_mut(leaf)?;
    let md = vue.as_any().downcast_ref::<VueMarkdown>()?;
    f(md)
}

/// Les commandes d'éditeur (via `active_editor` du cœur) et `editor:save-file`.
pub fn commandes(ws: &Weak<RefCell<Workspace>>) -> Vec<Command> {
    let mut v: Vec<Command> = commandes_editeur();
    let (ws0, ws2, ws3) = (ws.clone(), ws.clone(), ws.clone());
    v.push(
        Command::new(COMMANDE_ENREGISTRER, "Enregistrer le fichier")
            .with_hotkeys(vec![Hotkey::new(&[Modifier::Mod], "s")])
            .with_check_callback(move |checking| {
                let liens: Option<Liens> = avec_vue_active(&ws0, |v| Some(v.liens().clone()));
                let Some(liens) = liens else {
                    return Some(false);
                };
                if !checking {
                    liens.ecrire();
                }
                Some(true)
            }),
    );
    v.push(
        Command::new(COMMANDE_COLLER_BRUT, "Coller en texte brut").with_check_callback(
            move |checking| {
                // ★ Sans éditeur (lecture seule) : indisponible, comme toute commande d'éditeur.
                let liens =
                    avec_vue_active(&ws2, |v| (!v.lecture_seule()).then(|| v.liens().clone()));
                let Some(liens) = liens else {
                    return Some(false);
                };
                if !checking {
                    poster(&liens, ActionMenu::CollerTexteBrut);
                }
                Some(true)
            },
        ),
    );
    v.push(
        Command::new(COMMANDE_SUIVRE_LIEN, "Suivre le lien sous le curseur")
            .with_hotkeys(vec![Hotkey::new(&[Modifier::Alt], "Enter")])
            .with_check_callback(move |checking| {
                let cible = avec_vue_active(&ws3, |v| {
                    let l = v.liens().clone();
                    lien_a_suivre(&l).map(|c| (l, c))
                });
                let Some((liens, cible)) = cible else {
                    return Some(false);
                };
                if !checking {
                    suivre(&ws3, &liens, &cible);
                }
                Some(true)
            }),
    );
    v
}

/// Ouvre la cible du lien dans la feuille active (`openLinkText`). Un lien mort ou externe n'ouvre
/// rien. ★ Le workspace n'est emprunté qu'APRÈS la lecture de la vue (la commande y est appelée).
fn suivre(ws: &Weak<RefCell<Workspace>>, liens: &Liens, cible: &str) {
    let Some(f) = fichier_du_lien(liens, cible) else {
        eprintln!("fragment : markdown : lien sans fichier : {cible}");
        return;
    };
    if let Some(ws) = ws.upgrade()
        && let Ok(mut w) = ws.try_borrow_mut()
    {
        w.open_document(&f.path, NouvelleFeuille::Non, None);
    }
}

/// Enregistre la vue `markdown`, l'extension `md` et les commandes. Rend les retraits.
pub fn enregistrer(app: &AppCoeur, ctx: ContexteMarkdown) -> Vec<Disposer> {
    let mut retraits = vec![
        app.view_registry().register(
            MARKDOWN_VIEW_TYPE,
            fabrique(ctx, app.keymap().clone()),
            PLUGIN_MARKDOWN,
        ),
        app.extension_registry().register(
            EXTENSION_MD,
            MARKDOWN_VIEW_TYPE.to_owned(),
            PLUGIN_MARKDOWN,
        ),
    ];
    let ws = Rc::downgrade(app.workspace());
    for c in commandes(&ws) {
        retraits.push(app.commands().add_command(c, PLUGIN_MARKDOWN));
    }
    retraits
}
