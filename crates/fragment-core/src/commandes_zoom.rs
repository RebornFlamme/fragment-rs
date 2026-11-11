// Port de app/src/ui/coreCommands/AppCommandsPlugin.ts (les commandes de zoom)
//
// `app:zoom-in`, `app:zoom-out`, `app:reset-zoom`. Elles n'écrivent QUE le réglage
// (`Apparence::set_zoom`) : `appliquer_apparence` pose le zoom — un seul chemin.
//
// Raccourcis en touches PHYSIQUES (`code`), pour l'AZERTY : « + » y est Maj+« = », « - » est sur
// la touche 6, et Ctrl+0 y produit « à ». Chaque combinaison existe avec et sans Maj pour
// zoom-in (la touche « = » de l'AZERTY exige Maj pour faire « + »).
// ★ Dans un PDF, la portée de la vue passe AVANT la racine : Ctrl+= y reste le zoom du document
//   (c'est la vue PDF qui le déclare, pas ici).

use std::rc::Rc;

use crate::apparence::Apparence;
use crate::command::{Command, CommandManager};
use crate::keymap::types::{Hotkey, Modifier};
use crate::registry::Disposer;
use crate::zoom::{SensZoom, ZOOM_DEFAUT, zoom_suivant};

/// Le propriétaire des commandes `app:*`.
const OWNER_APP: &str = "app";

fn hotkeys(touches: &[&str], avec_maj: bool) -> Vec<Hotkey> {
    let mut v = Vec::new();
    for t in touches {
        v.push(Hotkey::new(&[Modifier::Mod], t));
        if avec_maj {
            v.push(Hotkey::new(&[Modifier::Mod, Modifier::Shift], t));
        }
    }
    v
}

/// Inscrit les trois commandes de zoom ; rend leurs retraits.
pub fn enregistrer_commandes_zoom(
    commands: &CommandManager,
    apparence: &Rc<Apparence>,
) -> Vec<Disposer> {
    let pas = |sens: SensZoom| {
        let a = apparence.clone();
        move || a.set_zoom(zoom_suivant(a.zoom(), sens))
    };
    let a = apparence.clone();
    vec![
        commands.add_command(
            Command::new("app:zoom-in", "Zoom avant")
                .with_hotkeys(hotkeys(&["Equal", "NumpadAdd", "+"], true))
                .with_callback(pas(SensZoom::Avant)),
            OWNER_APP,
        ),
        commands.add_command(
            Command::new("app:zoom-out", "Zoom arrière")
                .with_hotkeys(hotkeys(&["Minus", "NumpadSubtract", "-"], false))
                .with_callback(pas(SensZoom::Arriere)),
            OWNER_APP,
        ),
        commands.add_command(
            Command::new("app:reset-zoom", "Réinitialiser le zoom")
                .with_hotkeys(hotkeys(&["Digit0", "Numpad0"], false))
                .with_callback(move || a.set_zoom(ZOOM_DEFAUT)),
            OWNER_APP,
        ),
    ]
}
