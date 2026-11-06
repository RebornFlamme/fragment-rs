// Port de app/src/ui/core/plugins/registry/types.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Deux sources derrière une même abstraction (§C3) : c'est ce qui permet d'offrir les
//  deux mondes. Une source réunit un registre et les dépôts de ses auteurs. Il en existe
//  deux, de même schéma et d'URL de base différente : le registre Obsidian et notre
//  canal GitHub `fragment-releases`, tous deux proposés par le navigateur (Browse).
//
//  ★ Transposition : les méthodes `async` du TS sont BLOQUANTES ici. Le transport
//    ([`Reseau`]) est injecté ; l'hôte exécute l'appel hors du fil de l'interface
//    (exécuteur de fond de GPUI), comme le `requestUrl` de Fragment passait par le
//    processus principal d'Electron.
// ═══════════════════════════════════════════════════════════════════════════

use fragment_core::PluginManifest;
use serde::{Deserialize, Serialize};

/// Une entrée du catalogue (community-plugins.json) — désigne un dépôt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    /// `owner/repo` GitHub.
    pub repo: String,
    /// Branche pour lire manifest/README (défaut : HEAD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Téléchargements cumulés (community-plugin-stats.json), si disponible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<f64>,
    /// Timestamp ms de dernière mise à jour (stats), si disponible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<f64>,
}

/// Les deux sources du navigateur de plugins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceId {
    Fragment,
    Obsidian,
}

impl SourceId {
    /// Le texte du TS (`'fragment' | 'obsidian'`), utilisé dans les messages d'erreur.
    pub fn as_str(self) -> &'static str {
        match self {
            SourceId::Fragment => "fragment",
            SourceId::Obsidian => "obsidian",
        }
    }
}

/// Les assets d'une release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assets {
    pub main: String,
    pub manifest: String,
    /// `styles.css` est toléré absent.
    pub styles: Option<String>,
}

/// Erreurs d'une source de registre (les `throw new Error` du TS, mêmes messages).
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("[registry:{source_id}] catalogue indisponible ({status})")]
    CatalogueIndisponible { source_id: String, status: u16 },
    #[error("[registry:{source_id}] manifest introuvable pour {id}")]
    ManifestIntrouvable { source_id: String, id: String },
    #[error(
        "[registry:{source_id}] assets manquants pour {id}@{version} (tag « {version} » sans « v » ?)"
    )]
    AssetsManquants {
        source_id: String,
        id: String,
        version: String,
    },
    /// Le transport a échoué (hors réseau, DNS…).
    #[error("réseau : {0}")]
    Reseau(String),
    /// Le corps reçu n'est pas le JSON attendu.
    #[error("JSON invalide : {0}")]
    Json(String),
}

/// Le contrat d'une source de plugins (registre + dépôts).
pub trait PluginRegistrySource {
    fn id(&self) -> SourceId;
    fn label(&self) -> &str;

    /// Le catalogue (community-plugins.json).
    fn list(&self) -> Result<Vec<CatalogEntry>, RegistryError>;
    /// Le manifest courant du dépôt (dernière version publiée).
    fn fetch_manifest(&self, entry: &CatalogEntry) -> Result<PluginManifest, RegistryError>;
    /// Le README rendu par le navigateur (Markdown brut), vide s'il n'y en a pas.
    fn fetch_readme(&self, entry: &CatalogEntry) -> Result<String, RegistryError>;
    /// La version à installer pour cette app : la plus haute dont `minAppVersion`
    /// ≤ `app_version` (versions.json, sinon le manifest).
    ///
    /// ★ `None` = AUCUNE n'est compatible : l'UI l'affiche « Incompatible » et
    ///   n'installe pas (plus de repli silencieux sur une version qui exige un
    ///   Fragment plus récent). Build de dev (`0.0.0`) ou version absente : la
    ///   dernière publiée.
    fn resolve_version(
        &self,
        entry: &CatalogEntry,
        app_version: Option<&str>,
    ) -> Result<Option<String>, RegistryError>;
    /// Télécharge les assets de la release `version`. `styles` toléré absent.
    fn download_assets(&self, entry: &CatalogEntry, version: &str)
    -> Result<Assets, RegistryError>;
}
