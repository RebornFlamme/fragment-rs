// Port de app/src/ui/core/plugins/registry/GithubRegistrySource.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Une source GitHub (§C3) — le modèle d'Obsidian, paramétré par le dépôt de
//  registre. Obsidian et Fragment n'en sont que deux instances (voir sources.ts).
//
//  ★ URLs EXACTES du rapport (§4) : catalogue/manifest/README via
//    raw.githubusercontent.com ; assets via github.com/.../releases/download/ —
//    JAMAIS api.github.com (rate-limit anonyme). Le tag = `manifest.version` SANS `v`.
// ═══════════════════════════════════════════════════════════════════════════

use std::cmp::Ordering;
use std::collections::HashMap;

use fragment_core::{PluginManifest, compare_versions};
use serde::Deserialize;

use crate::registry_types::{Assets, CatalogEntry, PluginRegistrySource, RegistryError, SourceId};

/// La réponse d'une requête (`requestUrl` : statut + corps texte).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reponse {
    pub status: u16,
    pub text: String,
}

/// Le transport HTTP, injecté (un faux dépôt dans les tests, un client HTTP dans l'app).
///
/// `Err` : le transport lui-même a échoué (le `requestUrl` du TS lève).
pub trait Reseau {
    fn requete(&self, url: &str) -> Result<Reponse, String>;
}

pub struct GithubRegistrySource<R: Reseau> {
    id: SourceId,
    label: String,
    registry_repo: String,
    registry_branch: String,
    reseau: R,
}

#[derive(Deserialize)]
struct Stats {
    downloads: Option<f64>,
    updated: Option<f64>,
}

impl<R: Reseau> GithubRegistrySource<R> {
    /// `registry_branch` : `HEAD` si absent.
    pub fn new(
        id: SourceId,
        label: &str,
        registry_repo: &str,
        registry_branch: Option<&str>,
        reseau: R,
    ) -> Self {
        Self {
            id,
            label: label.to_string(),
            registry_repo: registry_repo.to_string(),
            registry_branch: registry_branch.unwrap_or("HEAD").to_string(),
            reseau,
        }
    }

    fn raw(repo: &str, branch: &str, file: &str) -> String {
        format!("https://raw.githubusercontent.com/{repo}/{branch}/{file}")
    }

    fn raw_de(entry: &CatalogEntry, file: &str) -> String {
        Self::raw(&entry.repo, entry.branch.as_deref().unwrap_or("HEAD"), file)
    }

    fn get(&self, url: &str) -> Result<Reponse, RegistryError> {
        self.reseau.requete(url).map_err(RegistryError::Reseau)
    }

    fn sid(&self) -> String {
        self.id.as_str().to_string()
    }
}

impl<R: Reseau> PluginRegistrySource for GithubRegistrySource<R> {
    fn id(&self) -> SourceId {
        self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn list(&self) -> Result<Vec<CatalogEntry>, RegistryError> {
        let res = self.get(&Self::raw(
            &self.registry_repo,
            &self.registry_branch,
            "community-plugins.json",
        ))?;
        if res.status != 200 {
            return Err(RegistryError::CatalogueIndisponible {
                source_id: self.sid(),
                status: res.status,
            });
        }
        let mut entries: Vec<CatalogEntry> =
            serde_json::from_str(&res.text).map_err(|e| RegistryError::Json(e.to_string()))?;

        // Stats (downloads / updated) — OPTIONNELLES : le canal Fragment naissant
        // peut ne pas les avoir. ★ Un échec (réseau, JSON) ne prive pas du catalogue.
        let stats = self.get(&Self::raw(
            &self.registry_repo,
            &self.registry_branch,
            "community-plugin-stats.json",
        ));
        if let Ok(stats) = stats
            && stats.status == 200
            && let Ok(table) = serde_json::from_str::<HashMap<String, Stats>>(&stats.text)
        {
            for e in &mut entries {
                if let Some(s) = table.get(&e.id) {
                    e.downloads = s.downloads;
                    e.updated = s.updated;
                }
            }
        }
        Ok(entries)
    }

    fn fetch_manifest(&self, entry: &CatalogEntry) -> Result<PluginManifest, RegistryError> {
        let res = self.get(&Self::raw_de(entry, "manifest.json"))?;
        if res.status != 200 {
            return Err(RegistryError::ManifestIntrouvable {
                source_id: self.sid(),
                id: entry.id.clone(),
            });
        }
        serde_json::from_str(&res.text).map_err(|e| RegistryError::Json(e.to_string()))
    }

    fn fetch_readme(&self, entry: &CatalogEntry) -> Result<String, RegistryError> {
        let res = self.get(&Self::raw_de(entry, "README.md"))?;
        Ok(if res.status == 200 {
            res.text
        } else {
            String::new()
        })
    }

    fn resolve_version(
        &self,
        entry: &CatalogEntry,
        app_version: Option<&str>,
    ) -> Result<Option<String>, RegistryError> {
        // La dernière version publiée = manifest.version. Mais elle peut exiger un
        // Fragment plus récent que le nôtre : on consulte alors `versions.json`
        // (mappe pluginVersion → minAppVersion, comme chez Obsidian) et on choisit
        // la PLUS HAUTE version dont minAppVersion ≤ appVersion. Compat clients
        // anciens : un plugin reste installable dans sa dernière version supportée.
        // ★ Rien de compatible → `None` (jamais de repli silencieux sur une version
        //   qui exige un Fragment plus récent). Build de dev (0.0.0) : la dernière.
        let manifest = self.fetch_manifest(entry)?;
        let app_version = match app_version {
            None | Some("") | Some("0.0.0") => return Ok(Some(manifest.version)),
            Some(v) => v,
        };

        // Pas de versions.json (statut ≠ 200, transport ou JSON en échec) : on retombe
        // sur la version du manifest.
        if let Ok(res) = self.get(&Self::raw_de(entry, "versions.json"))
            && res.status == 200
            && let Ok(table) = serde_json::from_str::<HashMap<String, String>>(&res.text)
        {
            // version → minAppVersion
            let mut best: Option<&String> = None;
            for (version, min_app) in &table {
                if compare_versions(min_app, app_version) == Ordering::Greater {
                    continue; // exige un client plus récent
                }
                if best.is_none_or(|b| compare_versions(version, b) == Ordering::Greater) {
                    best = Some(version);
                }
            }
            if let Some(b) = best {
                return Ok(Some(b.clone()));
            }
        }
        // Le manifest seul : compatible s'il n'exige rien de plus récent que nous.
        let exige = manifest
            .min_app_version
            .as_deref()
            .filter(|m| !m.is_empty());
        match exige {
            Some(min) if compare_versions(min, app_version) == Ordering::Greater => Ok(None),
            _ => Ok(Some(manifest.version)),
        }
    }

    fn download_assets(
        &self,
        entry: &CatalogEntry,
        version: &str,
    ) -> Result<Assets, RegistryError> {
        let base = format!(
            "https://github.com/{}/releases/download/{version}",
            entry.repo
        );
        let main = self.get(&format!("{base}/main.js"))?;
        let manifest = self.get(&format!("{base}/manifest.json"))?;
        if main.status != 200 || manifest.status != 200 {
            return Err(RegistryError::AssetsManquants {
                source_id: self.sid(),
                id: entry.id.clone(),
                version: version.to_string(),
            });
        }
        let styles = self.get(&format!("{base}/styles.css"))?;
        Ok(Assets {
            main: main.text,
            manifest: manifest.text,
            styles: (styles.status == 200).then_some(styles.text),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn catalogue() -> Vec<CatalogEntry> {
        vec![CatalogEntry {
            id: "demo".into(),
            name: "Demo".into(),
            author: "Alice".into(),
            description: "Un plugin.".into(),
            repo: "alice/demo".into(),
            branch: None,
            downloads: None,
            updated: None,
        }]
    }

    /// Un faux dépôt : le corps dépend de l'URL, les appels sont journalisés.
    struct FauxDepot {
        calls: Rc<RefCell<Vec<String>>>,
        min_app: Rc<RefCell<String>>,
        /// Vide = versions.json « absent » (JSON invalide → repli sur le manifest).
        versions: Rc<RefCell<String>>,
    }

    impl Reseau for FauxDepot {
        fn requete(&self, url: &str) -> Result<Reponse, String> {
            self.calls.borrow_mut().push(url.to_string());
            let corps = if url.ends_with("/community-plugins.json")
                && url.contains("/registry-repo/")
            {
                serde_json::to_string(&catalogue()).unwrap_or_default()
            } else if url.ends_with("/manifest.json") {
                format!(
                    r#"{{"id":"demo","name":"Demo","version":"1.2.3","minAppVersion":"{}","description":"Un plugin.","author":"Alice"}}"#,
                    self.min_app.borrow()
                )
            } else if url.ends_with("/versions.json") {
                self.versions.borrow().clone()
            } else if url.ends_with("/README.md") {
                "# Demo".into()
            } else if url.ends_with("/main.js") {
                "module.exports = {};".into()
            } else if url.ends_with("/styles.css") {
                ".demo {}".into()
            } else {
                String::new()
            };
            Ok(Reponse {
                status: 200,
                text: corps,
            })
        }
    }

    struct Banc {
        source: GithubRegistrySource<FauxDepot>,
        calls: Rc<RefCell<Vec<String>>>,
        min_app: Rc<RefCell<String>>,
        versions: Rc<RefCell<String>>,
    }

    fn setup() -> Banc {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let min_app = Rc::new(RefCell::new("0.0.0".to_string()));
        let versions = Rc::new(RefCell::new(String::new()));
        let source = GithubRegistrySource::new(
            SourceId::Fragment,
            "Fragment",
            "registry-repo/plugins",
            None,
            FauxDepot {
                calls: calls.clone(),
                min_app: min_app.clone(),
                versions: versions.clone(),
            },
        );
        Banc {
            source,
            calls,
            min_app,
            versions,
        }
    }

    fn a_appele(b: &Banc, url: &str) -> bool {
        b.calls.borrow().iter().any(|u| u == url)
    }

    #[test]
    fn list_lit_community_plugins_json_du_depot_de_registre_raw_githubusercontent() {
        let b = setup();
        let entries = b.source.list().expect("catalogue");
        assert_eq!(entries, catalogue());
        assert_eq!(
            b.calls.borrow()[0],
            "https://raw.githubusercontent.com/registry-repo/plugins/HEAD/community-plugins.json"
        );
    }

    #[test]
    fn resolve_version_rend_la_version_du_manifest_du_depot_auteur() {
        let b = setup();
        let v = b.source.resolve_version(&catalogue()[0], None).expect("v");
        assert_eq!(v.as_deref(), Some("1.2.3"));
        assert!(a_appele(
            &b,
            "https://raw.githubusercontent.com/alice/demo/HEAD/manifest.json"
        ));
    }

    #[test]
    fn resolve_version_versions_json_absent_repli_sur_la_version_du_manifest() {
        let b = setup();
        // versions.json vide : JSON invalide → repli sur manifest.version.
        let v = b
            .source
            .resolve_version(&catalogue()[0], Some("1.0.0"))
            .expect("v");
        assert_eq!(v.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn resolve_version_versions_json_present_plus_haute_version_compatible_avec_app_version() {
        let b = setup();
        // 1.2.3 exige un client 1.5.0 (trop récent) ; 1.1.0 exige 1.0.0 (ok) ;
        // 1.0.0 exige 0.9.0 (ok). Avec appVersion 1.0.0, la meilleure est 1.1.0.
        *b.versions.borrow_mut() = r#"{"1.0.0":"0.9.0","1.1.0":"1.0.0","1.2.3":"1.5.0"}"#.into();
        let v = b
            .source
            .resolve_version(&catalogue()[0], Some("1.0.0"))
            .expect("v");
        assert_eq!(v.as_deref(), Some("1.1.0"));
        assert!(a_appele(
            &b,
            "https://raw.githubusercontent.com/alice/demo/HEAD/versions.json"
        ));
    }

    #[test]
    fn resolve_version_rien_de_compatible_versions_json_null_pas_de_repli_silencieux() {
        let b = setup();
        *b.min_app.borrow_mut() = "2.0.0".into();
        *b.versions.borrow_mut() = r#"{"1.0.0":"1.5.0","1.2.3":"2.0.0"}"#.into();
        assert_eq!(
            b.source
                .resolve_version(&catalogue()[0], Some("1.0.0"))
                .expect("v"),
            None
        );
    }

    #[test]
    fn resolve_version_sans_versions_json_un_manifest_qui_exige_un_client_plus_recent_null() {
        let b = setup();
        *b.min_app.borrow_mut() = "1.5.0".into();
        assert_eq!(
            b.source
                .resolve_version(&catalogue()[0], Some("1.0.0"))
                .expect("v"),
            None
        );
        assert_eq!(
            b.source
                .resolve_version(&catalogue()[0], Some("1.5.0"))
                .expect("v")
                .as_deref(),
            Some("1.2.3")
        );
    }

    #[test]
    fn resolve_version_build_de_dev_la_derniere_version_quoi_qu_elle_exige() {
        let b = setup();
        *b.min_app.borrow_mut() = "9.0.0".into();
        *b.versions.borrow_mut() = r#"{"1.2.3":"9.0.0"}"#.into();
        assert_eq!(
            b.source
                .resolve_version(&catalogue()[0], Some("0.0.0"))
                .expect("v")
                .as_deref(),
            Some("1.2.3")
        );
    }

    #[test]
    fn download_assets_tape_releases_download_version_pas_api_github_com() {
        let b = setup();
        let assets = b
            .source
            .download_assets(&catalogue()[0], "1.2.3")
            .expect("assets");
        assert_eq!(assets.main, "module.exports = {};");
        assert_eq!(assets.styles.as_deref(), Some(".demo {}"));
        assert!(a_appele(
            &b,
            "https://github.com/alice/demo/releases/download/1.2.3/main.js"
        ));
        assert!(a_appele(
            &b,
            "https://github.com/alice/demo/releases/download/1.2.3/manifest.json"
        ));
        assert!(
            !b.calls
                .borrow()
                .iter()
                .any(|u| u.contains("api.github.com"))
        );
    }

    // ── ajouts du port : erreurs et branches non couvertes par le TS ──
    struct Statut(u16);
    impl Reseau for Statut {
        fn requete(&self, _: &str) -> Result<Reponse, String> {
            Ok(Reponse {
                status: self.0,
                text: String::new(),
            })
        }
    }

    #[test]
    fn un_catalogue_indisponible_leve_le_message_du_ts() {
        let s = GithubRegistrySource::new(SourceId::Obsidian, "Obsidian", "r/p", None, Statut(404));
        let e = s.list().expect_err("404");
        assert_eq!(
            e.to_string(),
            "[registry:obsidian] catalogue indisponible (404)"
        );
        let e = s.fetch_manifest(&catalogue()[0]).expect_err("404");
        assert_eq!(
            e.to_string(),
            "[registry:obsidian] manifest introuvable pour demo"
        );
        let e = s
            .download_assets(&catalogue()[0], "1.0.0")
            .expect_err("404");
        assert!(e.to_string().contains("assets manquants pour demo@1.0.0"));
        assert_eq!(s.fetch_readme(&catalogue()[0]).expect("readme"), "");
    }

    #[test]
    fn les_stats_enrichissent_le_catalogue_et_leur_echec_ne_le_prive_pas() {
        struct AvecStats(bool);
        impl Reseau for AvecStats {
            fn requete(&self, url: &str) -> Result<Reponse, String> {
                if url.ends_with("/community-plugins.json") {
                    return Ok(Reponse {
                        status: 200,
                        text: serde_json::to_string(&catalogue()).unwrap_or_default(),
                    });
                }
                if self.0 {
                    Ok(Reponse {
                        status: 200,
                        text: r#"{"demo":{"downloads":42,"updated":7}}"#.into(),
                    })
                } else {
                    Err("hors ligne".into())
                }
            }
        }
        let ok = GithubRegistrySource::new(SourceId::Fragment, "F", "r/p", None, AvecStats(true));
        let e = ok.list().expect("liste");
        assert_eq!((e[0].downloads, e[0].updated), (Some(42.0), Some(7.0)));
        let ko = GithubRegistrySource::new(SourceId::Fragment, "F", "r/p", None, AvecStats(false));
        assert_eq!(ko.list().expect("liste"), catalogue());
    }
}
