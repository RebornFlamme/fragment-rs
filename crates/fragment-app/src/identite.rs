//! L'identité visuelle de l'application : l'icône et les polices sans repli système.
//!
//! ★ L'icône n'est pas posée à l'exécution : GPUI charge, sous Windows, la ressource d'identifiant 1
//! de l'exécutable pour toutes ses fenêtres. `build.rs` y compile `assets/identite.rc`, qui désigne
//! `assets/brand/app-icon.ico` : l'exe (Explorateur, barre des tâches) et la barre de titre
//! portent donc la même icône. Les fichiers de `assets/brand/` sont des actifs de marque, hors de la
//! licence MIT du dépôt (voir `assets/brand/NOTICE`).
//!
//! Les polices : l'interface est DM Sans, EMBARQUÉE ; le code suit la pile `--font-monospace`, qui
//! reste une pile de polices SYSTÈME comme dans Fragment (ADR-005 §5). « Sans repli » veut dire :
//! chaque famille que l'application réclame se résout directement, jamais par le remplacement
//! silencieux de DirectWrite.

use fragment_theme::theme::Theme;
use fragment_ui::polices::{FAMILLE_INTERFACE, GRAISSES_INTERFACE, se_resout_sans_repli};
use gpui_kit::TextSystem;

/// L'icône de l'application en PNG (fenêtre des plateformes sans ressource d'exécutable).
pub const ICONE_PNG: &[u8] = include_bytes!("../assets/brand/icon.png");
/// L'icône de l'exécutable (ressource Windows), toutes tailles de 16 à 256 px.
pub const ICONE_ICO: &[u8] = include_bytes!("../assets/brand/app-icon.ico");

/// La famille que Chromium donne, sous Windows, à un nom générique de CSS (c'est ce que voit la
/// référence). `ui-monospace` n'est pas géré par Chromium : il passe son chemin. Ailleurs : `None`.
fn generique(nom: &str) -> Option<&'static str> {
    if !cfg!(windows) {
        return None;
    }
    match nom {
        "monospace" => Some("Consolas"),
        "sans-serif" => Some("Arial"),
        "serif" => Some("Times New Roman"),
        "system-ui" => Some("Segoe UI"),
        _ => None,
    }
}

/// La première famille de `pile` que le système de texte connaît (embarquée ou installée), à la
/// manière du `font-family` de CSS ; un nom générique vaut la famille de Chromium (`monospace` →
/// Consolas), donc une pile qui finit sur `monospace` n'est PAS un repli : c'est le comportement
/// attendu de Fragment. `None` si rien ne se résout (par exemple Consolas absente du système).
pub fn pile_resolue(systeme: &TextSystem, pile: &[&'static str]) -> Option<&'static str> {
    let connues = systeme.all_font_names();
    pile.iter().copied().find_map(|f| {
        let cible = generique(f).unwrap_or(f);
        connues.iter().any(|c| c == cible).then_some(cible)
    })
}

/// Les polices qui tombent sur un repli, une ligne par défaut (vide : tout se résout).
///
/// - DM Sans doit se résoudre en direct aux trois graisses embarquées ;
/// - la pile du code (`font_monospace`) doit désigner une famille réellement présente.
pub fn replis(systeme: &TextSystem) -> Vec<String> {
    let mut defauts = Vec::new();
    for graisse in GRAISSES_INTERFACE {
        if !se_resout_sans_repli(systeme, FAMILLE_INTERFACE, graisse) {
            defauts.push(format!(
                "{FAMILLE_INTERFACE} ({graisse:?}) : repli système de l'interface"
            ));
        }
    }
    let typo = Theme::sombre().typo;
    for (nom, pile) in [
        ("font_interface", typo.font_interface),
        ("font_monospace", typo.font_monospace),
    ] {
        if pile_resolue(systeme, pile).is_none() {
            defauts.push(format!(
                "{nom} : aucune famille de {pile:?} n'est installée"
            ));
        }
    }
    defauts
}

/// Au démarrage, après l'enregistrement des polices : signale chaque repli sur la sortie d'erreur.
pub fn signaler_replis(systeme: &TextSystem) {
    for defaut in replis(systeme) {
        eprintln!("fragment : police : {defaut}");
    }
}
