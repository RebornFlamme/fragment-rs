// Port de app/src/ui/views/markdown/EditorCommandsPlugin.ts (l'enregistrement : les commandes
// `editor:*` du modèle, plus `editor:save-file` qui a besoin de la vue) et de l'enregistrement de
// la vue `markdown` (registre des vues, extension `md`).
//! Le plugin cœur « Markdown » : la vue, l'extension `md`, les commandes d'éditeur.
//!
//! Les commandes `editor:*` du modèle passent par `CommandHost::active_editor` : la vue active
//! expose son éditeur (`View::editor_context`), absent en lecture seule (commande indisponible).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::{App as _, AppCoeur, Command, Disposer, Hotkey, Modifier, Workspace};
use fragment_markdown::commandes::actions::commandes_editeur;

use super::vue::{ContexteMarkdown, Liens, MARKDOWN_VIEW_TYPE, VueMarkdown, fabrique};

/// Le propriétaire des enregistrements (l'id du plugin cœur).
pub const PLUGIN_MARKDOWN: &str = "markdown";
/// L'extension que la vue ouvre.
pub const EXTENSION_MD: &str = "md";
/// `editor:save-file`.
pub const COMMANDE_ENREGISTRER: &str = "editor:save-file";

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
    let ws = ws.clone();
    v.push(
        Command::new(COMMANDE_ENREGISTRER, "Enregistrer le fichier")
            .with_hotkeys(vec![Hotkey::new(&[Modifier::Mod], "s")])
            .with_check_callback(move |checking| {
                let liens: Option<Liens> = avec_vue_active(&ws, |v| Some(v.liens().clone()));
                let Some(liens) = liens else {
                    return Some(false);
                };
                if !checking {
                    liens.ecrire();
                }
                Some(true)
            }),
    );
    v
}

/// Enregistre la vue `markdown`, l'extension `md` et les commandes. Rend les retraits.
pub fn enregistrer(app: &AppCoeur, ctx: ContexteMarkdown) -> Vec<Disposer> {
    let mut retraits = vec![
        app.view_registry()
            .register(MARKDOWN_VIEW_TYPE, fabrique(ctx), PLUGIN_MARKDOWN),
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
