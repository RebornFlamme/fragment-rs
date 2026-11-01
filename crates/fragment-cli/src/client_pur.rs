// Port de app/src/cli/clientPur.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LA COUCHE PURE DU CLIENT — les DÉCISIONS, extraites de main.ts (A15/A16).
//
//  Avant, ces choix vivaient dans main.ts, mêlés à `process.exit`, aux sockets
//  et au disque : la seule preuve du client était e2e, lente et lourde. Ici,
//  tout est PUR — entrées en mémoire, sortie = un verdict — donc testable en
//  isolation. Le binaire `fragment` reste la coquille qui rassemble les entrées
//  (E/S) et APPLIQUE ces verdicts.
//
//  Trois décisions : à qui parler (`choisir_cible`), quoi imprimer pour `vaults`
//  (`formater_coffres`), et comment traduire une erreur de connexion en code de
//  sortie (`reagir_erreur_connexion`).
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashSet;

use crate::protocol::{CliInstanceRecord, EXIT, est_tuyau_mort};
use crate::vault_registry::{CoffreConnu, meme_coffre};

// ── Choisir l'instance ─────────────────────────────────────────────────────

/// Le verdict : à qui parler, ou pourquoi on ne peut pas (avec le code GNU).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cible {
    Ok { instance: CliInstanceRecord },
    Echec { code: i32, message: String },
}

fn noms(instances: &[CliInstanceRecord]) -> String {
    instances
        .iter()
        .map(|i| i.vault_name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// L'instance visée par `vault=`, ou la seule qui tourne.
///
/// - `vault=` nommé et trouvé → cette instance ;
/// - `vault=` nommé, absent, d'autres tournent → `UNKNOWN_VAULT` (mauvais coffre) ;
/// - `vault=` nommé, rien ne tourne → `NO_INSTANCE` (pas d'app) ;
/// - pas de `vault=`, plusieurs → `AMBIGUOUS_VAULT` (précise lequel) ;
/// - pas de `vault=`, une seule → cette instance ;
/// - pas de `vault=`, aucune → `NO_INSTANCE`.
///
/// ★ La distinction UNKNOWN_VAULT / NO_INSTANCE est délibérée : un script doit
///   savoir s'il s'est trompé de coffre (2) ou s'il a oublié d'ouvrir l'app (5).
pub fn choisir_cible(instances: &[CliInstanceRecord], vault: Option<&str>) -> Cible {
    // Un `vault=` vide ne désigne rien (faux en JS) : on le traite comme absent.
    if let Some(vault) = vault.filter(|v| !v.is_empty()) {
        let trouve = instances
            .iter()
            .find(|i| meme_coffre(&i.vault_path, &i.vault_name, vault));
        if let Some(instance) = trouve {
            return Cible::Ok {
                instance: instance.clone(),
            };
        }
        if !instances.is_empty() {
            return Cible::Echec {
                code: EXIT.UNKNOWN_VAULT,
                message: format!(
                    "aucune instance ne tourne pour « {vault} ». Instances : {}",
                    noms(instances)
                ),
            };
        }
        return Cible::Echec {
            code: EXIT.NO_INSTANCE,
            message: format!(
                "aucune instance de Fragment ne tourne pour « {vault} ». Ouvre l’app sur ce coffre, puis réessaie."
            ),
        };
    }

    match instances {
        [] => Cible::Echec {
            code: EXIT.NO_INSTANCE,
            message:
                "aucune instance de Fragment ne tourne. Ouvre l’app sur ce coffre, puis réessaie."
                    .to_string(),
        },
        [seule] => Cible::Ok {
            instance: seule.clone(),
        },
        plusieurs => Cible::Echec {
            code: EXIT.AMBIGUOUS_VAULT,
            message: format!(
                "{} instances tournent ; précise vault=<nom>. Instances : {}",
                plusieurs.len(),
                noms(plusieurs)
            ),
        },
    }
}

// ── Formater `fragment vaults` ─────────────────────────────────────────────

fn norm_chemin(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// La sortie de `fragment vaults` : nom⇥chemin⇥état[⇥(courant)], une ligne par coffre.
pub fn formater_coffres<S: AsRef<str>>(connus: &[CoffreConnu], chemins_en_cours: &[S]) -> String {
    if connus.is_empty() {
        return "(aucun coffre connu)".to_string();
    }
    let en_cours: HashSet<String> = chemins_en_cours
        .iter()
        .map(|p| norm_chemin(p.as_ref()))
        .collect();
    connus
        .iter()
        .map(|c| {
            let etat = if en_cours.contains(&norm_chemin(&c.path)) {
                "en cours"
            } else {
                "—"
            };
            let courant = if c.current { "\t(courant)" } else { "" };
            format!("{}\t{}\t{etat}{courant}", c.name, c.path)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Réagir à une erreur de connexion ───────────────────────────────────────

/// Une erreur de `connect()` : son code symbolique (`ECONNREFUSED`…) et son message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ErreurConnexion {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionErreur {
    /// Vrai = index orphelin à purger avant de sortir (pid réutilisé, tuyau mort).
    pub purger: bool,
    pub code: i32,
    pub message: String,
}

/// Traduit une erreur de `connect()` en code de sortie + message, sans effet de bord.
pub fn reagir_erreur_connexion(err: &ErreurConnexion, nom_coffre: &str) -> ReactionErreur {
    if est_tuyau_mort(&err.code) {
        return ReactionErreur {
            purger: true,
            code: EXIT.NO_INSTANCE,
            message: format!(
                "l’instance « {nom_coffre} » ne répond plus ({}). Elle a probablement été fermée sans prévenir ; relance l’app.",
                err.code
            ),
        };
    }
    ReactionErreur {
        purger: false,
        code: EXIT.HANDLER_ERROR,
        message: format!(
            "connexion à « {nom_coffre} » : {} {}",
            err.code, err.message
        )
        .trim()
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A15/A16 — la couche pure du client, prouvée EN MÉMOIRE.

    fn rec_a(name: &str, path: &str) -> CliInstanceRecord {
        CliInstanceRecord {
            v: 1,
            vault_path: path.to_string(),
            vault_name: name.to_string(),
            pipe: format!(r"\\.\pipe\{name}"),
            token: "t".into(),
            pid: 1,
            version: "0".into(),
            started_at: 0,
        }
    }

    fn rec(name: &str) -> CliInstanceRecord {
        rec_a(name, &format!("/vaults/{name}"))
    }

    fn coffre(name: &str, path: &str, current: bool) -> CoffreConnu {
        CoffreConnu {
            path: path.to_string(),
            name: name.to_string(),
            last_opened: 0,
            current,
        }
    }

    fn coffre_simple(name: &str) -> CoffreConnu {
        coffre(name, &format!("/vaults/{name}"), false)
    }

    /// Le verdict d'un échec attendu : (code, message).
    fn echec(c: Cible) -> (i32, String) {
        match c {
            Cible::Echec { code, message } => (code, message),
            Cible::Ok { .. } => panic!("un échec était attendu"),
        }
    }

    const AUCUN: &[String] = &[];

    // ── choisir_cible — la découverte du client ──────────────────────────

    #[test]
    fn choisir_cible_une_seule_instance_pas_de_vault_on_la_prend() {
        let a = rec("Notes");
        assert_eq!(
            choisir_cible(std::slice::from_ref(&a), None),
            Cible::Ok { instance: a }
        );
    }

    #[test]
    fn choisir_cible_aucune_instance_pas_de_vault_no_instance_avec_la_marche_a_suivre() {
        let (code, message) = echec(choisir_cible(&[], None));
        assert_eq!(code, EXIT.NO_INSTANCE);
        assert!(message.contains("aucune instance"));
        assert!(message.contains("Ouvre l’app"));
    }

    #[test]
    fn choisir_cible_plusieurs_instances_pas_de_vault_ambiguous_vault_avec_les_noms() {
        let (code, message) = echec(choisir_cible(&[rec("Notes"), rec("Boulot")], None));
        assert_eq!(code, EXIT.AMBIGUOUS_VAULT);
        assert!(message.contains("Notes"));
        assert!(message.contains("Boulot"));
    }

    #[test]
    fn choisir_cible_vault_qui_correspond_par_nom_insensible_a_la_casse_cette_instance() {
        let a = rec("Notes");
        assert_eq!(
            choisir_cible(&[a.clone(), rec("Boulot")], Some("notes")),
            Cible::Ok { instance: a }
        );
    }

    #[test]
    fn choisir_cible_vault_qui_correspond_par_chemin_cette_instance() {
        let a = rec_a("Notes", "/home/u/Notes");
        assert_eq!(
            choisir_cible(std::slice::from_ref(&a), Some("/home/u/Notes/")),
            Cible::Ok { instance: a }
        );
    }

    #[test]
    fn choisir_cible_vault_introuvable_mais_d_autres_tournent_unknown_vault_avec_la_liste() {
        let (code, message) = echec(choisir_cible(&[rec("Notes")], Some("Inconnu")));
        assert_eq!(code, EXIT.UNKNOWN_VAULT);
        assert!(message.contains("Notes"));
    }

    #[test]
    fn choisir_cible_vault_nomme_mais_rien_ne_tourne_no_instance_pas_unknown_vault() {
        let (code, message) = echec(choisir_cible(&[], Some("Inconnu")));
        assert_eq!(code, EXIT.NO_INSTANCE);
        assert!(message.contains("Inconnu"));
        assert!(message.contains("Ouvre l’app"));
    }

    // ── formater_coffres — la sortie de `vaults` ─────────────────────────

    #[test]
    fn formater_coffres_liste_vide_un_message_clair_pas_une_chaine_vide() {
        assert_eq!(formater_coffres(&[], AUCUN), "(aucun coffre connu)");
    }

    #[test]
    fn formater_coffres_un_coffre_non_lance_se_termine_par_tiret() {
        assert_eq!(
            formater_coffres(&[coffre_simple("Notes")], AUCUN),
            "Notes\t/vaults/Notes\t—"
        );
    }

    #[test]
    fn formater_coffres_un_coffre_dont_le_chemin_tourne_est_marque_en_cours() {
        assert_eq!(
            formater_coffres(&[coffre_simple("Notes")], &["/vaults/Notes"]),
            "Notes\t/vaults/Notes\ten cours"
        );
    }

    #[test]
    fn formater_coffres_la_correspondance_ignore_casse_et_separateurs_backslash_slash_final() {
        // Le coffre connu en slash ; l'instance vivante en backslash + slash final.
        assert!(
            formater_coffres(&[coffre("X", "C:/X/Vault", false)], &[r"C:\X\Vault\"])
                .contains("\ten cours")
        );
    }

    #[test]
    fn formater_coffres_le_coffre_courant_porte_le_suffixe_courant() {
        assert_eq!(
            formater_coffres(&[coffre("Notes", "/vaults/Notes", true)], AUCUN),
            "Notes\t/vaults/Notes\t—\t(courant)"
        );
    }

    // ── reagir_erreur_connexion — erreur de connexion → code de sortie ───

    fn tuyau_mort(code: &str) {
        let r = reagir_erreur_connexion(
            &ErreurConnexion {
                code: code.to_string(),
                message: String::new(),
            },
            "Notes",
        );
        assert!(r.purger);
        assert_eq!(r.code, EXIT.NO_INSTANCE);
        assert!(r.message.contains("ne répond plus"));
        assert!(r.message.contains(code));
    }

    #[test]
    fn reagir_erreur_connexion_econnrefused_tuyau_mort_no_instance_purge() {
        tuyau_mort("ECONNREFUSED");
    }

    #[test]
    fn reagir_erreur_connexion_enoent_tuyau_mort_no_instance_purge() {
        tuyau_mort("ENOENT");
    }

    #[test]
    fn reagir_erreur_connexion_epipe_tuyau_mort_no_instance_purge() {
        tuyau_mort("EPIPE");
    }

    #[test]
    fn reagir_erreur_connexion_un_autre_code_handler_error_sans_purge() {
        let r = reagir_erreur_connexion(
            &ErreurConnexion {
                code: "EACCES".into(),
                message: "refusé".into(),
            },
            "Notes",
        );
        assert!(!r.purger);
        assert_eq!(r.code, EXIT.HANDLER_ERROR);
        assert!(r.message.contains("connexion à"));
    }
}
