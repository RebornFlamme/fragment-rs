// Port de app/src/ui/views/pdf/PdfSidebar.ts (arbre de la table des matières)
//
// L'arbre n'a pas d'éléments : chaque entrée est adressée par son CHEMIN (les
// indices successifs depuis la racine). L'état se réduit à deux choses — les
// entrées dépliées et l'entrée active ; une entrée avec enfants naît repliée.

use std::collections::HashSet;

use crate::pdf_outline::EntreeToc;

/// Texte affiché quand le document n'a pas de table des matières.
pub const MESSAGE_SANS_TABLE: &str = "No table of contents";

/// Indices successifs depuis la racine : `[1, 0]` = premier enfant de la 2e entrée.
pub type Chemin = Vec<usize>;

/// Où mène un clic sur une entrée (le `y` est en coordonnées PDF, `None` = haut de page).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Destination {
    pub page: u32,
    pub y: Option<f64>,
}

/// Une ligne VISIBLE de l'arbre, dans l'ordre d'affichage.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneToc {
    pub chemin: Chemin,
    pub titre: String,
    /// Retrait : 0 pour une racine.
    pub profondeur: usize,
    /// Le numéro affiché à droite ; absent si la destination est introuvable.
    pub page: Option<u32>,
    pub repliable: bool,
    pub replie: bool,
    pub active: bool,
}

/// La table des matières et son état d'affichage.
#[derive(Debug, Clone, Default)]
pub struct ArbreToc {
    entrees: Vec<EntreeToc>,
    deplies: HashSet<Chemin>,
    active: Option<Chemin>,
}

impl ArbreToc {
    /// Remplace l'arbre : tout est replié, aucune entrée active.
    pub fn remplacer(&mut self, entrees: Vec<EntreeToc>) {
        self.entrees = entrees;
        self.deplies.clear();
        self.active = None;
    }

    pub fn est_vide(&self) -> bool {
        self.entrees.is_empty()
    }

    pub fn active(&self) -> Option<&Chemin> {
        self.active.as_ref()
    }

    /// L'entrée adressée par `chemin`.
    pub fn entree(&self, chemin: &[usize]) -> Option<&EntreeToc> {
        let (premier, reste) = chemin.split_first()?;
        let mut courante = self.entrees.get(*premier)?;
        for i in reste {
            courante = courante.enfants.get(*i)?;
        }
        Some(courante)
    }

    pub fn est_replie(&self, chemin: &[usize]) -> bool {
        self.entree(chemin)
            .is_some_and(|e| !e.enfants.is_empty() && !self.deplies.contains(chemin))
    }

    /// Les lignes visibles : les enfants d'une entrée repliée sont omis.
    pub fn lignes(&self) -> Vec<LigneToc> {
        let mut sortie = Vec::new();
        self.parcourir(&self.entrees, &mut Vec::new(), &mut sortie);
        sortie
    }

    fn parcourir(&self, liste: &[EntreeToc], chemin: &mut Chemin, sortie: &mut Vec<LigneToc>) {
        for (i, entree) in liste.iter().enumerate() {
            chemin.push(i);
            let repliable = !entree.enfants.is_empty();
            let replie = repliable && !self.deplies.contains(chemin);
            sortie.push(LigneToc {
                chemin: chemin.clone(),
                titre: entree.titre.clone(),
                profondeur: chemin.len() - 1,
                page: entree.page,
                repliable,
                replie,
                active: self.active.as_ref() == Some(chemin),
            });
            if repliable && !replie {
                self.parcourir(&entree.enfants, chemin, sortie);
            }
            chemin.pop();
        }
    }

    /// Le chevron : replie ou déplie. Sans effet sur une entrée sans enfants.
    pub fn basculer(&mut self, chemin: &[usize]) {
        if self.entree(chemin).is_none_or(|e| e.enfants.is_empty()) {
            return;
        }
        if !self.deplies.remove(chemin) {
            self.deplies.insert(chemin.to_vec());
        }
    }

    /// Un clic sur la ligne (hors chevron) : l'entrée devient active, et la vue va à sa page.
    pub fn cliquer(&mut self, chemin: &[usize]) -> Option<Destination> {
        let entree = self.entree(chemin)?;
        let (page, y) = (entree.page, entree.y);
        self.active = Some(chemin.to_vec());
        page.map(|page| Destination { page, y })
    }

    /// Le chemin de la DERNIÈRE entrée, dans l'ordre de lecture, qui commence au plus
    /// tard à `page` ; `None` si aucune ne précède la page.
    pub fn chemin_pour_page(&self, page: u32) -> Option<Chemin> {
        fn chercher(
            liste: &[EntreeToc],
            chemin: &mut Chemin,
            page: u32,
            meilleur: &mut Option<Chemin>,
        ) {
            for (i, entree) in liste.iter().enumerate() {
                chemin.push(i);
                if entree.page.is_some_and(|p| p <= page) {
                    *meilleur = Some(chemin.clone());
                }
                chercher(&entree.enfants, chemin, page, meilleur);
                chemin.pop();
            }
        }
        let mut meilleur = None;
        chercher(&self.entrees, &mut Vec::new(), page, &mut meilleur);
        meilleur
    }

    /// « Reveal page in table of contents » : déplie les ancêtres de l'entrée de la page
    /// et la rend active. Rend son chemin (la vue la centre), `None` si rien ne précède.
    pub fn reveler(&mut self, page: u32) -> Option<Chemin> {
        let chemin = self.chemin_pour_page(page)?;
        for n in 1..chemin.len() {
            self.deplies.insert(chemin[..n].to_vec());
        }
        self.active = Some(chemin.clone());
        Some(chemin)
    }
}
