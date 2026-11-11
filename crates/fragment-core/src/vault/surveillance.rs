// Port de app/src/electron/vaultWatcher.ts (moteur natif récursif) et du branchement de main.ts
//! La surveillance du coffre par `notify`, dans un fil dédié.
//!
//! ★ UN SEUL moteur, `notify` (inotify sous Linux, ReadDirectoryChangesW sous Windows, FSEvents
//!   sous macOS) : le TS en avait deux (natif récursif + chokidar) parce que `fs.watch` ne sait
//!   pas surveiller un arbre sous Linux ; `notify` le sait, le choix de plateforme disparaît.
//!
//! ★ Le fil ne touche JAMAIS au coffre (`Rc`, pas `Send`) : il pousse des couples
//!   (type, chemin de coffre) dans un canal, que l'hôte vide sur son propre fil
//!   ([`Surveillance::essayer_recevoir`], ou `FileSystemAdapter::pomper` qui rappelle le coffre).
//!
//! ★ Un événement du système ne dit pas ce qui s'est passé : « rename » couvre l'apparition,
//!   la disparition, et la sauvegarde atomique d'un éditeur (écrire à côté puis renommer PAR-
//!   DESSUS un fichier connu). Chaque chemin touché est donc regroupé pendant une courte
//!   fenêtre, puis le DISQUE tranche : absent → `Delete` ; présent et vu apparaître →
//!   `Create` puis `Modify` (les deux traitements du coffre sont idempotents et exclusifs, un
//!   seul agit, le bon) ; présent sinon → `Modify`. Un renommage sort donc comme un `Delete`
//!   de l'ancien chemin et un `Create` du nouveau, et un dossier apparu est parcouru : le
//!   système ne dit rien de son contenu déjà là.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::event::{EventKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

use super::adaptateur::ChangeType;
use super::coffre::dans_la_config;

/// Le délai de regroupement : le système notifie PENDANT l'écriture, et Windows tire souvent deux
/// événements pour une sauvegarde. Sans regroupement, le coffre relit un fichier inachevé.
pub const REGROUPEMENT_PAR_DEFAUT: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum SurveillanceError {
    #[error("surveillance du coffre : {0}")]
    Notify(#[from] notify::Error),
    #[error("surveillance du coffre : {0}")]
    Io(#[from] std::io::Error),
}

/// Rappel de réveil : appelé DEPUIS LE FIL de surveillance quand un changement est prêt, pour
/// que l'hôte pompe sans scruter (il le redirige vers sa boucle d'événements).
pub type Reveil = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct OptionsSurveillance {
    pub regroupement: Duration,
    pub reveil: Option<Reveil>,
}

impl Default for OptionsSurveillance {
    fn default() -> Self {
        Self {
            regroupement: REGROUPEMENT_PAR_DEFAUT,
            reveil: None,
        }
    }
}

/// Ce qu'on ne rapporte pas : tout chemin dont un segment commence par un point (`.git`,
/// `.trash`, fichiers temporaires des éditeurs), SAUF le dossier de configuration du coffre —
/// `hotkeys.json` y vit, et l'éditer à la main doit prendre effet à chaud.
pub fn doit_ignorer(chemin_de_coffre: &str) -> bool {
    if dans_la_config(chemin_de_coffre) {
        return false;
    }
    chemin_de_coffre.split('/').any(|s| s.starts_with('.'))
}

/// Chemin absolu → chemin de coffre (`/`, relatif). `None` hors de la racine ou pour la racine.
pub fn chemin_de_coffre(racine: &Path, absolu: &Path) -> Option<String> {
    let relatif = absolu.strip_prefix(racine).ok()?;
    let morceaux: Vec<&str> = relatif
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    if morceaux.is_empty() {
        return None;
    }
    Some(morceaux.join("/"))
}

// ═══ Le regroupement (pur, à temps injecté) ═══════════════════════════════════

struct Vu {
    echeance: Instant,
    /// Un événement de création ou de renommage est passé : il rend l'apparition et la
    /// disparition indiscernables, on s'en souvient jusqu'à la vidange.
    apparition: bool,
}

/// Coalesce les événements par chemin : le dernier repousse l'échéance.
pub struct Regroupeur {
    delai: Duration,
    en_attente: HashMap<String, Vu>,
}

impl Regroupeur {
    pub fn new(delai: Duration) -> Self {
        Self {
            delai,
            en_attente: HashMap::new(),
        }
    }

    pub fn noter(&mut self, chemin: &str, apparition: bool, maintenant: Instant) {
        let deja = self.en_attente.get(chemin).is_some_and(|v| v.apparition);
        self.en_attente.insert(
            chemin.to_owned(),
            Vu {
                echeance: maintenant + self.delai,
                apparition: deja || apparition,
            },
        );
    }

    /// Retire et rend les chemins dont l'échéance est passée, avec leur drapeau.
    pub fn echus(&mut self, maintenant: Instant) -> Vec<(String, bool)> {
        let prets: Vec<String> = self
            .en_attente
            .iter()
            .filter(|(_, v)| v.echeance <= maintenant)
            .map(|(c, _)| c.clone())
            .collect();
        prets
            .into_iter()
            .filter_map(|c| self.en_attente.remove(&c).map(|v| (c, v.apparition)))
            .collect()
    }

    pub fn prochaine_echeance(&self) -> Option<Instant> {
        self.en_attente.values().map(|v| v.echeance).min()
    }

    pub fn est_vide(&self) -> bool {
        self.en_attente.is_empty()
    }
}

// ═══ Le disque tranche ════════════════════════════════════════════════════════

fn absolu_de(racine: &Path, chemin: &str) -> PathBuf {
    chemin
        .split('/')
        .fold(racine.to_path_buf(), |p, s| p.join(s))
}

const RANG_DELETE: u8 = 0;
const RANG_CREATE: u8 = 1;
const RANG_MODIFY: u8 = 2;

/// Parcourt un dossier apparu : tout son contenu visible est « créé » (parents d'abord, via
/// le tri final).
fn parcourir(racine: &Path, dossier: &str, sortie: &mut BTreeSet<(String, u8)>) {
    let Ok(entrees) = std::fs::read_dir(absolu_de(racine, dossier)) else {
        return;
    };
    for entree in entrees.flatten() {
        let Some(nom) = entree.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let chemin = format!("{dossier}/{nom}");
        if doit_ignorer(&chemin) {
            continue;
        }
        sortie.insert((chemin.clone(), RANG_CREATE));
        if entree.file_type().is_ok_and(|t| t.is_dir()) {
            parcourir(racine, &chemin, sortie);
        }
    }
}

/// Transforme les chemins échus en événements ordonnés : les suppressions d'abord, puis les
/// créations (parents avant enfants), puis les modifications.
pub fn trancher(racine: &Path, echus: &[(String, bool)]) -> Vec<(ChangeType, String)> {
    let mut sortie: BTreeSet<(String, u8)> = BTreeSet::new();
    for (chemin, apparition) in echus {
        let Ok(meta) = std::fs::metadata(absolu_de(racine, chemin)) else {
            sortie.insert((chemin.clone(), RANG_DELETE));
            continue;
        };
        if meta.is_dir() {
            // Un dossier n'a pas de contenu à modifier : seule son apparition compte.
            if *apparition {
                sortie.insert((chemin.clone(), RANG_CREATE));
                parcourir(racine, chemin, &mut sortie);
            }
        } else if *apparition {
            sortie.insert((chemin.clone(), RANG_CREATE));
            sortie.insert((chemin.clone(), RANG_MODIFY));
        } else {
            sortie.insert((chemin.clone(), RANG_MODIFY));
        }
    }
    let mut ordre: Vec<(u8, String)> = sortie.into_iter().map(|(c, r)| (r, c)).collect();
    ordre.sort();
    ordre
        .into_iter()
        .map(|(rang, chemin)| {
            let kind = match rang {
                RANG_DELETE => ChangeType::Delete,
                RANG_CREATE => ChangeType::Create,
                _ => ChangeType::Modify,
            };
            (kind, chemin)
        })
        .collect()
}

// ═══ Le fil ═══════════════════════════════════════════════════════════════════

enum Message {
    Evenement(notify::Result<notify::Event>),
    Arret,
}

/// La surveillance en marche. L'arrêt est propre : [`arreter`](Self::arreter) (ou le `Drop`)
/// débranche le moteur, réveille le fil et l'attend.
pub struct Surveillance {
    sortie: Receiver<(ChangeType, String)>,
    commande: Sender<Message>,
    watcher: Option<RecommendedWatcher>,
    fil: Option<JoinHandle<()>>,
}

impl Surveillance {
    /// Pose UN abonnement récursif sur `racine` et lance le fil de regroupement.
    pub fn demarrer(
        racine: &Path,
        options: OptionsSurveillance,
    ) -> Result<Self, SurveillanceError> {
        let racine = racine.to_path_buf();
        let (commande, entree) = mpsc::channel::<Message>();
        let (envoi, sortie) = mpsc::channel::<(ChangeType, String)>();

        let relais = commande.clone();
        let mut watcher = notify::recommended_watcher(move |res| {
            // Le fil est parti : plus personne à qui parler.
            let _ = relais.send(Message::Evenement(res));
        })?;
        watcher.watch(&racine, RecursiveMode::Recursive)?;

        let fil = std::thread::Builder::new()
            .name("fragment-surveillance".to_owned())
            .spawn(move || boucle(&racine, &options, &entree, &envoi))?;

        Ok(Self {
            sortie,
            commande,
            watcher: Some(watcher),
            fil: Some(fil),
        })
    }

    pub fn essayer_recevoir(&self) -> Option<(ChangeType, String)> {
        self.sortie.try_recv().ok()
    }

    /// Attend au plus `delai` un changement (hôtes sans boucle d'événements, tests).
    pub fn recevoir_avant(&self, delai: Duration) -> Option<(ChangeType, String)> {
        self.sortie.recv_timeout(delai).ok()
    }

    pub fn arreter(mut self) {
        self.fermer();
    }

    fn fermer(&mut self) {
        // D'abord le moteur (plus d'événements), puis le fil.
        self.watcher = None;
        let _ = self.commande.send(Message::Arret);
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
    }
}

impl Drop for Surveillance {
    fn drop(&mut self) {
        self.fermer();
    }
}

fn est_apparition(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_))
    )
}

fn boucle(
    racine: &Path,
    options: &OptionsSurveillance,
    entree: &Receiver<Message>,
    envoi: &Sender<(ChangeType, String)>,
) {
    let mut groupe = Regroupeur::new(options.regroupement);
    loop {
        let attente = match groupe.prochaine_echeance() {
            Some(e) => e.saturating_duration_since(Instant::now()),
            None => Duration::from_secs(3600),
        };
        match entree.recv_timeout(attente) {
            Ok(Message::Evenement(Ok(evt))) => {
                if !matches!(evt.kind, EventKind::Access(_)) {
                    let apparition = est_apparition(&evt.kind);
                    for chemin in &evt.paths {
                        let Some(c) = chemin_de_coffre(racine, chemin) else {
                            continue;
                        };
                        if !doit_ignorer(&c) {
                            groupe.noter(&c, apparition, Instant::now());
                        }
                    }
                }
            }
            Ok(Message::Evenement(Err(err))) => log::error!("[watcher] {err}"),
            Ok(Message::Arret) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let echus = groupe.echus(Instant::now());
        if echus.is_empty() {
            continue;
        }
        let mut pousse = false;
        for evt in trancher(racine, &echus) {
            if envoi.send(evt).is_err() {
                return; // plus de lecteur
            }
            pousse = true;
        }
        if pousse && let Some(reveil) = &options.reveil {
            reveil();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_les_points_sauf_la_config() {
        assert!(doit_ignorer(".git/HEAD"));
        assert!(doit_ignorer("notes/.brouillon.md"));
        assert!(doit_ignorer(".trash/a.md"));
        assert!(!doit_ignorer("notes/a.md"));
        assert!(!doit_ignorer(".fragment/hotkeys.json"));
        assert!(!doit_ignorer(".fragment"));
    }

    #[test]
    fn chemin_de_coffre_relatif_et_slash() {
        let racine = Path::new("racine");
        assert_eq!(
            chemin_de_coffre(racine, &racine.join("a").join("b.md")),
            Some("a/b.md".to_owned())
        );
        assert_eq!(chemin_de_coffre(racine, racine), None);
        assert_eq!(chemin_de_coffre(racine, Path::new("ailleurs/x")), None);
    }

    #[test]
    fn regroupeur_repousse_l_echeance_et_retient_l_apparition() {
        let t0 = Instant::now();
        let mut g = Regroupeur::new(Duration::from_millis(100));
        g.noter("a.md", true, t0);
        g.noter("a.md", false, t0 + Duration::from_millis(60));
        assert!(g.echus(t0 + Duration::from_millis(120)).is_empty());
        let pret = g.echus(t0 + Duration::from_millis(160));
        assert_eq!(pret, vec![("a.md".to_owned(), true)]);
        assert!(g.est_vide());
    }

    #[test]
    fn trancher_absent_donne_delete() {
        let dossier = std::env::temp_dir().join("fragment-trancher-inexistant");
        let evts = trancher(&dossier, &[("n.md".to_owned(), false)]);
        assert_eq!(evts, vec![(ChangeType::Delete, "n.md".to_owned())]);
    }
}
