// Port de app/src/shared/cli/instanceIndex.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'INDEX DES INSTANCES — « qui tourne, et comment lui parler ? »
//
//  `<userData>/cli-instances/<id>.json`, un fichier par instance. Écrit quand
//  le serveur écoute, retiré à la sortie. Le client froid liste ce dossier.
//
//  ★ POURQUOI ici, et pas dans le nom du tuyau : le fichier porte le JETON. Le
//    dossier userData est ACLé au profil de l'utilisateur ; un tuyau nommé,
//    lui, est ouvert à tout processus de la session. Le secret est donc là où
//    l'OS le protège, et le tuyau ne contient qu'une empreinte publique.
//
//  ★ POURQUOI un fichier PAR instance : deux coffres ouverts dans deux
//    fenêtres écrivent au même moment. Un fichier unique en lecture-
//    modification-écriture perdrait l'un des deux ; deux fichiers ne se voient
//    jamais.
//
//  Un fichier ORPHELIN (crash, app tuée par un e2e) est possible :
//  `instances_vivantes` SONDE le tuyau de chacun et purge ceux où personne
//  n'écoute — appelée par le client à chaque commande ET par le serveur à son
//  démarrage.
//
//  ★ Vivant = le TUYAU répond, pas le PID : sous Windows les PID sont recyclés
//    très vite (un orphelin « vivait » par le pid d'un autre processus) et
//    « accès refusé » ne prouve rien. 24 orphelins d'e2e rendaient tout
//    `fragment …` ambigu (« 25 instances tournent »).
//
//  PORT : la sonde du tuyau et le test de pid sont de l'E/S réelle (sockets,
//  processus) — hors du modèle pur. Elles sont injectées par le trait
//  [`SondeInstances`] ; l'implémentation réelle viendra avec le transport.
// ═══════════════════════════════════════════════════════════════════════════

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::nom_du_dossier;
use crate::pipe::instance_id_for;
use crate::protocol::{CLI_PROTOCOL_VERSION, CliInstanceRecord};

pub const DOSSIER_INSTANCES: &str = "cli-instances";

/// Une erreur d'E/S sur l'index, avec le chemin fautif.
#[derive(Debug, thiserror::Error)]
pub enum ErreurIndex {
    #[error("index des instances : {chemin} : {source}")]
    Io {
        chemin: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("index des instances : JSON illisible à écrire : {0}")]
    Json(#[from] serde_json::Error),
}

fn io_err(chemin: &Path) -> impl FnOnce(io::Error) -> ErreurIndex + '_ {
    move |source| ErreurIndex::Io {
        chemin: chemin.to_path_buf(),
        source,
    }
}

pub fn chemin_index(user_data: &Path, vault_path: &str) -> PathBuf {
    user_data
        .join(DOSSIER_INSTANCES)
        .join(format!("{}.json", instance_id_for(vault_path)))
}

/// Ce que l'appelant fournit ; le reste (`v`, `vaultName`, `pid`, `startedAt`) est rempli ici.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceAPublier {
    pub vault_path: String,
    pub pipe: String,
    pub token: String,
    pub version: String,
}

fn maintenant_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Écrit l'index de l'instance et rend son chemin.
pub fn publier_instance(
    user_data: &Path,
    instance: &InstanceAPublier,
) -> Result<PathBuf, ErreurIndex> {
    let complet = CliInstanceRecord {
        v: CLI_PROTOCOL_VERSION,
        vault_name: nom_du_dossier(&instance.vault_path),
        vault_path: instance.vault_path.clone(),
        pipe: instance.pipe.clone(),
        token: instance.token.clone(),
        pid: std::process::id(),
        version: instance.version.clone(),
        started_at: maintenant_ms(),
    };
    let chemin = chemin_index(user_data, &instance.vault_path);
    let dossier = user_data.join(DOSSIER_INSTANCES);
    fs::create_dir_all(&dossier).map_err(io_err(&dossier))?;

    // Écriture ATOMIQUE : un client qui liste le dossier à cet instant précis
    // ne doit jamais lire un JSON à moitié écrit. Le rename est atomique sur
    // le même volume ; le `.tmp` n'a pas l'extension .json, donc n'est pas listé.
    let mut temporaire = chemin.clone().into_os_string();
    temporaire.push(".tmp");
    let temporaire = PathBuf::from(temporaire);
    ecrire_prive(&temporaire, &serde_json::to_string_pretty(&complet)?)
        .map_err(io_err(&temporaire))?;
    fs::rename(&temporaire, &chemin).map_err(io_err(&chemin))?;
    Ok(chemin)
}

/// ★ Le fichier porte le jeton : lisible par le seul propriétaire (0600) sous POSIX ;
///   sous Windows c'est l'ACL du profil utilisateur qui protège `userData`.
fn ecrire_prive(chemin: &Path, contenu: &str) -> io::Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut fichier = options.open(chemin)?;
    fichier.write_all(contenu.as_bytes())
}

/// Retire l'index (sans erreur s'il n'existe pas : `force: true` côté TS).
///
/// ★ Une seule version, SYNCHRONE : le TS en avait deux (`retirerInstance` async et
///   `retirerInstanceSync` pour la fermeture de la fenêtre, où une promesse ne
///   finirait pas). En Rust, tout est déjà synchrone.
pub fn retirer_instance(user_data: &Path, vault_path: &str) -> Result<(), ErreurIndex> {
    let chemin = chemin_index(user_data, vault_path);
    match fs::remove_file(&chemin) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ErreurIndex::Io { chemin, source: e }),
    }
}

/// Toutes les instances déclarées — vivantes ou orphelines, l'appelant tranche.
pub fn lister_instances(user_data: &Path) -> Vec<CliInstanceRecord> {
    let Ok(entrees) = fs::read_dir(user_data.join(DOSSIER_INSTANCES)) else {
        return Vec::new();
    };
    let mut noms: Vec<PathBuf> = entrees
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    noms.sort(); // ordre déterministe (readdir n'en garantit aucun)

    noms.iter()
        .filter_map(|chemin| {
            // Fichier en cours d'écriture, ou corrompu : on l'ignore.
            let brut = fs::read_to_string(chemin).ok()?;
            let record: CliInstanceRecord = serde_json::from_str(&brut).ok()?;
            (record.v == CLI_PROTOCOL_VERSION).then_some(record)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatTuyau {
    Vivant,
    Mort,
    Incertain,
}

/// Les deux questions que l'index pose au système, hors du modèle pur.
pub trait SondeInstances {
    /// Un processus existe-t-il ? (`kill(pid, 0)` côté TS ; « accès refusé » = oui,
    /// il existe mais n'est pas à nous — peut-être un pid recyclé : on sondera.)
    fn processus_vivant(&self, pid: u32) -> bool;

    /// Quelqu'un écoute-t-il sur ce tuyau ? `Mort` = connexion refusée (orphelin
    /// sûr) ; `Incertain` = ni acceptée ni refusée à temps (délai de 800 ms côté
    /// TS) — l'index est GARDÉ : une app figée n'est pas une app fermée.
    fn sonder_tuyau(&self, pipe: &str) -> EtatTuyau;
}

/// Les instances qui RÉPONDENT ; les orphelins sont effacés au passage (un
/// orphelin qui reste rend `vault=` obligatoire pour toujours). Un pid mort est
/// un raccourci gratuit ; sinon on sonde le tuyau.
///
/// ★ Le TS sondait tous les tuyaux EN PARALLÈLE (délai de 800 ms chacun). Ici la
///   boucle est séquentielle : à l'implémentation réelle de [`SondeInstances`] de
///   paralléliser si le pire cas (N instances figées) compte.
pub fn instances_vivantes(
    user_data: &Path,
    sonde: &dyn SondeInstances,
) -> Result<Vec<CliInstanceRecord>, ErreurIndex> {
    let mut vivantes = Vec::new();
    for instance in lister_instances(user_data) {
        let etat = if sonde.processus_vivant(instance.pid) {
            sonde.sonder_tuyau(&instance.pipe)
        } else {
            EtatTuyau::Mort
        };
        if etat == EtatTuyau::Mort {
            retirer_instance(user_data, &instance.vault_path)?;
        } else {
            vivantes.push(instance);
        }
    }
    Ok(vivantes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::DossierTemp;
    use std::collections::HashSet;

    /// Une sonde en mémoire : seuls les tuyaux listés « écoutent ».
    struct SondeFausse {
        ecoutent: HashSet<String>,
        pid_vivant: bool,
    }

    impl SondeInstances for SondeFausse {
        fn processus_vivant(&self, _pid: u32) -> bool {
            self.pid_vivant
        }
        fn sonder_tuyau(&self, pipe: &str) -> EtatTuyau {
            if self.ecoutent.contains(pipe) {
                EtatTuyau::Vivant
            } else {
                EtatTuyau::Mort
            }
        }
    }

    fn a_publier(vault: &str, pipe: &str) -> InstanceAPublier {
        InstanceAPublier {
            vault_path: vault.into(),
            pipe: pipe.into(),
            token: "t".into(),
            version: "0".into(),
        }
    }

    // La purge des orphelins : vivant = le TUYAU répond. Le pid ne suffit pas —
    // sous Windows il est recyclé (24 orphelins d'e2e « vivaient » par le pid d'un
    // autre processus et rendaient `fragment` ambigu).
    #[test]
    fn instances_vivantes_garde_l_instance_qui_repond_purge_l_orpheline_meme_si_son_pid_vit() {
        let ud = DossierTemp::new("frag-idx-");
        let vivant = "pipe-vivant";
        // Les deux records portent NOTRE pid (vivant) : seul le tuyau tranche.
        publier_instance(ud.chemin(), &a_publier("/v/vivant", vivant)).unwrap();
        publier_instance(ud.chemin(), &a_publier("/v/orphelin", "pipe-orphelin")).unwrap();

        let sonde = SondeFausse {
            ecoutent: HashSet::from([vivant.to_string()]),
            pid_vivant: true,
        };
        let vivantes = instances_vivantes(ud.chemin(), &sonde).unwrap();
        let noms: Vec<_> = vivantes.iter().map(|i| i.vault_name.as_str()).collect();
        assert_eq!(noms, ["vivant"]);
        assert_eq!(
            fs::read_dir(ud.chemin().join("cli-instances"))
                .unwrap()
                .count(),
            1
        );
    }

    // ── Ajouts du port ───────────────────────────────────────────────────

    #[test]
    fn un_pid_mort_purge_sans_sonder_le_tuyau() {
        let ud = DossierTemp::new("frag-idx-");
        publier_instance(ud.chemin(), &a_publier("/v/a", "p")).unwrap();
        let sonde = SondeFausse {
            ecoutent: HashSet::from(["p".to_string()]), // le tuyau « répondrait »…
            pid_vivant: false,                          // …mais le pid est mort : raccourci
        };
        assert!(instances_vivantes(ud.chemin(), &sonde).unwrap().is_empty());
        assert!(lister_instances(ud.chemin()).is_empty());
    }

    #[test]
    fn un_tuyau_incertain_est_garde() {
        struct Fige;
        impl SondeInstances for Fige {
            fn processus_vivant(&self, _: u32) -> bool {
                true
            }
            fn sonder_tuyau(&self, _: &str) -> EtatTuyau {
                EtatTuyau::Incertain
            }
        }
        let ud = DossierTemp::new("frag-idx-");
        publier_instance(ud.chemin(), &a_publier("/v/a", "p")).unwrap();
        assert_eq!(instances_vivantes(ud.chemin(), &Fige).unwrap().len(), 1);
    }

    #[test]
    fn publier_ecrit_le_record_complet_sans_fichier_temporaire() {
        let ud = DossierTemp::new("frag-idx-");
        let chemin = publier_instance(ud.chemin(), &a_publier("C:\\coffres\\Notes", "p")).unwrap();
        assert_eq!(chemin, chemin_index(ud.chemin(), "C:\\coffres\\Notes"));
        let liste = lister_instances(ud.chemin());
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0].v, CLI_PROTOCOL_VERSION);
        assert_eq!(liste[0].vault_name, "Notes");
        assert_eq!(liste[0].pid, std::process::id());
        // Le `.tmp` a été renommé : un seul fichier dans le dossier.
        assert_eq!(
            fs::read_dir(ud.chemin().join(DOSSIER_INSTANCES))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn lister_ignore_les_fichiers_corrompus_ou_d_une_autre_version() {
        let ud = DossierTemp::new("frag-idx-");
        publier_instance(ud.chemin(), &a_publier("/v/ok", "p")).unwrap();
        let dossier = ud.chemin().join(DOSSIER_INSTANCES);
        fs::write(dossier.join("casse.json"), "{pas du json").unwrap();
        fs::write(
            dossier.join("v2.json"),
            r#"{"v":2,"vaultPath":"/x","vaultName":"x","pipe":"p","token":"t","pid":1,"version":"0","startedAt":0}"#,
        )
        .unwrap();
        fs::write(dossier.join("note.txt"), "pas un index").unwrap();
        let noms: Vec<_> = lister_instances(ud.chemin())
            .into_iter()
            .map(|i| i.vault_name)
            .collect();
        assert_eq!(noms, ["ok"]);
    }

    #[test]
    fn retirer_instance_est_sans_erreur_si_absente() {
        let ud = DossierTemp::new("frag-idx-");
        retirer_instance(ud.chemin(), "/v/jamais-publie").unwrap();
    }

    #[test]
    fn lister_un_dossier_absent_rend_vide() {
        let ud = DossierTemp::new("frag-idx-");
        assert!(lister_instances(&ud.chemin().join("inexistant")).is_empty());
    }
}
