//! Le FAUX processus `claude` : il rejoue un transcript enregistré, sans jamais lancer le binaire.
//!
//! Format d'une fixture (une ligne par entrée, réécrite depuis les transcripts de S9) :
//! - `> {json}` : ce que le harness écrit (le faux attend qu'il l'ait écrit pour continuer) ;
//! - `< {json}` : ce que le binaire répond ;
//! - `! texte` : une ligne de stderr ;
//! - `= n` : le code de sortie.
//!
//! Le faux ne compare pas ce qu'il reçoit : il le GARDE ([`FauxLanceur::ecrits`]), et les tests
//! l'examinent. Il garde aussi chaque [`Lancement`] (arguments, environnement, dossier).
//! ★ S'il attend une écriture qui ne vient pas, il rend la main (`Fin`) au lieu de bloquer le test.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use crate::harness_claude::processus::{
    ErreurProcessus, FIN_STDERR, Lancement, Lanceur, Lecture, Processus, derniers_caracteres,
};

/// Une entrée de fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entree {
    Ecrit,
    Lu(String),
}

/// Une fixture lue.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fixture {
    entrees: Vec<Entree>,
    stderr: String,
    code: Option<i32>,
}

impl Fixture {
    /// Lit le texte d'une fixture (voir le format du module).
    pub fn lire(texte: &str) -> Self {
        let mut f = Fixture::default();
        for l in texte.lines() {
            if let Some(json) = l.strip_prefix("< ") {
                f.entrees.push(Entree::Lu(json.to_string()));
            } else if l.starts_with('>') {
                f.entrees.push(Entree::Ecrit);
            } else if let Some(err) = l.strip_prefix("! ") {
                f.stderr.push_str(err);
                f.stderr.push('\n');
            } else if let Some(code) = l.strip_prefix("= ") {
                f.code = code.trim().parse().ok();
            }
        }
        f
    }

    /// Les lignes que le binaire écrit, dans l'ordre (sans les attentes).
    pub fn sorties(&self) -> Vec<&str> {
        self.entrees
            .iter()
            .filter_map(|e| match e {
                Entree::Lu(l) => Some(l.as_str()),
                Entree::Ecrit => None,
            })
            .collect()
    }
}

#[derive(Default)]
struct Etat {
    fixtures: VecDeque<Fixture>,
    lancements: Vec<Lancement>,
    ecrits: Vec<String>,
    erreur_de_lancement: Option<String>,
}

/// Le lanceur de test : une fixture par lancement, dans l'ordre.
#[derive(Clone, Default)]
pub struct FauxLanceur(Rc<RefCell<Etat>>);

impl FauxLanceur {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute la fixture du prochain lancement.
    pub fn rejouer(&self, texte: &str) -> &Self {
        self.0.borrow_mut().fixtures.push_back(Fixture::lire(texte));
        self
    }

    /// Le prochain lancement échoue (binaire absent).
    pub fn echouer(&self, cause: &str) {
        self.0.borrow_mut().erreur_de_lancement = Some(cause.to_string());
    }

    /// Les lancements reçus.
    pub fn lancements(&self) -> Vec<Lancement> {
        self.0.borrow().lancements.clone()
    }

    /// Tout ce que le harness a écrit sur stdin, tous lancements confondus.
    pub fn ecrits(&self) -> Vec<String> {
        self.0.borrow().ecrits.clone()
    }
}

impl Lanceur for FauxLanceur {
    fn lancer(&self, l: &Lancement) -> Result<Box<dyn Processus>, ErreurProcessus> {
        let mut e = self.0.borrow_mut();
        if let Some(cause) = e.erreur_de_lancement.take() {
            return Err(ErreurProcessus::Lancement {
                programme: l.programme.display().to_string(),
                cause,
            });
        }
        e.lancements.push(l.clone());
        let f = e.fixtures.pop_front().unwrap_or_default();
        Ok(Box::new(FauxProcessus {
            etat: Rc::clone(&self.0),
            restant: f.entrees.into_iter().collect(),
            stderr: f.stderr,
            code: f.code,
            ecrits: 0,
            consommes: 0,
            entree_fermee: false,
            attentes: 0,
        }))
    }
}

/// Au-delà de ce nombre de lectures vides d'affilée, le faux abandonne (`Fin`).
const ATTENTES_MAX: u32 = 2000;

struct FauxProcessus {
    etat: Rc<RefCell<Etat>>,
    restant: VecDeque<Entree>,
    stderr: String,
    code: Option<i32>,
    /// Les lignes écrites par le harness dans CE processus.
    ecrits: usize,
    /// Les attentes `>` déjà satisfaites.
    consommes: usize,
    entree_fermee: bool,
    attentes: u32,
}

impl Processus for FauxProcessus {
    fn ecrire(&mut self, ligne: &str) -> Result<(), ErreurProcessus> {
        if self.entree_fermee {
            return Err(ErreurProcessus::EntreeFermee);
        }
        self.etat.borrow_mut().ecrits.push(ligne.to_string());
        self.ecrits += 1;
        Ok(())
    }

    fn lire(&mut self, _delai: Duration) -> Lecture {
        loop {
            match self.restant.front() {
                None => return Lecture::Fin,
                Some(Entree::Lu(_)) => {
                    self.attentes = 0;
                    if let Some(Entree::Lu(l)) = self.restant.pop_front() {
                        return Lecture::Ligne(l);
                    }
                }
                Some(Entree::Ecrit) => {
                    if self.ecrits > self.consommes {
                        self.consommes += 1;
                        self.restant.pop_front();
                        continue;
                    }
                    self.attentes += 1;
                    if self.attentes > ATTENTES_MAX || self.entree_fermee {
                        self.restant.clear();
                        return Lecture::Fin;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    return Lecture::Rien;
                }
            }
        }
    }

    fn fermer_entree(&mut self) {
        self.entree_fermee = true;
    }

    fn attendre(&mut self, _delai: Duration) -> Option<i32> {
        self.code
    }

    fn fin_stderr(&self) -> String {
        derniers_caracteres(&self.stderr, FIN_STDERR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn lancement() -> Lancement {
        Lancement {
            programme: PathBuf::from("claude"),
            arguments: vec![],
            cwd: PathBuf::from("."),
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn le_faux_attend_ce_que_le_harness_doit_ecrire() {
        let l = FauxLanceur::new();
        l.rejouer("> {}\n< {\"a\":1}\n! oups\n= 3\n");
        let mut p = l.lancer(&lancement()).unwrap();
        assert_eq!(p.lire(Duration::ZERO), Lecture::Rien);
        p.ecrire("{}").unwrap();
        assert_eq!(p.lire(Duration::ZERO), Lecture::Ligne("{\"a\":1}".into()));
        assert_eq!(p.lire(Duration::ZERO), Lecture::Fin);
        assert_eq!(p.attendre(Duration::ZERO), Some(3));
        assert_eq!(p.fin_stderr(), "oups\n");
        assert_eq!(l.ecrits(), ["{}"]);
    }
}
