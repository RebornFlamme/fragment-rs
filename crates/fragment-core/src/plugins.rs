// Port de app/src/ui/core/plugins.ts (le MODÈLE d'activation/chargement, pas l'évaluation JS)
//! Le chargeur de plugins (§11.2) — « l'app est une coquille + des plugins ».
//!
//! Deux populations, MÊME cycle de vie :
//!   - internes : des fabriques du binaire, enregistrées par `seedCore`. C'est le
//!     dogfooding du §11.6 — si markdown et l'explorateur vivent de l'API
//!     publique, l'API publique suffit ;
//!   - communautaires : un dossier `.fragment/plugins/<id>/` lu via le STOCKAGE
//!     (le chargeur ne touche jamais le disque directement), matérialisé par un
//!     `CommunityLoader` — l'hôte JS de `fragment-plugins`, qui évalue `main.js`
//!     avec son shim `require` et injecte `styles.css`.
//!
//! §11.4, à ne jamais oublier : le shim n'est PAS une sandbox. Un plugin
//! s'exécute avec les pleins pouvoirs de l'hôte — c'est le choix de gouvernance
//! d'Obsidian, assumé ici aussi. La seule « sécurité » est institutionnelle.
//!
//! Ce fichier porte les INVARIANTS : installé (FS) ≠ activé (JSON) ≠ chargé
//! (mémoire) ; mode restreint par défaut ; `reconcile` comme seule
//! matérialisation ; les refus d'`install_plugin` ; mise à jour = rechargement.
//! Les opérations sont synchrones (le stockage l'est côté GPUI), là où le TS
//! attendait des promesses.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ordered_map::OrderedMap;
use crate::plugin::{Plugin, PluginManifest};
use crate::semver::{compare_versions, is_newer_version};

/// Un id de plugin du catalogue (Obsidian : minuscules, chiffres, tirets). ★ Il
/// devient un NOM DE DOSSIER (`plugins/<id>/`) : rien d'autre ne passe (ni `..`,
/// ni `/`, ni `.`), vérifié AVANT tout téléchargement. (Le `ID_PLUGIN` du TS.)
pub fn id_plugin_valide(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("identifiant de plugin invalide : « {0} » (attendu : minuscules, chiffres et tirets)")]
    InvalidId(String),
    #[error("manifest illisible pour {id}@{version}")]
    UnreadableManifest { id: String, version: String },
    #[error(
        "le manifest de la release déclare l'id « {declared} », le catalogue « {expected} » : installation refusée"
    )]
    ManifestIdMismatch { declared: String, expected: String },
    #[error("{id}@{version} exige Fragment {min} ou plus récent (version actuelle : {current})")]
    RequiresNewerApp {
        id: String,
        version: String,
        min: String,
        current: String,
    },
    #[error("{0} est un plugin interne : il ne se recharge pas")]
    Internal(String),
    #[error("plugin introuvable : {path} (avec un `id` égal à « {id} »)")]
    NotFound { id: String, path: String },
    #[error("[plugins] inconnu : {0}")]
    Unknown(String),
    #[error("[plugins] stockage : {0}")]
    Io(#[from] std::io::Error),
    /// Une source de catalogue indisponible ou en erreur.
    #[error("[plugins] source : {0}")]
    Source(String),
    /// Le chargeur communautaire n'a pas pu matérialiser le plugin.
    #[error("[plugins] chargement : {0}")]
    Load(String),
}

// ── L'abstraction à deux sources (§C3, plugins/registry/types.ts) ──────────

/// Une entrée du catalogue (community-plugins.json) — désigne un dépôt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub downloads: Option<u64>,
    /// Timestamp ms de dernière mise à jour (stats), si disponible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<u64>,
}

/// Laquelle des deux sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceId {
    Fragment,
    Obsidian,
}

/// Les fichiers d'une release. `styles` toléré absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAssets {
    pub main: String,
    pub manifest: String,
    pub styles: Option<String>,
}

/// Le contrat d'une source de plugins (registre + dépôts).
pub trait PluginRegistrySource {
    fn id(&self) -> SourceId;
    fn label(&self) -> &str;
    /// Le catalogue (community-plugins.json).
    fn list(&self) -> Result<Vec<CatalogEntry>, PluginError>;
    /// Le manifest courant du dépôt (dernière version publiée).
    fn fetch_manifest(&self, entry: &CatalogEntry) -> Result<PluginManifest, PluginError>;
    /// Le README rendu par le navigateur (Markdown brut).
    fn fetch_readme(&self, entry: &CatalogEntry) -> Result<String, PluginError>;
    /// La version à installer pour cette app : la plus haute dont `minAppVersion`
    /// ≤ `app_version` (versions.json, sinon le manifest). ★ `None` = AUCUNE n'est
    /// compatible : l'UI l'affiche « Incompatible » et n'installe pas. Build de
    /// dev (`0.0.0`) ou version absente : la dernière publiée.
    fn resolve_version(
        &self,
        entry: &CatalogEntry,
        app_version: &str,
    ) -> Result<Option<String>, PluginError>;
    /// Télécharge les assets de la release `version`.
    fn download_assets(
        &self,
        entry: &CatalogEntry,
        version: &str,
    ) -> Result<PluginAssets, PluginError>;
}

/// Le contenu d'un dossier (`adapter.list`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    pub files: Vec<String>,
    pub folders: Vec<String>,
}

/// Le sous-ensemble de l'adapter du coffre que le chargeur utilise. Chemins
/// relatifs à la racine du coffre, séparateur `/`.
pub trait PluginStorage {
    fn exists(&self, path: &str) -> std::io::Result<bool>;
    fn list(&self, path: &str) -> std::io::Result<Listing>;
    fn read(&self, path: &str) -> std::io::Result<String>;
    fn write(&self, path: &str, data: &str) -> std::io::Result<()>;
    fn mkdir(&self, path: &str) -> std::io::Result<()>;
    fn remove(&self, path: &str) -> std::io::Result<()>;
    fn rmdir(&self, path: &str, recursive: bool) -> std::io::Result<()>;
}

/// Ce qui matérialise un plugin COMMUNAUTAIRE depuis son dossier : l'hôte JS
/// (évaluation de `main.js`, shim `require`, `styles.css`). Rend l'instance NON
/// chargée ; le chargeur appelle `load()`. Le retrait des styles s'enregistre
/// sur le Component du plugin, comme le reste.
pub trait CommunityLoader {
    fn instantiate(
        &self,
        dir: &str,
        manifest: &PluginManifest,
    ) -> Result<Rc<dyn Plugin>, PluginError>;
}

/// La fabrique d'un plugin interne (`seedCore`) : instanciée et chargée
/// exactement comme un plugin communautaire — AUCUN privilège.
pub type PluginCtor = Rc<dyn Fn(PluginManifest) -> Rc<dyn Plugin>>;

/// Une mise à jour disponible pour un plugin installé (§C4).
#[derive(Clone)]
pub struct PluginUpdate {
    pub version: String,
    pub source: Rc<dyn PluginRegistrySource>,
    pub entry: CatalogEntry,
}

impl std::fmt::Debug for PluginUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginUpdate")
            .field("version", &self.version)
            .field("entry", &self.entry.id)
            .finish()
    }
}

pub struct Plugins {
    storage: Rc<dyn PluginStorage>,
    loader: Rc<dyn CommunityLoader>,
    /// Le `configDir` du coffre (`.fragment`).
    config_dir: String,
    /// La version de Fragment (`app.version`) ; `0.0.0` = build de dev.
    app_version: RefCell<String>,

    /// Les instances chargées, par id.
    plugins: RefCell<OrderedMap<Rc<dyn Plugin>>>,
    /// Les manifestes découverts (chargés ou non) — INSTALLÉ = présent ici (FS).
    manifests: RefCell<OrderedMap<PluginManifest>>,
    /// Les ids ACTIVÉS (community-plugins.json). ACTIVÉ ≠ installé (§5.1 rapport).
    enabled_plugins: RefCell<Vec<String>>,
    /// Mises à jour disponibles, par id — peuplé par `check_for_updates()` (§7 : sur
    /// action utilisateur, JAMAIS au boot). Vide tant qu'on n'a pas vérifié.
    updates: RefCell<HashMap<String, PluginUpdate>>,
    /// Mode restreint (§11.4) : aucun plugin communautaire chargé. Convention
    /// d'Obsidian : `community-plugins.json` ABSENT ⇒ restreint. Un coffre neuf
    /// (ou cloné) est donc inoffensif à l'ouverture.
    restricted: Cell<bool>,
    /// Cette fenêtre MATÉRIALISE-t-elle les plugins ? (fenêtre principale = oui).
    /// Les fenêtres réglages/navigateur ont leur propre App SANS workspace : elles
    /// gèrent la liste et persistent, mais ne doivent JAMAIS charger un plugin
    /// communautaire (§0.5). Le boot de la fenêtre principale met ceci à true.
    is_host: Cell<bool>,
    /// La version (manifest) de chaque plugin communautaire CHARGÉ — pour que
    /// `reconcile` recharge un plugin mis à jour par une autre fenêtre.
    loaded_versions: RefCell<HashMap<String, String>>,
    /// Les fabriques internes enregistrées par seedCore, en attente de load.
    internal: RefCell<OrderedMap<(PluginManifest, PluginCtor)>>,
}

impl std::fmt::Debug for Plugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugins")
            .field("charges", &self.plugins.borrow().len())
            .field("installes", &self.manifests.borrow().len())
            .field("actives", &self.enabled_plugins.borrow().len())
            .field("restricted", &self.restricted.get())
            .finish()
    }
}

impl Plugins {
    pub fn new(
        storage: Rc<dyn PluginStorage>,
        loader: Rc<dyn CommunityLoader>,
        config_dir: &str,
        app_version: &str,
    ) -> Self {
        Self {
            storage,
            loader,
            config_dir: config_dir.to_owned(),
            app_version: RefCell::new(app_version.to_owned()),
            plugins: RefCell::new(OrderedMap::new()),
            manifests: RefCell::new(OrderedMap::new()),
            enabled_plugins: RefCell::new(Vec::new()),
            updates: RefCell::new(HashMap::new()),
            restricted: Cell::new(true),
            is_host: Cell::new(false),
            loaded_versions: RefCell::new(HashMap::new()),
            internal: RefCell::new(OrderedMap::new()),
        }
    }

    // ── Lecture de l'état ────────────────────────────────────────────────

    pub fn app_version(&self) -> String {
        self.app_version.borrow().clone()
    }

    pub fn set_app_version(&self, version: &str) {
        *self.app_version.borrow_mut() = version.to_owned();
    }

    pub fn restricted(&self) -> bool {
        self.restricted.get()
    }

    pub fn is_host(&self) -> bool {
        self.is_host.get()
    }

    pub fn set_is_host(&self, on: bool) {
        self.is_host.set(on);
    }

    /// L'instance chargée, s'il tourne.
    pub fn get_plugin(&self, id: &str) -> Option<Rc<dyn Plugin>> {
        self.plugins.borrow().get(id).cloned()
    }

    pub fn is_loaded(&self, id: &str) -> bool {
        self.plugins.borrow().contains_key(id)
    }

    /// Les ids chargés, dans l'ordre de chargement.
    pub fn loaded_ids(&self) -> Vec<String> {
        self.plugins.borrow().keys().map(str::to_owned).collect()
    }

    pub fn get_manifest(&self, id: &str) -> Option<PluginManifest> {
        self.manifests.borrow().get(id).cloned()
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.manifests.borrow().contains_key(id)
    }

    pub fn manifests(&self) -> Vec<PluginManifest> {
        self.manifests.borrow().values().cloned().collect()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled_plugins.borrow().iter().any(|e| e == id)
    }

    pub fn enabled_plugins(&self) -> Vec<String> {
        self.enabled_plugins.borrow().clone()
    }

    pub fn get_update(&self, id: &str) -> Option<PluginUpdate> {
        self.updates.borrow().get(id).cloned()
    }

    pub fn update_count(&self) -> usize {
        self.updates.borrow().len()
    }

    pub fn plugins_dir(&self) -> String {
        format!("{}/plugins", self.config_dir)
    }

    fn enabled_path(&self) -> String {
        format!("{}/community-plugins.json", self.config_dir)
    }

    /// Un id chargé qui n'est pas un plugin interne est communautaire.
    fn is_community(&self, id: &str) -> bool {
        !self.internal.borrow().contains_key(id)
    }

    /// Les manifestes INSTALLÉS communautaires (hors internes) — pour l'UI.
    pub fn community_manifests(&self) -> Vec<PluginManifest> {
        self.manifests
            .borrow()
            .values()
            .filter(|m| self.is_community(&m.id))
            .cloned()
            .collect()
    }

    /// seedCore passe par ici — AUCUN privilège : la fabrique sera appelée et le
    /// plugin chargé exactement comme un plugin communautaire.
    pub fn register_internal_plugin(&self, manifest: PluginManifest, ctor: PluginCtor) {
        self.manifests
            .borrow_mut()
            .insert(manifest.id.clone(), manifest.clone());
        self.internal
            .borrow_mut()
            .insert(manifest.id.clone(), (manifest, ctor));
    }

    /// §16.1 étape 5 — après le Vault, avant que l'UI ne dépende des vues.
    /// Internes d'abord (markdown, explorateur), puis : lire l'état activé/restreint,
    /// scanner les manifestes installés, et RÉCONCILIER (charger les activés).
    pub fn initialize(&self) -> Result<(), PluginError> {
        self.load_internal_plugins();
        self.load_enabled()?;
        self.scan_manifests()?;
        self.reconcile();
        Ok(())
    }

    /// Charge les plugins INTERNES (seedCore) — commandes, vues, onglets de réglages.
    /// SANS toucher aux communautaires (pas de reconcile). C'est ce dont une fenêtre
    /// NON-hôte (réglages) a besoin : le corpus de commandes pour l'éditeur de
    /// raccourcis, les onglets cœur, sans matérialiser un plugin communautaire une
    /// seconde fois (§0.5 — seule la fenêtre principale reconcilie). Idempotent :
    /// `instantiate` ignore un plugin déjà chargé.
    pub fn load_internal_plugins(&self) {
        let internes: Vec<(PluginManifest, PluginCtor)> =
            self.internal.borrow().values().cloned().collect();
        for (manifest, ctor) in internes {
            self.instantiate(&manifest, &ctor);
        }
    }

    // ── Installé (FS) ────────────────────────────────────────────────────────

    /// Peuple `manifests` pour TOUS les dossiers de `plugins/` — SANS rien charger.
    /// C'est « installé » : l'UI (et la fenêtre de réglages, sans workspace) s'en
    /// sert pour lister, indépendamment de ce qui tourne.
    pub fn scan_manifests(&self) -> Result<(), PluginError> {
        let dossier = self.plugins_dir();
        if !self.storage.exists(&dossier)? {
            return Ok(());
        }

        for dir in self.storage.list(&dossier)?.folders {
            let chemin = format!("{dir}/manifest.json");
            let brut = match self.storage.read(&chemin) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("[plugins] manifest illisible : {dir} ({e})");
                    continue;
                }
            };
            match serde_json::from_str::<PluginManifest>(&brut) {
                Ok(manifest) if !manifest.id.is_empty() && !manifest.name.is_empty() => {
                    self.manifests
                        .borrow_mut()
                        .insert(manifest.id.clone(), manifest);
                }
                Ok(_) => log::warn!("[plugins] manifest invalide, ignoré : {dir}"),
                Err(e) => log::error!("[plugins] manifest illisible : {dir} ({e})"),
            }
        }
        Ok(())
    }

    // ── Activé (JSON) + mode restreint ───────────────────────────────────────

    /// Lit `community-plugins.json` → `enabled_plugins`. Fichier ABSENT ⇒ mode
    /// restreint (rien d'activé). Tolérant : un JSON cassé vaut « rien d'activé ».
    pub fn load_enabled(&self) -> Result<(), PluginError> {
        let chemin = self.enabled_path();
        if !self.storage.exists(&chemin)? {
            self.restricted.set(true);
            self.enabled_plugins.borrow_mut().clear();
            return Ok(());
        }
        self.restricted.set(false);
        let ids: Vec<String> =
            match serde_json::from_str::<Vec<serde_json::Value>>(&self.storage.read(&chemin)?) {
                Ok(valeurs) => valeurs
                    .into_iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect(),
                Err(_) => Vec::new(),
            };
        *self.enabled_plugins.borrow_mut() = ids;
        Ok(())
    }

    fn save_enabled(&self) -> Result<(), PluginError> {
        self.storage.mkdir(&self.config_dir)?;
        let json = serde_json::to_string_pretty(&*self.enabled_plugins.borrow())
            .map_err(|e| PluginError::Load(e.to_string()))?;
        self.storage.write(&self.enabled_path(), &json)?;
        Ok(())
    }

    /// RÉCONCILIE l'état chargé avec `enabled_plugins`/`restricted` : charge les
    /// activés pas encore chargés, décharge les communautaires plus activés (ou
    /// tous si restreint). Les internes ne sont jamais touchés. C'est ce qu'appelle
    /// la fenêtre PRINCIPALE quand l'état change (au boot, ou via IPC cross-fenêtre).
    pub fn reconcile(&self) {
        // Décharge d'abord ce qui ne doit plus tourner.
        for id in self.loaded_ids() {
            if !self.is_community(&id) {
                continue;
            }
            if self.restricted.get() || !self.is_enabled(&id) {
                self.disable_plugin(&id);
            }
        }
        if self.restricted.get() {
            return;
        }

        // Charge les activés installés pas encore chargés — et RECHARGE ceux dont la
        // version sur disque n'est plus celle qui tourne (★ mis à jour depuis la
        // fenêtre de réglages / le navigateur : elles écrivent, la principale relit).
        for id in self.enabled_plugins() {
            let Some(manifest) = self.get_manifest(&id) else {
                continue;
            };
            if self.is_loaded(&id) {
                let meme_version =
                    self.loaded_versions.borrow().get(&id) == Some(&manifest.version);
                if !self.is_community(&id) || meme_version {
                    continue;
                }
                self.disable_plugin(&id);
            }
            if let Err(err) =
                self.load_community_plugin(&format!("{}/{id}", self.plugins_dir()), &manifest)
            {
                // Un plugin cassé ne bloque ni l'app ni les autres.
                log::error!("[plugins] échec du chargement de {id} : {err}");
            }
        }
    }

    /// Active/désactive un plugin et PERSISTE (community-plugins.json), SANS
    /// charger/décharger ici — c'est `reconcile()` qui matérialise (fenêtre
    /// principale). La fenêtre de réglages appelle ceci puis diffuse ; le reconcile
    /// a lieu dans la fenêtre qui tient le workspace.
    pub fn set_enabled(&self, id: &str, on: bool) -> Result<(), PluginError> {
        {
            let mut actives = self.enabled_plugins.borrow_mut();
            if on {
                if !actives.iter().any(|e| e == id) {
                    actives.push(id.to_owned());
                }
            } else {
                actives.retain(|e| e != id);
            }
        }
        self.save_enabled()
    }

    /// Bascule le mode restreint. ON ⇒ supprime community-plugins.json (convention
    /// d'Obsidian) ; OFF ⇒ le (re)crée. Persiste ; le reconcile suit (ici ou via IPC).
    pub fn set_restricted(&self, on: bool) -> Result<(), PluginError> {
        self.restricted.set(on);
        if on {
            let chemin = self.enabled_path();
            if self.storage.exists(&chemin)? {
                self.storage.remove(&chemin)?;
            }
            Ok(())
        } else {
            self.save_enabled()
        }
    }

    /// Active + persiste + matérialise (pour un appel DIRECT dans la fenêtre principale).
    pub fn enable_plugin_and_save(&self, id: &str) -> Result<(), PluginError> {
        self.set_enabled(id, true)?;
        self.reconcile();
        Ok(())
    }

    /// RECHARGE À CHAUD un plugin COMMUNAUTAIRE depuis le disque — son `main.js`
    /// frais (le chargeur relit le fichier à chaque fois) — et l'ACTIVE (persisté ;
    /// sort du mode restreint au besoin). C'est la voie d'un plugin écrit à la
    /// volée (l'agent : `fragment plugin-reload id=`). Contrairement à `reconcile`,
    /// l'erreur de chargement REMONTE : celui qui a écrit le code doit la lire.
    pub fn reload_plugin(&self, id: &str) -> Result<(), PluginError> {
        if !self.is_community(id) {
            return Err(PluginError::Internal(id.to_owned()));
        }
        let manifest = self.relire_manifeste(id)?;
        self.disable_plugin(id);
        if self.restricted.get() {
            self.set_restricted(false)?;
        }
        self.set_enabled(id, true)?;
        self.load_community_plugin(&format!("{}/{id}", self.plugins_dir()), &manifest)
    }

    /// Relit `plugins/<id>/manifest.json` (frais) ; erreur claire s'il manque.
    fn relire_manifeste(&self, id: &str) -> Result<PluginManifest, PluginError> {
        self.manifests.borrow_mut().remove(id);
        self.scan_manifests()?;
        self.get_manifest(id).ok_or_else(|| PluginError::NotFound {
            id: id.to_owned(),
            path: format!("{}/{id}/manifest.json", self.plugins_dir()),
        })
    }

    /// Désactive + persiste + matérialise.
    pub fn disable_plugin_and_save(&self, id: &str) -> Result<(), PluginError> {
        self.set_enabled(id, false)?;
        self.reconcile();
        Ok(())
    }

    // ── Installer / désinstaller (§C4) ─────────────────────────────────────────

    /// Télécharge les assets d'une release et les écrit dans `plugins/<id>/`.
    /// INSTALLÉ, pas activé (installé ≠ activé) : l'utilisateur active ensuite.
    /// Sert aussi à mettre à jour / rétrograder (écrase main.js/manifest/styles ;
    /// data.json intact). AUCUN téléchargement sans action utilisateur (§7).
    ///
    /// ★ Refuse (RIEN n'est écrit) : un id hors `id_plugin_valide` (avant tout
    ///   réseau), un manifest téléchargé illisible ou d'un AUTRE id que l'entrée du
    ///   catalogue, un `minAppVersion` plus récent que l'app (sauf build de dev `0.0.0`).
    pub fn install_plugin(
        &self,
        source: &dyn PluginRegistrySource,
        entry: &CatalogEntry,
        version: &str,
    ) -> Result<(), PluginError> {
        if !id_plugin_valide(&entry.id) {
            return Err(PluginError::InvalidId(entry.id.clone()));
        }
        let assets = source.download_assets(entry, version)?;
        let parsed: PluginManifest = serde_json::from_str(&assets.manifest).map_err(|_| {
            PluginError::UnreadableManifest {
                id: entry.id.clone(),
                version: version.to_owned(),
            }
        })?;
        if parsed.id != entry.id {
            return Err(PluginError::ManifestIdMismatch {
                declared: parsed.id,
                expected: entry.id.clone(),
            });
        }
        let app_version = self.app_version();
        if let Some(min) = &parsed.min_app_version
            && !app_version.is_empty()
            && app_version != "0.0.0"
            && compare_versions(min, &app_version) == std::cmp::Ordering::Greater
        {
            return Err(PluginError::RequiresNewerApp {
                id: entry.id.clone(),
                version: version.to_owned(),
                min: min.clone(),
                current: app_version,
            });
        }
        let dir = format!("{}/{}", self.plugins_dir(), entry.id);
        self.storage.mkdir(&dir)?;
        self.storage
            .write(&format!("{dir}/main.js"), &assets.main)?;
        self.storage
            .write(&format!("{dir}/manifest.json"), &assets.manifest)?;
        if let Some(styles) = &assets.styles {
            self.storage.write(&format!("{dir}/styles.css"), styles)?;
        }
        self.manifests.borrow_mut().insert(entry.id.clone(), parsed);
        Ok(())
    }

    /// §7 — cherche une version plus récente de CHAQUE plugin communautaire installé
    /// dans les catalogues des `sources` (`app.pluginRegistries`), et remplit `updates`.
    /// SUR ACTION UTILISATEUR uniquement : c'est du réseau, jamais au boot. La
    /// première source qui connaît un id gagne (Fragment avant Obsidian, l'ordre des
    /// registres). Un catalogue indisponible n'interrompt pas les autres.
    pub fn check_for_updates(&self, sources: &[Rc<dyn PluginRegistrySource>]) {
        self.updates.borrow_mut().clear();
        let installed = self.community_manifests();
        if installed.is_empty() {
            return;
        }
        let app_version = self.app_version();

        for source in sources {
            let Ok(catalog) = source.list() else {
                continue; // source indisponible → on tente la suivante
            };
            for manifest in &installed {
                if self.updates.borrow().contains_key(&manifest.id) {
                    continue; // déjà trouvée ailleurs
                }
                let Some(entry) = catalog.iter().find(|e| e.id == manifest.id) else {
                    continue;
                };
                // Version irrésolue pour ce plugin : on n'en fait pas une MAJ.
                // `None` = aucune version compatible : pas une MAJ non plus (le navigateur le montre).
                if let Ok(Some(version)) = source.resolve_version(entry, &app_version)
                    && is_newer_version(&version, &manifest.version)
                {
                    self.updates.borrow_mut().insert(
                        manifest.id.clone(),
                        PluginUpdate {
                            version,
                            source: source.clone(),
                            entry: entry.clone(),
                        },
                    );
                }
            }
        }
    }

    /// Applique la mise à jour trouvée pour `id` (réinstalle la version cible —
    /// écrase main.js/manifest/styles, data.json intact). Retire l'entrée de
    /// `updates`. No-op si aucune MAJ connue.
    ///
    /// ★ MISE À JOUR = RECHARGEMENT : s'il TOURNE dans cette fenêtre, il est
    ///   déchargé puis rechargé depuis le disque (le nouveau code, sans redémarrer).
    ///   Sans toucher au mode restreint ni à la liste activée (un plugin chargé est
    ///   déjà activé) — contrairement à `reload_plugin`. Ailleurs (fenêtre de
    ///   réglages), c'est le `reconcile` de la principale qui voit la version changer.
    ///   L'erreur de chargement REMONTE à l'appelant.
    pub fn update_plugin(&self, id: &str) -> Result<(), PluginError> {
        let Some(update) = self.get_update(id) else {
            return Ok(());
        };
        self.install_plugin(update.source.as_ref(), &update.entry, &update.version)?;
        self.updates.borrow_mut().remove(id);
        if self.is_loaded(id) && self.is_community(id) {
            let manifest = self.relire_manifeste(id)?;
            self.disable_plugin(id);
            self.load_community_plugin(&format!("{}/{id}", self.plugins_dir()), &manifest)?;
        }
        Ok(())
    }

    /// Supprime `plugins/<id>/` (y compris data.json → perte des réglages, assumé)
    /// et retire l'id de la liste activée. Décharge d'abord s'il tournait.
    pub fn uninstall_plugin(&self, id: &str) -> Result<(), PluginError> {
        self.disable_plugin(id);
        if !self.restricted.get() {
            self.enabled_plugins.borrow_mut().retain(|e| e != id);
            self.save_enabled()?;
        }
        self.storage
            .rmdir(&format!("{}/{id}", self.plugins_dir()), true)?;
        self.manifests.borrow_mut().remove(id);
        self.updates.borrow_mut().remove(id);
        Ok(())
    }

    /// §11.2 reconstitué : le chargeur matérialise le plugin depuis `dir` (lecture
    /// de main.js, shim, styles), on l'instancie, on le charge.
    fn load_community_plugin(
        &self,
        dir: &str,
        manifest: &PluginManifest,
    ) -> Result<(), PluginError> {
        let instance = self.loader.instantiate(dir, manifest)?;
        self.instancier(manifest, instance);
        self.loaded_versions
            .borrow_mut()
            .insert(manifest.id.clone(), manifest.version.clone());
        Ok(())
    }

    fn instantiate(&self, manifest: &PluginManifest, ctor: &PluginCtor) {
        if self.is_loaded(&manifest.id) {
            return;
        }
        let instance = ctor(manifest.clone());
        self.instancier(manifest, instance);
    }

    fn instancier(&self, manifest: &PluginManifest, instance: Rc<dyn Plugin>) {
        if self.is_loaded(&manifest.id) {
            return;
        }
        self.plugins
            .borrow_mut()
            .insert(manifest.id.clone(), instance.clone());
        instance.load(); // → on_load() : le plugin remplit les registres (§3.2)
    }

    /// §16.4 — la désactivation est L'EXACT MIROIR : `unload()` rejoue tous les
    /// register* en sens inverse, les registres se vident (garde d'identité
    /// comprise), les vues ouvertes du plugin dégradent en placeholder — et se
    /// rematérialiseront à la réactivation, via viewRegistry.on('changed').
    pub fn disable_plugin(&self, id: &str) {
        let Some(instance) = self.plugins.borrow_mut().remove(id) else {
            return;
        };
        instance.unload();
        self.loaded_versions.borrow_mut().remove(id);
    }

    pub fn enable_plugin(&self, id: &str) -> Result<(), PluginError> {
        let interne = self.internal.borrow().get(id).cloned();
        if let Some((manifest, ctor)) = interne {
            self.instantiate(&manifest, &ctor);
            return Ok(());
        }
        let manifest = self
            .get_manifest(id)
            .ok_or_else(|| PluginError::Unknown(id.to_owned()))?;
        self.load_community_plugin(&format!("{}/{id}", self.plugins_dir()), &manifest)
    }
}

#[cfg(test)]
mod tests {
    //! Le cœur de la gestion des plugins (§C1/§C2) : installé (FS) ≠ activé (JSON),
    //! mode restreint, reconcile (charge/décharge live). Le `main.js` du TS était
    //! évalué par `new Function` ; ici un `CommunityLoader` bidon lit `main.js`
    //! dans le stockage et pose son contenu comme MARQUE au `on_load` — ce que
    //! faisait `window.__marque`.
    use super::*;
    use crate::command::tests::faire_app;
    use crate::component::{Component, ComponentState};
    use crate::plugin::tests::HoteBidon;
    use crate::plugin::{PluginBase, PluginHost};
    use crate::registry_hub::RegistryHub;
    use std::collections::BTreeMap;

    // ── Un adapter de coffre en mémoire (le sous-ensemble utilisé par Plugins) ──
    #[derive(Default)]
    struct MockAdapter {
        files: RefCell<BTreeMap<String, String>>,
    }

    impl MockAdapter {
        fn set(&self, p: &str, v: &str) {
            self.files.borrow_mut().insert(p.to_owned(), v.to_owned());
        }
        fn get(&self, p: &str) -> Option<String> {
            self.files.borrow().get(p).cloned()
        }
        fn has(&self, p: &str) -> bool {
            self.files.borrow().contains_key(p)
        }
        fn keys(&self) -> Vec<String> {
            self.files.borrow().keys().cloned().collect()
        }
    }

    fn prefixe(p: &str) -> String {
        if p.ends_with('/') {
            p.to_owned()
        } else {
            format!("{p}/")
        }
    }

    impl PluginStorage for MockAdapter {
        fn exists(&self, p: &str) -> std::io::Result<bool> {
            if self.has(p) {
                return Ok(true);
            }
            let prefixe = prefixe(p);
            Ok(self.keys().iter().any(|k| k.starts_with(&prefixe)))
        }
        fn list(&self, p: &str) -> std::io::Result<Listing> {
            let prefixe = prefixe(p);
            let mut listing = Listing::default();
            for k in self.keys() {
                let Some(rest) = k.strip_prefix(&prefixe) else {
                    continue;
                };
                match rest.find('/') {
                    None => listing.files.push(k.clone()),
                    Some(slash) => {
                        let dossier = format!("{prefixe}{}", &rest[..slash]);
                        if !listing.folders.contains(&dossier) {
                            listing.folders.push(dossier);
                        }
                    }
                }
            }
            Ok(listing)
        }
        fn read(&self, p: &str) -> std::io::Result<String> {
            self.get(p).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, format!("ENOENT {p}"))
            })
        }
        fn write(&self, p: &str, data: &str) -> std::io::Result<()> {
            self.set(p, data);
            Ok(())
        }
        fn mkdir(&self, _p: &str) -> std::io::Result<()> {
            Ok(()) // no-op en mémoire
        }
        fn remove(&self, p: &str) -> std::io::Result<()> {
            self.files.borrow_mut().remove(p);
            Ok(())
        }
        fn rmdir(&self, p: &str, _recursive: bool) -> std::io::Result<()> {
            let prefixe = prefixe(p);
            self.files
                .borrow_mut()
                .retain(|k, _| k != p && !k.starts_with(&prefixe));
            Ok(())
        }
    }

    // ── Le plugin matérialisé : pose la marque (le contenu de main.js) au on_load ──
    struct PluginMarque {
        base: PluginBase,
        code: String,
        marque: Rc<RefCell<String>>,
    }

    impl Component for PluginMarque {
        fn state(&self) -> &ComponentState {
            &self.base.state
        }
        fn on_load(&self) {
            *self.marque.borrow_mut() = self.code.clone();
        }
    }

    impl Plugin for PluginMarque {
        fn plugin(&self) -> &PluginBase {
            &self.base
        }
    }

    struct LoaderBidon {
        storage: Rc<MockAdapter>,
        hote: Rc<dyn PluginHost>,
        marque: Rc<RefCell<String>>,
    }

    impl CommunityLoader for LoaderBidon {
        fn instantiate(
            &self,
            dir: &str,
            manifest: &PluginManifest,
        ) -> Result<Rc<dyn Plugin>, PluginError> {
            let code = self.storage.read(&format!("{dir}/main.js"))?;
            Ok(Rc::new(PluginMarque {
                base: PluginBase::new(self.hote.clone(), manifest.clone()),
                code,
                marque: self.marque.clone(),
            }))
        }
    }

    const PLUGIN_CODE: &str = "const { Plugin } = require('fragment'); module.exports = class extends Plugin { onload() {} };";

    struct Banc {
        adapter: Rc<MockAdapter>,
        plugins: Plugins,
        marque: Rc<RefCell<String>>,
        _app: Rc<dyn crate::command::CommandHost>,
    }

    fn make_app() -> Banc {
        let adapter = Rc::new(MockAdapter::default());
        // Un plugin communautaire installé : dossier + manifest + main.js.
        adapter.set(
            ".fragment/plugins/demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"1.0.0"}"#,
        );
        adapter.set(".fragment/plugins/demo/main.js", PLUGIN_CODE);
        let (app, commands) = faire_app(None);
        let hote: Rc<dyn PluginHost> = Rc::new(HoteBidon {
            commands,
            registry: RegistryHub::new(),
        });
        let marque = Rc::new(RefCell::new(String::new()));
        let loader = Rc::new(LoaderBidon {
            storage: adapter.clone(),
            hote,
            marque: marque.clone(),
        });
        let plugins = Plugins::new(adapter.clone(), loader, ".fragment", "");
        Banc {
            adapter,
            plugins,
            marque,
            _app: app,
        }
    }

    fn json_ids(adapter: &MockAdapter, p: &str) -> Vec<String> {
        serde_json::from_str(&adapter.get(p).unwrap()).unwrap()
    }

    // ── Une source bidon, paramétrable (manifest distant, code, compteur) ──
    struct SourceBidon {
        entries: Vec<CatalogEntry>,
        manifeste_distant: RefCell<serde_json::Value>,
        code: RefCell<String>,
        styles: Option<String>,
        telechargements: Cell<usize>,
        /// `None` = `resolveVersion` rend `null` (incompatible).
        resout: bool,
    }

    impl SourceBidon {
        fn new(entries: Vec<CatalogEntry>, manifeste: serde_json::Value) -> Self {
            Self {
                entries,
                manifeste_distant: RefCell::new(manifeste),
                code: RefCell::new("module.exports = class {};".to_owned()),
                styles: None,
                telechargements: Cell::new(0),
                resout: true,
            }
        }
        fn version(&self) -> String {
            self.manifeste_distant.borrow()["version"]
                .as_str()
                .unwrap_or("")
                .to_owned()
        }
    }

    impl PluginRegistrySource for SourceBidon {
        fn id(&self) -> SourceId {
            SourceId::Fragment
        }
        fn label(&self) -> &str {
            "Fragment"
        }
        fn list(&self) -> Result<Vec<CatalogEntry>, PluginError> {
            Ok(self.entries.clone())
        }
        fn fetch_manifest(&self, _e: &CatalogEntry) -> Result<PluginManifest, PluginError> {
            serde_json::from_value(self.manifeste_distant.borrow().clone())
                .map_err(|e| PluginError::Source(e.to_string()))
        }
        fn fetch_readme(&self, _e: &CatalogEntry) -> Result<String, PluginError> {
            Ok(String::new())
        }
        fn resolve_version(
            &self,
            _e: &CatalogEntry,
            _app: &str,
        ) -> Result<Option<String>, PluginError> {
            Ok(self.resout.then(|| self.version()))
        }
        fn download_assets(
            &self,
            _e: &CatalogEntry,
            _v: &str,
        ) -> Result<PluginAssets, PluginError> {
            self.telechargements.set(self.telechargements.get() + 1);
            Ok(PluginAssets {
                main: self.code.borrow().clone(),
                manifest: self.manifeste_distant.borrow().to_string(),
                styles: self.styles.clone(),
            })
        }
    }

    fn entree(id: &str) -> CatalogEntry {
        CatalogEntry {
            id: id.to_owned(),
            name: if id == "demo" { "Demo" } else { "New" }.to_owned(),
            author: "A".to_owned(),
            description: String::new(),
            repo: format!("a/{id}"),
            branch: None,
            downloads: None,
            updated: None,
        }
    }

    fn manifeste(id: &str, name: &str, version: &str) -> serde_json::Value {
        serde_json::json!({ "id": id, "name": name, "version": version })
    }

    // Plugins — installé ≠ activé, mode restreint, reconcile

    #[test]
    fn coffre_neuf_pas_de_community_plugins_json_mode_restreint_rien_de_charge() {
        let b = make_app();
        b.plugins.load_enabled().unwrap();
        b.plugins.scan_manifests().unwrap();
        b.plugins.reconcile();

        assert!(b.plugins.restricted());
        assert!(b.plugins.is_installed("demo")); // installé
        assert!(!b.plugins.is_loaded("demo")); // pas chargé
    }

    #[test]
    fn activer_un_plugin_persiste_la_liste_et_le_reconcile_le_charge() {
        let b = make_app();
        b.plugins.load_enabled().unwrap();
        b.plugins.scan_manifests().unwrap();
        b.plugins.set_restricted(false).unwrap(); // sort du mode restreint
        b.plugins.set_enabled("demo", true).unwrap(); // persiste
        b.plugins.reconcile(); // matérialise

        assert!(b.plugins.is_loaded("demo"));
        assert_eq!(
            json_ids(&b.adapter, ".fragment/community-plugins.json"),
            vec!["demo"]
        );
    }

    #[test]
    fn desactiver_decharge_mais_garde_le_plugin_installe_fichiers_intacts() {
        let b = make_app();
        b.plugins.load_enabled().unwrap();
        b.plugins.scan_manifests().unwrap();
        b.plugins.set_restricted(false).unwrap();
        b.plugins.enable_plugin_and_save("demo").unwrap();
        assert!(b.plugins.is_loaded("demo"));

        b.plugins.disable_plugin_and_save("demo").unwrap();
        assert!(!b.plugins.is_loaded("demo")); // déchargé
        assert!(b.plugins.is_installed("demo")); // toujours installé
        assert!(b.adapter.has(".fragment/plugins/demo/main.js")); // fichiers intacts
        assert!(json_ids(&b.adapter, ".fragment/community-plugins.json").is_empty());
    }

    #[test]
    fn mode_restreint_active_a_chaud_decharge_les_communautaires_retire_le_json() {
        let b = make_app();
        b.plugins.load_enabled().unwrap();
        b.plugins.scan_manifests().unwrap();
        b.plugins.set_restricted(false).unwrap();
        b.plugins.enable_plugin_and_save("demo").unwrap();
        assert!(b.plugins.is_loaded("demo"));

        b.plugins.set_restricted(true).unwrap();
        b.plugins.reconcile();

        assert!(!b.plugins.is_loaded("demo"));
        assert!(!b.adapter.has(".fragment/community-plugins.json"));
    }

    #[test]
    fn un_id_active_mais_sans_dossier_installe_est_ignore_pas_de_crash() {
        let b = make_app();
        b.adapter
            .set(".fragment/community-plugins.json", r#"["fantome"]"#);
        b.plugins.load_enabled().unwrap();
        b.plugins.scan_manifests().unwrap();
        b.plugins.reconcile();

        assert!(b.plugins.is_enabled("fantome"));
        assert!(!b.plugins.is_loaded("fantome"));
    }

    // Plugins — install / uninstall (§C4)

    fn source_newp() -> SourceBidon {
        let mut s = SourceBidon::new(vec![entree("newp")], manifeste("newp", "New", "2.0.0"));
        s.styles = Some(".x {}".to_owned());
        s
    }

    #[test]
    fn install_plugin_ecrit_les_assets_sur_disque_et_marque_le_plugin_installe_pas_active() {
        let b = make_app();
        let source = source_newp();
        b.plugins
            .install_plugin(&source, &entree("newp"), "2.0.0")
            .unwrap();

        assert_eq!(
            b.adapter.get(".fragment/plugins/newp/main.js").as_deref(),
            Some("module.exports = class {};")
        );
        assert!(b.adapter.has(".fragment/plugins/newp/manifest.json"));
        assert_eq!(
            b.adapter
                .get(".fragment/plugins/newp/styles.css")
                .as_deref(),
            Some(".x {}")
        );
        assert!(b.plugins.is_installed("newp")); // installé
        assert!(!b.plugins.is_enabled("newp")); // pas activé
    }

    #[test]
    fn uninstall_plugin_supprime_le_dossier_et_retire_des_manifestes() {
        let b = make_app();
        let source = source_newp();
        b.plugins
            .install_plugin(&source, &entree("newp"), "2.0.0")
            .unwrap();
        b.plugins.uninstall_plugin("newp").unwrap();

        assert!(!b.adapter.has(".fragment/plugins/newp/main.js"));
        assert!(!b.adapter.has(".fragment/plugins/newp/manifest.json"));
        assert!(!b.plugins.is_installed("newp"));
    }

    // Plugins — checkForUpdates / updatePlugin (§7)
    // 'demo' est installé en v1.0.0 (make_app). La source annonce `latest`.

    fn banc_avec_source(latest: &str) -> (Banc, Vec<Rc<dyn PluginRegistrySource>>) {
        let b = make_app();
        b.plugins.set_app_version("1.0.0");
        let source: Rc<dyn PluginRegistrySource> = Rc::new(SourceBidon::new(
            vec![entree("demo")],
            manifeste("demo", "Demo", latest),
        ));
        (b, vec![source])
    }

    #[test]
    fn detecte_une_version_catalogue_plus_recente_que_la_version_installee() {
        let (b, sources) = banc_avec_source("1.2.0");
        b.plugins.scan_manifests().unwrap();
        b.plugins.check_for_updates(&sources);

        assert_eq!(
            b.plugins.get_update("demo").map(|u| u.version).as_deref(),
            Some("1.2.0")
        );
    }

    #[test]
    fn aucune_maj_quand_le_catalogue_n_est_pas_plus_recent() {
        let (b, sources) = banc_avec_source("1.0.0");
        b.plugins.scan_manifests().unwrap();
        b.plugins.check_for_updates(&sources);

        assert!(b.plugins.get_update("demo").is_none());
    }

    #[test]
    fn update_plugin_reinstalle_la_version_cible_et_vide_l_entree_updates() {
        let (b, sources) = banc_avec_source("1.2.0");
        b.plugins.scan_manifests().unwrap();
        b.plugins.check_for_updates(&sources);
        b.plugins.update_plugin("demo").unwrap();

        let manifest: PluginManifest = serde_json::from_str(
            &b.adapter
                .get(".fragment/plugins/demo/manifest.json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.version, "1.2.0");
        assert!(b.plugins.get_update("demo").is_none());
    }

    // Plugins — validations à l'installation, MAJ = rechargement

    fn banc_validations() -> (Banc, Rc<SourceBidon>) {
        let b = make_app();
        b.plugins.set_app_version("1.0.0");
        let source = Rc::new(SourceBidon::new(
            vec![entree("newp")],
            manifeste("newp", "New", "2.0.0"),
        ));
        (b, source)
    }

    #[test]
    fn un_id_hors_a_z0_9_est_refuse_avant_tout_telechargement() {
        let (b, source) = banc_validations();
        for id in ["../evil", "a/b", "Majuscule", "point.id", ""] {
            let err = b
                .plugins
                .install_plugin(
                    source.as_ref(),
                    &CatalogEntry {
                        id: id.to_owned(),
                        ..entree("newp")
                    },
                    "2.0.0",
                )
                .unwrap_err();
            assert!(
                err.to_string().contains("identifiant de plugin invalide"),
                "{id}: {err}"
            );
        }
        assert_eq!(source.telechargements.get(), 0);
        assert!(
            b.adapter
                .keys()
                .iter()
                .all(|k| k.starts_with(".fragment/plugins/demo/"))
        );
    }

    #[test]
    fn un_manifest_d_un_autre_id_que_le_catalogue_refuse_rien_n_est_ecrit() {
        let (b, source) = banc_validations();
        *source.manifeste_distant.borrow_mut() = manifeste("autre", "X", "2.0.0");
        let err = b
            .plugins
            .install_plugin(source.as_ref(), &entree("newp"), "2.0.0")
            .unwrap_err();
        assert!(err.to_string().contains("déclare l'id « autre »"), "{err}");
        assert!(!b.adapter.has(".fragment/plugins/newp/main.js"));
        assert!(!b.adapter.has(".fragment/plugins/autre/main.js"));
        assert!(!b.plugins.is_installed("newp"));
    }

    #[test]
    fn min_app_version_plus_recent_que_l_app_refuse_egal_ou_plus_ancien_installe() {
        let (b, source) = banc_validations();
        *source.manifeste_distant.borrow_mut() = serde_json::json!({ "id": "newp", "name": "New", "version": "2.0.0", "minAppVersion": "1.5.0" });
        let err = b
            .plugins
            .install_plugin(source.as_ref(), &entree("newp"), "2.0.0")
            .unwrap_err();
        assert!(err.to_string().contains("exige Fragment 1.5.0"), "{err}");
        assert!(!b.adapter.has(".fragment/plugins/newp/main.js"));

        *source.manifeste_distant.borrow_mut() = serde_json::json!({ "id": "newp", "name": "New", "version": "2.0.0", "minAppVersion": "1.0.0" });
        b.plugins
            .install_plugin(source.as_ref(), &entree("newp"), "2.0.0")
            .unwrap();
        assert!(b.plugins.is_installed("newp"));
    }

    #[test]
    fn build_de_dev_0_0_0_min_app_version_ne_bloque_pas() {
        let (b, source) = banc_validations();
        b.plugins.set_app_version("0.0.0");
        *source.manifeste_distant.borrow_mut() = serde_json::json!({ "id": "newp", "name": "New", "version": "2.0.0", "minAppVersion": "9.9.9" });
        b.plugins
            .install_plugin(source.as_ref(), &entree("newp"), "2.0.0")
            .unwrap();
        assert!(b.adapter.has(".fragment/plugins/newp/main.js"));
    }

    #[test]
    fn resolve_version_null_incompatible_pas_une_mise_a_jour() {
        let (b, _) = banc_validations();
        let mut nulle = SourceBidon::new(vec![entree("demo")], manifeste("demo", "Demo", "2.0.0"));
        nulle.resout = false;
        let sources: Vec<Rc<dyn PluginRegistrySource>> = vec![Rc::new(nulle)];
        b.plugins.scan_manifests().unwrap();
        b.plugins.check_for_updates(&sources);
        assert_eq!(b.plugins.update_count(), 0);
    }

    /// Le `pluginCode(marque)` du TS : ici, le contenu de main.js EST la marque.
    fn plugin_code(marque: &str) -> String {
        marque.to_owned()
    }

    #[test]
    fn update_plugin_d_un_plugin_charge_le_nouveau_code_tourne_restreint_actives_intacts() {
        let (b, source) = banc_validations();
        let sources: Vec<Rc<dyn PluginRegistrySource>> = vec![source.clone()];
        *source.manifeste_distant.borrow_mut() = manifeste("newp", "New", "1.0.0");
        *source.code.borrow_mut() = plugin_code("v1");
        b.plugins.load_enabled().unwrap();
        b.plugins.set_restricted(false).unwrap();
        b.plugins
            .install_plugin(source.as_ref(), &entree("newp"), "1.0.0")
            .unwrap();
        b.plugins.enable_plugin_and_save("newp").unwrap();
        assert_eq!(*b.marque.borrow(), "v1");
        let avant = b.plugins.get_plugin("newp").unwrap();

        *source.manifeste_distant.borrow_mut() = manifeste("newp", "New", "1.0.1");
        *source.code.borrow_mut() = plugin_code("v2");
        b.plugins.check_for_updates(&sources);
        assert_eq!(
            b.plugins.get_update("newp").map(|u| u.version).as_deref(),
            Some("1.0.1")
        );
        b.plugins.update_plugin("newp").unwrap();

        assert_eq!(*b.marque.borrow(), "v2");
        assert!(!Rc::ptr_eq(&b.plugins.get_plugin("newp").unwrap(), &avant));
        assert!(!b.plugins.restricted());
        assert_eq!(
            json_ids(&b.adapter, ".fragment/community-plugins.json"),
            vec!["newp"]
        );
    }

    #[test]
    fn update_plugin_d_un_plugin_non_charge_fenetre_de_reglages_ecrit_ne_charge_rien() {
        let (b, source) = banc_validations();
        let sources: Vec<Rc<dyn PluginRegistrySource>> = vec![source.clone()];
        *source.manifeste_distant.borrow_mut() = manifeste("newp", "New", "1.0.0");
        b.plugins
            .install_plugin(source.as_ref(), &entree("newp"), "1.0.0")
            .unwrap();
        *source.manifeste_distant.borrow_mut() = manifeste("newp", "New", "1.0.1");
        b.plugins.check_for_updates(&sources);
        b.plugins.update_plugin("newp").unwrap();
        assert!(!b.plugins.is_loaded("newp"));
        let manifest: PluginManifest = serde_json::from_str(
            &b.adapter
                .get(".fragment/plugins/newp/manifest.json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.version, "1.0.1");
    }

    #[test]
    fn reconcile_recharge_un_plugin_dont_la_version_sur_disque_a_change_maj_par_une_autre_fenetre()
    {
        let (b, _) = banc_validations();
        b.plugins.load_enabled().unwrap();
        b.plugins.scan_manifests().unwrap();
        b.plugins.set_restricted(false).unwrap();
        b.adapter
            .set(".fragment/plugins/demo/main.js", &plugin_code("d1"));
        b.plugins.enable_plugin_and_save("demo").unwrap();
        assert_eq!(*b.marque.borrow(), "d1");

        // Rien n'a changé : pas de rechargement.
        let avant = b.plugins.get_plugin("demo").unwrap();
        b.plugins.reconcile();
        assert!(Rc::ptr_eq(&b.plugins.get_plugin("demo").unwrap(), &avant));

        // Une autre fenêtre a écrit la v1.0.1.
        b.adapter.set(
            ".fragment/plugins/demo/manifest.json",
            r#"{"id":"demo","name":"Demo","version":"1.0.1"}"#,
        );
        b.adapter
            .set(".fragment/plugins/demo/main.js", &plugin_code("d2"));
        b.plugins.scan_manifests().unwrap();
        b.plugins.reconcile();
        assert_eq!(*b.marque.borrow(), "d2");
        assert!(!Rc::ptr_eq(&b.plugins.get_plugin("demo").unwrap(), &avant));
    }

    #[test]
    fn un_plugin_interne_se_charge_par_sa_fabrique_et_ne_se_recharge_pas() {
        // Ajouté au port : `register_internal_plugin`, `load_internal_plugins`, `reload_plugin` refusé.
        let b = make_app();
        let (_app, commands) = faire_app(None);
        let hote: Rc<dyn PluginHost> = Rc::new(HoteBidon {
            commands,
            registry: RegistryHub::new(),
        });
        let marque = b.marque.clone();
        b.plugins.register_internal_plugin(
            PluginManifest::new("interne", "Interne", "0.0.1"),
            Rc::new(move |manifest| {
                Rc::new(PluginMarque {
                    base: PluginBase::new(hote.clone(), manifest),
                    code: "interne".to_owned(),
                    marque: marque.clone(),
                })
            }),
        );
        b.plugins.initialize().unwrap();
        assert!(b.plugins.is_loaded("interne"));
        assert_eq!(*b.marque.borrow(), "interne");
        assert!(
            b.plugins
                .community_manifests()
                .iter()
                .all(|m| m.id != "interne")
        );
        assert!(matches!(
            b.plugins.reload_plugin("interne"),
            Err(PluginError::Internal(_))
        ));
        // Un reconcile en mode restreint ne touche pas aux internes.
        b.plugins.reconcile();
        assert!(b.plugins.is_loaded("interne"));
    }
}
