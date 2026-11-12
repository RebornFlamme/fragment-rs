// Port de app/src/ui/views/pdf/PdfFindBar.ts (état de la barre, options, compteur, menu des réglages)
//
//! Le modèle de la barre de recherche PDF : pas de GPUI, donc testable sans fenêtre.
//!
//! ★ La barre ne cherche pas : la correspondance (casse, mots entiers, accents, ligatures, coupures
//!   de fin de ligne) est celle de [`fragment_pdf::recherche::Recherche`] ; ce modèle n'est que
//!   l'interface — le champ, les réglages, le compteur « n of m » — et l'envoi des événements
//!   `find`. C'est le partage de pdf.js entre sa barre et son contrôleur.
//!
//! Le menu des réglages ([`Menu`]) dépose ses choix dans une file partagée : ses rappels n'ont pas
//! accès au modèle (ils vivent plus longtemps qu'un emprunt). La vue appelle
//! [`BarreRecherche::traiter`] ensuite, qui rejoue la file et rend les types de recherche à
//! relancer.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_pdf::recherche::{
    EtatRecherche, EtatTrouve, OptionsRecherche, PageRecherche, Recherche, ResultatRecherche,
    Surlignage, TypeRecherche,
};

use crate::menu::Menu;

/// Les libellés (`aria-label`) des contrôles de la barre : le contrat que vérifie la spec e2e.
pub const LIBELLES: [&str; 5] = [
    "Match case",
    "Previous match",
    "Next match",
    "Search settings",
    "Close",
];

/// Un réglage du menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reglage {
    ToutSurligner,
    Diacritiques,
    MotEntier,
}

type File = Rc<RefCell<Vec<Reglage>>>;

/// Le résultat d'une recherche vide : rien trouvé, rien à montrer, pas d'erreur.
fn resultat_vide() -> ResultatRecherche {
    ResultatRecherche {
        etat: EtatTrouve::Trouve,
        precedent: false,
        courant: 0,
        total: 0,
        page_cible: None,
    }
}

/// L'état de la barre de recherche.
pub struct BarreRecherche {
    ouverte: bool,
    valeur: String,
    /// Tout le contenu du champ est sélectionné (Ctrl+F, ouverture) : la prochaine frappe le
    /// remplace.
    tout_selectionne: bool,
    options: OptionsRecherche,
    recherche: Option<Recherche>,
    resultat: ResultatRecherche,
    /// Le menu des réglages est ouvert (remis à zéro par `on_hide`, d'où le partage).
    menu_ouvert: Rc<Cell<bool>>,
    file: File,
}

impl Default for BarreRecherche {
    fn default() -> Self {
        Self::new()
    }
}

impl BarreRecherche {
    /// Une barre fermée, vide, sans document.
    pub fn new() -> Self {
        Self {
            ouverte: false,
            valeur: String::new(),
            tout_selectionne: false,
            options: OptionsRecherche::default(),
            recherche: None,
            resultat: resultat_vide(),
            menu_ouvert: Rc::new(Cell::new(false)),
            file: Rc::new(RefCell::new(Vec::new())),
        }
    }

    // ── Ouverture / fermeture ────────────────────────────────────────────

    /// La barre est-elle ouverte ?
    pub fn ouverte(&self) -> bool {
        self.ouverte
    }

    /// Ctrl+F : ouvre la barre (ou re-sélectionne le champ). Rend vrai si la barre vient de
    /// s'ouvrir avec une requête à relancer.
    pub fn ouvrir(&mut self) -> bool {
        let neuve = !self.ouverte;
        self.ouverte = true;
        self.tout_selectionne = true;
        neuve && !self.valeur.trim().is_empty()
    }

    /// Échap / ✕ : ferme la barre ; les surlignages disparaissent (`findbarclose`).
    pub fn fermer(&mut self) {
        if !self.ouverte {
            return;
        }
        self.ouverte = false;
        self.tout_selectionne = false;
        if let Some(r) = self.recherche.as_mut() {
            self.resultat = r.fermer();
        }
    }

    // ── Le champ ─────────────────────────────────────────────────────────

    /// Le contenu du champ.
    pub fn valeur(&self) -> &str {
        &self.valeur
    }

    /// Le contenu est-il entièrement sélectionné ?
    pub fn tout_selectionne(&self) -> bool {
        self.tout_selectionne
    }

    /// Sélectionne tout le champ (Ctrl+F dans le champ).
    pub fn selectionner_tout(&mut self) {
        self.tout_selectionne = true;
    }

    /// Une requête non vide (`trim`) est-elle saisie ?
    pub fn a_une_requete(&self) -> bool {
        !self.valeur.trim().is_empty()
    }

    /// Du texte tapé : remplace la sélection ou s'ajoute. Rend vrai si le contenu a changé.
    pub fn saisir(&mut self, texte: &str) -> bool {
        if texte.is_empty() || texte.chars().any(char::is_control) {
            return false;
        }
        if self.tout_selectionne {
            self.valeur.clear();
            self.tout_selectionne = false;
        }
        self.valeur.push_str(texte);
        true
    }

    /// Retour arrière. Rend vrai si le contenu a changé.
    pub fn effacer(&mut self) -> bool {
        let avant = self.valeur.len();
        if self.tout_selectionne {
            self.valeur.clear();
            self.tout_selectionne = false;
        } else {
            self.valeur.pop();
        }
        self.valeur.len() != avant
    }

    // ── Réglages ─────────────────────────────────────────────────────────

    /// Les réglages en vigueur.
    pub fn options(&self) -> OptionsRecherche {
        self.options
    }

    /// Applique des réglages restaurés (état éphémère).
    pub fn definir_options(&mut self, options: OptionsRecherche) {
        self.options = options;
    }

    /// Aa : le bouton de casse est-il actif ?
    pub fn casse_active(&self) -> bool {
        self.options.casse
    }

    /// Aa : bascule la casse. Rend le type de recherche à relancer.
    pub fn basculer_casse(&mut self) -> TypeRecherche {
        self.options.casse = !self.options.casse;
        TypeRecherche::ChangementCasse
    }

    /// Le menu du bouton « réglages » est-il ouvert ?
    pub fn menu_ouvert(&self) -> bool {
        self.menu_ouvert.get()
    }

    fn empiler(&self, reglage: Reglage) -> impl Fn() + 'static {
        let file = self.file.clone();
        move || file.borrow_mut().push(reglage)
    }

    /// Le menu des réglages : trois entrées cochables, dans l'ordre de la barre d'origine.
    pub fn menu_options(&self) -> Menu {
        let mut menu = Menu::new();
        for (titre, reglage, coche) in [
            (
                "Highlight all",
                Reglage::ToutSurligner,
                self.options.tout_surligner,
            ),
            (
                "Match diacritics",
                Reglage::Diacritiques,
                self.options.diacritiques,
            ),
            ("Whole words", Reglage::MotEntier, self.options.mot_entier),
        ] {
            let action = self.empiler(reglage);
            menu.add_item(|i| {
                i.set_title(titre).set_checked(coche).on_click(action);
            });
        }
        let ouvert = self.menu_ouvert.clone();
        ouvert.set(true);
        menu.on_hide(move || ouvert.set(false));
        menu
    }

    /// Rejoue les choix du menu : chaque réglage change, et on rend le type de recherche à relancer
    /// pour chacun (un par choix, dans l'ordre).
    pub fn traiter(&mut self) -> Vec<TypeRecherche> {
        let choix: Vec<Reglage> = self.file.borrow_mut().drain(..).collect();
        choix
            .into_iter()
            .map(|r| match r {
                Reglage::ToutSurligner => {
                    self.options.tout_surligner = !self.options.tout_surligner;
                    TypeRecherche::ChangementToutSurligner
                }
                Reglage::Diacritiques => {
                    self.options.diacritiques = !self.options.diacritiques;
                    TypeRecherche::ChangementDiacritiques
                }
                Reglage::MotEntier => {
                    self.options.mot_entier = !self.options.mot_entier;
                    TypeRecherche::ChangementMotEntier
                }
            })
            .collect()
    }

    // ── Recherche ────────────────────────────────────────────────────────

    /// Le texte de toutes les pages est lu : la barre peut chercher. Remplace la recherche d'un
    /// document précédent.
    pub fn charger(&mut self, pages: Vec<PageRecherche>) {
        self.recherche = Some(Recherche::new(pages));
        self.resultat = resultat_vide();
    }

    /// Oublie le document (le texte va changer).
    pub fn decharger(&mut self) {
        self.recherche = None;
        self.resultat = resultat_vide();
    }

    /// Le texte du document est-il chargé ?
    pub fn a_un_document(&self) -> bool {
        self.recherche.is_some()
    }

    /// Lance ou relance la recherche (l'événement `find`). `page_courante` : la page affichée (à
    /// partir de 1) ; `page_visible` : la page (à partir de 1) est-elle à l'écran ?
    ///
    /// `None` si la barre est fermée ou si le document n'est pas encore lu.
    pub fn chercher(
        &mut self,
        type_: TypeRecherche,
        vers_arriere: bool,
        page_courante: usize,
        page_visible: &dyn Fn(usize) -> bool,
    ) -> Option<ResultatRecherche> {
        if !self.ouverte {
            return None;
        }
        let etat = EtatRecherche {
            type_,
            requete: self.valeur.clone(),
            vers_arriere,
            options: self.options,
        };
        let recherche = self.recherche.as_mut()?;
        let mut resultat = recherche.chercher(etat, page_courante, page_visible);
        // Un champ vide n'a pas de résultat à montrer (le compteur repart à zéro).
        if self.valeur.trim().is_empty() {
            resultat = resultat_vide();
        }
        self.resultat = resultat;
        Some(resultat)
    }

    /// ↑ / ↓ (Maj+Entrée / Entrée) : l'occurrence précédente (`-1`) ou suivante (`1`) ; la
    /// recherche boucle d'elle-même. Sans effet si la barre est fermée ou le champ vide.
    pub fn suivant(
        &mut self,
        sens: i8,
        page_courante: usize,
        page_visible: &dyn Fn(usize) -> bool,
    ) -> Option<ResultatRecherche> {
        if !self.ouverte || !self.a_une_requete() {
            return None;
        }
        self.chercher(TypeRecherche::Encore, sens < 0, page_courante, page_visible)
    }

    /// L'occurrence courante : (page à partir de 1, début dans le texte brut de la page).
    pub fn occurrence_courante(&self) -> Option<(usize, usize)> {
        let r = self.recherche.as_ref()?;
        let (page, rang) = r.selection()?;
        let (index, _) = *r.occurrences_page(page).get(rang)?;
        Some((page + 1, index))
    }

    /// Le dernier résultat.
    pub fn resultat(&self) -> ResultatRecherche {
        self.resultat
    }

    /// Le compteur : « n of m », « No results », « … », ou vide si le champ est vide.
    pub fn compteur(&self) -> String {
        if self.recherche.is_none() && self.a_une_requete() {
            // Le texte des pages n'est pas encore lu.
            return "…".to_string();
        }
        self.resultat.texte(&self.valeur)
    }

    /// La barre passe en rouge (`has-no-results`) ?
    pub fn aucun_resultat(&self) -> bool {
        self.resultat.aucun_resultat(&self.valeur)
    }

    /// Les surlignages d'une page (à partir de 1) : tous avec « Highlight all », le courant seul
    /// sinon ; aucun une fois la barre fermée.
    pub fn surlignages_page(&self, page: usize) -> Vec<Surlignage> {
        match (&self.recherche, page.checked_sub(1)) {
            (Some(r), Some(i)) => r.surlignages_page(i),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn pages(textes: &[&str]) -> Vec<PageRecherche> {
        textes
            .iter()
            .map(|t| PageRecherche::depuis_texte_brut(t))
            .collect()
    }

    fn visible(_: usize) -> bool {
        false
    }

    #[test]
    fn la_saisie_remplace_la_selection_puis_s_ajoute() {
        let mut b = BarreRecherche::new();
        b.ouvrir();
        assert!(b.tout_selectionne());
        assert!(b.saisir("a"));
        assert!(b.saisir("b"));
        assert_eq!(b.valeur(), "ab");
        assert!(b.effacer());
        assert_eq!(b.valeur(), "a");
        // Tout sélectionné : effacer vide le champ.
        b.selectionner_tout();
        assert!(b.effacer());
        assert_eq!(b.valeur(), "");
        // Une touche de contrôle n'est pas du texte.
        assert!(!b.saisir("\n"));
    }

    #[test]
    fn le_compteur_attend_les_pages_puis_compte() {
        let mut b = BarreRecherche::new();
        b.ouvrir();
        b.saisir("x");
        assert_eq!(b.compteur(), "…");
        b.charger(pages(&["x y", "z"]));
        b.chercher(TypeRecherche::Nouvelle, false, 1, &visible);
        assert_eq!(b.compteur(), "1 of 1");
        b.saisir("q");
        b.chercher(TypeRecherche::Nouvelle, false, 1, &visible);
        assert_eq!(b.compteur(), "No results");
        assert!(b.aucun_resultat());
    }

    #[test]
    fn un_champ_vide_remet_le_compteur_a_zero() {
        let mut b = BarreRecherche::new();
        b.charger(pages(&["x"]));
        b.ouvrir();
        b.saisir("x");
        b.chercher(TypeRecherche::Nouvelle, false, 1, &visible);
        assert_eq!(b.compteur(), "1 of 1");
        b.effacer();
        b.chercher(TypeRecherche::Nouvelle, false, 1, &visible);
        assert_eq!(b.compteur(), "");
    }

    #[test]
    fn fermee_la_barre_ne_cherche_pas_et_ne_surligne_rien() {
        let mut b = BarreRecherche::new();
        b.charger(pages(&["x"]));
        assert!(
            b.chercher(TypeRecherche::Nouvelle, false, 1, &visible)
                .is_none()
        );
        b.ouvrir();
        b.saisir("x");
        b.chercher(TypeRecherche::Nouvelle, false, 1, &visible);
        assert_eq!(b.surlignages_page(1).len(), 1);
        b.fermer();
        assert!(b.surlignages_page(1).is_empty());
        assert!(!b.ouverte());
    }

    #[test]
    fn le_menu_coche_les_reglages_et_les_rejoue() {
        let mut b = BarreRecherche::new();
        let m = b.menu_options();
        assert_eq!(
            m.forme(),
            vec!["Highlight all", "Match diacritics", "Whole words"]
        );
        assert!(b.menu_ouvert());
        // Les rappels déposent, `traiter` applique.
        let types = {
            b.empiler(Reglage::MotEntier)();
            b.traiter()
        };
        assert_eq!(types, vec![TypeRecherche::ChangementMotEntier]);
        assert!(b.options().mot_entier);
        assert_eq!(b.basculer_casse(), TypeRecherche::ChangementCasse);
        assert!(b.casse_active());
    }
}
