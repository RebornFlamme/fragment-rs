//! CLI : protocole, index d'instances, shim, registre des coffres, client `fragment`.
//!
//! Modèle PUR uniquement (program.md §5, phase 1) : le transport réel (tuyau nommé,
//! sockets) viendra avec `fragment-app`. Ici : le contrat du fil, l'index des
//! instances (disque seulement, la sonde du tuyau est injectée), le shim, le registre
//! des coffres, les décisions du client et le traitement d'une requête côté serveur.

pub mod client_pur;
pub mod instance_index;
pub mod pipe;
pub mod protocol;
pub mod shim;
pub mod vault_path;
pub mod vault_registry;

/// Nom de dossier d'un chemin, séparateurs `/` ET `\` reconnus.
///
/// ★ `path.basename` de Node dépend de la plateforme ; les index et le registre
///   contiennent des chemins Windows que la CI Linux doit aussi savoir lire.
pub(crate) fn nom_du_dossier(chemin: &str) -> String {
    let sans_fin = chemin.trim_end_matches(['/', '\\']);
    match sans_fin.rfind(['/', '\\']) {
        Some(i) => sans_fin[i + 1..].to_string(),
        None => sans_fin.to_string(),
    }
}

/// Aides de test : un dossier temporaire supprimé à la chute (pas de dépendance `tempfile`).
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COMPTEUR: AtomicUsize = AtomicUsize::new(0);

    pub struct DossierTemp(PathBuf);

    impl DossierTemp {
        pub fn new(prefixe: &str) -> Self {
            let n = COMPTEUR.fetch_add(1, Ordering::SeqCst);
            let chemin = std::env::temp_dir().join(format!(
                "{prefixe}{}-{n}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            std::fs::create_dir_all(&chemin).unwrap();
            Self(chemin)
        }

        pub fn chemin(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for DossierTemp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
