// Port de app/src/ui/theme/ThemePlugin.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les thèmes du coffre : le plugin cœur `theme`, sans interface.
//
//  `.fragment/themes/<Nom>/theme.json` (ADR-005 §4), choisi par `cssTheme` dans
//  `appearance.json` (réglages, ou `theme:set name=<Nom>`). Le contenu est un objet plat
//  de jetons posé PAR-DESSUS la palette de Fragment par la cascade ; l'accent choisi dans
//  les réglages passe en DERNIER, après le thème. Un `theme.css` (celui de Fragment) n'est
//  pas interprété : une notice le dit, une seule fois par thème.
//
//  ★ Ici pas d'horloge ni d'exécuteur : le temps est injecté (`Horloge`) et l'hôte rappelle
//    `sonder` à l'échéance (`prochaine_echeance`), comme pour `ConfigStore`.
//  ★ Cette struct ne connaît ni GPUI ni une vue : elle sait calculer le `Theme` d'un mode et
//    prévenir (`on_change`) quand il faut le recalculer. Le re-rendu est dans fragment-ui.
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::rc::{Rc, Weak};

use fragment_core::{
    Apparence, DataAdapter, Debouncer, EventRef, Horloge, Vault, VaultEvent, VaultEventType,
    debounce,
};

use crate::cascade::surcharges_depuis_json;
use crate::manager::{Desabonner, ThemeManager};
use crate::theme::{Mode, Theme};
use crate::theme_model::{
    chemin_du_theme, chemin_du_theme_json, dossier_des_themes, effet_d_un_changement,
    lister_themes, nom_valide,
};

/// Les abonnés de `on_change`, par identifiant.
type Abonnes = Rc<RefCell<Vec<(u64, Rc<dyn Fn()>)>>>;

/// Ouvre un dossier du coffre dans l'explorateur du système.
pub type Ouvrir = Rc<dyn Fn(&str) -> io::Result<()>>;

/// Le regroupement des événements du dossier des thèmes (rafale d'une sauvegarde d'éditeur).
pub const REGROUPEMENT_MS: u64 = 50;

/// Le jeton de l'accent, posé en dernier.
const JETON_ACCENT: &str = "--color-accent";

/// Un choix de thème refusé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurTheme {
    /// Le nom n'est pas celui d'un thème installé.
    Inconnu {
        nom: String,
        installes: Vec<String>,
        dossier: String,
    },
}

impl fmt::Display for ErreurTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inconnu {
                nom,
                installes,
                dossier,
            } => {
                let liste = if installes.is_empty() {
                    "(aucun)".to_owned()
                } else {
                    installes.join(", ")
                };
                write!(
                    f,
                    "thème inconnu : {nom}. Installés : {liste} — dans {dossier}/<nom>/theme.json"
                )
            }
        }
    }
}

impl std::error::Error for ErreurTheme {}

/// Ce que le dernier `appliquer` a lu (comparé au suivant pour savoir s'il faut annoncer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Applique {
    /// Le thème effectivement posé (`""` = aucun).
    actif: String,
    /// Les paires du `theme.json`.
    surcharges: Vec<(String, String)>,
    /// L'accent choisi (`""` = celui du thème).
    accent: String,
}

#[derive(Default)]
struct Etat {
    themes: Vec<String>,
    applique: Applique,
    /// Les avertissements de lecture du `theme.json` actif.
    avertissements: Vec<String>,
    /// Les thèmes dont le `theme.css` a déjà été signalé.
    css_signales: HashSet<String>,
}

/// Les thèmes du coffre.
pub struct ThemesCoffre {
    adaptateur: Rc<dyn DataAdapter>,
    config_dir: String,
    apparence: Rc<Apparence>,
    horloge: Horloge,
    notifier: Rc<dyn Fn(&str)>,
    ouvrir: Ouvrir,
    etat: RefCell<Etat>,
    abonnes: Abonnes,
    prochain_abonne: Cell<u64>,
    recharger_du: Rc<Cell<bool>>,
    relire_du: Rc<Cell<bool>>,
    rechargement: RefCell<Debouncer<()>>,
    relecture: RefCell<Debouncer<()>>,
}

impl ThemesCoffre {
    /// - `config_dir` : le dossier de configuration du coffre (`.fragment`) ;
    /// - `notifier` : affiche une notice (le `theme.css` non interprété) ;
    /// - `ouvrir` : ouvre un dossier DU COFFRE (chemin relatif) dans l'explorateur du système.
    pub fn new(
        adaptateur: Rc<dyn DataAdapter>,
        config_dir: &str,
        apparence: Rc<Apparence>,
        horloge: Horloge,
        notifier: Rc<dyn Fn(&str)>,
        ouvrir: Ouvrir,
    ) -> Rc<Self> {
        let recharger_du = Rc::new(Cell::new(false));
        let relire_du = Rc::new(Cell::new(false));
        let (a, b) = (recharger_du.clone(), relire_du.clone());
        Rc::new(Self {
            adaptateur,
            config_dir: config_dir.to_owned(),
            apparence,
            horloge,
            notifier,
            ouvrir,
            etat: RefCell::new(Etat::default()),
            abonnes: Rc::new(RefCell::new(Vec::new())),
            prochain_abonne: Cell::new(0),
            recharger_du,
            relire_du,
            rechargement: RefCell::new(debounce(move |()| a.set(true), REGROUPEMENT_MS, true)),
            relecture: RefCell::new(debounce(move |()| b.set(true), REGROUPEMENT_MS, true)),
        })
    }

    // ── Lecture ──────────────────────────────────────────────────────────────

    /// Le thème effectivement posé (`""` = celui de Fragment).
    pub fn actif(&self) -> String {
        self.etat.borrow().applique.actif.clone()
    }

    /// Les avertissements de lecture du `theme.json` actif (JSON illisible, valeur non texte).
    pub fn avertissements(&self) -> Vec<String> {
        self.etat.borrow().avertissements.clone()
    }

    /// Le thème d'un `mode` : la palette de Fragment, le `theme.json` actif, puis l'accent choisi.
    pub fn theme(&self, mode: Mode) -> Theme {
        self.theme_et_avertissements(mode).0
    }

    /// Comme [`Self::theme`], avec les avertissements de la cascade (jeton inconnu, valeur
    /// illisible…) et ceux de la lecture du fichier.
    pub fn theme_et_avertissements(&self, mode: Mode) -> (Theme, Vec<String>) {
        let etat = self.etat.borrow();
        let mut paires = etat.applique.surcharges.clone();
        // ★ L'accent des réglages est la DERNIÈRE surcharge ; une valeur vide est ignorée.
        if !etat.applique.accent.is_empty() {
            paires.push((JETON_ACCENT.to_owned(), etat.applique.accent.clone()));
        }
        let (theme, mut avertissements) = Theme::avec_surcharges(mode, &paires);
        let mut tous = etat.avertissements.clone();
        tous.append(&mut avertissements);
        (theme, tous)
    }

    // ── Liste ────────────────────────────────────────────────────────────────

    /// Relit les dossiers de `.fragment/themes`, et annonce si la liste a changé.
    pub fn relire_liste(&self) -> io::Result<()> {
        if self.relire_sans_annoncer() {
            self.annoncer();
        }
        Ok(())
    }

    fn relire_sans_annoncer(&self) -> bool {
        let dossier = dossier_des_themes(&self.config_dir);
        let themes = if self.adaptateur.exists(&dossier) {
            match self.adaptateur.list(&dossier) {
                Ok(liste) => {
                    let a_un_theme = |d: &str| {
                        self.adaptateur.exists(&format!("{d}/theme.json"))
                            || self.adaptateur.exists(&format!("{d}/theme.css"))
                    };
                    lister_themes(&liste.folders, &a_un_theme)
                }
                // Un dossier illisible : la liste est vide, ce n'est pas une panne.
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let mut etat = self.etat.borrow_mut();
        if etat.themes == themes {
            return false;
        }
        etat.themes = themes;
        true
    }

    // ── Application ──────────────────────────────────────────────────────────

    /// Lit le réglage `cssTheme`, le `theme.json` du thème et l'accent choisi, et annonce si
    /// quelque chose a changé. Idempotent.
    pub fn appliquer(&self) {
        if self.appliquer_sans_annoncer() {
            self.annoncer();
        }
    }

    fn appliquer_sans_annoncer(&self) -> bool {
        let nom = self.apparence.theme_css();
        let accent = self.apparence.accent();
        let mut actif = String::new();
        let mut surcharges = Vec::new();
        let mut avertissements = Vec::new();
        if !nom.is_empty() && nom_valide(&nom) {
            let json = chemin_du_theme_json(&self.config_dir, &nom);
            let css = chemin_du_theme(&self.config_dir, &nom);
            let a_json = self.adaptateur.exists(&json);
            let a_css = self.adaptateur.exists(&css);
            if a_json {
                match self.adaptateur.read(&json) {
                    Ok(texte) => {
                        let (paires, mut avert) = surcharges_depuis_json(&texte);
                        surcharges = paires;
                        avertissements.append(&mut avert);
                        actif = nom.clone();
                    }
                    Err(e) => avertissements.push(format!("{nom} : lecture impossible : {e}")),
                }
            } else if a_css {
                // Un thème qui n'a que son theme.css reste choisi : la notice dit pourquoi
                // rien ne change.
                actif = nom.clone();
            } else {
                avertissements.push(format!(
                    "thème introuvable : {nom} — celui de Fragment reste"
                ));
            }
            if a_css {
                self.signaler_css(&nom);
            }
        }
        let applique = Applique {
            actif,
            surcharges,
            accent,
        };
        let mut etat = self.etat.borrow_mut();
        etat.avertissements = avertissements;
        if etat.applique == applique {
            return false;
        }
        etat.applique = applique;
        true
    }

    /// La notice du `theme.css` : une seule fois par thème.
    fn signaler_css(&self, nom: &str) {
        let neuf = self.etat.borrow_mut().css_signales.insert(nom.to_owned());
        if neuf {
            (self.notifier)(&format!(
                "Le thème « {nom} » contient un theme.css : Fragment ne l'interprète pas. \
                 Les jetons se déclarent dans un theme.json."
            ));
        }
    }

    // ── Commandes ────────────────────────────────────────────────────────────

    /// `theme:list` : les thèmes du coffre, `*` devant l'actif.
    pub fn lister(&self) -> String {
        // La relecture annonce si la liste a changé, comme le fait la commande TS.
        let _ = self.relire_liste();
        let etat = self.etat.borrow();
        let actif = &etat.applique.actif;
        let mut lignes = vec![format!(
            "{} (défaut)",
            if actif.is_empty() { '*' } else { ' ' }
        )];
        for t in &etat.themes {
            lignes.push(format!("{} {t}", if t == actif { '*' } else { ' ' }));
        }
        lignes.join("\n")
    }

    /// `theme:set name=` : choisit le thème (`""` = celui de Fragment). Rend le message de la CLI.
    pub fn choisir(&self, nom: &str) -> Result<String, ErreurTheme> {
        let liste_changee = self.relire_sans_annoncer();
        if !nom.is_empty() {
            let etat = self.etat.borrow();
            if !nom_valide(nom) || !etat.themes.iter().any(|t| t == nom) {
                let erreur = ErreurTheme::Inconnu {
                    nom: nom.to_owned(),
                    installes: etat.themes.clone(),
                    dossier: dossier_des_themes(&self.config_dir),
                };
                drop(etat);
                if liste_changee {
                    self.annoncer();
                }
                return Err(erreur);
            }
        }
        self.apparence.set_theme_css(nom);
        // ★ Un seul réveil pour la liste ET le thème : l'écriture du réglage a déjà pu appliquer
        //   (via `lier_apparence`), `appliquer_sans_annoncer` rend alors `false`.
        let applique = self.appliquer_sans_annoncer();
        if liste_changee || applique {
            self.annoncer();
        }
        Ok(if nom.is_empty() {
            "thème : défaut".to_owned()
        } else {
            format!("thème : {nom}")
        })
    }

    // ── Temps réel ───────────────────────────────────────────────────────────

    /// Un changement du disque (chemin relatif au coffre) : programme le rechargement et/ou la
    /// relecture de la liste, regroupés.
    pub fn sur_evenement_brut(&self, chemin: &str) {
        let effet = effet_d_un_changement(chemin, &self.config_dir, &self.apparence.theme_css());
        let maintenant = (self.horloge)();
        if effet.recharger {
            self.rechargement.borrow_mut().call((), maintenant);
        }
        if effet.liste {
            self.relecture.borrow_mut().call((), maintenant);
        }
    }

    /// Fait ce qui est dû à l'heure de l'horloge. Rend `true` si la liste ou le thème actif a
    /// changé (les abonnés ont alors été prévenus, une seule fois).
    pub fn sonder(&self) -> bool {
        let maintenant = (self.horloge)();
        self.rechargement.borrow_mut().poll(maintenant);
        self.relecture.borrow_mut().poll(maintenant);
        let mut change = false;
        if self.relire_du.replace(false) {
            change |= self.relire_sans_annoncer();
        }
        if self.recharger_du.replace(false) {
            change |= self.appliquer_sans_annoncer();
        }
        if change {
            self.annoncer();
        }
        change
    }

    /// La prochaine échéance (ms de l'horloge), pour que l'hôte arme son minuteur.
    pub fn prochaine_echeance(&self) -> Option<u64> {
        let a = self.rechargement.borrow().next_deadline();
        let b = self.relecture.borrow().next_deadline();
        match (a, b) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (x, y) => x.or(y),
        }
    }

    /// S'abonne aux événements `raw` du coffre (le disque a bougé). L'abonnement vit tant que la
    /// référence rendue est gardée.
    pub fn lier_coffre(self: &Rc<Self>, coffre: &Vault) -> EventRef {
        let faible: Weak<Self> = Rc::downgrade(self);
        coffre.on(VaultEventType::Raw, move |e| {
            if let (Some(ce), VaultEvent::Raw { path }) = (faible.upgrade(), e) {
                ce.sur_evenement_brut(path);
            }
        })
    }

    /// Suit le réglage (`cssTheme`, accent) : appliqué sans attente quand `appearance.json` change.
    pub fn lier_apparence(self: &Rc<Self>) -> EventRef {
        let faible: Weak<Self> = Rc::downgrade(self);
        self.apparence.magasin().sur_changement(move |_| {
            if let Some(ce) = faible.upgrade() {
                ce.appliquer();
            }
        })
    }

    fn annoncer(&self) {
        // Copie : un abonné peut se désabonner pendant l'appel.
        let abonnes: Vec<Rc<dyn Fn()>> = self
            .abonnes
            .borrow()
            .iter()
            .map(|(_, f)| f.clone())
            .collect();
        for f in abonnes {
            f();
        }
    }
}

impl ThemeManager for ThemesCoffre {
    fn list(&self) -> Vec<String> {
        self.etat.borrow().themes.clone()
    }

    fn refresh(&self) -> io::Result<()> {
        self.relire_liste()
    }

    fn open_folder(&self) -> io::Result<()> {
        let dossier = dossier_des_themes(&self.config_dir);
        self.adaptateur.mkdir(&dossier)?;
        (self.ouvrir)(&dossier)
    }

    fn on_change(&self, cb: Box<dyn Fn()>) -> Desabonner {
        let id = self.prochain_abonne.get();
        self.prochain_abonne.set(id + 1);
        self.abonnes.borrow_mut().push((id, Rc::from(cb)));
        // ★ Le désabonnement ne garde pas `self` : il retire par id dans la liste partagée.
        let abonnes = self.abonnes.clone();
        Box::new(move || abonnes.borrow_mut().retain(|(i, _)| *i != id))
    }
}
