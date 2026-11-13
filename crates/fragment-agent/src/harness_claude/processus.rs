// Port de app/src/ui/views/agent/AgentHost.ts (le lancement du processus, sa fin de stderr) et de
// sdkProcess.mjs (le transport), réécrits pour le binaire `claude` seul (ADR-016 §1, §3, §4)
//
// Le TRANSPORT d'un tour : un processus enfant, des lignes JSON sur stdin et stdout, la fin de
// stderr pour dire pourquoi il est mort. Le contrôleur (`tour.rs`) ne voit que le trait
// [`Processus`] : le vrai est [`LanceurSysteme`], celui des tests rejoue une fixture
// (`faux.rs`). Aucun `async`, aucune dépendance : deux fils de lecture et un canal.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Au plus ce nombre de caractères gardés de stderr : c'est le message d'une mort.
pub const FIN_STDERR: usize = 2000;

/// Une erreur du transport.
#[derive(Debug, thiserror::Error)]
pub enum ErreurProcessus {
    #[error("impossible de lancer « {programme} » : {cause}")]
    Lancement { programme: String, cause: String },
    #[error("l'entrée du processus est fermée")]
    EntreeFermee,
    #[error("écriture vers le processus : {0}")]
    Ecriture(String),
}

/// Ce qu'il faut pour lancer un tour : construit par `commande.rs`, PUR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lancement {
    pub programme: PathBuf,
    pub arguments: Vec<String>,
    pub cwd: PathBuf,
    /// L'environnement COMPLET du processus (rien n'est hérité en plus).
    pub env: BTreeMap<String, String>,
}

/// Ce que rend une lecture de stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lecture {
    /// Une ligne, telle quelle (sans le saut de ligne).
    Ligne(String),
    /// Rien pendant le délai : le processus vit encore.
    Rien,
    /// Le flux est fermé : le processus est parti (ou part).
    Fin,
}

/// Un processus `claude` en cours.
pub trait Processus {
    /// Écrit une ligne sur stdin (le saut de ligne est ajouté).
    fn ecrire(&mut self, ligne: &str) -> Result<(), ErreurProcessus>;
    /// Attend au plus `delai` une ligne de stdout.
    fn lire(&mut self, delai: Duration) -> Lecture;
    /// Ferme stdin : le binaire finit proprement.
    fn fermer_entree(&mut self);
    /// Attend la sortie au plus `delai`, puis tue le processus. Rend le code de sortie s'il est connu.
    fn attendre(&mut self, delai: Duration) -> Option<i32>;
    /// La fin de stderr (au plus [`FIN_STDERR`] caractères).
    fn fin_stderr(&self) -> String;
}

/// Ce qui lance un processus.
pub trait Lanceur {
    fn lancer(&self, l: &Lancement) -> Result<Box<dyn Processus>, ErreurProcessus>;
}

/// Garde les `n` derniers caractères (jamais au milieu d'un caractère).
pub fn derniers_caracteres(s: &str, n: usize) -> String {
    let total = s.chars().count();
    s.chars().skip(total.saturating_sub(n)).collect()
}

// ── Le vrai processus ────────────────────────────────────────────────────────

/// Lance le vrai binaire.
#[derive(Debug, Default, Clone, Copy)]
pub struct LanceurSysteme;

struct ProcessusSysteme {
    enfant: Child,
    stdin: Option<ChildStdin>,
    lignes: Receiver<Option<String>>,
    stderr: Arc<Mutex<String>>,
    fini: bool,
}

impl Lanceur for LanceurSysteme {
    fn lancer(&self, l: &Lancement) -> Result<Box<dyn Processus>, ErreurProcessus> {
        let mut cmd = Command::new(&l.programme);
        cmd.args(&l.arguments)
            .current_dir(&l.cwd)
            // ★ Rien n'est hérité : l'environnement est calculé (sans CLAUDE*, ANTHROPIC*).
            .env_clear()
            .envs(&l.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW : pas de console qui clignote à chaque tour.
            cmd.creation_flags(0x0800_0000);
        }
        let erreur = |cause: String| ErreurProcessus::Lancement {
            programme: l.programme.display().to_string(),
            cause,
        };
        let mut enfant = cmd.spawn().map_err(|e| erreur(e.to_string()))?;
        let stdout = enfant
            .stdout
            .take()
            .ok_or_else(|| erreur("pas de stdout".into()))?;
        let stderr_flux = enfant
            .stderr
            .take()
            .ok_or_else(|| erreur("pas de stderr".into()))?;
        let stdin = enfant.stdin.take();

        // Le LECTEUR : stdout ligne à ligne vers le canal ; `None` = flux fermé.
        let (tx, lignes) = mpsc::channel();
        thread::spawn(move || {
            for ligne in BufReader::new(stdout).lines() {
                let Ok(ligne) = ligne else { break };
                if tx.send(Some(ligne)).is_err() {
                    return;
                }
            }
            let _ = tx.send(None);
        });
        // La FIN DE STDERR, par morceaux (un binaire bavard ne remplit pas la mémoire).
        let stderr = Arc::new(Mutex::new(String::new()));
        let copie = Arc::clone(&stderr);
        thread::spawn(move || {
            let mut flux = BufReader::new(stderr_flux);
            let mut tampon = [0u8; 4096];
            while let Ok(n) = flux.read(&mut tampon) {
                if n == 0 {
                    break;
                }
                if let Ok(mut s) = copie.lock() {
                    s.push_str(&String::from_utf8_lossy(&tampon[..n]));
                    if s.len() > FIN_STDERR * 4 {
                        *s = derniers_caracteres(&s, FIN_STDERR);
                    }
                }
            }
        });
        Ok(Box::new(ProcessusSysteme {
            enfant,
            stdin,
            lignes,
            stderr,
            fini: false,
        }))
    }
}

impl Processus for ProcessusSysteme {
    fn ecrire(&mut self, ligne: &str) -> Result<(), ErreurProcessus> {
        let stdin = self.stdin.as_mut().ok_or(ErreurProcessus::EntreeFermee)?;
        stdin
            .write_all(ligne.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| ErreurProcessus::Ecriture(e.to_string()))
    }

    fn lire(&mut self, delai: Duration) -> Lecture {
        if self.fini {
            return Lecture::Fin;
        }
        match self.lignes.recv_timeout(delai) {
            Ok(Some(l)) => Lecture::Ligne(l),
            Ok(None) | Err(RecvTimeoutError::Disconnected) => {
                self.fini = true;
                Lecture::Fin
            }
            Err(RecvTimeoutError::Timeout) => Lecture::Rien,
        }
    }

    fn fermer_entree(&mut self) {
        self.stdin.take();
    }

    fn attendre(&mut self, delai: Duration) -> Option<i32> {
        let debut = Instant::now();
        while debut.elapsed() < delai {
            if let Ok(Some(s)) = self.enfant.try_wait() {
                return s.code();
            }
            // Vider stdout : un enfant ne doit pas rester bloqué sur un tuyau plein.
            let _ = self.lignes.recv_timeout(Duration::from_millis(20));
        }
        let _ = self.enfant.kill();
        self.enfant.wait().ok().and_then(|s| s.code())
    }

    fn fin_stderr(&self) -> String {
        self.stderr
            .lock()
            .map(|s| derniers_caracteres(&s, FIN_STDERR))
            .unwrap_or_default()
    }
}

impl Drop for ProcessusSysteme {
    fn drop(&mut self) {
        // Un tour abandonné (panique, erreur) ne laisse pas de processus derrière lui.
        if let Ok(None) = self.enfant.try_wait() {
            let _ = self.enfant.kill();
            let _ = self.enfant.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_fin_de_stderr_garde_les_derniers_caracteres() {
        assert_eq!(derniers_caracteres("abcdé", 2), "dé");
        assert_eq!(derniers_caracteres("ab", 10), "ab");
    }

    #[test]
    fn un_binaire_absent_est_une_erreur_de_lancement() {
        let l = Lancement {
            programme: PathBuf::from("binaire-qui-n-existe-pas-fragment"),
            arguments: vec![],
            cwd: std::env::temp_dir(),
            env: BTreeMap::new(),
        };
        assert!(matches!(
            LanceurSysteme.lancer(&l),
            Err(ErreurProcessus::Lancement { .. })
        ));
    }
}
