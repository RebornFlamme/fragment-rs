// Port de app/src/ui/core/settings/theme.ts (+ `accentChoisi` de settings/config.ts, testé
// dans theme.test.ts)
//
// LE SCHÉMA DE COULEURS — `theme-dark` / `theme-light`, comme Obsidian. `system` suit l'OS
// en direct. La partie pure : quel schéma est effectivement affiché, quel accent est
// choisi. Poser les classes sur `<body>` et écouter l'OS sont l'affaire de fragment-ui /
// fragment-theme.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaCouleurs {
    #[default]
    System,
    Dark,
    Light,
}

/// Le schéma effectivement affiché (sans `System`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaEffectif {
    Dark,
    Light,
}

/// Le schéma effectivement affiché pour un réglage (pur).
pub fn schema_effectif(schema: SchemaCouleurs, os_sombre: bool) -> SchemaEffectif {
    match schema {
        SchemaCouleurs::System if os_sombre => SchemaEffectif::Dark,
        SchemaCouleurs::System => SchemaEffectif::Light,
        SchemaCouleurs::Dark => SchemaEffectif::Dark,
        SchemaCouleurs::Light => SchemaEffectif::Light,
    }
}

impl SchemaCouleurs {
    /// `applyAppearance` : tout ce qui n'est ni `dark` ni `light` retombe sur `system`.
    pub fn depuis_reglage(valeur: Option<&str>) -> Self {
        match valeur {
            Some("dark") => Self::Dark,
            Some("light") => Self::Light,
            _ => Self::System,
        }
    }
}

/// Le défaut d'avant « accent du thème » : encore écrit dans d'anciens `appearance.json`.
pub const ANCIEN_ACCENT_PAR_DEFAUT: &str = "#646cff";

/// L'accent CHOISI par l'utilisateur, ou `""` = celui du thème. Pur.
///
/// `None` couvre `undefined` et les valeurs non textuelles du TS.
pub fn accent_choisi(valeur: Option<&str>) -> String {
    let Some(valeur) = valeur else {
        return String::new();
    };
    let v = valeur.trim();
    if v.to_lowercase() == ANCIEN_ACCENT_PAR_DEFAUT {
        String::new()
    } else {
        v.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SchemaCouleurs::*;

    #[test]
    fn system_suit_los_force_lignore() {
        assert_eq!(schema_effectif(System, true), SchemaEffectif::Dark);
        assert_eq!(schema_effectif(System, false), SchemaEffectif::Light);
        assert_eq!(schema_effectif(Dark, false), SchemaEffectif::Dark);
        assert_eq!(schema_effectif(Light, true), SchemaEffectif::Light);
    }

    #[test]
    fn vide_ou_ancien_defaut_laccent_du_theme() {
        assert_eq!(accent_choisi(Some("")), "");
        assert_eq!(accent_choisi(Some("#646cff")), "");
        assert_eq!(accent_choisi(Some("#646CFF")), "");
        assert_eq!(accent_choisi(None), "");
        assert_eq!(accent_choisi(Some("#e8559a")), "#e8559a");
    }

    // ─── Ajoutés ───
    #[test]
    fn un_reglage_inconnu_retombe_sur_system() {
        assert_eq!(SchemaCouleurs::depuis_reglage(Some("dark")), Dark);
        assert_eq!(SchemaCouleurs::depuis_reglage(Some("sepia")), System);
        assert_eq!(SchemaCouleurs::depuis_reglage(None), System);
    }
}
