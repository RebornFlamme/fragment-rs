// Port de app/src/ui/core/keymap/Keymap.ts (l'écoute `keydown` de la fenêtre, qui tend chaque frappe
// à la portée du sommet) et de HotkeyManager.ts (la liaison attrape-tout de la racine).
//
//! Le pont clavier : une frappe GPUI devient la `KeyEvent` du cœur et part dans le [`Keymap`] de la
//! fenêtre (portées empilées, Échap, comparaison stricte des modificateurs : tout cela est dans le
//! cœur, ce module ne fait que traduire et router).
//!
//! - [`evenement_de`] : `Keystroke` de GPUI → [`KeyEvent`] (touche logique `key`, touche physique
//!   `code`, modificateurs) ;
//! - [`traiter`] : la frappe dans le `Keymap` ; vrai si une portée l'a consommée ;
//! - [`brancher`] : la fenêtre et son `Keymap`, servis par UN intercepteur de l'application.
//!
//! ★ L'intercepteur (`App::intercept_keystrokes`) passe AVANT les actions et les écouteurs de
//!   GPUI, comme l'écoute en phase de capture de Fragment : une frappe consommée par le Keymap
//!   (`Some(false)`) est arrêtée (`stop_propagation`) et n'atteint pas le champ de saisie dessous ;
//!   une frappe non reconnue (une lettre tapée dans la palette, ↑↓) continue son chemin.
//! ★ GPUI ne donne pas la touche PHYSIQUE (`KeyboardEvent.code`) : on la déduit de `key`, qui est
//!   « le caractère imprimé sur la touche » en équivalent ASCII. Les liaisons du cœur en `code`
//!   (`Equal`, `Minus`, `Digit0` : le zoom, pensé pour l'AZERTY) se résolvent ainsi. Le pavé
//!   numérique (`NumpadAdd`…) n'est pas distingué par GPUI : n.a. (`parite.toml`).
//! ★ GPUI ne transmet pas l'auto-répétition à l'intercepteur : `repeat` reste faux.

use std::collections::HashMap;
use std::rc::Rc;

use fragment_core::{CommandManager, KeyEvent, Keymap, Modifiers};
use gpui_kit::{App, Global, Keystroke, Window, WindowId};

/// La touche LOGIQUE du DOM (`KeyboardEvent.key`) pour un nom de touche de GPUI.
fn nom_dom(cle: &str) -> Option<&'static str> {
    Some(match cle {
        "escape" => "Escape",
        "enter" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "insert" => "Insert",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "space" => " ",
        "control" => "Control",
        "shift" => "Shift",
        "alt" => "Alt",
        "platform" => "Meta",
        "f1" => "F1",
        "f2" => "F2",
        "f3" => "F3",
        "f4" => "F4",
        "f5" => "F5",
        "f6" => "F6",
        "f7" => "F7",
        "f8" => "F8",
        "f9" => "F9",
        "f10" => "F10",
        "f11" => "F11",
        "f12" => "F12",
        _ => return None,
    })
}

/// La touche PHYSIQUE (`KeyboardEvent.code`) déduite du nom de touche de GPUI.
pub fn touche_physique(cle: &str) -> String {
    let mut lettres = cle.chars();
    if let (Some(c), None) = (lettres.next(), lettres.next()) {
        if c.is_ascii_alphabetic() {
            return format!("Key{}", c.to_ascii_uppercase());
        }
        if c.is_ascii_digit() {
            return format!("Digit{c}");
        }
        let code = match c {
            '=' => "Equal",
            '-' => "Minus",
            '[' => "BracketLeft",
            ']' => "BracketRight",
            ';' => "Semicolon",
            '\'' => "Quote",
            ',' => "Comma",
            '.' => "Period",
            '/' => "Slash",
            '\\' => "Backslash",
            '`' => "Backquote",
            _ => return cle.to_owned(),
        };
        return code.to_owned();
    }
    match cle {
        "space" => "Space".to_owned(),
        autre => nom_dom(autre).map_or_else(|| autre.to_owned(), str::to_owned),
    }
}

/// La touche LOGIQUE : le nom DOM d'une touche nommée ; sinon le caractère produit (`key_char`,
/// « + » pour Maj+« = »), ou, quand GPUI n'en donne pas (avec Ctrl), le caractère de la touche,
/// en capitale sous Maj comme le fait le DOM pour une lettre.
pub fn touche_logique(k: &Keystroke) -> String {
    if let Some(nom) = nom_dom(&k.key) {
        return nom.to_owned();
    }
    if let Some(c) = &k.key_char
        && c.chars().count() == 1
        && !c.chars().all(char::is_control)
    {
        return c.clone();
    }
    if k.modifiers.shift && k.key.chars().count() == 1 {
        return k.key.to_uppercase();
    }
    k.key.clone()
}

/// La frappe GPUI en `KeyEvent` du cœur.
pub fn evenement_de(k: &Keystroke) -> KeyEvent {
    KeyEvent {
        key: touche_logique(k),
        code: touche_physique(&k.key),
        repeat: false,
        mods: Modifiers {
            ctrl: k.modifiers.control,
            meta: k.modifiers.platform,
            alt: k.modifiers.alt,
            shift: k.modifiers.shift,
        },
    }
}

/// Tend la frappe au `Keymap` ; vrai si une portée l'a consommée (l'hôte l'arrête alors).
pub fn traiter(keymap: &Keymap, k: &Keystroke) -> bool {
    keymap.handle_key_event(&evenement_de(k)) == Some(false)
}

/// Ce qu'il faut faire après une frappe consommée (la synchronisation du zoom de l'app, posé par
/// une commande qui n'a pas le contexte GPUI).
pub type ApresFrappe = Rc<dyn Fn(&mut App)>;

/// Les fenêtres branchées, et l'intercepteur unique de l'application.
#[derive(Default)]
struct Pont {
    fenetres: HashMap<WindowId, (Keymap, Option<ApresFrappe>)>,
    installe: bool,
}

impl Global for Pont {}

/// Branche `window` sur `keymap` : ses frappes passeront désormais par le Keymap. `apres` est
/// appelé après chaque frappe consommée. Une seconde fenêtre (réglages) a son propre Keymap.
pub fn brancher(window: &Window, cx: &mut App, keymap: Keymap, apres: Option<ApresFrappe>) {
    let id = window.window_handle().window_id();
    let pont = cx.default_global::<Pont>();
    pont.fenetres.insert(id, (keymap, apres));
    if pont.installe {
        return;
    }
    pont.installe = true;
    cx.intercept_keystrokes(|evenement, window, cx| {
        let id = window.window_handle().window_id();
        let Some((keymap, apres)) = cx
            .try_global::<Pont>()
            .and_then(|p| p.fenetres.get(&id).cloned())
        else {
            return;
        };
        if traiter(&keymap, &evenement.keystroke) {
            cx.stop_propagation();
            if let Some(a) = apres {
                a(cx);
            }
            window.refresh();
        }
    })
    .detach();
}

/// Débranche `window` (fermée) : ses frappes ne vont plus nulle part.
pub fn debrancher(window: &Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    if cx.has_global::<Pont>() {
        cx.global_mut::<Pont>().fenetres.remove(&id);
    }
}

/// Enregistre les commandes de scène `pixels:clavier` : aucune, les scènes tapent par l'action
/// `key` (qui passe par ce pont comme une vraie frappe).
pub fn commandes_de_scene(_commandes: &CommandManager) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn frappe(texte: &'static str) -> Keystroke {
        Keystroke::parse(texte).unwrap()
    }

    #[test]
    fn une_lettre_donne_sa_touche_physique_et_sa_capitale_sous_maj() {
        let e = evenement_de(&frappe("ctrl-shift-p"));
        assert_eq!((e.key.as_str(), e.code.as_str()), ("P", "KeyP"));
        assert!(e.mods.ctrl && e.mods.shift && !e.mods.alt && !e.mods.meta);
    }

    #[test]
    fn egal_moins_et_zero_sont_les_touches_du_zoom() {
        assert_eq!(evenement_de(&frappe("ctrl-=")).code, "Equal");
        assert_eq!(evenement_de(&frappe("ctrl--")).code, "Minus");
        assert_eq!(evenement_de(&frappe("ctrl-0")).code, "Digit0");
    }

    #[test]
    fn les_touches_nommees_prennent_le_nom_du_dom() {
        assert_eq!(evenement_de(&frappe("escape")).key, "Escape");
        assert_eq!(evenement_de(&frappe("up")).key, "ArrowUp");
        assert_eq!(evenement_de(&frappe("enter")).code, "Enter");
        assert_eq!(evenement_de(&frappe("space")).code, "Space");
    }
}
