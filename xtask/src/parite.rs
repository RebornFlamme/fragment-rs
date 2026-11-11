//! `cargo xtask parite` : table de parité des tests (program.md §6 et §7).
//!
//! Lit `crates/*/parite.toml` et `crates/*/parite.d/*.toml` (modules fusionnés par crate), écrit `docs/parite-tests.md`. Trois formes de fichier
//! coexistent dans le dépôt, et ce module les accepte toutes :
//!
//! - **Schéma A** : une table par module (`[nom]`) avec `ts`, `tests_ts`, `source` (entier),
//!   `portes` et `non_portes = [{ cas, raison }]`.
//! - **Schémas B, C, D** : des `[[module]]` avec `ts` ou `source` (chemin), `cas_source`,
//!   `cas_portes`, `cas_ajoutes`, `notes` ou `note`, et des `[[module.non_portes]]` ou
//!   `[[module.non_porte]]` (`quoi` ou `cas`, `raison` ou `pourquoi`).
//! - Métadonnées ignorées : une table `[crate]`, une clé `crate = "…"`, une table `[total]`.
//!   Le total est recalculé à partir des modules.
//!
//! Porte de phase 1 : au moins 90 % des cas source portés. Un module non porté sans
//! raison est signalé. Code de sortie ≠ 0 si un fichier est illisible ou si la porte
//! n'est pas atteinte.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, bail};

/// Seuil de la porte de phase 1, en pourcents des cas source.
const PORTE_PHASE_1: usize = 90;
const DOSSIER_CRATES: &str = "crates";
const FICHIER_PARITE: &str = "parite.toml";
/// Un fichier par tâche : deux tâches d'une même crate n'écrivent plus le même fichier.
const DOSSIER_PARITE_D: &str = "parite.d";
const SORTIE: &str = "docs/parite-tests.md";

/// Un cas du TS non porté, avec la raison écrite dans le `parite.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonPorte {
    pub cas: String,
    pub raison: String,
}

/// Un module porté : un fichier TS et ses cas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub nom: String,
    pub fichier_ts: String,
    pub cas_source: usize,
    pub portes: usize,
    pub ajoutes: usize,
    pub non_portes: Vec<NonPorte>,
}

impl Module {
    /// Nombre de cas source non portés (écart entre la source et les portés).
    #[must_use]
    pub fn non_portes_nombre(&self) -> usize {
        self.cas_source - self.portes
    }

    /// Vrai si des cas manquent et qu'aucune raison n'est écrite.
    #[must_use]
    pub fn sans_raison(&self) -> bool {
        self.non_portes_nombre() > 0 && self.non_portes.iter().all(|np| np.raison.trim().is_empty())
    }
}

/// Compteurs agrégés (un module, un crate ou le total).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Compte {
    pub source: usize,
    pub portes: usize,
    pub ajoutes: usize,
    pub non_portes: usize,
}

impl Compte {
    /// Ajoute les compteurs d'un module.
    pub fn ajouter(&mut self, module: &Module) {
        self.source += module.cas_source;
        self.portes += module.portes;
        self.ajoutes += module.ajoutes;
        self.non_portes += module.non_portes_nombre();
    }

    /// Vrai si au moins `PORTE_PHASE_1` % des cas source sont portés (comparaison entière).
    #[must_use]
    pub fn porte_atteinte(&self) -> bool {
        self.portes * 100 >= self.source * PORTE_PHASE_1
    }

    /// Taux de cas portés en pourcents (100 si la source est vide).
    #[must_use]
    pub fn taux(&self) -> f64 {
        if self.source == 0 {
            100.0
        } else {
            self.portes as f64 * 100.0 / self.source as f64
        }
    }
}

/// Un crate : ses modules, ou l'erreur qui a empêché de lire son `parite.toml`.
#[derive(Debug)]
pub struct CrateParite {
    pub nom: String,
    pub modules: Vec<Module>,
    pub erreur: Option<String>,
}

/// Bilan de la rédaction : total, signalements et fichiers illisibles.
#[derive(Debug, Default)]
pub struct Bilan {
    pub total: Compte,
    pub sans_raison: Vec<String>,
    pub erreurs: Vec<String>,
}

/// Lit un `parite.toml` (les trois schémas) et rend ses modules.
///
/// # Errors
///
/// Rend une erreur si le TOML est invalide ou si un module n'a pas de fichier TS,
/// de nombre de cas source ou de nombre de cas portés, ou si `portes > source`.
pub fn analyser(texte: &str) -> anyhow::Result<Vec<Module>> {
    let racine: toml::Table = texte.parse().context("TOML invalide")?;
    let mut modules = Vec::new();
    for (cle, valeur) in &racine {
        match (cle.as_str(), valeur) {
            ("crate" | "total", _) => {}
            ("module", toml::Value::Array(entrees)) => {
                for (rang, entree) in entrees.iter().enumerate() {
                    let t = entree
                        .as_table()
                        .with_context(|| format!("[[module]] n°{} : table attendue", rang + 1))?;
                    let module =
                        module(None, t).with_context(|| format!("[[module]] n°{}", rang + 1))?;
                    modules.push(module);
                }
            }
            (nom, toml::Value::Table(t)) => {
                let module = module(Some(nom), t).with_context(|| format!("module « {nom} »"))?;
                modules.push(module);
            }
            _ => {}
        }
    }
    Ok(modules)
}

/// Construit un module à partir de sa table (schéma A : nom = clé ; B-D : nom = `rs`).
fn module(nom: Option<&str>, t: &toml::Table) -> anyhow::Result<Module> {
    let fichier_ts = texte(t, &["ts", "source"]).context("fichier TS absent (`ts` ou `source`)")?;
    let cas_source = entier(t, &["cas_source", "source"])
        .context("nombre de cas source absent (`source` ou `cas_source`)")?;
    let portes = entier(t, &["cas_portes", "portes"])
        .context("nombre de cas portés absent (`portes` ou `cas_portes`)")?;
    if portes > cas_source {
        bail!("{fichier_ts} : {portes} cas portés pour {cas_source} cas source");
    }
    let nom = nom
        .map(str::to_owned)
        .or_else(|| texte(t, &["rs", "rust"]))
        .unwrap_or_else(|| fichier_ts.clone());
    Ok(Module {
        nom,
        fichier_ts,
        cas_source,
        portes,
        ajoutes: entier(t, &["cas_ajoutes", "ajoutes"]).unwrap_or(0),
        non_portes: non_portes(t)?,
    })
}

/// Première chaîne présente parmi `cles`.
fn texte(t: &toml::Table, cles: &[&str]) -> Option<String> {
    cles.iter()
        .find_map(|c| t.get(*c)?.as_str())
        .map(str::to_owned)
}

/// Premier entier présent parmi `cles` (une chaîne sous `source` n'est pas un compte).
fn entier(t: &toml::Table, cles: &[&str]) -> Option<usize> {
    cles.iter()
        .find_map(|c| t.get(*c)?.as_integer())
        .and_then(|n| usize::try_from(n).ok())
}

/// Lit `non_portes` (schéma A, B) ou `non_porte` (schéma C, D).
fn non_portes(t: &toml::Table) -> anyhow::Result<Vec<NonPorte>> {
    let mut sortie = Vec::new();
    for cle in ["non_portes", "non_porte"] {
        let Some(valeur) = t.get(cle) else {
            continue;
        };
        let entrees = valeur
            .as_array()
            .with_context(|| format!("`{cle}` : tableau attendu"))?;
        for entree in entrees {
            let et = entree
                .as_table()
                .with_context(|| format!("`{cle}` : entrée en table attendue"))?;
            sortie.push(NonPorte {
                cas: texte(et, &["cas", "quoi"]).unwrap_or_else(|| "(sans intitulé)".to_owned()),
                raison: texte(et, &["raison", "pourquoi"]).unwrap_or_default(),
            });
        }
    }
    Ok(sortie)
}

/// Les `*.toml` de `parite.d/`, triés par nom (vide si le dossier n'existe pas).
fn fichiers_parite_d(dossier: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut fichiers: Vec<_> = entrees
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "toml"))
        .collect();
    fichiers.sort();
    fichiers
}

/// Lit `crates/*/parite.toml` et `crates/*/parite.d/*.toml` (modules fusionnés par crate). Rend les crates qui en ont un, et les noms des autres.
///
/// # Errors
///
/// Rend une erreur si le dossier des crates est illisible. Un `parite.toml` illisible
/// n'interrompt pas la lecture : il est noté dans `CrateParite::erreur`.
pub fn lire_crates(dossier: &Path) -> anyhow::Result<(Vec<CrateParite>, Vec<String>)> {
    let mut noms: Vec<String> = std::fs::read_dir(dossier)
        .with_context(|| format!("dossier {} illisible", dossier.display()))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    noms.sort();

    let mut avec = Vec::new();
    let mut sans = Vec::new();
    for nom in noms {
        let racine = dossier.join(&nom);
        let mut fichiers = Vec::new();
        let principal = racine.join(FICHIER_PARITE);
        if principal.is_file() {
            fichiers.push(principal);
        }
        fichiers.extend(fichiers_parite_d(&racine.join(DOSSIER_PARITE_D)));
        if fichiers.is_empty() {
            sans.push(nom);
            continue;
        }
        // Les modules de tous les fichiers sont fusionnés dans la table de la crate ; la première
        // erreur rend la crate illisible.
        let mut modules = Vec::new();
        let mut erreur = None;
        for fichier in &fichiers {
            let resultat = std::fs::read_to_string(fichier)
                .with_context(|| format!("lecture de {}", fichier.display()))
                .and_then(|texte| analyser(&texte));
            match resultat {
                Ok(mut lus) => modules.append(&mut lus),
                Err(e) => {
                    erreur = Some(format!("{nom} : {e:#}"));
                    modules.clear();
                    break;
                }
            }
        }
        avec.push(CrateParite {
            nom,
            modules,
            erreur,
        });
    }
    Ok((avec, sans))
}

/// Échappe une cellule de tableau Markdown.
fn echappe(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// Cellule « Non portés (raison) » d'un module.
fn cellule_non_portes(module: &Module) -> String {
    let n = module.non_portes_nombre();
    if n == 0 {
        return "—".to_owned();
    }
    if module.sans_raison() {
        return format!("**{n} non porté(s) sans raison**");
    }
    let raisons: Vec<String> = module
        .non_portes
        .iter()
        .map(|np| format!("{} : {}", np.cas, np.raison))
        .collect();
    echappe(&format!("{n} — {}", raisons.join(" ; ")))
}

/// Rédige `docs/parite-tests.md` ; rend le texte et le bilan.
///
/// # Errors
///
/// Rend une erreur si l'écriture dans la chaîne échoue (ne devrait pas arriver).
pub fn rediger(crates: &[CrateParite], sans_parite: &[String]) -> anyhow::Result<(String, Bilan)> {
    let mut bilan = Bilan::default();
    for c in crates {
        if let Some(erreur) = &c.erreur {
            bilan.erreurs.push(erreur.clone());
        }
        for m in &c.modules {
            bilan.total.ajouter(m);
            if m.sans_raison() {
                bilan.sans_raison.push(format!("{} / {}", c.nom, m.nom));
            }
        }
    }

    let total = bilan.total;
    let mut s = String::new();
    writeln!(s, "# Parité des tests\n")?;
    writeln!(
        s,
        "Généré par `cargo xtask parite` à partir des `crates/*/parite.toml`. Ne pas éditer à la main : modifier le `parite.toml` du crate, puis relancer la commande.\n"
    )?;
    writeln!(
        s,
        "Règle de phase 1 : au moins {PORTE_PHASE_1} % des cas source portés ; chaque cas non porté porte sa raison. « Cas source » = cas du TS d'origine ; « Portés » = `#[test]` de même nom en snake_case ; « Ajoutés » = tests Rust sans équivalent TS (hors du taux).\n"
    )?;

    writeln!(s, "## Total\n")?;
    writeln!(s, "| Cas source | Portés | Ajoutés | Non portés | Taux |")?;
    writeln!(s, "|---|---|---|---|---|")?;
    writeln!(
        s,
        "| {} | {} | {} | {} | {:.1} % |\n",
        total.source,
        total.portes,
        total.ajoutes,
        total.non_portes,
        total.taux()
    )?;
    if total.porte_atteinte() {
        writeln!(
            s,
            "**PORTE ATTEINTE** : {:.1} % des cas source portés.\n",
            total.taux()
        )?;
    } else {
        writeln!(
            s,
            "**porte non atteinte** : {:.1} % des cas source portés, seuil {PORTE_PHASE_1} %.\n",
            total.taux()
        )?;
    }
    let signale = |liste: &[String]| {
        if liste.is_empty() {
            "aucun".to_owned()
        } else {
            liste.join(", ")
        }
    };
    writeln!(
        s,
        "- Modules non portés sans raison : {}",
        signale(&bilan.sans_raison)
    )?;
    writeln!(s, "- Fichiers illisibles : {}\n", signale(&bilan.erreurs))?;

    writeln!(s, "## Crates sans modules portés\n")?;
    writeln!(
        s,
        "{} (pas de `parite.toml`).\n",
        if sans_parite.is_empty() {
            "aucun".to_owned()
        } else {
            sans_parite.join(", ")
        }
    )?;

    for c in crates {
        writeln!(s, "## {}\n", c.nom)?;
        if let Some(erreur) = &c.erreur {
            writeln!(s, "**Fichier illisible** : {}\n", echappe(erreur))?;
            continue;
        }
        writeln!(
            s,
            "| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |"
        )?;
        writeln!(s, "|---|---|---|---|---|---|")?;
        let mut synthese = Compte::default();
        for m in &c.modules {
            synthese.ajouter(m);
            writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} |",
                echappe(&m.nom),
                echappe(&m.fichier_ts),
                m.cas_source,
                m.portes,
                m.ajoutes,
                cellule_non_portes(m)
            )?;
        }
        writeln!(s)?;
        writeln!(
            s,
            "| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |"
        )?;
        writeln!(s, "|---|---|---|---|---|---|")?;
        writeln!(
            s,
            "| {} | {} | {} | {} | {} | {:.1} % |\n",
            c.nom,
            synthese.source,
            synthese.portes,
            synthese.ajoutes,
            synthese.non_portes,
            synthese.taux()
        )?;
    }
    Ok((s, bilan))
}

/// Point d'entrée de `cargo xtask parite`. Rend `Ok(true)` si la porte est atteinte
/// et qu'aucun fichier n'est illisible.
pub fn run(args: &[String]) -> anyhow::Result<bool> {
    if let Some(arg) = args.first() {
        bail!("option inconnue : {arg}");
    }
    let (crates, sans_parite) = lire_crates(Path::new(DOSSIER_CRATES))?;
    let (rapport, bilan) = rediger(&crates, &sans_parite)?;
    let sortie = Path::new(SORTIE);
    if let Some(parent) = sortie.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(sortie, &rapport).with_context(|| format!("écriture de {SORTIE}"))?;

    let total = bilan.total;
    println!(
        "parite : {}/{} cas source portés ({:.1} %), {SORTIE} écrit",
        total.portes,
        total.source,
        total.taux()
    );
    for module in &bilan.sans_raison {
        eprintln!("xtask parite : module non porté sans raison : {module}");
    }
    for erreur in &bilan.erreurs {
        eprintln!("xtask parite : fichier illisible : {erreur}");
    }
    let atteinte = total.porte_atteinte();
    if !atteinte {
        eprintln!(
            "xtask parite : porte non atteinte ({:.1} % < {PORTE_PHASE_1} %)",
            total.taux()
        );
    }
    Ok(atteinte && bilan.erreurs.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schéma A : une table par module.
    const SCHEMA_A: &str = r#"
[events]
ts = "events.ts"
tests_ts = "events.test.ts"
source = 13
portes = 11
non_portes = [
  { cas = "Events.off(name, cb)", raison = "identité de fonction" },
  { cas = "poignée DOM", raison = "DOM" },
]

[contrat]
ts = "contract.ts + types.ts + api.ts"
tests_ts = ""
source = 0
portes = 0
non_portes = []
"#;

    /// Schéma B : `[crate]`, `[[module]]` avec `notes` et `[[module.non_portes]]` (quoi/raison).
    const SCHEMA_B: &str = r#"
[crate]
name = "fragment-core"
tache = "fr-p1-core-a"

[[module]]
ts = "app/src/ui/core/lru.ts"
rs = "crates/fragment-core/src/lru.rs"
cas_source = 3
cas_portes = 3
cas_ajoutes = 0
notes = "HashMap + compteur d'usage"

[[module]]
ts = "app/src/ui/core/events.ts"
rs = "crates/fragment-core/src/events.rs"
cas_source = 13
cas_portes = 8
cas_ajoutes = 4
notes = "Poignée partagée"

[[module.non_portes]]
quoi = "Events.off(name, cb)"
raison = "pas d'identité de fermeture"
"#;

    /// Schéma C : `[[module.non_porte]]` (cas/raison), `[total]` à ignorer.
    const SCHEMA_C: &str = r#"
[crate]
name = "fragment-artefacts"
reference = "app/src/ui/core/artefacts/"

[[module]]
ts = "ui/core/artefacts/anchor.ts"
test_ts = "ui/core/artefacts/anchor.test.ts"
rs = "src/anchor/mod.rs"
cas_source = 27
cas_portes = 27
note = "Myers"

[[module]]
ts = "ui/core/artefacts/ArtefactManager.ts"
rs = "src/artefact_manager.rs"
cas_source = 31
cas_portes = 30
note = "Synchrone"

[[module.non_porte]]
cas = "un événement sans fournisseur est ignoré"
raison = "teste DocumentManager.notify"

[total]
cas_source = 58
"#;

    /// Schéma D : clé `crate` en chaîne, `source` et `rust` en chaînes, `[[module.non_porte]]`.
    const SCHEMA_D: &str = r#"
crate = "fragment-text"
tache = "fr-p1-text"

[[module]]
source = "ui/core/editor/findPassage.ts"
tests_source = "ui/core/editor/findPassage.test.ts"
rust = "src/find_passage.rs"
tests_rust = "tests/find_passage.rs"
cas_source = 4
cas_portes = 4
cas_ajoutes = 1
note = "UTF-16"

[[module]]
source = "shared/htmlBridge.ts (partie pure)"
tests_source = "shared/htmlBridge.test.ts"
rust = "src/html_bridge.rs"
cas_source = 7
cas_portes = 5
cas_ajoutes = 6
note = "inject_html_bridge"

[[module.non_porte]]
cas = "le script est du JS valide"
raison = "JavaScript exécuté par la page"

[[module.non_porte]]
cas = "échappements des regex"
raison = "propriété de la chaîne JS"
"#;

    fn trouve<'a>(modules: &'a [Module], nom: &str) -> &'a Module {
        modules.iter().find(|m| m.nom == nom).unwrap()
    }

    #[test]
    fn lit_le_schema_a_par_table() {
        let modules = analyser(SCHEMA_A).unwrap();
        assert_eq!(modules.len(), 2);
        let events = trouve(&modules, "events");
        assert_eq!(events.fichier_ts, "events.ts");
        assert_eq!(events.cas_source, 13);
        assert_eq!(events.portes, 11);
        assert_eq!(events.non_portes.len(), 2);
        assert!(!events.sans_raison());
    }

    #[test]
    fn lit_le_schema_b_avec_non_portes_quoi_raison() {
        let modules = analyser(SCHEMA_B).unwrap();
        assert_eq!(modules.len(), 2);
        let lru = trouve(&modules, "crates/fragment-core/src/lru.rs");
        assert_eq!(lru.fichier_ts, "app/src/ui/core/lru.ts");
        assert_eq!(lru.non_portes_nombre(), 0);
        let evenements = trouve(&modules, "crates/fragment-core/src/events.rs");
        assert_eq!(evenements.ajoutes, 4);
        assert_eq!(evenements.non_portes[0].cas, "Events.off(name, cb)");
        assert_eq!(
            evenements.non_portes[0].raison,
            "pas d'identité de fermeture"
        );
    }

    #[test]
    fn lit_le_schema_c_et_ignore_le_total() {
        let modules = analyser(SCHEMA_C).unwrap();
        assert_eq!(modules.len(), 2, "[total] ne doit pas devenir un module");
        let manager = trouve(&modules, "src/artefact_manager.rs");
        assert_eq!(manager.non_portes_nombre(), 1);
        assert_eq!(manager.non_portes.len(), 1);
        assert!(!manager.sans_raison());
    }

    #[test]
    fn lit_le_schema_d_avec_source_en_chaine() {
        let modules = analyser(SCHEMA_D).unwrap();
        assert_eq!(modules.len(), 2);
        let bridge = trouve(&modules, "src/html_bridge.rs");
        assert_eq!(bridge.fichier_ts, "shared/htmlBridge.ts (partie pure)");
        assert_eq!(bridge.cas_source, 7);
        assert_eq!(bridge.portes, 5);
        assert_eq!(bridge.ajoutes, 6);
        assert_eq!(bridge.non_portes.len(), 2);
    }

    #[test]
    fn refuse_un_module_sans_nombre_de_cas() {
        let texte = "[x]\nts = \"a.ts\"\nportes = 1\n";
        assert!(analyser(texte).is_err());
    }

    #[test]
    fn refuse_plus_de_portes_que_de_cas_source() {
        let texte = "[x]\nts = \"a.ts\"\nsource = 2\nportes = 3\nnon_portes = []\n";
        assert!(analyser(texte).is_err());
    }

    #[test]
    fn refuse_le_toml_invalide() {
        assert!(analyser("[x\nts = ").is_err());
    }

    #[test]
    fn signale_un_module_non_porte_sans_raison() {
        let texte = "[x]\nts = \"a.ts\"\nsource = 5\nportes = 3\nnon_portes = []\n";
        let modules = analyser(texte).unwrap();
        assert!(modules[0].sans_raison());
    }

    #[test]
    fn porte_atteinte_a_partir_de_90_pour_cent_exact() {
        let juste = Compte {
            source: 10,
            portes: 9,
            ajoutes: 0,
            non_portes: 1,
        };
        assert!(juste.porte_atteinte());
        let trop_bas = Compte {
            source: 100,
            portes: 89,
            ajoutes: 0,
            non_portes: 11,
        };
        assert!(!trop_bas.porte_atteinte());
        assert!((trop_bas.taux() - 89.0).abs() < 1e-9);
    }

    #[test]
    fn source_vide_compte_comme_atteinte() {
        let vide = Compte::default();
        assert!(vide.porte_atteinte());
        assert!((vide.taux() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn echappe_les_barres_verticales() {
        assert_eq!(echappe("a|b\nc"), "a\\|b c");
    }

    #[test]
    fn rediger_signale_et_compte_les_erreurs() {
        let crates = vec![
            CrateParite {
                nom: "fragment-agent".to_owned(),
                modules: analyser(SCHEMA_A).unwrap(),
                erreur: None,
            },
            CrateParite {
                nom: "fragment-cassé".to_owned(),
                modules: Vec::new(),
                erreur: Some("fragment-cassé : TOML invalide".to_owned()),
            },
        ];
        let (rapport, bilan) = rediger(&crates, &["fragment-ui".to_owned()]).unwrap();
        assert_eq!(bilan.erreurs.len(), 1);
        assert_eq!(bilan.total.source, 13);
        assert_eq!(bilan.total.portes, 11);
        // 11 portés sur 13 = 84,6 % : sous le seuil, et le fichier illisible est signalé.
        assert!(rapport.contains("**porte non atteinte**"));
        assert!(!bilan.total.porte_atteinte());
        assert!(rapport.contains("**Fichier illisible**"));
        assert!(rapport.contains("fragment-ui"));
    }

    #[test]
    fn rediger_ecrit_porte_non_atteinte_sous_le_seuil() {
        let crates = vec![CrateParite {
            nom: "x".to_owned(),
            modules: analyser("[m]\nts = \"m.ts\"\nsource = 10\nportes = 5\nnon_portes = [{ cas = \"c\", raison = \"r\" }]\n").unwrap(),
            erreur: None,
        }];
        let (rapport, bilan) = rediger(&crates, &[]).unwrap();
        assert!(!bilan.total.porte_atteinte());
        assert!(rapport.contains("porte non atteinte"));
    }

    #[test]
    fn fusionne_parite_d_dans_la_table_de_la_crate() {
        let racine = std::env::temp_dir().join(format!("xtask-parite-d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&racine);
        let crate_dir = racine.join("fragment-x");
        std::fs::create_dir_all(crate_dir.join("parite.d")).unwrap();
        std::fs::create_dir_all(racine.join("fragment-vide")).unwrap();
        std::fs::write(
            crate_dir.join("parite.toml"),
            "[a]
ts = \"a.ts\"
source = 2
portes = 2
non_portes = []
",
        )
        .unwrap();
        std::fs::write(
            crate_dir.join("parite.d/fr-t-un.toml"),
            "[b]
ts = \"b.ts\"
source = 3
portes = 3
non_portes = []
",
        )
        .unwrap();
        std::fs::write(
            crate_dir.join("parite.d/fr-t-deux.toml"),
            "[[module]]
ts = \"c.ts\"
rs = \"c\"
cas_source = 1
cas_portes = 1
",
        )
        .unwrap();
        let (crates, sans) = lire_crates(&racine).unwrap();
        let _ = std::fs::remove_dir_all(&racine);
        assert_eq!(sans, vec!["fragment-vide".to_owned()]);
        assert_eq!(crates.len(), 1);
        assert!(crates[0].erreur.is_none());
        let mut noms: Vec<_> = crates[0].modules.iter().map(|m| m.nom.as_str()).collect();
        noms.sort_unstable();
        assert_eq!(noms, ["a", "b", "c"]);
    }
}
