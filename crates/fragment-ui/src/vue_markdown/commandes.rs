// Port de app/src/ui/views/markdown/EditorCommandsPlugin.ts (l'enregistrement : les commandes
// `editor:*` du modèle, plus `editor:save-file` qui a besoin de la vue) et de l'enregistrement de
// la vue `markdown` (registre des vues, extension `md`).
//! Le plugin cœur « Markdown » : la vue, l'extension `md`, les commandes d'éditeur.
//!
//! ★ Le cœur ne sait pas encore donner l'éditeur actif (`CommandHost::active_editor` rend
//!   `None`) : les `editor_callback` du modèle ne se déclencheraient jamais. Chaque commande est
//!   donc réenveloppée en `check_callback` qui cherche ELLE-MÊME la vue Markdown de la feuille
//!   active et lui demande son contexte d'éditeur (absent en lecture seule : la commande est
//!   alors indisponible, comme chez Fragment).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::command::{EditorCallback, EditorContext};
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

/// Le contexte d'éditeur de la vue active (`None` : pas de note, ou note en lecture seule).
pub fn editeur_actif(ws: &Weak<RefCell<Workspace>>) -> Option<EditorContext> {
    avec_vue_active(ws, VueMarkdown::contexte_editeur)
}

/// Réenveloppe une commande d'éditeur pour qu'elle trouve son éditeur dans le Workspace.
fn sur_la_note_active(c: Command, ws: Weak<RefCell<Workspace>>) -> Command {
    let Some(cb): Option<EditorCallback> = c.editor_callback.clone() else {
        return c;
    };
    let mut n = Command::new(&c.id, &c.name).with_check_callback(move |checking| {
        let Some(ctx) = editeur_actif(&ws) else {
            return Some(false);
        };
        if !checking {
            cb(&ctx);
        }
        Some(true)
    });
    n.hotkeys = c.hotkeys;
    n.repeatable = c.repeatable;
    n.icon = c.icon;
    n
}

/// Les commandes d'éditeur, branchées sur la note active.
pub fn commandes(ws: &Weak<RefCell<Workspace>>) -> Vec<Command> {
    let mut v: Vec<Command> = commandes_editeur()
        .into_iter()
        .map(|c| sur_la_note_active(c, ws.clone()))
        .collect();
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
