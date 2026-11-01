// Port de app/src/ui/core/keymap/types.ts
//! §12.5 — les contrats du clavier. Scope, Keymap, HotkeyManager et Command les
//! lisent tous les quatre.
//!
//! S'y ajoute ce que le DOM fournissait en TS et que GPUI devra fournir ici :
//! la frappe elle-même (`KeyEvent`), ses modificateurs (`Modifiers`) et
//! l'événement déclencheur d'une commande (`InputEvent`, le `app.lastEvent`).

use serde::{Deserialize, Serialize};

/// Un modificateur.
///
/// ★ `Mod` est le seul qu'on devrait écrire dans du code de commande : il
///   ABSTRAIT Cmd sur macOS et Ctrl partout ailleurs. Écrire `Ctrl` en dur, c'est
///   fabriquer un raccourci qui sera faux sur un Mac — et l'utilisateur mac ne
///   pourra que le personnaliser, pas le réparer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modifier {
    Mod,
    Ctrl,
    Meta,
    Shift,
    Alt,
}

impl Modifier {
    /// Le nom tel qu'il s'écrit dans hotkeys.json et dans la forme canonique.
    pub fn as_str(self) -> &'static str {
        match self {
            Modifier::Mod => "Mod",
            Modifier::Ctrl => "Ctrl",
            Modifier::Meta => "Meta",
            Modifier::Shift => "Shift",
            Modifier::Alt => "Alt",
        }
    }

    /// L'inverse (pour `decompile_modifiers`) ; `None` pour un nom inconnu.
    pub fn parse(nom: &str) -> Option<Modifier> {
        Some(match nom {
            "Mod" => Modifier::Mod,
            "Ctrl" => Modifier::Ctrl,
            "Meta" => Modifier::Meta,
            "Shift" => Modifier::Shift,
            "Alt" => Modifier::Alt,
            _ => return None,
        })
    }
}

/// Une combinaison. `key` est la valeur logique (`KeyboardEvent.key`) : 'P', 'F2', 'ArrowUp'.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

impl Hotkey {
    pub fn new(modifiers: &[Modifier], key: &str) -> Self {
        Self {
            modifiers: modifiers.to_vec(),
            key: key.to_owned(),
        }
    }
}

/// Les modificateurs enfoncés au moment d'un événement (le quadruplet
/// `ctrlKey/metaKey/altKey/shiftKey` d'un événement DOM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Une frappe, telle que le Keymap la reçoit (le `KeyboardEvent` du DOM, réduit
/// à ce qu'on lit : la touche logique, la touche physique, l'auto-répétition,
/// les modificateurs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// La valeur LOGIQUE — ce que la disposition clavier produit (`'m'`, `'Escape'`).
    pub key: String,
    /// La valeur PHYSIQUE (`KeyboardEvent.code`) — l'emplacement (`'Semicolon'`).
    pub code: String,
    /// La touche est-elle maintenue (auto-répétition) ?
    pub repeat: bool,
    pub mods: Modifiers,
}

impl KeyEvent {
    /// Une frappe sans modificateur, `code` déduit de `key` comme le fait un
    /// clavier pour une lettre (`'p'` → `'KeyP'`) ; sinon `code = key`.
    pub fn new(key: &str) -> Self {
        let code = match key.chars().next() {
            Some(c) if key.chars().count() == 1 && c.is_ascii_alphabetic() => {
                format!("Key{}", c.to_ascii_uppercase())
            }
            _ => key.to_owned(),
        };
        Self {
            key: key.to_owned(),
            code,
            repeat: false,
            mods: Modifiers::default(),
        }
    }

    pub fn with_code(mut self, code: &str) -> Self {
        self.code = code.to_owned();
        self
    }

    pub fn with_mods(mut self, mods: Modifiers) -> Self {
        self.mods = mods;
        self
    }

    pub fn repeated(mut self) -> Self {
        self.repeat = true;
        self
    }
}

/// Un événement de pointeur réduit à ce que `is_mod_event` lit : le bouton
/// (1 = molette) et les modificateurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerEvent {
    pub button: u8,
    pub mods: Modifiers,
}

/// L'événement qui a déclenché une commande (`app.lastEvent`, §8.5) : une
/// commande n'a pas accès à l'événement, mais elle peut lire celui-ci pour
/// décider entre remplacer la feuille, ouvrir un onglet ou splitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Pointer(PointerEvent),
}

impl InputEvent {
    pub fn mods(&self) -> Modifiers {
        match self {
            InputEvent::Key(k) => k.mods,
            InputEvent::Pointer(p) => p.mods,
        }
    }
}

/// Où ouvrir : un onglet, un split, une fenêtre (le `PaneType` de
/// `workspace/types.ts`). Défini ici parce que `Keymap::is_mod_event` le rend ;
/// le modèle du workspace (`core-b`) le RÉEXPORTE, il ne le redéfinit pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneType {
    Tab,
    Split,
    Window,
}

/// Ce que le Keymap a déduit de la frappe, et qu'il passe aux handlers.
///
/// ★ DEUX représentations de la touche, et il faut les deux :
///
/// - `key` est la valeur LOGIQUE — ce que la disposition clavier produit.
///   Sur un AZERTY, la touche à droite de « L » donne `'m'`.
/// - `vkey` est la valeur PHYSIQUE (`KeyboardEvent.code`) — l'emplacement.
///   La même touche donne `'Semicolon'`, parce que c'est là qu'est le
///   point-virgule sur un QWERTY.
///
///   Un raccourci enregistré sur `key` suit la disposition ; enregistré sur
///   `vkey`, il suit l'emplacement physique. `is_match` accepte les deux, ce qui
///   permet à un raccourci de survivre à un changement de disposition sans que
///   l'auteur ait à choisir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapContext {
    /// Forme canonique compilée : trié, joint par virgules. `"Ctrl,Shift"`.
    pub modifiers: String,
    pub key: String,
    pub vkey: String,
}

/// Ce qu'un handler de frappe rend.
///
/// ★ La valeur de retour porte TROIS sens distincts, et c'est le cœur de la
///   propagation :
///
/// ```text
///     Some(false)  « j'ai consommé la frappe » → preventDefault + stopPropagation
///     Some(true)   « j'ai traité, mais laisse passer »
///     None         « pas pour moi » → on continue la chaîne
/// ```
pub type KeymapResult = Option<bool>;

/// Un handler de frappe.
pub type KeymapEventListener = dyn Fn(&KeyEvent, &KeymapContext) -> KeymapResult;

/// Une liaison compilée par le HotkeyManager — sans handler, l'id suffit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiaisonCompilee {
    pub modifiers: String,
    pub key: String,
}
