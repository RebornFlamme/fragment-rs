//! La palette de commandes : la porte d'entrée GÉNÉRIQUE sur le registre de commandes.
//!
//! Là où la barre latérale et les menus n'exposent que les commandes qu'on y a mises, la palette les
//! expose toutes (celles que `list_commands` juge disponibles). C'est une [`ElementsFlous`] dont les
//! éléments sont les commandes ; [`PalettePlugin`] porte ce qui dure entre deux ouvertures : les
//! commandes épinglées (une préférence) et les récentes (un cache).
//!
//! ★ La liste est calculée UNE fois par ouverture : `list_commands` interroge la disponibilité de
//! chaque commande enregistrée, c'est le prix d'une ouverture et non d'une frappe. Chaque ouverture
//! crée une modale neuve, donc la disponibilité est réévaluée à la suivante, jamais pendant.
//!
//! ★ Chaque persistance écrit SA donnée : exécuter une commande ne touche que les récents, et seul
//! le basculement d'une épingle écrit les préférences ([`StockagePalette`]).

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_core::{
    Command, CommandManager, FuzzyMatch, Hotkey, HotkeyManager, InputEvent, Modifier, format_hotkey,
};

use crate::suggest::{
    Auxiliaire, ElementsFlous, Flou, Instruction, Ligne, ModaleSuggestion, ParametresSuggestion,
    Segment, surligner,
};

/// L'identité de la modale : une seule palette ouverte à la fois.
pub const CLE_MODALE: &str = "command-palette";
/// Au-delà, la liste des récentes cesse d'être « récente ».
pub const MAX_RECENTS: usize = 100;

/// Les préférences de la palette (persistées : une préférence).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptionsPalette {
    /// Les ids épinglés, dans l'ordre choisi par l'utilisateur.
    pub epingles: Vec<String>,
}

/// Où la palette range ce qui dure.
pub trait StockagePalette {
    /// Les RÉCENTS : un cache, réécrit à chaque exécution.
    fn sauver_recents(&self, recents: &[String]);
    /// Les OPTIONS (épingles) : écrites seulement quand elles changent vraiment.
    fn sauver_options(&self, options: &OptionsPalette);
}

/// Un stockage qui ne garde rien (scènes de pixels).
pub struct StockageMemoire;

impl StockagePalette for StockageMemoire {
    fn sauver_recents(&self, _recents: &[String]) {}
    fn sauver_options(&self, _options: &OptionsPalette) {}
}

/// Les commandes disponibles maintenant.
pub type ListeCommandes = Rc<dyn Fn() -> Vec<Rc<Command>>>;
/// Les raccourcis à afficher pour un id de commande.
pub type Raccourcis = Rc<dyn Fn(&str) -> Vec<Hotkey>>;
/// L'exécution d'une commande (rend « a-t-elle tourné ? »).
pub type Executeur = Rc<dyn Fn(&Command, Option<InputEvent>) -> bool>;

/// D'où la palette tire ses commandes, leurs raccourcis, et comment elle en exécute une.
#[derive(Clone)]
pub struct SourcePalette {
    /// Les commandes DISPONIBLES maintenant (`list_commands`).
    pub commandes: ListeCommandes,
    /// Les raccourcis à AFFICHER pour un id (personnalisation, sinon défaut).
    pub raccourcis: Raccourcis,
    /// L'exécution (`execute_command` : pose `last_event`, absorbe les paniques).
    pub executer: Executeur,
}

impl SourcePalette {
    /// La source branchée sur le `CommandManager` et, s'il existe, le `HotkeyManager`.
    pub fn depuis_gestionnaires(
        commandes: Rc<CommandManager>,
        touches: Option<Rc<HotkeyManager>>,
    ) -> Self {
        let liste = commandes.clone();
        let executer = commandes;
        Self {
            commandes: Rc::new(move || liste.list_commands()),
            raccourcis: Rc::new(move |id| {
                touches
                    .as_ref()
                    .map(|h| h.hotkeys_for_command(id))
                    .unwrap_or_default()
            }),
            executer: Rc::new(move |c, evt| executer.execute_command(c, evt)),
        }
    }
}

// ── L'ordre ───────────────────────────────────────────────────────────────────

/// La lettre sans son accent (les accents du français et de leurs voisins latins).
fn sans_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        autre => autre,
    }
}

fn plier(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(sans_accent)
        .collect()
}

/// Un ordre alphabétique proche de `localeCompare` : sans casse ni accents, puis la minuscule avant
/// la majuscule. ★ Approchée (pas d'ICU) : seul l'ordre à requête vide en dépend.
pub fn comparer_noms(a: &str, b: &str) -> Ordering {
    plier(a).cmp(&plier(b)).then_with(|| b.cmp(a))
}

/// Re-trie `commandes` pour que les ids présents dans `ordre` viennent en premier, dans cet ordre ;
/// les autres gardent leur ordre relatif (tri stable).
fn trier_selon(commandes: &mut [Rc<Command>], ordre: &[String]) {
    let index: HashMap<&str, usize> = ordre
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    commandes.sort_by_key(|c| index.get(c.id.as_str()).copied().unwrap_or(usize::MAX));
}

// ── Le plugin ─────────────────────────────────────────────────────────────────

/// Le plugin interne « Palette de commandes ».
pub struct PalettePlugin {
    options: RefCell<OptionsPalette>,
    recents: RefCell<Vec<String>>,
    source: SourcePalette,
    stockage: Rc<dyn StockagePalette>,
    curseur: Cell<bool>,
}

impl PalettePlugin {
    /// Une palette sans épingle ni récente (elles se chargent après, voir [`Self::charger`]).
    pub fn new(source: SourcePalette, stockage: Rc<dyn StockagePalette>) -> Self {
        Self {
            options: RefCell::new(OptionsPalette::default()),
            recents: RefCell::new(Vec::new()),
            source,
            stockage,
            curseur: Cell::new(true),
        }
    }

    /// Charge les préférences et les récentes (en arrière-plan : la palette marche avant).
    pub fn charger(&self, options: Option<OptionsPalette>, recents: Option<Vec<String>>) {
        if let Some(o) = options {
            *self.options.borrow_mut() = o;
        }
        if let Some(r) = recents {
            *self.recents.borrow_mut() = r;
        }
    }

    /// Dessiner (ou non) le curseur du champ ; faux dans les scènes de pixels.
    pub fn avec_curseur(&self, dessiner: bool) {
        self.curseur.set(dessiner);
    }

    /// Cette commande est-elle épinglée ?
    pub fn est_epinglee(&self, id: &str) -> bool {
        self.options.borrow().epingles.iter().any(|e| e == id)
    }

    /// Épingle ou désépingle une commande, puis persiste les options.
    ///
    /// ★ Le SEUL point qui écrit les préférences : on n'écrit une persistance que quand SA donnée
    /// change.
    pub fn basculer_epingle(&self, id: &str) {
        {
            let mut o = self.options.borrow_mut();
            match o.epingles.iter().position(|e| e == id) {
                Some(i) => {
                    o.epingles.remove(i);
                }
                None => o.epingles.push(id.to_owned()),
            }
        }
        self.stockage.sauver_options(&self.options.borrow());
    }

    /// Les commandes à afficher, dans l'ordre : épinglées, puis récentes, puis le reste.
    ///
    /// ★ Le double tri : alphabétique, PUIS re-tri stable par rang de récence. Une commande absente
    /// des récentes a un rang infini et garde sa place alphabétique relative.
    pub fn commandes(&self) -> Vec<Rc<Command>> {
        let liste = (self.source.commandes)();
        let epingles = self.options.borrow().epingles.clone();
        let (mut epinglees, mut reste): (Vec<_>, Vec<_>) = if epingles.is_empty() {
            (Vec::new(), liste)
        } else {
            liste
                .into_iter()
                .partition(|c| epingles.iter().any(|e| *e == c.id))
        };
        trier_selon(&mut epinglees, &epingles);
        reste.sort_by(|a, b| comparer_noms(&a.name, &b.name));
        trier_selon(&mut reste, &self.recents.borrow());
        epinglees.extend(reste);
        epinglees
    }

    /// Le choix de l'utilisateur : exécute (par le gestionnaire, qui absorbe les erreurs d'un
    /// plugin), remonte la commande en tête des récentes, et n'écrit QUE les récentes.
    pub fn executer(&self, commande: &Command, evenement: Option<InputEvent>) {
        (self.source.executer)(commande, evenement);
        let mut r = self.recents.borrow_mut();
        r.retain(|id| *id != commande.id);
        r.insert(0, commande.id.clone());
        r.truncate(MAX_RECENTS);
        self.stockage.sauver_recents(&r);
    }

    /// Les récentes, la plus récente en tête.
    pub fn recents(&self) -> Vec<String> {
        self.recents.borrow().clone()
    }

    /// Les ids épinglés.
    pub fn epingles(&self) -> Vec<String> {
        self.options.borrow().epingles.clone()
    }

    /// Demande l'ouverture de la palette (sans effet si elle est déjà ouverte).
    pub fn ouvrir(self: &Rc<Self>) {
        let adaptateur = AdaptateurPalette {
            plugin: self.clone(),
            cache: None,
        };
        let parametres = ParametresSuggestion {
            placeholder: "Saisir une commande…".to_owned(),
            instructions: vec![
                Instruction::new("↑↓", "naviguer"),
                Instruction::new("↵", "exécuter"),
                Instruction::new("esc", "fermer"),
            ],
            curseur: self.curseur.get(),
            ..ParametresSuggestion::default()
        };
        ModaleSuggestion::ouvrir(CLE_MODALE, Flou(adaptateur), parametres);
    }

    /// Enregistre `command-palette:open` (Mod+P) dans le gestionnaire de commandes.
    pub fn enregistrer(self: &Rc<Self>, commandes: &CommandManager) -> fragment_core::Disposer {
        let plugin = self.clone();
        let mut commande = Command::new(
            "command-palette:open",
            "Palette de commandes: Ouvrir la palette de commandes",
        )
        .with_hotkeys(vec![Hotkey::new(&[Modifier::Mod], "p")])
        .with_callback(move || plugin.ouvrir());
        commande.icon = Some("square-terminal".to_owned());
        commandes.add_command(commande, "command-palette")
    }
}

// ── La modale : les trois trous de la palette ─────────────────────────────────

struct AdaptateurPalette {
    plugin: Rc<PalettePlugin>,
    cache: Option<Vec<Rc<Command>>>,
}

/// Le rang UTF-16 de la fin du préfixe « Plugin: » (les plages de match sont en UTF-16).
fn fin_du_prefixe(nom: &str) -> Option<(usize, usize)> {
    let i = nom.find(": ")?;
    let octets = i + 2;
    Some((octets, nom[..octets].encode_utf16().count()))
}

impl ElementsFlous for AdaptateurPalette {
    type Element = Rc<Command>;

    fn elements(&mut self) -> Vec<Rc<Command>> {
        let plugin = &self.plugin;
        self.cache.get_or_insert_with(|| plugin.commandes()).clone()
    }

    fn texte(&self, commande: &Rc<Command>) -> String {
        commande.name.clone()
    }

    /// Une ligne : le nom (préfixe du plugin grisé) à gauche, raccourcis et épingle à droite.
    ///
    /// Le nom est DÉCOUPÉ sur le premier « : » (contrepartie du préfixage de `add_command`) ; le
    /// surlignage a été calculé sur le nom entier, d'où le décalage passé au second morceau.
    fn ligne(&self, valeur: &FuzzyMatch<Rc<Command>>) -> Ligne {
        let commande = &valeur.item;
        let nom = &commande.name;
        let plages = &valeur.r#match.matches;
        let mut segments = Vec::new();
        let mut ajouter = |texte: &str, decalage: usize, prefixe: bool| {
            for (morceau, surligne) in surligner(texte, plages, decalage) {
                segments.push(Segment {
                    texte: morceau,
                    prefixe,
                    surligne,
                });
            }
        };
        match fin_du_prefixe(nom) {
            Some((octets, unites)) => {
                ajouter(&nom[..octets], 0, true);
                ajouter(&nom[octets..], unites, false);
            }
            None => ajouter(nom, 0, false),
        }
        let mut aux: Vec<Auxiliaire> = (self.plugin.source.raccourcis)(&commande.id)
            .iter()
            .map(|h| Auxiliaire::Raccourci(format_hotkey(h)))
            .collect();
        if self.plugin.est_epinglee(&commande.id) {
            aux.push(Auxiliaire::Epingle);
        }
        Ligne { segments, aux }
    }

    fn choisir_element(&mut self, commande: Rc<Command>, evenement: Option<InputEvent>) {
        self.plugin.executer(&commande, evenement);
    }
}

// ── Les scènes ────────────────────────────────────────────────────────────────

/// Enregistre les commandes de scène de la palette : `command-palette:open`, et les deux commandes
/// « Application » que la requête de `t1-palette` trouve sur Fragment.
///
/// ★ Les noms et raccourcis sont ceux de Fragment (relevés sur la référence). La palette des scènes
/// n'a pas le gestionnaire de commandes en main (le contrat de `commandes_de_scene` ne donne qu'une
/// référence) : elle liste ces commandes elle-même, et le curseur du champ est masqué comme dans les
/// captures de Fragment.
pub fn commandes_de_scene(commandes: &CommandManager) {
    crate::modal::vider_demandes();
    let fixtures = [
        (
            "app:toggle-left-sidebar",
            "Application: Afficher / masquer le panneau de gauche",
            Hotkey::new(&[Modifier::Ctrl], "&"),
        ),
        (
            "app:toggle-right-sidebar",
            "Application: Afficher / masquer le panneau de droite",
            Hotkey::new(&[Modifier::Ctrl, Modifier::Shift], "%"),
        ),
    ];
    let mut touches: HashMap<String, Vec<Hotkey>> = HashMap::new();
    let mut ids = Vec::new();
    for (id, nom, hotkey) in fixtures {
        touches.insert(id.to_owned(), vec![hotkey.clone()]);
        ids.push(id);
        let commande = Command::new(id, nom)
            .with_hotkeys(vec![hotkey])
            .with_callback(|| {});
        // Le retrait n'est jamais appelé : la commande vit autant que le gestionnaire de la scène.
        let _ = commandes.add_command(commande, "app");
    }
    ids.push("command-palette:open");
    touches.insert(
        "command-palette:open".to_owned(),
        vec![Hotkey::new(&[Modifier::Mod], "p")],
    );

    // La liste est remplie APRÈS l'enregistrement : le plugin n'existe qu'une fois la source bâtie,
    // et `command-palette:open` doit y figurer (Fragment la liste aussi).
    let toutes: Rc<RefCell<Vec<Rc<Command>>>> = Rc::new(RefCell::new(Vec::new()));
    let source = SourcePalette {
        commandes: Rc::new({
            let toutes = toutes.clone();
            move || toutes.borrow().clone()
        }),
        raccourcis: Rc::new(move |id| touches.get(id).cloned().unwrap_or_default()),
        executer: Rc::new(|c, _| {
            if let Some(cb) = &c.callback {
                cb();
                true
            } else {
                false
            }
        }),
    };
    let plugin = Rc::new(PalettePlugin::new(source, Rc::new(StockageMemoire)));
    plugin.avec_curseur(false);
    let _ = plugin.enregistrer(commandes);
    *toutes.borrow_mut() = ids
        .iter()
        .filter_map(|id| commandes.find_command(id))
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_ordre_alphabetique_ignore_casse_et_accents() {
        assert_eq!(comparer_noms("Éditeur", "Explorateur"), Ordering::Less);
        assert_eq!(comparer_noms("abc", "ABD"), Ordering::Less);
        assert_eq!(comparer_noms("a", "A"), Ordering::Less);
    }

    #[test]
    fn le_prefixe_se_coupe_sur_le_premier_deux_points() {
        assert_eq!(fin_du_prefixe("Plugin: Faire"), Some((8, 8)));
        assert_eq!(fin_du_prefixe("Sans préfixe"), None);
        // « É » vaut 2 octets mais 1 unité UTF-16 : 7 octets, 6 unités.
        assert_eq!(fin_du_prefixe("Édit: x"), Some((7, 6)));
    }
}
