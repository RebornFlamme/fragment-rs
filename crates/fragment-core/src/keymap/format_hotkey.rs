// Port de app/src/ui/core/keymap/formatHotkey.ts
//! Comment une combinaison s'ÉCRIT pour l'utilisateur.
//!
//! macOS a ses glyphes et son ordre canonique (⌃ ⌥ ⇧ ⌘ — c'est celui des menus
//! système, les utilisateurs mac le lisent sans y penser) ; ailleurs, des mots
//! joints par « + ». `Mod` est résolu vers le modificateur natif AVANT d'être
//! écrit : on n'affiche jamais « Mod » à quelqu'un.

use super::matching::{Plateforme, la_plateforme};
use super::types::{Hotkey, Modifier};

/// L'ordre canonique (le même sur les deux plateformes).
const ORDRE: [Modifier; 4] = [
    Modifier::Ctrl,
    Modifier::Alt,
    Modifier::Shift,
    Modifier::Meta,
];

fn glyphe_mac(m: Modifier) -> &'static str {
    match m {
        Modifier::Ctrl => "⌃",
        Modifier::Alt => "⌥",
        Modifier::Shift => "⇧",
        Modifier::Meta => "⌘",
        Modifier::Mod => unreachable!("Mod est résolu avant l'écriture"),
    }
}

fn mot_autre(m: Modifier) -> &'static str {
    match m {
        Modifier::Ctrl => "Ctrl",
        Modifier::Alt => "Alt",
        Modifier::Shift => "Shift",
        Modifier::Meta => "Win",
        Modifier::Mod => unreachable!("Mod est résolu avant l'écriture"),
    }
}

/// Les touches dont le nom DOM n'est pas ce qu'on veut lire.
fn nom_de_touche(key: &str) -> String {
    match key {
        "ArrowUp" => "↑".to_owned(),
        "ArrowDown" => "↓".to_owned(),
        "ArrowLeft" => "←".to_owned(),
        "ArrowRight" => "→".to_owned(),
        "Enter" => "↵".to_owned(),
        "Escape" => "Esc".to_owned(),
        "Backspace" => "⌫".to_owned(),
        "Delete" => "Suppr".to_owned(),
        " " => "Espace".to_owned(),
        "Tab" => "Tab".to_owned(),
        // Les gestes du trackpad, liés comme des touches (swipe.rs).
        "SwipeRight" => "Balayage →".to_owned(),
        "SwipeLeft" => "Balayage ←".to_owned(),
        // Une lettre seule s'écrit en capitale : « Ctrl+P », pas « Ctrl+p ».
        _ if key.chars().count() == 1 => key.to_uppercase(),
        _ => key.to_owned(),
    }
}

/// L'écriture pour la plateforme courante.
pub fn format_hotkey(hotkey: &Hotkey) -> String {
    format_hotkey_on(hotkey, la_plateforme())
}

pub fn format_hotkey_on(hotkey: &Hotkey, plateforme: Plateforme) -> String {
    let mac = plateforme == Plateforme::MacOs;
    let natif = plateforme.natif();

    // Résolution de Mod, dédoublonnage (Mod + Ctrl sur Windows = Ctrl), puis
    // remise dans l'ordre canonique de la plateforme.
    let presents: Vec<Modifier> = hotkey
        .modifiers
        .iter()
        .map(|m| if *m == Modifier::Mod { natif } else { *m })
        .collect();
    let mods = ORDRE.iter().copied().filter(|m| presents.contains(m));

    if mac {
        let mut s: String = mods.map(glyphe_mac).collect();
        s.push_str(&nom_de_touche(&hotkey.key));
        return s;
    }
    let mut parties: Vec<String> = mods.map(|m| mot_autre(m).to_owned()).collect();
    parties.push(nom_de_touche(&hotkey.key));
    parties.join("+")
}

#[cfg(test)]
mod tests {
    //! Les cas `formatHotkey` de fuzzySearch.test.ts (où ils vivent en TS).
    use super::*;
    use Modifier::{Alt, Ctrl, Mod, Shift};

    #[test]
    fn ecrit_des_mots_joints_par_plus_hors_macos_mod_resolu_en_ctrl() {
        assert_eq!(
            format_hotkey_on(&Hotkey::new(&[Mod, Shift], "p"), Plateforme::Autre),
            "Ctrl+Shift+P"
        );
    }

    #[test]
    fn ecrit_des_glyphes_sur_macos_dans_l_ordre_du_systeme() {
        assert_eq!(
            format_hotkey_on(&Hotkey::new(&[Shift, Mod, Alt], "p"), Plateforme::MacOs),
            "⌥⇧⌘P"
        );
    }

    #[test]
    fn dedoublonne_mod_et_son_modificateur_natif() {
        assert_eq!(
            format_hotkey_on(&Hotkey::new(&[Mod, Ctrl], "k"), Plateforme::Autre),
            "Ctrl+K"
        );
    }

    #[test]
    fn traduit_les_touches_speciales() {
        assert_eq!(
            format_hotkey_on(&Hotkey::new(&[], "ArrowUp"), Plateforme::Autre),
            "↑"
        );
        assert_eq!(
            format_hotkey_on(&Hotkey::new(&[Alt], "Enter"), Plateforme::Autre),
            "Alt+↵"
        );
    }
}
