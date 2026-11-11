//! Les polices embarquées : DM Sans (400, 500, 600) et, quand son fichier est déposé, JetBrains Mono.
//!
//! ★ `--font-interface` résout vers « DM Sans » SANS repli système : les pixels ne doivent pas
//! dépendre des polices de la machine. Les fichiers sont ceux de `assets/fonts/*.ttf` (licence OFL,
//! voir `assets/fonts/OFL.txt`), enregistrés auprès du `TextSystem` de GPUI au démarrage.

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;

use gpui_kit::{
    App, Font, FontFeatures, FontWeight, Pixels, Result, Styled, TextRun, TextSystem,
    WindowTextSystem, font, px,
};

use crate::echelle::Echelle;
use crate::embarque::POLICES;

/// La famille de l'interface (`--font-interface`, `--font-text`).
pub const FAMILLE_INTERFACE: &str = "DM Sans";
/// La famille monospace (`--font-monospace`) : embarquée dès que ses fichiers sont dans `assets/fonts`.
pub const FAMILLE_MONOSPACE: &str = "JetBrains Mono";

/// Les graisses embarquées de DM Sans.
pub const GRAISSES_INTERFACE: [FontWeight; 3] =
    [FontWeight::NORMAL, FontWeight::MEDIUM, FontWeight::SEMIBOLD];

/// ★ Le crénage, demandé EXPLICITEMENT. GPUI donne à DirectWrite un objet « typographie » (`liga`, `clig`,
/// `calt`) ; DirectWrite n'applique alors plus `kern` de lui-même. Mesuré sur DM Sans 12 px : sans cette
/// demande « notice » est 0,24 px trop large, « Fermer » 0,25 px, « Enregistré » 0,76 px (les paires `ot`,
/// `Fe`, `re`, `st`… ne sont pas serrées) ; avec elle les largeurs sont celles de HarfBuzz et de Chromium
/// à 0,001 px près. C'est l'écart d'environ 1 % de `fr-t1.3`.
pub fn caracteristiques() -> FontFeatures {
    FontFeatures(Arc::new(vec![("kern".into(), 1)]))
}

/// La police de l'interface à la graisse demandée (crénage compris).
pub fn interface(graisse: FontWeight) -> Font {
    let mut f = font(FAMILLE_INTERFACE);
    f.weight = graisse;
    f.features = caracteristiques();
    f
}

/// La famille de l'interface et ses caractéristiques (crénage) sur un élément : à utiliser au lieu de
/// `.font_family(FAMILLE_INTERFACE)`, sans quoi le texte est mesuré sans crénage.
pub fn famille_interface<E: Styled>(e: E) -> E {
    e.font_family(FAMILLE_INTERFACE)
        .font_features(caracteristiques())
}

thread_local! {
    /// Le mesureur de lignes, lié au `TextSystem` de l'application (reconstruit s'il change).
    static MESUREUR: RefCell<Option<(Arc<TextSystem>, WindowTextSystem)>> = const { RefCell::new(None) };
}

/// La largeur EXACTE (fractionnaire, en pixels logiques) d'un texte d'une ligne, mesurée comme GPUI le
/// compose (DM Sans, crénage), à `taille` px logiques.
///
/// ★ GPUI arrondit au pixel SUPÉRIEUR la largeur d'une feuille de texte (`elements/text.rs`, `.ceil()`) ;
/// Chromium garde la fraction et n'arrondit que l'arête à la peinture. Une boîte « ajustée au texte » est
/// donc plus large d'un pixel dans GPUI dès que la fraction dépasse zéro : on mesure ici, et
/// [`largeur_ajustee`] rend l'entier de Chromium.
pub fn largeur_texte(cx: &App, texte: &str, graisse: FontWeight, taille: f32) -> f32 {
    let zoom = Echelle::courante(cx).zoom();
    let systeme = cx.text_system().clone();
    MESUREUR.with(|m| {
        let mut m = m.borrow_mut();
        if !m.as_ref().is_some_and(|(s, _)| Arc::ptr_eq(s, &systeme)) {
            *m = Some((systeme.clone(), WindowTextSystem::new(systeme)));
        }
        let Some((_, mesureur)) = m.as_ref() else {
            return 0.0;
        };
        let ligne = TextRun {
            len: texte.len(),
            font: interface(graisse),
            color: Default::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let l = mesureur.layout_line(texte, px(taille * zoom), &[ligne], None);
        f32::from(l.width) / zoom
    })
}

/// La largeur entière d'une boîte ajustée au texte, comme la peint Chromium : `texte + ajout` (rembourrage
/// et bordures, logiques) arrondi au pixel écran le plus proche, la moitié vers le haut. ★ Chromium cumule
/// des fractions et n'arrondit que les arêtes : une boîte de 63,05 px se peint sur 63 px, de 132,34 sur 132.
pub fn largeur_ajustee(cx: &App, texte: f32, ajout: f32) -> Pixels {
    let zoom = Echelle::courante(cx).zoom();
    px(((texte + ajout) * zoom + 0.5 + 1e-3).floor())
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

    /// ★ `fr-t1.3` : le crénage est demandé explicitement. Largeurs de HarfBuzz (celles de Chromium) :
    /// « notice » 35,028 px à 12 px, « Une notice de test » 102,336, « Calques » 45,048 (sans paire serrée).
    #[test]
    fn les_largeurs_suivent_le_crenage_de_chromium() {
        let systeme = Arc::new(systeme_de_test());
        enregistrer(&systeme).expect("polices enregistrées");
        let mesureur = WindowTextSystem::new(systeme);
        let largeur = |texte: &str, caracteristiques: FontFeatures| {
            let mut f = interface(FontWeight::NORMAL);
            f.features = caracteristiques;
            let ligne = TextRun {
                len: texte.len(),
                font: f,
                color: Default::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(mesureur.layout_line(texte, px(12.0), &[ligne], None).width)
        };
        for (texte, attendu) in [
            ("notice", 35.028),
            ("Une notice de test", 102.336),
            ("Calques", 45.048),
        ] {
            let mesure = largeur(texte, caracteristiques());
            assert!(
                (mesure - attendu).abs() < 0.01,
                "{texte} : {mesure} au lieu de {attendu}"
            );
        }
        // Contre-épreuve : sans la demande, DirectWrite ne serre pas les paires (« notice » +0,24 px).
        let sans = largeur("notice", FontFeatures::default());
        assert!(sans > 35.2, "sans crénage : {sans}");
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
