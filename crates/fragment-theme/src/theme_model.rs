// Port de app/src/ui/theme/themeModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les thèmes du coffre, comme Obsidian — le modèle PUR (testé).
//
//  Un thème = un DOSSIER `.fragment/themes/<Nom>/` qui contient `theme.css`
//  (+ `manifest.json` facultatif) ; le nom du dossier fait foi. Le thème actif
//  = `cssTheme` dans `appearance.json` (`''` = celui de Fragment).
//
//  ★ ADR-005 §4 : dans fragment-rs le fichier du thème est `theme.json` (objet plat
//    de jetons), le `theme.css` n'étant PAS interprété. Le modèle reste celui du TS
//    (même arbre de dossiers, mêmes règles de nom / de liste / de rechargement) ; les
//    deux noms de fichier sont exposés, `FICHIER_DU_THEME` garde celui du TS pour que
//    les tests portés soient identiques.
// ═══════════════════════════════════════════════════════════════════════════

use unicode_normalization::UnicodeNormalization;

/// Le fichier de thème de Fragment (TS).
pub const FICHIER_DU_THEME: &str = "theme.css";

/// Le fichier de thème de fragment-rs (ADR-005 §4) : un objet plat `{ jeton CSS: valeur }`.
pub const FICHIER_DU_THEME_JSON: &str = "theme.json";

/// Le dossier des thèmes d'un coffre.
pub fn dossier_des_themes(config_dir: &str) -> String {
    format!("{config_dir}/themes")
}

/// Le `theme.css` d'un thème.
pub fn chemin_du_theme(config_dir: &str, nom: &str) -> String {
    format!("{}/{nom}/{FICHIER_DU_THEME}", dossier_des_themes(config_dir))
}

/// Le `theme.json` d'un thème (ADR-005 §4).
pub fn chemin_du_theme_json(config_dir: &str, nom: &str) -> String {
    format!(
        "{}/{nom}/{FICHIER_DU_THEME_JSON}",
        dossier_des_themes(config_dir)
    )
}

/// Un nom de thème est un nom de DOSSIER : rien qui sorte du dossier des thèmes.
///
/// ★ `trim()` de JS retire les blancs Unicode comme `str::trim` ; `/[\\/:*?"<>|]/` = ces 9 caractères.
pub fn nom_valide(nom: &str) -> bool {
    !nom.trim().is_empty()
        && nom == nom.trim()
        && !nom.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
        && nom != "."
        && nom != ".."
}

/// Le nom d'un chemin de dossier du coffre (`a/b/c` → `c`).
fn base(chemin: &str) -> &str {
    &chemin[chemin.rfind('/').map_or(0, |i| i + 1)..]
}

/// Clé de tri « sensibilité base » (`localeCompare(..., { sensitivity: 'base' })`) :
/// ni casse ni accents.
fn cle_de_tri(s: &str) -> String {
    s.to_lowercase()
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect()
}

/// Les thèmes installés, triés (sans tenir compte de la casse) : les dossiers du
/// dossier des thèmes qui ont un `theme.css`. `a_un_theme` dit si un dossier en a un.
///
/// ★ Le tri `localeCompare` de JS suit l'ICU de la langue ; ici casse et accents sont
///   ignorés (clé NFD en minuscules), tri STABLE comme `Array.sort`. Pour des noms de
///   dossier latins ordinaires l'ordre est celui d'ICU ; les ordres exotiques (CJK…)
///   suivent les points de code.
pub fn lister_themes(dossiers: &[String], a_un_theme: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut noms: Vec<String> = dossiers
        .iter()
        .filter(|d| nom_valide(base(d)) && a_un_theme(d))
        .map(|d| base(d).to_string())
        .collect();
    noms.sort_by_key(|n| cle_de_tri(n));
    noms
}

/// Ce qu'un changement sur le disque demande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffetDUnChangement {
    /// Le thème actif a changé.
    pub recharger: bool,
    /// Un thème a pu apparaître ou disparaître (le dossier des thèmes lui-même, ou un de ses enfants directs).
    pub liste: bool,
}

/// Ce qu'un changement sur le disque (chemin relatif au coffre) demande.
pub fn effet_d_un_changement(chemin: &str, config_dir: &str, actif: &str) -> EffetDUnChangement {
    let dossier = dossier_des_themes(config_dir);
    let dans_le_dossier = chemin == dossier || chemin.starts_with(&format!("{dossier}/"));
    if !dans_le_dossier {
        return EffetDUnChangement {
            recharger: false,
            liste: false,
        };
    }
    let reste: Vec<&str> = if chemin == dossier {
        Vec::new()
    } else {
        chemin[dossier.len() + 1..].split('/').collect()
    };
    EffetDUnChangement {
        recharger: !actif.is_empty() && reste.first() == Some(&actif),
        // Le dossier, un thème, ou son theme.css (créé / supprimé).
        liste: reste.len() <= 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn chemins() {
        assert_eq!(dossier_des_themes(".fragment"), ".fragment/themes");
        assert_eq!(
            chemin_du_theme(".fragment", "Rose"),
            ".fragment/themes/Rose/theme.css"
        );
    }

    #[test]
    fn un_nom_est_un_nom_de_dossier() {
        assert!(nom_valide("Rose"));
        assert!(nom_valide("Minimal Theme"));
        for n in ["", " ", "..", ".", "a/b", "a\\b", "c:", " Rose"] {
            assert!(!nom_valide(n), "{n:?} devrait être refusé");
        }
    }

    #[test]
    fn liste_les_dossiers_qui_ont_un_theme_css_tries_sans_casse() {
        let avec: HashSet<&str> = [
            ".fragment/themes/rose",
            ".fragment/themes/Bleu",
            ".fragment/themes/zen",
        ]
        .into();
        let dossiers: Vec<String> = [
            ".fragment/themes/zen",
            ".fragment/themes/vide",
            ".fragment/themes/rose",
            ".fragment/themes/Bleu",
        ]
        .map(String::from)
        .into();
        assert_eq!(
            lister_themes(&dossiers, &|d| avec.contains(d)),
            vec!["Bleu", "rose", "zen"]
        );
    }

    #[test]
    fn un_changement_sur_le_disque_recharger_le_theme_actif_relire_la_liste() {
        let e = |p: &str, actif: &str| effet_d_un_changement(p, ".fragment", actif);
        let r = |recharger, liste| EffetDUnChangement { recharger, liste };
        assert_eq!(e(".fragment/themes/Rose/theme.css", "Rose"), r(true, true));
        assert_eq!(
            e(".fragment/themes/Rose/sous/x.css", "Rose"),
            r(true, false)
        );
        assert_eq!(e(".fragment/themes/Bleu", "Rose"), r(false, true));
        assert_eq!(e(".fragment/themes", "Rose"), r(false, true));
        assert_eq!(e(".fragment/themes/Rose/theme.css", ""), r(false, true));
        assert_eq!(e(".fragment/appearance.json", "Rose"), r(false, false));
        assert_eq!(e(".fragment/themesX/a", "Rose"), r(false, false));
    }

    // ── ajouts du port (ADR-005 §4 et tri sans accents) ──
    #[test]
    fn le_theme_json_de_fragment_rs() {
        assert_eq!(
            chemin_du_theme_json(".fragment", "Rose"),
            ".fragment/themes/Rose/theme.json"
        );
    }

    #[test]
    fn le_tri_ignore_les_accents_comme_localecompare_base() {
        let dossiers: Vec<String> = ["d/Zèbre", "d/Été", "d/eau", "d/Abeille"]
            .map(String::from)
            .into();
        assert_eq!(
            lister_themes(&dossiers, &|_| true),
            vec!["Abeille", "eau", "Été", "Zèbre"]
        );
    }
}
