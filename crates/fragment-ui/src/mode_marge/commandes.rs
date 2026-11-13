// Port de app/src/ui/coreCommands/marginMode.ts (`installerMargeDeLecture` : les trois commandes
// sur la feuille active, leur retrait) ; le tableau des commandes est le modèle déjà porté dans
// `fragment_plugins::margin_mode`.
//
//! Les commandes `workspace:open-margin` (balayage `SwipeRight`), `workspace:close-margin`
//! (`SwipeLeft`) et `workspace:toggle-margin` (palette, bouton de l'en-tête), branchées sur le vrai
//! Workspace et sur l'état [`MargeDeLecture`].
//!
//! Comme tout raccourci, le geste agit sur la feuille ACTIVE ; il passe par les portées comme une
//! frappe, donc une vue qui lie `SwipeRight` dans sa portée le prend avant la commande.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::{CheckResult, Command, CommandManager, Disposer, Hotkey, ItemId, Workspace};
use fragment_plugins::margin_mode::{COMMANDES_MARGE, CommandeMarge, appliquer};

use super::etat::MargeDeLecture;

/// Le propriétaire des commandes (le plugin interne « Espace de travail »).
pub const OWNER: &str = "workspace";

/// L'id complet d'une commande de marge (`workspace:` + son id).
pub fn id_complet(c: &CommandeMarge) -> String {
    format!("{OWNER}:{}", c.id)
}

/// La feuille active, si sa vue montre un document ET qu'elle a un parent dans l'arbre (une feuille
/// du workspace, pas une vue embarquée ni un widget) : `(feuille, (document, parent))`.
fn feuille_active(w: &Workspace) -> Option<(ItemId, (bool, bool))> {
    let leaf = w.active_leaf()?;
    let document = w.leaf(leaf)?.view().get_document().is_some();
    let parent = w.arbre().parent(leaf).is_some();
    Some((leaf, (document, parent)))
}

fn verifier(
    ws: &Weak<RefCell<Workspace>>,
    marge: &Rc<MargeDeLecture>,
    commande: &CommandeMarge,
    checking: bool,
) -> CheckResult {
    // L'emprunt est rendu AVANT d'agir : un abonné au changement peut relire le Workspace.
    let trouve = ws
        .upgrade()
        .and_then(|ws| ws.try_borrow().ok().and_then(|w| feuille_active(&w)));
    let Some((leaf, vue)) = trouve else {
        return Some(false);
    };
    let Some(voulu) = appliquer(commande, Some(vue), marge.est_ouverte(leaf)) else {
        return Some(false);
    };
    if !checking {
        marge.poser(leaf, voulu);
    }
    Some(true)
}

/// Enregistre les trois commandes. Les retraits rendus enlèvent les commandes ; le dernier ferme
/// la marge de toutes les feuilles (le `register` de Fragment, au déchargement du plugin).
pub fn enregistrer_commandes(
    commandes: &CommandManager,
    workspace: &Rc<RefCell<Workspace>>,
    marge: &Rc<MargeDeLecture>,
) -> Vec<Disposer> {
    let mut retraits: Vec<Disposer> = COMMANDES_MARGE
        .iter()
        .map(|spec| {
            let ws = Rc::downgrade(workspace);
            let m = Rc::clone(marge);
            let s = *spec;
            let mut c = Command::new(&id_complet(spec), spec.name)
                .with_check_callback(move |checking| verifier(&ws, &m, &s, checking));
            c.icon = Some(spec.icon.to_owned());
            if let Some(touche) = spec.hotkey {
                c = c.with_hotkeys(vec![Hotkey::new(&[], touche)]);
            }
            commandes.add_command(c, OWNER)
        })
        .collect();
    let m = Rc::clone(marge);
    retraits.push(Box::new(move || m.tout_fermer()));
    retraits
}
