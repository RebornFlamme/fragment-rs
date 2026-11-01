//! Petit utilitaire interne : une expression régulière compilée une fois, sans `unwrap`.
//!
//! ★ Les motifs sont des littéraux écrits ici et testés ; si l'un était invalide,
//!   `Motif` ne reconnaîtrait RIEN (au lieu de paniquer) et les tests du module
//!   concerné le diraient aussitôt.

use regex::Regex;

pub(crate) struct Motif(Option<Regex>);

impl Motif {
    pub(crate) fn new(motif: &str) -> Self {
        Motif(Regex::new(motif).ok())
    }

    pub(crate) fn is_match(&self, s: &str) -> bool {
        self.0.as_ref().is_some_and(|r| r.is_match(s))
    }

    pub(crate) fn captures<'t>(&self, s: &'t str) -> Option<regex::Captures<'t>> {
        self.0.as_ref().and_then(|r| r.captures(s))
    }

    pub(crate) fn regex(&self) -> Option<&Regex> {
        self.0.as_ref()
    }

    pub(crate) fn replace_all(&self, s: &str, rep: &str) -> String {
        match &self.0 {
            Some(r) => r.replace_all(s, rep).into_owned(),
            None => s.to_string(),
        }
    }
}
