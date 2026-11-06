//! Les polices embarquées : DM Sans (400, 500, 600) et, quand son fichier est déposé, JetBrains Mono.
//!
//! ★ `--font-interface` résout vers « DM Sans » SANS repli système : les pixels ne doivent pas
//! dépendre des polices de la machine. Les fichiers sont ceux de `assets/fonts/*.ttf` (licence OFL,
//! voir `assets/fonts/OFL.txt`), enregistrés auprès du `TextSystem` de GPUI au démarrage.

use std::borrow::Cow;

use gpui_kit::{Font, FontWeight, Result, TextSystem, font};

use crate::embarque::POLICES;

/// La famille de l'interface (`--font-interface`, `--font-text`).
pub const FAMILLE_INTERFACE: &str = "DM Sans";
/// La famille monospace (`--font-monospace`) : embarquée dès que ses fichiers sont dans `assets/fonts`.
pub const FAMILLE_MONOSPACE: &str = "JetBrains Mono";

/// Les graisses embarquées de DM Sans.
pub const GRAISSES_INTERFACE: [FontWeight; 3] =
    [FontWeight::NORMAL, FontWeight::MEDIUM, FontWeight::SEMIBOLD];

/// La police de l'interface à la graisse demandée.
pub fn interface(graisse: FontWeight) -> Font {
    let mut f = font(FAMILLE_INTERFACE);
    f.weight = graisse;
    f
}

/// Enregistre toutes les polices embarquées auprès du `TextSystem` (à appeler une fois, au boot).
pub fn enregistrer(systeme: &TextSystem) -> Result<()> {
    let fichiers: Vec<Cow<'static, [u8]>> =
        POLICES.iter().map(|(_, o)| Cow::Borrowed(*o)).collect();
    systeme.add_fonts(fichiers)
}

/// Vrai si la famille se résout DIRECTEMENT (pas de repli) à cette graisse.
pub fn se_resout_sans_repli(systeme: &TextSystem, famille: &str, graisse: FontWeight) -> bool {
    let mut demande = font(famille.to_owned());
    demande.weight = graisse;
    let id = systeme.resolve_font(&demande);
    // `get_font_for_id` ne renvoie que les entrées résolues en direct : un repli donne `None`.
    systeme
        .get_font_for_id(id)
        .is_some_and(|f| f.family == famille)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::platform::current_platform;

    /// Un `TextSystem` réel (celui de la plateforme, DirectWrite sous Windows), sans fenêtre.
    /// ★ `current_platform(true)` (sans tête) donne un système de texte VIDE : il faut `false`.
    fn systeme_de_test() -> TextSystem {
        TextSystem::new(current_platform(false).text_system())
    }

    #[test]
    fn les_trois_graisses_de_dm_sans_sont_embarquees() {
        let noms: Vec<&str> = POLICES.iter().map(|(n, _)| *n).collect();
        for attendu in ["DMSans-400", "DMSans-500", "DMSans-600"] {
            assert!(noms.contains(&attendu), "police absente : {attendu}");
        }
        assert!(
            POLICES.iter().all(|(_, o)| o.len() > 10_000),
            "un fichier est vide"
        );
    }

    #[test]
    fn dm_sans_se_resout_sans_repli() {
        let systeme = systeme_de_test();
        enregistrer(&systeme).expect("polices enregistrées");
        assert!(
            systeme
                .all_font_names()
                .iter()
                .any(|n| n == FAMILLE_INTERFACE),
            "la famille est connue : {:?}",
            systeme.all_font_names()
        );
        for graisse in GRAISSES_INTERFACE {
            assert!(
                se_resout_sans_repli(&systeme, FAMILLE_INTERFACE, graisse),
                "repli système à la graisse {graisse:?}"
            );
        }
    }

    #[test]
    fn les_graisses_donnent_des_polices_distinctes() {
        let systeme = systeme_de_test();
        enregistrer(&systeme).expect("polices enregistrées");
        let ids: Vec<_> = GRAISSES_INTERFACE
            .iter()
            .map(|g| systeme.resolve_font(&interface(*g)))
            .collect();
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
    }

    #[test]
    fn une_famille_inconnue_se_resout_par_repli() {
        // Contre-épreuve : le test de résolution directe sait dire « repli ».
        let systeme = systeme_de_test();
        enregistrer(&systeme).expect("polices enregistrées");
        assert!(!se_resout_sans_repli(
            &systeme,
            "Famille Qui N'Existe Pas",
            FontWeight::NORMAL
        ));
    }
}
