// Port de app/src/shared/cli/pipe.ts (tests : app/src/shared/cli/protocol.test.ts, bloc `pipeNameFor`)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le NOM du tuyau, dérivé du chemin du coffre.
//
//  Le nom du tuyau est DÉRIVÉ du chemin du coffre : un client froid le calcule
//  sans aucune E/S. Le secret n'entre PAS dedans — il vit dans le fichier
//  d'index, protégé par les ACL du profil utilisateur (voir `instance_index`).
//
//  ★ Sous Windows, `\\.\pipe\<nom>` est un objet du NOYAU : il disparaît avec le
//    processus. Aucun fichier orphelin après un crash — c'est le cas favorable
//    de cette plateforme. Sous POSIX, c'est un fichier `.sock` dans le dossier
//    d'exécution, qu'il faut savoir nettoyer au bind (serveur).
// ═══════════════════════════════════════════════════════════════════════════

use sha2::{Digest, Sha256};

/// La plateforme pour laquelle on nomme le tuyau (rend la fonction pure et testable
/// sur les deux OS sans lire `cfg!`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlateformeTuyau {
    Windows,
    Posix,
}

impl PlateformeTuyau {
    /// La plateforme de la compilation en cours.
    pub fn courante() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }
}

/// L'empreinte (16 hex) d'un chemin de coffre.
///
/// ★ Casse ignorée, séparateurs unifiés, `/` final retiré : Windows ignore la casse,
///   et deux graphies du même dossier doivent donner le même tuyau.
fn empreinte(vault_path: &str) -> String {
    let norm = vault_path
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase();
    let hash = Sha256::digest(norm.as_bytes());
    let mut hex = String::with_capacity(16);
    for octet in hash.iter().take(8) {
        hex.push_str(&format!("{octet:02x}"));
    }
    hex
}

/// Le nom du tuyau, pour une plateforme donnée. `dossier_runtime` n'est lu que
/// sous POSIX (`$XDG_RUNTIME_DIR`, à défaut le dossier temporaire).
pub fn pipe_name_for_plateforme(
    vault_path: &str,
    plateforme: PlateformeTuyau,
    dossier_runtime: &str,
) -> String {
    let empreinte = empreinte(vault_path);
    match plateforme {
        PlateformeTuyau::Windows => format!(r"\\.\pipe\fragment-{empreinte}"),
        PlateformeTuyau::Posix => {
            format!(
                "{}/fragment-{empreinte}.sock",
                dossier_runtime.trim_end_matches('/')
            )
        }
    }
}

/// Le nom du tuyau sur la plateforme courante.
pub fn pipe_name_for(vault_path: &str) -> String {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    pipe_name_for_plateforme(vault_path, PlateformeTuyau::courante(), &runtime)
}

/// L'empreinte seule — c'est le nom du fichier d'index de l'instance.
pub fn instance_id_for(vault_path: &str) -> String {
    empreinte(vault_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_name_for_est_deterministe_et_insensible_a_la_casse_et_aux_separateurs() {
        let a = pipe_name_for("C:\\Users\\x\\Coffre");
        assert_eq!(pipe_name_for("c:/users/x/coffre/"), a);
        assert_ne!(pipe_name_for("C:\\Users\\x\\Autre"), a);
    }

    #[test]
    fn pipe_name_for_ne_contient_que_l_empreinte_jamais_le_chemin() {
        for p in [PlateformeTuyau::Windows, PlateformeTuyau::Posix] {
            let nom = pipe_name_for_plateforme("C:\\Users\\julie\\Secret", p, "/run/user/1000");
            let bas = nom.to_lowercase();
            assert!(!bas.contains("julie") && !bas.contains("secret"), "{nom}");
        }
    }

    #[test]
    fn instance_id_for_est_l_empreinte_nue() {
        let id = instance_id_for("C:\\x");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        assert!(pipe_name_for("C:\\x").contains(&id));
    }

    // ── Ajouts du port ───────────────────────────────────────────────────

    #[test]
    fn la_forme_du_nom_suit_la_plateforme() {
        let id = instance_id_for("/v/x");
        assert_eq!(
            pipe_name_for_plateforme("/v/x", PlateformeTuyau::Windows, "/tmp"),
            format!(r"\\.\pipe\fragment-{id}")
        );
        assert_eq!(
            pipe_name_for_plateforme("/v/x", PlateformeTuyau::Posix, "/run/user/1/"),
            format!("/run/user/1/fragment-{id}.sock")
        );
    }
}
