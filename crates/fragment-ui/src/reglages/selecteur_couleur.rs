// Port de app/src/ui/core/settings/components/ColorComponent.ts (le comportement : une valeur
// `#rrggbb` choisie dans la boîte de couleur du système, rappelée telle quelle).
//! La boîte de couleur d'un réglage (l'accent).
//!
//! ★ Fragment utilise `<input type="color">` : une pastille ronde qui ouvre la boîte de couleur
//! NATIVE du système. On ne dessine donc aucun sélecteur (« on reproduit, on ne redessine pas ») :
//! la pastille est celle de `fenetre.rs`, et son clic appelle une [`BoiteCouleur`]. Sous Windows,
//! [`BoiteNative`] ouvre `ChooseColorW` (ADR-018) ; ailleurs elle ne fait rien (écart noté dans
//! `parite.toml`). Un test injecte une fausse boîte.

use fragment_theme::Couleur;

use super::fenetre::{couleur_depuis_hex, hex_de};

/// Une boîte de couleur : reçoit la valeur courante (`#rrggbb`), rend la valeur choisie, ou `None`
/// si l'utilisateur annule (ou si la plateforme n'en a pas).
pub trait BoiteCouleur {
    fn choisir(&self, initiale: &str) -> Option<String>;
}

/// La boîte du système : `ChooseColorW` sous Windows, rien ailleurs.
pub struct BoiteNative;

impl BoiteCouleur for BoiteNative {
    fn choisir(&self, initiale: &str) -> Option<String> {
        let c = couleur_depuis_hex(initiale);
        let rvb = boite_systeme(rvb_vers_colorref(c.r, c.g, c.b))?;
        let (r, g, b) = colorref_vers_rvb(rvb);
        Some(hex_de(Couleur::octets(r, g, b, 255)))
    }
}

/// Ouvre la boîte, rend la valeur choisie et applique-la par `rappel` ; `true` si elle a changé.
///
/// Une annulation, ou une valeur identique à l'actuelle, n'applique rien (l'évènement `input` du
/// champ natif ne part pas non plus).
pub fn choisir_et_appliquer(
    boite: &dyn BoiteCouleur,
    valeur: &str,
    rappel: Option<&dyn Fn(String)>,
) -> bool {
    match boite.choisir(valeur) {
        Some(hex) if hex != valeur => {
            if let Some(r) = rappel {
                r(hex);
            }
            true
        }
        _ => false,
    }
}

/// `COLORREF` Windows : `0x00BBGGRR`.
fn rvb_vers_colorref(r: u8, g: u8, b: u8) -> u32 {
    u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16
}

fn colorref_vers_rvb(c: u32) -> (u8, u8, u8) {
    (
        (c & 0xff) as u8,
        (c >> 8 & 0xff) as u8,
        (c >> 16 & 0xff) as u8,
    )
}

/// ★ SEUL `unsafe` du module (ADR-018) : un appel FFI à `ChooseColorW`, sur une structure
/// entièrement initialisée ici et qui ne survit pas à l'appel. Fenêtre propriétaire : la fenêtre
/// au premier plan (celle des réglages, où l'on vient de cliquer). Rend le `COLORREF` choisi.
#[cfg(windows)]
#[allow(unsafe_code)]
fn boite_systeme(initiale: u32) -> Option<u32> {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::Controls::Dialogs::{
        CC_FULLOPEN, CC_RGBINIT, CHOOSECOLOR_FLAGS, CHOOSECOLORW, ChooseColorW,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let mut perso = [COLORREF(0x00ff_ffff); 16];
    let mut ch = CHOOSECOLORW {
        lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: unsafe { GetForegroundWindow() },
        rgbResult: COLORREF(initiale),
        lpCustColors: perso.as_mut_ptr(),
        Flags: CHOOSECOLOR_FLAGS(CC_RGBINIT.0 | CC_FULLOPEN.0),
        ..Default::default()
    };
    // SAFETY : `ch` est valide et `lpCustColors` pointe seize `COLORREF` vivants pendant l'appel.
    unsafe { ChooseColorW(&mut ch) }
        .as_bool()
        .then_some(ch.rgbResult.0)
}

#[cfg(not(windows))]
fn boite_systeme(_initiale: u32) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fausse(Option<&'static str>);
    impl BoiteCouleur for Fausse {
        fn choisir(&self, _: &str) -> Option<String> {
            self.0.map(str::to_owned)
        }
    }

    #[test]
    fn colorref_est_en_bgr() {
        assert_eq!(rvb_vers_colorref(0x12, 0x34, 0x56), 0x0056_3412);
        assert_eq!(colorref_vers_rvb(0x0056_3412), (0x12, 0x34, 0x56));
    }

    #[test]
    fn une_couleur_choisie_est_rappelee() {
        let recu = RefCell::new(None);
        let rappel = |h: String| *recu.borrow_mut() = Some(h);
        assert!(choisir_et_appliquer(
            &Fausse(Some("#12ab34")),
            "#646cff",
            Some(&rappel)
        ));
        assert_eq!(recu.borrow().as_deref(), Some("#12ab34"));
    }

    #[test]
    fn annuler_ou_rechoisir_la_meme_couleur_n_applique_rien() {
        let recu = RefCell::new(None);
        let rappel = |h: String| *recu.borrow_mut() = Some(h);
        assert!(!choisir_et_appliquer(
            &Fausse(None),
            "#646cff",
            Some(&rappel)
        ));
        assert!(!choisir_et_appliquer(
            &Fausse(Some("#646cff")),
            "#646cff",
            Some(&rappel)
        ));
        assert!(recu.borrow().is_none());
    }
}
