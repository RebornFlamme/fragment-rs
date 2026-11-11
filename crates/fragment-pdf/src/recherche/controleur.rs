// Port de app/src/ui/views/pdf/PdfFindBar.ts (état `find`, compteur, fermeture) et du
// contrôleur de recherche de pdf.js (web/pdf_find_controller, Apache-2.0) : la
// correspondance courante, la navigation cyclique et le compte « n of m ».
//
// Le contrôleur d'origine est asynchrone (le texte des pages arrive page à page, la
// frappe est temporisée). Ici toutes les pages sont fournies d'avance et chaque appel
// rend son résultat sur-le-champ : la temporisation de la frappe est l'affaire de la
// vue ([`DELAI_FRAPPE_MS`]) et l'état « en attente » n'existe que comme valeur.
//
// ★ Les offsets rendus sont dans le texte BRUT de la page (concaténation des chaînes
//   des items, sauts de ligne compensés), pas dans notre couche : voir
//   `crate::pdf_find_offsets` pour les ramener.

use super::correspondance::{OptionsRecherche, trouver};
use super::normalisation::{Diffs, index_d_origine, normaliser};

/// Délai que la vue laisse passer après une frappe avant de lancer une recherche de
/// type [`TypeRecherche::Nouvelle`] (celui du contrôleur d'origine).
pub const DELAI_FRAPPE_MS: u64 = 250;

/// Un item de texte d'une page, tel que le moteur le livre (texte brut, sans normalisation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTexte {
    /// La chaîne de l'item.
    pub texte: String,
    /// L'item termine une ligne (le contrôleur y ajoute un saut de ligne).
    pub fin_de_ligne: bool,
}

impl ItemTexte {
    /// Un item sans fin de ligne.
    pub fn new(texte: &str) -> Self {
        Self {
            texte: texte.to_string(),
            fin_de_ligne: false,
        }
    }
}

/// Le texte d'une page, prêt à être cherché.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRecherche {
    contenu: String,
    diffs: Option<Diffs>,
    a_diacritiques: bool,
}

impl PageRecherche {
    /// Une page sans texte (ou dont l'extraction a échoué).
    pub fn vide() -> Self {
        Self {
            contenu: String::new(),
            diffs: None,
            a_diacritiques: false,
        }
    }

    /// Une page à partir de ses items : leurs chaînes mises bout à bout, un saut de
    /// ligne après chaque fin de ligne, puis la normalisation.
    pub fn depuis_items(items: &[ItemTexte]) -> Self {
        let mut brut = String::new();
        for item in items {
            brut.push_str(&item.texte);
            if item.fin_de_ligne {
                brut.push('\n');
            }
        }
        Self::depuis_texte_brut(&brut)
    }

    /// Une page à partir de son texte brut déjà assemblé.
    pub fn depuis_texte_brut(brut: &str) -> Self {
        let n = normaliser(brut);
        Self {
            contenu: n.texte,
            diffs: Some(n.diffs),
            a_diacritiques: n.a_diacritiques,
        }
    }

    /// Le texte normalisé, celui où l'on cherche.
    pub fn contenu(&self) -> &str {
        &self.contenu
    }
}

/// Pourquoi la barre relance (le `type` de l'événement `find` du TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRecherche {
    /// Nouvelle requête (la vue temporise la frappe).
    Nouvelle,
    /// Suivant / précédent.
    Encore,
    /// Aa a changé.
    ChangementCasse,
    /// « Whole words » a changé.
    ChangementMotEntier,
    /// « Match diacritics » a changé.
    ChangementDiacritiques,
    /// « Highlight all » a changé.
    ChangementToutSurligner,
}

/// L'état `find` complet que la barre envoie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatRecherche {
    /// Pourquoi on relance.
    pub type_: TypeRecherche,
    /// Le contenu du champ.
    pub requete: String,
    /// Maj+Entrée : aller vers l'arrière.
    pub vers_arriere: bool,
    /// Les réglages de la barre.
    pub options: OptionsRecherche,
}

/// `FindState` de pdf.js.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatTrouve {
    /// Une occurrence est sélectionnée.
    Trouve,
    /// Aucune occurrence dans le document.
    NonTrouve,
    /// La recherche a bouclé (dernière page → première, ou l'inverse).
    Boucle,
    /// Les pages ne sont pas toutes lues (jamais rendu ici : le modèle est synchrone).
    EnAttente,
}

/// Une occurrence : page (à partir de 0), début et longueur en unités UTF-16 du texte brut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correspondance {
    /// Index de la page (0 = première).
    pub page: usize,
    /// Début dans le texte brut de la page.
    pub index: usize,
    /// Longueur dans le texte brut de la page.
    pub longueur: usize,
}

/// Un surlignage à poser sur une page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surlignage {
    /// Début dans le texte brut de la page.
    pub index: usize,
    /// Longueur dans le texte brut de la page.
    pub longueur: usize,
    /// C'est l'occurrence courante.
    pub selectionne: bool,
}

/// Ce que la barre affiche après une recherche (`updatefindcontrolstate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultatRecherche {
    /// L'état de la recherche.
    pub etat: EtatTrouve,
    /// La recherche allait vers l'arrière.
    pub precedent: bool,
    /// Rang de l'occurrence courante (à partir de 1), 0 si aucune.
    pub courant: usize,
    /// Nombre d'occurrences dans le document.
    pub total: usize,
    /// La page (à partir de 1) à montrer : celle de l'occurrence courante.
    pub page_cible: Option<usize>,
}

impl ResultatRecherche {
    /// Le texte du compteur de la barre : « n of m », « No results » ou « … », vide si
    /// le champ est vide.
    pub fn texte(&self, requete: &str) -> String {
        texte_compteur(requete, self.etat, self.courant, self.total)
    }

    /// La barre passe-t-elle en rouge (aucun résultat) ?
    pub fn aucun_resultat(&self, requete: &str) -> bool {
        self.etat == EtatTrouve::NonTrouve && !self.texte(requete).is_empty()
    }
}

/// Le compteur tel que la barre l'écrit (`majCompteur` du TS).
pub fn texte_compteur(requete: &str, etat: EtatTrouve, courant: usize, total: usize) -> String {
    if requete.trim().is_empty() {
        String::new()
    } else if etat == EtatTrouve::NonTrouve {
        "No results".to_string()
    } else if total == 0 {
        "…".to_string()
    } else {
        format!("{courant} of {total}")
    }
}

/// Où en est la navigation : la page et l'occurrence où l'on est passé.
#[derive(Debug, Clone, Copy)]
struct Curseur {
    page: usize,
    occurrence: Option<usize>,
    boucle: bool,
}

/// La recherche d'un document : ses pages, la requête, l'occurrence courante.
#[derive(Debug, Clone)]
pub struct Recherche {
    pages: Vec<PageRecherche>,
    etat: Option<EtatRecherche>,
    /// Occurrences par page, en offsets du texte brut.
    occurrences: Vec<Vec<(usize, usize)>>,
    total: usize,
    /// Les résultats ne valent plus : la prochaine recherche les recalcule.
    perime: bool,
    selection: Option<(usize, usize)>,
    curseur: Curseur,
    pages_a_chercher: i64,
    surligner: bool,
    dernier: ResultatRecherche,
}

impl Recherche {
    /// La recherche d'un document dont on connaît le texte de toutes les pages.
    pub fn new(pages: Vec<PageRecherche>) -> Self {
        Self {
            pages,
            etat: None,
            occurrences: Vec::new(),
            total: 0,
            perime: false,
            selection: None,
            curseur: Curseur {
                page: 0,
                occurrence: None,
                boucle: false,
            },
            pages_a_chercher: 0,
            surligner: false,
            dernier: ResultatRecherche {
                etat: EtatTrouve::Trouve,
                precedent: false,
                courant: 0,
                total: 0,
                page_cible: None,
            },
        }
    }

    /// Les occurrences (page, rang) sélectionnée, si elle existe — pages et rang à partir de 0.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// Les occurrences d'une page, en offsets du texte brut.
    pub fn occurrences_page(&self, page: usize) -> &[(usize, usize)] {
        self.occurrences.get(page).map_or(&[], Vec::as_slice)
    }

    /// Le dernier résultat rendu.
    pub fn dernier(&self) -> ResultatRecherche {
        self.dernier
    }

    /// Les surlignages sont-ils affichés ? (faux après [`Self::fermer`])
    pub fn surligne(&self) -> bool {
        self.surligner
    }

    /// Le compte courant : (rang de l'occurrence courante à partir de 1, total).
    pub fn compteur(&self) -> (usize, usize) {
        let mut courant = 0;
        let total = self.total;
        if let Some((page, rang)) = self.selection {
            courant = self.occurrences[..page.min(self.occurrences.len())]
                .iter()
                .map(Vec::len)
                .sum::<usize>()
                + rang
                + 1;
        }
        if courant < 1 || courant > total {
            (0, 0)
        } else {
            (courant, total)
        }
    }

    /// Les surlignages d'une page : toutes ses occurrences avec « Highlight all », la
    /// courante seule sinon ; aucun une fois la barre fermée.
    pub fn surlignages_page(&self, page: usize) -> Vec<Surlignage> {
        if !self.surligner {
            return Vec::new();
        }
        let tout = self.etat.as_ref().is_some_and(|e| e.options.tout_surligner);
        let courante = self.selection.filter(|(p, _)| *p == page).map(|(_, r)| r);
        self.occurrences_page(page)
            .iter()
            .enumerate()
            .filter(|(i, _)| tout || Some(*i) == courante)
            .map(|(i, (index, longueur))| Surlignage {
                index: *index,
                longueur: *longueur,
                selectionne: Some(i) == courante,
            })
            .collect()
    }

    /// Lance ou relance la recherche (l'événement `find` du TS).
    ///
    /// - `page_courante` : la page affichée, à partir de 1 ; une nouvelle requête part de là ;
    /// - `page_visible` : la page (à partir de 1) est-elle à l'écran ? Passer d'une
    ///   occurrence à l'autre depuis une page que le lecteur a quittée repart de la page
    ///   affichée.
    pub fn chercher(
        &mut self,
        etat: EtatRecherche,
        page_courante: usize,
        page_visible: &dyn Fn(usize) -> bool,
    ) -> ResultatRecherche {
        let sale = self.etat.is_none() || self.doit_recalculer(&etat, page_courante, page_visible);
        if sale {
            self.perime = true;
        }
        let type_ = etat.type_;
        self.etat = Some(etat);
        match type_ {
            TypeRecherche::ChangementToutSurligner if !self.perime => {
                // Seul le surlignage change : rien à recalculer ni à sélectionner.
                self.surligner = true;
                self.dernier
            }
            _ => self.occurrence_suivante(page_courante),
        }
    }

    /// Les résultats actuels valent-ils encore pour ce nouvel état ? (`#shouldDirtyMatch`)
    fn doit_recalculer(
        &self,
        nouveau: &EtatRecherche,
        page_courante: usize,
        page_visible: &dyn Fn(usize) -> bool,
    ) -> bool {
        let Some(ancien) = &self.etat else {
            return true;
        };
        if nouveau.requete != ancien.requete {
            return true;
        }
        match nouveau.type_ {
            TypeRecherche::Encore => self.selection.is_some_and(|(page, _)| {
                let numero = page + 1;
                numero >= 1
                    && numero <= self.pages.len()
                    && numero != page_courante
                    && !page_visible(numero)
            }),
            TypeRecherche::ChangementToutSurligner => false,
            _ => true,
        }
    }

    /// La barre se ferme : les surlignages disparaissent (l'événement `findbarclose`).
    pub fn fermer(&mut self) -> ResultatRecherche {
        self.surligner = false;
        self.dernier = self.resultat(EtatTrouve::Trouve, false, None);
        self.dernier
    }

    fn resultat(
        &self,
        etat: EtatTrouve,
        precedent: bool,
        page_cible: Option<usize>,
    ) -> ResultatRecherche {
        let (courant, total) = self.compteur();
        ResultatRecherche {
            etat,
            precedent,
            courant,
            total,
            page_cible,
        }
    }

    /// Recalcule les occurrences de toutes les pages (`#calculateMatch`).
    fn calculer(&mut self) {
        let Some(etat) = &self.etat else {
            return;
        };
        let requete = normaliser(&etat.requete).texte;
        self.occurrences = self
            .pages
            .iter()
            .map(|page| {
                if requete.is_empty() {
                    return Vec::new();
                }
                trouver(&requete, &page.contenu, page.a_diacritiques, etat.options)
                    .into_iter()
                    .filter_map(|(index, longueur)| match &page.diffs {
                        Some(diffs) => index_d_origine(diffs, index, longueur),
                        None => Some((index, longueur)),
                    })
                    .collect()
            })
            .collect();
        self.total = self.occurrences.iter().map(Vec::len).sum();
    }

    /// `#nextMatch` : l'occurrence suivante (ou précédente), en bouclant.
    fn occurrence_suivante(&mut self, page_courante: usize) -> ResultatRecherche {
        let Some(etat) = self.etat.as_ref() else {
            return self.dernier;
        };
        let precedent = etat.vers_arriere;
        let vide = normaliser(&etat.requete).texte.is_empty();
        let nb_pages = self.pages.len();
        self.surligner = true;
        if self.perime {
            self.perime = false;
            self.selection = None;
            self.curseur = Curseur {
                page: page_courante.saturating_sub(1),
                occurrence: None,
                boucle: false,
            };
            self.calculer();
        }
        if vide || nb_pages == 0 {
            self.dernier = self.resultat(
                if vide {
                    EtatTrouve::Trouve
                } else {
                    EtatTrouve::NonTrouve
                },
                false,
                None,
            );
            return self.dernier;
        }
        self.pages_a_chercher = nb_pages as i64;
        if let Some(occ) = self.curseur.occurrence {
            let nb = self.occurrences_page(self.curseur.page).len();
            if (!precedent && occ + 1 < nb) || (precedent && occ > 0) {
                self.curseur.occurrence = Some(if precedent { occ - 1 } else { occ + 1 });
                return self.mettre_a_jour(true, precedent);
            }
            self.avancer_page(precedent);
        }
        loop {
            let page = self.curseur.page;
            let nb = self.occurrences_page(page).len();
            if nb > 0 {
                self.curseur.occurrence = Some(if precedent { nb - 1 } else { 0 });
                return self.mettre_a_jour(true, precedent);
            }
            self.avancer_page(precedent);
            if self.curseur.boucle {
                self.curseur.occurrence = None;
                if self.pages_a_chercher < 0 {
                    return self.mettre_a_jour(false, precedent);
                }
            }
        }
    }

    /// `#advanceOffsetPage` : la page suivante, ou la première après la dernière.
    fn avancer_page(&mut self, precedent: bool) {
        let nb_pages = self.pages.len();
        self.curseur.occurrence = None;
        self.pages_a_chercher -= 1;
        if precedent {
            if self.curseur.page == 0 {
                self.curseur.page = nb_pages - 1;
                self.curseur.boucle = true;
            } else {
                self.curseur.page -= 1;
            }
        } else {
            self.curseur.page += 1;
            if self.curseur.page >= nb_pages {
                self.curseur.page = 0;
                self.curseur.boucle = true;
            }
        }
    }

    /// `#updateMatch` : retient l'occurrence trouvée (ou l'absence de résultat).
    fn mettre_a_jour(&mut self, trouve: bool, precedent: bool) -> ResultatRecherche {
        let boucle = self.curseur.boucle;
        self.curseur.boucle = false;
        let mut etat = EtatTrouve::NonTrouve;
        if trouve && let Some(occ) = self.curseur.occurrence {
            self.selection = Some((self.curseur.page, occ));
            etat = if boucle {
                EtatTrouve::Boucle
            } else {
                EtatTrouve::Trouve
            };
        }
        let cible = self.selection.map(|(p, _)| p + 1);
        self.dernier = self.resultat(etat, precedent, cible);
        self.dernier
    }
}
