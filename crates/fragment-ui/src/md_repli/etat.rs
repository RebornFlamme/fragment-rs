// Port de l'état du repli d'app/src/ui/views/markdown/decorationPlugin.ts : `hoveredLine`, `foldCtl`
// (état partagé chevron ↔ puce), `toggleFoldAt`, et du remappage de `foldState` aux éditions.
//! L'état de repli d'UN éditeur, côté interface : ce qui est replié, la ligne survolée, le contrôle
//! pressé ou survolé (chevron ou puce liée).
//!
//! Le modèle (plages repliables, remappage, lignes cachées) est celui de `fragment_markdown::blocs` ;
//! ce module y ajoute le survol et la traduction d'un changement de texte (octets) en éditions UTF-16.

use fragment_markdown::blocs::{
    AnalyseRepli, Edition, EtatRepli, chasser_curseur, plage_proprietes,
};
use fragment_markdown::syntaxe::{Arbre, Changement};
use fragment_text::utf16::{byte_to_utf16, utf16_len};

/// L'interaction en cours sur un contrôle de repli (`foldCtl`) : la ligne sous contrôle et si elle
/// est pressée. Chevron et puce de la même ligne la lisent tous les deux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controle {
    pub ligne: usize,
    pub actif: bool,
}

/// L'état visuel d'un chevron (ou de la puce liée).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    Aucune,
    /// `is-hover` : survolé, directement ou par la puce liée.
    Survol,
    /// `is-active` : pressé.
    Actif,
}

/// Un chevron à dessiner dans la gouttière.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chevron {
    pub ligne: usize,
    /// Replié : chevron vers la droite (`›`), couleur de lien. Sinon vers le bas (`⌄`).
    pub replie: bool,
    pub interaction: Interaction,
}

/// Le repli d'un éditeur.
#[derive(Debug, Clone)]
pub struct Repli {
    etat: EtatRepli,
    analyse: AnalyseRepli,
    survol: Option<usize>,
    controle: Option<Controle>,
}

impl Repli {
    #[must_use]
    pub fn new(texte: &str, arbre: &Arbre) -> Repli {
        Repli {
            etat: EtatRepli::default(),
            analyse: AnalyseRepli::new(texte, arbre),
            survol: None,
            controle: None,
        }
    }

    #[must_use]
    pub fn etat(&self) -> &EtatRepli {
        &self.etat
    }

    #[must_use]
    pub fn analyse(&self) -> &AnalyseRepli {
        &self.analyse
    }

    /// Le texte a changé de `ancien` à `nouveau` (arbre de `nouveau`) : les plis suivent l'édition,
    /// l'analyse est refaite.
    pub fn apres_changement(&mut self, ancien: &str, nouveau: &str, arbre: &Arbre) {
        if ancien != nouveau {
            let c = Changement::entre(ancien, nouveau);
            let e = Edition {
                de: byte_to_utf16(ancien, c.de),
                fin_ancien: byte_to_utf16(ancien, c.fin_ancien),
                fin_nouveau: byte_to_utf16(nouveau, c.fin_nouveau),
            };
            self.etat.remapper(&[e]);
        }
        self.analyse = AnalyseRepli::new(nouveau, arbre);
        // Les lignes ont pu bouger : le survol sera relu au prochain mouvement.
        self.controle = None;
    }

    /// Le clic sur le chevron (ou la puce liée) de `ligne` : `toggleFoldAt`. Rend vrai si l'état a
    /// changé.
    pub fn basculer(&mut self, ligne: usize) -> bool {
        let change = self.etat.basculer(&self.analyse, ligne);
        self.controle = Some(Controle { ligne, actif: true });
        change
    }

    /// Le bouton est relâché sur le contrôle (`mouseup`) : il reste survolé.
    pub fn relacher(&mut self) -> bool {
        match self.controle {
            Some(c) if c.actif => {
                self.controle = Some(Controle {
                    ligne: c.ligne,
                    actif: false,
                });
                true
            }
            _ => false,
        }
    }

    /// La souris entre sur le contrôle de `ligne` (`None` : elle en sort).
    pub fn survoler_controle(&mut self, ligne: Option<usize>) -> bool {
        let v = ligne.map(|ligne| Controle {
            ligne,
            actif: false,
        });
        let change = self.controle != v;
        self.controle = v;
        change
    }

    /// La ligne sous la souris (`hoveredLine`), `None` quand elle quitte l'éditeur. Comme le TS, un
    /// changement de ligne libère le contrôle (filet de sécurité d'un `mouseleave` manqué).
    pub fn survoler_ligne(&mut self, ligne: Option<usize>) -> bool {
        let mut change = self.survol != ligne;
        self.survol = ligne;
        if let Some(c) = self.controle
            && Some(c.ligne) != ligne
        {
            self.controle = None;
            change = true;
        }
        change
    }

    #[must_use]
    pub fn ligne_survolee(&self) -> Option<usize> {
        self.survol
    }

    /// L'interaction partagée de la ligne `ligne`.
    #[must_use]
    pub fn interaction(&self, ligne: usize) -> Interaction {
        match self.controle {
            Some(c) if c.ligne == ligne && c.actif => Interaction::Actif,
            Some(c) if c.ligne == ligne => Interaction::Survol,
            _ => Interaction::Aucune,
        }
    }

    /// Le chevron de `ligne`, s'il est visible : ligne repliée (toujours) ou survolée ET repliable.
    #[must_use]
    pub fn chevron(&self, ligne: usize) -> Option<Chevron> {
        let replie = self.etat.ligne_repliee(&self.analyse, ligne);
        let visible = replie || (self.survol == Some(ligne) && self.analyse.plage(ligne).is_some());
        visible.then(|| Chevron {
            ligne,
            replie,
            interaction: self.interaction(ligne),
        })
    }

    /// Où une sélection vide en `pos` (UTF-16) doit atterrir dans `texte` : hors du bloc Properties
    /// et hors des plis (`transactionFilter`).
    #[must_use]
    pub fn chasser(&self, texte: &str, pos: usize) -> usize {
        let proprietes = plage_proprietes(texte).map(|p| p.utf16);
        chasser_curseur(pos, utf16_len(texte), proprietes, &self.etat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::syntaxe::analyser;

    const DOC: &str = "## A\ntexte A\n### A1\ntexte A1\n## B\ntexte B";

    #[test]
    fn chevron_seulement_au_survol_ou_replie() {
        let mut r = Repli::new(DOC, &analyser(DOC));
        assert_eq!(r.chevron(0), None);
        r.survoler_ligne(Some(0));
        assert_eq!(
            r.chevron(0),
            Some(Chevron {
                ligne: 0,
                replie: false,
                interaction: Interaction::Aucune
            })
        );
        // Une ligne non repliable n'a pas de chevron, même survolée.
        r.survoler_ligne(Some(1));
        assert_eq!(r.chevron(1), None);
        r.basculer(0);
        r.survoler_ligne(None);
        assert!(r.chevron(0).is_some_and(|c| c.replie));
    }

    #[test]
    fn le_controle_est_partage_et_libere_au_changement_de_ligne() {
        let mut r = Repli::new(DOC, &analyser(DOC));
        r.survoler_ligne(Some(0));
        r.survoler_controle(Some(0));
        assert_eq!(r.interaction(0), Interaction::Survol);
        r.basculer(0);
        assert_eq!(r.interaction(0), Interaction::Actif);
        r.relacher();
        assert_eq!(r.interaction(0), Interaction::Survol);
        r.survoler_ligne(Some(4));
        assert_eq!(r.interaction(0), Interaction::Aucune);
    }

    #[test]
    fn les_plis_suivent_une_frappe_au_dessus() {
        let mut r = Repli::new(DOC, &analyser(DOC));
        r.basculer(2);
        let avant = r.etat().plis().to_vec();
        let nouveau = format!("x{DOC}");
        r.apres_changement(DOC, &nouveau, &analyser(&nouveau));
        let apres = r.etat().plis().to_vec();
        assert_eq!(apres.len(), 1);
        assert_eq!(apres[0].from, avant[0].from + 1);
        assert!(r.etat().ligne_repliee(r.analyse(), 2));
    }

    #[test]
    fn le_curseur_ne_reste_pas_dans_un_pli() {
        let mut r = Repli::new(DOC, &analyser(DOC));
        r.basculer(0);
        // « texte A1 » est caché : le curseur remonte au bout de « ## A ».
        let pos = DOC.find("texte A1").unwrap();
        assert_eq!(r.chasser(DOC, pos), 4);
    }
}
