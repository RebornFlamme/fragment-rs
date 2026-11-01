// Port de app/src/ui/core/keymap/matching.ts
//! §12.5 — LA LOGIQUE CLAVIER PURE (compilation des modificateurs, comparaison).
//!
//! Extraite de Keymap.ts pour casser le cycle Keymap ↔ Scope (A23) : `Scope`
//! n'a besoin que de `compile_modifiers` et `is_match`, et `Keymap` a besoin de
//! `Scope`. Ces fonctions PURES ne dépendent ni de l'un ni de l'autre.
//!
//! `Keymap` réexpose ces fonctions en associées (délégateurs) pour l'API
//! publique ; ce module en est la source unique.

use super::types::{Modifier, Modifiers};

/// La plateforme, pour résoudre `Mod`. Détectée à la compilation (`target_os`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plateforme {
    MacOs,
    Autre,
}

/// La plateforme courante.
pub fn la_plateforme() -> Plateforme {
    if cfg!(target_os = "macos") {
        Plateforme::MacOs
    } else {
        Plateforme::Autre
    }
}

impl Plateforme {
    /// Le modificateur natif derrière `Mod`.
    pub fn natif(self) -> Modifier {
        match self {
            Plateforme::MacOs => Modifier::Meta,
            Plateforme::Autre => Modifier::Ctrl,
        }
    }
}

/// Ce qui doit correspondre pour qu'une liaison se déclenche. `None` = indifférent.
/// Volontairement structurel : une liaison de `Scope` comme une liaison compilée
/// du HotkeyManager satisfont toutes les deux cette forme.
pub trait Liaison {
    fn modifiers(&self) -> Option<&str>;
    fn key(&self) -> Option<&str>;
}

/// Les modificateurs d'un événement, en forme canonique.
pub fn get_modifiers(mods: &Modifiers) -> String {
    let mut liste = Vec::with_capacity(4);
    if mods.ctrl {
        liste.push(Modifier::Ctrl);
    }
    if mods.meta {
        liste.push(Modifier::Meta);
    }
    if mods.alt {
        liste.push(Modifier::Alt);
    }
    if mods.shift {
        liste.push(Modifier::Shift);
    }
    compile_modifiers(&liste)
}

/// `[Mod, Shift]` → `"Ctrl,Shift"`. Résout, trie, joint — sur la plateforme courante.
pub fn compile_modifiers(modifiers: &[Modifier]) -> String {
    compile_modifiers_on(modifiers, la_plateforme())
}

/// `compile_modifiers` pour une plateforme donnée (tests, affichage).
///
/// Le tri est lexicographique sur les noms (`Alt < Ctrl < Meta < Shift`),
/// comme le `.sort()` de JavaScript : c'est ce qui rend l'ordre d'écriture
/// indifférent, et la comparaison se réduit alors à une égalité de chaînes.
pub fn compile_modifiers_on(modifiers: &[Modifier], plateforme: Plateforme) -> String {
    let mut noms: Vec<&'static str> = modifiers
        .iter()
        .map(|m| {
            if *m == Modifier::Mod {
                plateforme.natif()
            } else {
                *m
            }
        })
        .map(Modifier::as_str)
        .collect();
    noms.sort_unstable();
    noms.join(",")
}

/// L'inverse — pour AFFICHER une liaison enregistrée en dur.
///
/// Le modificateur natif de la plateforme redevient `Mod`, ce qui est la
/// bonne réponse à « comment nommer cette combinaison à l'utilisateur ? ».
pub fn decompile_modifiers(compiles: &str) -> Vec<Modifier> {
    decompile_modifiers_on(compiles, la_plateforme())
}

/// `decompile_modifiers` pour une plateforme donnée. Un nom inconnu est ignoré.
pub fn decompile_modifiers_on(compiles: &str, plateforme: Plateforme) -> Vec<Modifier> {
    if compiles.is_empty() {
        return Vec::new();
    }
    let natif = plateforme.natif();
    compiles
        .split(',')
        .filter_map(Modifier::parse)
        .map(|m| if m == natif { Modifier::Mod } else { m })
        .collect()
}

/// Une touche purement modificatrice (`Control` seul n'est pas un raccourci).
pub fn is_modifier_key(key: &str) -> bool {
    matches!(key, "Control" | "Alt" | "Shift" | "Meta" | "OS")
}

/// Ce modificateur-là est-il enfoncé ? `Mod` se résout selon la plateforme courante.
pub fn is_modifier(mods: &Modifiers, modifier: Modifier) -> bool {
    is_modifier_on(mods, modifier, la_plateforme())
}

pub fn is_modifier_on(mods: &Modifiers, modifier: Modifier, plateforme: Plateforme) -> bool {
    match modifier {
        Modifier::Ctrl => mods.ctrl,
        Modifier::Meta => mods.meta,
        Modifier::Alt => mods.alt,
        Modifier::Shift => mods.shift,
        Modifier::Mod => match plateforme {
            Plateforme::MacOs => mods.meta,
            Plateforme::Autre => mods.ctrl,
        },
    }
}

/// La liaison correspond-elle à la frappe ?
///
/// ★ Les modificateurs sont comparés à l'IDENTIQUE, pas par inclusion :
///   `Mod+P` ne se déclenche PAS sur `Mod+Shift+P`. Sans quoi tout raccourci
///   serait le préfixe de tous ses sur-ensembles.
///
/// ★ La touche est acceptée sur `vkey` (emplacement physique) OU sur `key`
///   sans tenir compte de la casse. Le second point n'est pas une commodité :
///   `Shift+A` produit `key == "A"` tandis que `A` seul produit `"a"`, et
///   une liaison écrite `"A"` doit marcher dans les deux cas — la
///   différenciation par Shift est déjà faite par les modificateurs.
pub fn is_match(liaison: &dyn Liaison, ctx: &super::types::KeymapContext) -> bool {
    if let Some(m) = liaison.modifiers()
        && m != ctx.modifiers
    {
        return false;
    }
    match liaison.key() {
        None => true,
        Some(k) => {
            k == ctx.vkey
                || k.eq_ignore_ascii_case(&ctx.key)
                || k.to_lowercase() == ctx.key.to_lowercase()
        }
    }
}
