// Port de app/src/ui/coreCommands/WorkspaceCommandsPlugin.ts (les commandes `workspace:*`) et
// de app/src/ui/coreCommands/AppCommandsPlugin.ts (les commandes `app:*` du workspace : docks
// et historique ; le zoom vient avec fr-t1-reglages).
//
// Ces commandes n'apportent AUCUNE mécanique : découper, épingler, fermer, montrer un onglet
// existent dans le Workspace (le glisser, le bouton ✕, les menus les appellent). Elles leur
// donnent un id — donc une place dans la palette et un raccourci.
//
// Pas ici, et pourquoi :
// - `workspace:open` (CLI) : sa validation est dans fragment-plugins (`workspace_commands`) ;
//   son attente de stabilité du défilement est du rendu.
// - la marge de lecture : un autre module de Fragment (`marginMode.ts`).
// - `app:zoom-*` (fr-t1-reglages), `app:reload` (l'hôte), `app:delete-file` (le Vault, T2).
// ★ Le TABLEAU de ces commandes existe aussi dans fragment-plugins (le modèle d'avant le
//   workspace vivant) ; le cœur ne peut pas en dépendre, d'où la seconde copie, branchée sur
//   le vrai Workspace. À fusionner quand fragment-plugins basculera dessus.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::SplitDirection;
use super::ViewState;
use super::arbre::ItemId;
use super::espace::{NouvelleFeuille, Workspace};
use crate::command::{CheckResult, Command, CommandManager};
use crate::keymap::types::{Hotkey, Modifier};
use crate::registry::Disposer;

/// Le propriétaire des commandes `workspace:*`.
pub const OWNER_WORKSPACE: &str = "workspace";
/// Le propriétaire des commandes `app:*`.
pub const OWNER_APP: &str = "app";

type Ws = Weak<RefCell<Workspace>>;

/// Emprunte le Workspace le temps d'agir. ★ `try_borrow_mut` : une commande lancée pendant
/// qu'il est déjà emprunté (depuis un de ses écouteurs) est indisponible, elle ne panique pas.
fn avec<R>(ws: &Ws, f: impl FnOnce(&mut Workspace) -> Option<R>) -> Option<R> {
    let ws = ws.upgrade()?;
    let mut w = ws.try_borrow_mut().ok()?;
    f(&mut w)
}

/// La feuille active du CENTRE : les docks ne sont pas des onglets de document.
fn feuille_active(w: &Workspace) -> Option<ItemId> {
    w.active_leaf()
        .filter(|&a| w.arbre().get_root(a) == w.root_split())
}

/// Une commande disponible dès qu'il y a une feuille active au centre.
fn sur_feuille(
    ws: &Ws,
    agir: impl Fn(&mut Workspace, ItemId) + 'static,
) -> impl Fn(bool) -> CheckResult + 'static {
    let ws = ws.clone();
    move |checking| {
        let fait = avec(&ws, |w| {
            let leaf = feuille_active(w)?;
            if !checking {
                agir(w, leaf);
            }
            Some(())
        });
        Some(fait.is_some())
    }
}

/// Une commande qui agit sur la PILE de la feuille active (au moins deux onglets) et sur le
/// rang de l'active dedans.
fn sur_pile(
    ws: &Ws,
    agir: impl Fn(&mut Workspace, ItemId, usize) + 'static,
) -> impl Fn(bool) -> CheckResult + 'static {
    let ws = ws.clone();
    move |checking| {
        let fait = avec(&ws, |w| {
            let leaf = feuille_active(w)?;
            let pile = w.arbre().parent(leaf).filter(|&p| w.arbre().est_pile(p))?;
            let enfants = w.arbre().children(pile);
            if enfants.len() < 2 {
                return None;
            }
            let i = enfants.iter().position(|&c| c == leaf)?;
            if !checking {
                agir(w, pile, i);
            }
            Some(())
        });
        Some(fait.is_some())
    }
}

/// Va à l'onglet voisin (`delta` = +1 suivant, −1 précédent), cyclique.
///
/// ★ On repart de l'onglet RÉELLEMENT VISIBLE de la pile, puis on montre la cible ET on
///   l'active : les deux vérités (onglet visible, feuille active) restent en phase. (Le défaut
///   corrigé dans Fragment : repartir de la feuille active, qui avait dérivé, faisait rebondir
///   le défilement sur le même onglet.)
pub fn aller_a_onglet_voisin(w: &mut Workspace, pile: ItemId, delta: isize) {
    let n = w.arbre().children(pile).len();
    if n == 0 {
        return;
    }
    let courant = w.arbre().active_index(pile).unwrap_or(0);
    let n_i = n as isize;
    let cible = (((courant as isize + delta) % n_i + n_i) % n_i) as usize;
    w.select_tab(pile, cible);
    let leaf = w.arbre().children(pile).get(cible).copied();
    w.set_active_leaf(leaf);
}

/// ★ Le split d'Obsidian ouvre la MÊME vue dans le nouveau pane : on divise pour voir deux
///   endroits d'un même document, pas pour avoir un pane vide. L'épinglage n'est pas copié.
fn diviser(w: &mut Workspace, leaf: ItemId, direction: SplitDirection) {
    let Some(vs) = w.leaf(leaf).map(|l| l.get_view_state()) else {
        return;
    };
    let Some(nouvelle) = w.create_leaf_by_split(leaf, direction) else {
        return;
    };
    w.set_view_state(
        nouvelle,
        ViewState {
            type_: vs.type_,
            state: vs.state,
            active: Some(true),
            pinned: None,
        },
        None,
    );
}

fn ajouter(
    commands: &CommandManager,
    id: &str,
    name: &str,
    hotkeys: Vec<Hotkey>,
    check: impl Fn(bool) -> CheckResult + 'static,
    owner: &str,
) -> Disposer {
    commands.add_command(
        Command::new(id, name)
            .with_check_callback(check)
            .with_hotkeys(hotkeys),
        owner,
    )
}

/// Inscrit les commandes `workspace:*` ; rend leurs retraits.
pub fn enregistrer_commandes_workspace(
    commands: &CommandManager,
    workspace: &Rc<RefCell<Workspace>>,
) -> Vec<Disposer> {
    use Modifier::{Ctrl, Mod, Shift};
    let ws = Rc::downgrade(workspace);
    let w2 = ws.clone();
    let nouvel_onglet = move |checking: bool| -> CheckResult {
        if checking {
            return Some(true);
        }
        let fait = avec(&w2, |w| {
            let leaf = w.get_leaf(NouvelleFeuille::Onglet).ok()?;
            w.set_active_leaf(Some(leaf));
            Some(())
        });
        Some(fait.is_some())
    };
    vec![
        ajouter(
            commands,
            "workspace:split-vertical",
            "Diviser à droite",
            vec![],
            sur_feuille(&ws, |w, l| diviser(w, l, SplitDirection::Vertical)),
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:split-horizontal",
            "Diviser en bas",
            vec![],
            sur_feuille(&ws, |w, l| diviser(w, l, SplitDirection::Horizontal)),
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:new-tab",
            "Nouvel onglet",
            vec![Hotkey::new(&[Mod], "t")],
            nouvel_onglet,
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:next-tab",
            "Onglet suivant",
            vec![Hotkey::new(&[Ctrl], "Tab")],
            sur_pile(&ws, |w, p, _| aller_a_onglet_voisin(w, p, 1)),
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:previous-tab",
            "Onglet précédent",
            vec![Hotkey::new(&[Ctrl, Shift], "Tab")],
            sur_pile(&ws, |w, p, _| aller_a_onglet_voisin(w, p, -1)),
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:close",
            "Fermer l’onglet",
            vec![Hotkey::new(&[Mod], "w")],
            sur_feuille(&ws, Workspace::detach),
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:close-others",
            "Fermer les autres onglets",
            vec![],
            sur_pile(&ws, |w, p, i| {
                let autres: Vec<ItemId> = w
                    .arbre()
                    .children(p)
                    .iter()
                    .enumerate()
                    .filter(|&(j, _)| j != i)
                    .map(|(_, &c)| c)
                    .collect();
                for c in autres {
                    w.detach(c);
                }
            }),
            OWNER_WORKSPACE,
        ),
        ajouter(
            commands,
            "workspace:toggle-pin",
            "Épingler / désépingler",
            vec![],
            sur_feuille(&ws, Workspace::toggle_pinned),
            OWNER_WORKSPACE,
        ),
    ]
}

/// Inscrit les commandes `app:*` du workspace (docks, historique) ; rend leurs retraits.
pub fn enregistrer_commandes_app(
    commands: &CommandManager,
    workspace: &Rc<RefCell<Workspace>>,
) -> Vec<Disposer> {
    use Modifier::{Ctrl, Shift};
    let ws = Rc::downgrade(workspace);
    let dock = |gauche: bool| {
        let ws = ws.clone();
        move |checking: bool| -> CheckResult {
            if checking {
                return Some(true);
            }
            let fait = avec(&ws, |w| {
                let d = if gauche {
                    w.left_split()
                } else {
                    w.right_split()
                };
                w.toggle(d);
                Some(())
            });
            Some(fait.is_some())
        }
    };
    let historique = |arriere: bool| {
        let ws = ws.clone();
        move |checking: bool| -> CheckResult {
            let fait = avec(&ws, |w| {
                let leaf = w.active_leaf()?;
                let h = &w.leaf(leaf)?.history;
                let possible = if arriere {
                    h.can_go_back()
                } else {
                    h.can_go_forward()
                };
                if !possible {
                    return None;
                }
                if !checking {
                    if arriere {
                        w.go_back(leaf);
                    } else {
                        w.go_forward(leaf);
                    }
                }
                Some(())
            });
            Some(fait.is_some())
        }
    };
    vec![
        // Ctrl+& : sur AZERTY, « & » est la touche 1 NON majuscule — la frappe ne porte que Ctrl.
        ajouter(
            commands,
            "app:toggle-left-sidebar",
            "Afficher / masquer le panneau de gauche",
            vec![Hotkey::new(&[Ctrl], "&")],
            dock(true),
            OWNER_APP,
        ),
        // Ctrl+% : sur AZERTY, « % » EXIGE Maj ; les modificateurs se comparent à l'identique,
        // il faut donc déclarer Maj.
        ajouter(
            commands,
            "app:toggle-right-sidebar",
            "Afficher / masquer le panneau de droite",
            vec![Hotkey::new(&[Ctrl, Shift], "%")],
            dock(false),
            OWNER_APP,
        ),
        // Les boutons ← → de l'en-tête de vue les EXÉCUTENT. Pas de raccourci : le Mod+Alt+←
        // d'Obsidian est AltGr sous Windows.
        ajouter(
            commands,
            "app:go-back",
            "Revenir en arrière",
            vec![],
            historique(true),
            OWNER_APP,
        ),
        ajouter(
            commands,
            "app:go-forward",
            "Aller en avant",
            vec![],
            historique(false),
            OWNER_APP,
        ),
    ]
}
