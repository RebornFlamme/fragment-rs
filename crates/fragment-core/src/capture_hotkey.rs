// Port de app/src/ui/core/keymap/captureHotkey.ts
//! CAPTURE D'UN RACCOURCI (§Hotkeys) — « appuyez sur une combinaison ».
//!
//! On pousse une portée de CAPTURE (`Scope::capture`) sur la pile du Keymap : elle
//! reçoit la frappe AVANT tout raccourci global et la consomme. Ainsi, pendant la
//! saisie, `Mod+P` n'ouvre pas la palette — on lit la combinaison, point.
//!
//! Le Keymap filtre déjà les touches purement modificatrices (`is_modifier_key`) en
//! amont : notre handler ne voit qu'une « vraie » touche. Échap ANNULE (résultat
//! `None`). Le disposer rendu retire la portée (à appeler si on abandonne).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::command::Command;
use crate::keymap::{Hotkey, Keymap, Scope};

/// Le résultat remis une fois : la combinaison, ou `None` si Échap a annulé.
type Resultat = Box<dyn FnOnce(Option<Hotkey>)>;

/// Capture la prochaine combinaison. Rend le disposer : annuler la capture
/// (aucun résultat émis).
pub fn capture_hotkey(
    keymap: &Keymap,
    on_result: impl FnOnce(Option<Hotkey>) + 'static,
) -> Box<dyn FnOnce()> {
    let done = Rc::new(Cell::new(false));
    let on_result: Rc<RefCell<Option<Resultat>>> = Rc::new(RefCell::new(Some(Box::new(on_result))));
    // La portée se dépile depuis son propre handler : lien faible, sinon elle
    // se tiendrait elle-même en vie.
    let portee: Rc<RefCell<Option<crate::keymap::WeakScope>>> = Rc::new(RefCell::new(None));

    let finish = {
        let (done, on_result, portee, keymap) = (
            done.clone(),
            on_result.clone(),
            portee.clone(),
            keymap.clone(),
        );
        move |hotkey: Option<Hotkey>| {
            if done.get() {
                return;
            }
            done.set(true);
            if let Some(scope) = portee.borrow().as_ref().and_then(|w| w.upgrade()) {
                keymap.pop_scope(&scope);
            }
            if let Some(f) = on_result.borrow_mut().take() {
                f(hotkey);
            }
        }
    };

    let scope = Scope::capture(move |evt, _ctx| {
        if evt.key == "Escape" {
            finish(None);
            return;
        }
        finish(Some(Hotkey {
            modifiers: Keymap::decompile_modifiers(&Keymap::get_modifiers(&evt.mods)),
            key: evt.key.clone(),
        }));
    });
    *portee.borrow_mut() = Some(scope.downgrade());
    keymap.push_scope(&scope);

    // Disposer : annuler la capture (aucun résultat émis). Il tient la portée
    // en vie tant que la capture n'est pas finie.
    let keymap = keymap.clone();
    Box::new(move || {
        if !done.get() {
            done.set(true);
            keymap.pop_scope(&scope);
        }
    })
}

/// Une commande utilise-t-elle DÉJÀ cette combinaison ? Rend le NOM de la première
/// commande en conflit (hors `except_id`), ou `None`. Compare la forme compilée des
/// modificateurs + la touche (insensible à la casse pour les lettres).
///
/// `commands` : les commandes enregistrées (`CommandManager::entries`) ;
/// `hotkeys_for_command` : `HotkeyManager::hotkeys_for_command`.
pub fn find_hotkey_conflict<'a>(
    commands: impl IntoIterator<Item = &'a Command>,
    hotkeys_for_command: impl Fn(&str) -> Vec<Hotkey>,
    hotkey: &Hotkey,
    except_id: &str,
) -> Option<String> {
    let target_mods = Keymap::compile_modifiers(&hotkey.modifiers);
    let target_key = hotkey.key.to_lowercase();
    for command in commands {
        if command.id == except_id {
            continue;
        }
        for h in hotkeys_for_command(&command.id) {
            if Keymap::compile_modifiers(&h.modifiers) == target_mods
                && h.key.to_lowercase() == target_key
            {
                return Some(command.name.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Modifier::{Mod, Shift};
    use crate::keymap::tests::{ctrl, frappe};
    use crate::keymap::{KeyEvent, Modifiers};

    fn fake_key_event(key: &str, mods: Modifiers) -> KeyEvent {
        frappe(key, &format!("Key{}", key.to_uppercase()), mods)
    }

    // captureHotkey

    #[test]
    fn capture_la_prochaine_combinaison_ctrl_mod() {
        let keymap = Keymap::new();
        let result = Rc::new(RefCell::new(None));
        let r = result.clone();
        let _annuler = capture_hotkey(&keymap, move |h| *r.borrow_mut() = Some(h));

        keymap.handle_key_event(&fake_key_event("p", ctrl()));
        assert_eq!(*result.borrow(), Some(Some(Hotkey::new(&[Mod], "p"))));
        // La portée de capture est retirée : la pile est revenue à la racine.
        assert_eq!(keymap.scope(), *keymap.root_scope());
    }

    #[test]
    fn echap_annule_resultat_null() {
        let keymap = Keymap::new();
        let result = Rc::new(RefCell::new(None));
        let r = result.clone();
        let _annuler = capture_hotkey(&keymap, move |h| *r.borrow_mut() = Some(h));

        keymap.handle_key_event(&fake_key_event("Escape", Modifiers::default()));
        assert_eq!(*result.borrow(), Some(None));
    }

    #[test]
    fn le_disposer_retire_la_portee_sans_emettre_de_resultat() {
        let keymap = Keymap::new();
        let called = Rc::new(Cell::new(false));
        let c = called.clone();
        let cancel = capture_hotkey(&keymap, move |_| c.set(true));
        cancel();
        keymap.handle_key_event(&fake_key_event("p", ctrl()));
        assert!(!called.get());
        assert_eq!(keymap.scope(), *keymap.root_scope());
    }

    // findHotkeyConflict

    fn commandes() -> Vec<Command> {
        vec![
            Command::new("a:x", "Command X"),
            Command::new("b:y", "Command Y"),
        ]
    }

    fn hotkeys_for(id: &str) -> Vec<Hotkey> {
        if id == "a:x" {
            vec![Hotkey::new(&[Mod], "p")]
        } else {
            vec![]
        }
    }

    #[test]
    fn trouve_la_commande_deja_liee_a_la_meme_combinaison() {
        assert_eq!(
            find_hotkey_conflict(&commandes(), hotkeys_for, &Hotkey::new(&[Mod], "p"), "b:y")
                .as_deref(),
            Some("Command X")
        );
    }

    #[test]
    fn exclut_la_commande_elle_meme() {
        assert_eq!(
            find_hotkey_conflict(&commandes(), hotkeys_for, &Hotkey::new(&[Mod], "p"), "a:x"),
            None
        );
    }

    #[test]
    fn pas_de_conflit_pour_une_combinaison_differente() {
        assert_eq!(
            find_hotkey_conflict(
                &commandes(),
                hotkeys_for,
                &Hotkey::new(&[Shift], "p"),
                "b:y"
            ),
            None
        );
    }
}
