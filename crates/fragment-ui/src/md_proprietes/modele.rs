// Port de app/src/ui/views/markdown/properties/PropertiesWidget.ts (la part sans DOM : écriture,
// validation des clés, menus, glisser) et de properties/frontmatterField.ts (suppression du bloc).
//! Le contrôleur PUR des Properties : (texte du document, geste de l'utilisateur) → ÉDITION du
//! document et effets sur l'interface.
//!
//! Le document est la seule vérité : à chaque geste, le bloc est relu dans le texte (`plage_proprietes`
//! puis `parse_properties`), la liste de propriétés est modifiée, et le résultat revient sous la
//! forme d'une [`Edition`] (un remplacement d'octets) que la vue applique comme UNE transaction
//! isolée de l'historique voisin.
//!
//! ★ Un état seulement, hors du texte : la LIGNE D'AJOUT (clé en cours de saisie, type choisi
//!   d'avance). Elle n'existe pas dans le YAML tant que sa clé n'est pas validée.

use fragment_core::MetadataTypeManager;
use fragment_markdown::blocs::plage_proprietes;
use fragment_markdown::frontmatter_model::{self as fm, Property, PropertyType, PropertyValue};

/// Un remplacement dans le document, en OCTETS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edition {
    pub de: usize,
    pub a: usize,
    pub texte: String,
    /// Où poser le curseur de l'éditeur après l'édition (`None` : ne pas y toucher).
    pub curseur: Option<usize>,
}

impl Edition {
    /// Le document après l'édition.
    #[must_use]
    pub fn appliquer(&self, doc: &str) -> String {
        format!("{}{}{}", &doc[..self.de], self.texte, &doc[self.a..])
    }
}

/// Où se pose le focus clavier, dans le widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Le champ clé de la ligne d'ajout.
    CleAttente,
    /// L'éditeur de valeur de la propriété `i`.
    Valeur(usize),
    /// Rendu à l'éditeur de texte (le bloc a disparu).
    Texte,
}

/// Ce que la vue doit faire en plus de l'édition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    /// Une notice (clé en double, presse-papiers vide).
    Notice(String),
    /// Déplier les Properties (on ajoute dans un bloc replié).
    Deplier,
    Focus(Focus),
    /// Remettre le presse-papiers.
    Presse(String),
}

/// Le résultat d'un geste.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sortie {
    pub edition: Option<Edition>,
    pub effets: Vec<Effet>,
}

impl Sortie {
    fn vide() -> Self {
        Self::default()
    }

    fn avec(edition: Option<Edition>) -> Self {
        Self {
            edition,
            effets: Vec::new(),
        }
    }

    fn effet(mut self, e: Effet) -> Self {
        self.effets.push(e);
        self
    }
}

// ─── Les types ────────────────────────────────────────────────────────────

/// Les types proposés par le menu « Property type », dans l'ordre d'Obsidian (`tags` est inféré).
pub const TYPES: [PropertyType; 6] = [
    PropertyType::Checkbox,
    PropertyType::Date,
    PropertyType::Datetime,
    PropertyType::Multitext,
    PropertyType::Number,
    PropertyType::Text,
];

/// L'id écrit dans `types.json`.
#[must_use]
pub fn id_du_type(t: PropertyType) -> &'static str {
    match t {
        PropertyType::Text => "text",
        PropertyType::Multitext => "multitext",
        PropertyType::Number => "number",
        PropertyType::Checkbox => "checkbox",
        PropertyType::Date => "date",
        PropertyType::Datetime => "datetime",
        PropertyType::Tags => "tags",
    }
}

/// Le type d'un id de `types.json` ; un id inconnu vaut « texte » (comme `infoType`).
#[must_use]
pub fn type_de_l_id(id: &str) -> PropertyType {
    match id {
        "multitext" => PropertyType::Multitext,
        "number" => PropertyType::Number,
        "checkbox" => PropertyType::Checkbox,
        "date" => PropertyType::Date,
        "datetime" => PropertyType::Datetime,
        "tags" => PropertyType::Tags,
        _ => PropertyType::Text,
    }
}

/// Le libellé du menu.
#[must_use]
pub fn libelle_du_type(t: PropertyType) -> &'static str {
    match t {
        PropertyType::Text => "Text",
        PropertyType::Multitext => "List",
        PropertyType::Number => "Number",
        PropertyType::Checkbox => "Checkbox",
        PropertyType::Date => "Date",
        PropertyType::Datetime => "Date & time",
        PropertyType::Tags => "Tags",
    }
}

/// L'icône (nom Lucide du registre `icones`).
#[must_use]
pub fn icone_du_type(t: PropertyType) -> &'static str {
    match t {
        PropertyType::Text => "text",
        PropertyType::Multitext => "list",
        PropertyType::Number => "binary",
        PropertyType::Checkbox => "square-check",
        PropertyType::Date => "calendar",
        PropertyType::Datetime => "clock",
        PropertyType::Tags => "tags",
    }
}

/// Les types assignés du coffre (`.fragment/types.json`), vus par le widget.
pub trait RegistreTypes {
    fn assigne(&self, cle: &str) -> Option<String>;
    fn definir(&self, cle: &str, id: &str);
    fn retirer(&self, cle: &str);
}

impl RegistreTypes for MetadataTypeManager {
    fn assigne(&self, cle: &str) -> Option<String> {
        self.type_assigne(cle)
    }
    fn definir(&self, cle: &str, id: &str) {
        self.definir_type(cle, id);
    }
    fn retirer(&self, cle: &str) {
        self.retirer_type(cle);
    }
}

/// Le type AFFICHÉ : l'assigné, sinon l'inféré.
#[must_use]
pub fn type_affiche(reg: &dyn RegistreTypes, p: &Property) -> PropertyType {
    reg.assigne(&p.key)
        .map_or_else(|| fm::infer_type(&p.key, &p.value), |id| type_de_l_id(&id))
}

// ─── Le bloc dans le document ─────────────────────────────────────────────

/// Le bloc Properties valide de tête : sa plage (octets) et ses propriétés.
#[derive(Debug, Clone, PartialEq)]
pub struct BlocProps {
    pub de: usize,
    pub a: usize,
    pub props: Vec<Property>,
}

/// Lit le bloc de tête (`None` : pas de frontmatter valide, la source reste visible).
#[must_use]
pub fn lire_bloc(texte: &str) -> Option<BlocProps> {
    let plage = plage_proprietes(texte)?;
    let r = fm::frontmatter_range(&fm::doc_of(texte))?;
    let props = fm::parse_properties(&r.yaml)?;
    Some(BlocProps {
        de: plage.octets.from,
        a: plage.octets.to,
        props,
    })
}

/// L'édition qui écrit `props` à la place du bloc ; liste vide : le bloc disparaît avec le saut de
/// ligne qui le suit (le curseur reprend sa place).
fn ecrire(texte: &str, bloc: &BlocProps, props: &[Property]) -> Edition {
    if props.is_empty() {
        let a = (bloc.a + 1).min(texte.len());
        return Edition {
            de: bloc.de,
            a,
            texte: String::new(),
            curseur: Some(bloc.de),
        };
    }
    Edition {
        de: bloc.de,
        a: bloc.a,
        texte: fm::serialize_frontmatter(props),
        curseur: None,
    }
}

// ─── Les menus (données pures) ────────────────────────────────────────────

/// L'action d'une entrée de menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Assigner un type à la clé (`None` : retour à « Automatic »).
    Type(String, Option<PropertyType>),
    /// Le type de la ligne d'ajout (sa clé n'existe pas encore).
    TypeAttente(Option<PropertyType>),
    Couper(usize),
    Copier(usize),
    Coller(usize),
    Supprimer(usize),
    ToutEffacer,
}

/// Une entrée de menu, avant sa traduction en [`crate::menu::MenuItem`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntreeMenu {
    pub titre: String,
    pub icone: &'static str,
    pub coche: bool,
    pub avertissement: bool,
    pub section: &'static str,
    pub action: Option<Action>,
    pub sous_menu: Vec<EntreeMenu>,
}

impl EntreeMenu {
    fn simple(titre: &str, icone: &'static str, section: &'static str, a: Action) -> Self {
        Self {
            titre: titre.to_owned(),
            icone,
            coche: false,
            avertissement: false,
            section,
            action: Some(a),
            sous_menu: Vec::new(),
        }
    }
}

fn entrees_de_types(
    auto: String,
    assigne: Option<PropertyType>,
    de: impl Fn(Option<PropertyType>) -> Action,
) -> Vec<EntreeMenu> {
    let mut v = vec![EntreeMenu {
        coche: assigne.is_none(),
        ..EntreeMenu::simple(&auto, "loader", "", de(None))
    }];
    for t in TYPES {
        v.push(EntreeMenu {
            coche: assigne == Some(t),
            ..EntreeMenu::simple(libelle_du_type(t), icone_du_type(t), "", de(Some(t)))
        });
    }
    v
}

// ─── Les valeurs saisies ──────────────────────────────────────────────────

/// Un champ texte vide vaut « rien » (`clé:`), sinon le texte.
#[must_use]
pub fn valeur_texte(saisie: &str) -> PropertyValue {
    if saisie.is_empty() {
        PropertyValue::Null
    } else {
        PropertyValue::Text(saisie.to_owned())
    }
}

/// Un champ nombre : vide → rien ; sinon `Number(saisie)` (entier si possible). Une saisie qui n'est
/// pas un nombre est ignorée par le champ HTML : ici, `None`.
#[must_use]
pub fn valeur_nombre(saisie: &str) -> Option<PropertyValue> {
    let s = saisie.trim();
    if s.is_empty() {
        return Some(PropertyValue::Null);
    }
    if let Ok(n) = s.parse::<i64>() {
        return Some(PropertyValue::Int(n));
    }
    s.parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
        .map(PropertyValue::Float)
}

/// Ce que le champ nombre affiche : un nombre, ou un texte qui en est un, sinon rien.
#[must_use]
pub fn affichage_nombre(v: &PropertyValue) -> String {
    match v {
        PropertyValue::Int(_) | PropertyValue::Float(_) => fm::as_text(v),
        PropertyValue::Text(s) if valeur_nombre(s).is_some_and(|n| n != PropertyValue::Null) => {
            s.clone()
        }
        _ => String::new(),
    }
}

/// Ce que le champ date (ou date et heure) affiche : la valeur si elle a la bonne forme.
#[must_use]
pub fn affichage_date(v: &PropertyValue, avec_heure: bool) -> String {
    let PropertyValue::Text(t) = v else {
        return String::new();
    };
    let chiffres = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    let jour_ok = t.len() >= 10
        && chiffres(&t[..4], 4)
        && t.as_bytes()[4] == b'-'
        && chiffres(&t[5..7], 2)
        && t.as_bytes()[7] == b'-'
        && chiffres(&t[8..10], 2);
    if !jour_ok {
        return String::new();
    }
    if !avec_heure {
        return if t.len() == 10 {
            t.clone()
        } else {
            String::new()
        };
    }
    let reste = &t[10..];
    let ok = reste.len() >= 6
        && matches!(reste.as_bytes()[0], b'T' | b' ')
        && chiffres(&reste[1..3], 2)
        && reste.as_bytes()[3] == b':'
        && chiffres(&reste[4..6], 2);
    if ok {
        format!("{}T{}", &t[..10], &reste[1..6])
    } else {
        String::new()
    }
}

/// L'état de la case d'une propriété : `true` ou le texte « true ».
#[must_use]
pub fn est_coche(v: &PropertyValue) -> bool {
    matches!(v, PropertyValue::Bool(true)) || matches!(v, PropertyValue::Text(s) if s == "true")
}

// ─── Le contrôleur ────────────────────────────────────────────────────────

/// La ligne d'ajout, tant que sa clé n'est pas validée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attente {
    /// Ce qui est tapé dans le champ clé.
    pub cle: String,
    /// Le type choisi AVANT que la clé existe (appliqué à sa validation).
    pub type_choisi: Option<PropertyType>,
}

/// Les Properties d'un document : la ligne d'ajout et le mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proprietes {
    lecture_seule: bool,
    attente: Option<Attente>,
}

impl Proprietes {
    #[must_use]
    pub fn new(lecture_seule: bool) -> Self {
        Self {
            lecture_seule,
            attente: None,
        }
    }

    pub fn poser_lecture_seule(&mut self, v: bool) {
        self.lecture_seule = v;
    }

    #[must_use]
    pub fn lecture_seule(&self) -> bool {
        self.lecture_seule
    }

    /// La ligne d'ajout, si elle est ouverte.
    #[must_use]
    pub fn attente(&self) -> Option<&Attente> {
        self.attente.as_ref()
    }

    /// Le type de la ligne d'ajout : le choisi d'avance, sinon « texte ».
    #[must_use]
    pub fn type_de_l_attente(&self) -> PropertyType {
        self.attente
            .as_ref()
            .and_then(|a| a.type_choisi)
            .unwrap_or(PropertyType::Text)
    }

    /// « Add property » : une ligne vide, champ clé focalisé ; écrite à la validation de la clé.
    /// `replie` : le bloc est replié, on le déplie d'abord.
    pub fn ajouter(&mut self, replie: bool) -> Sortie {
        if self.lecture_seule {
            return Sortie::vide();
        }
        self.attente = Some(Attente::default());
        let mut s = Sortie::vide();
        if replie {
            s = s.effet(Effet::Deplier);
        }
        s.effet(Effet::Focus(Focus::CleAttente))
    }

    /// Le champ clé de la ligne d'ajout a changé.
    pub fn saisir_cle_attente(&mut self, cle: &str) {
        if let Some(a) = &mut self.attente {
            cle.clone_into(&mut a.cle);
        }
    }

    /// `---` vient de créer le bloc : on ouvre une propriété (replié : jamais, il est neuf).
    pub fn apres_creation_du_bloc(&mut self) -> Sortie {
        self.ajouter(false)
    }

    /// ↑ depuis le texte : le focus entre dans la dernière valeur ; sans propriété, une ligne
    /// d'ajout s'ouvre.
    pub fn focus_derniere(&mut self, texte: &str) -> Option<Sortie> {
        let bloc = lire_bloc(texte)?;
        if bloc.props.is_empty() {
            if self.lecture_seule {
                return None;
            }
            return Some(self.ajouter(false));
        }
        Some(Sortie::vide().effet(Effet::Focus(Focus::Valeur(bloc.props.len() - 1))))
    }

    // ── Les clés ──────────────────────────────────────────────────────────

    /// Valide la clé de la ligne d'ajout (blur, ou Entrée : `entree`). Vide : la ligne est
    /// abandonnée, et sur un bloc VIDE le bloc entier disparaît.
    pub fn valider_cle_attente(
        &mut self,
        texte: &str,
        reg: &dyn RegistreTypes,
        cle: &str,
        entree: bool,
    ) -> Sortie {
        let Some(bloc) = lire_bloc(texte) else {
            self.attente = None;
            return Sortie::vide();
        };
        let cle = cle.trim();
        if cle.is_empty() {
            self.attente = None;
            if bloc.props.is_empty() {
                return self.effacer_bloc(texte, &bloc);
            }
            return Sortie::vide();
        }
        if bloc.props.iter().any(|q| q.key == cle) {
            return Sortie::vide().effet(Effet::Notice(doublon(cle)));
        }
        let type_choisi = self.attente.take().and_then(|a| a.type_choisi);
        let n = bloc.props.len();
        let props = fm::insert_at(&bloc.props, n, nulle(cle));
        let mut s = Sortie::avec(Some(ecrire(texte, &bloc, &props)));
        if let Some(t) = type_choisi {
            reg.definir(cle, id_du_type(t));
        }
        if entree {
            s = s.effet(Effet::Focus(Focus::Valeur(n)));
        }
        s
    }

    /// Valide la clé de la propriété `i` : vide, elle est retirée ; changée, renommée.
    pub fn valider_cle(&mut self, texte: &str, i: usize, cle: &str, entree: bool) -> Sortie {
        let Some(bloc) = lire_bloc(texte) else {
            return Sortie::vide();
        };
        let Some(p) = bloc.props.get(i) else {
            return Sortie::vide();
        };
        let cle = cle.trim();
        if self.lecture_seule {
            return Sortie::vide();
        }
        if cle.is_empty() {
            return Sortie::avec(Some(ecrire(texte, &bloc, &fm::remove_at(&bloc.props, i))));
        }
        let mut s = Sortie::vide();
        if cle != p.key {
            if bloc
                .props
                .iter()
                .enumerate()
                .any(|(j, q)| j != i && q.key == cle)
            {
                return s.effet(Effet::Notice(doublon(cle)));
            }
            s = Sortie::avec(Some(ecrire(
                texte,
                &bloc,
                &fm::set_key(&bloc.props, i, cle),
            )));
        }
        if entree {
            s = s.effet(Effet::Focus(Focus::Valeur(i)));
        }
        s
    }

    /// Échap dans la clé d'une ligne d'ajout : abandonne la ligne ; un bloc vide disparaît.
    pub fn abandonner_attente(&mut self, texte: &str) -> Sortie {
        self.valider_cle_attente(texte, &SansTypes, "", false)
    }

    // ── Les valeurs ───────────────────────────────────────────────────────

    /// Écrit la valeur de la propriété `i`.
    pub fn definir_valeur(&mut self, texte: &str, i: usize, valeur: PropertyValue) -> Sortie {
        self.modifier(texte, |props| {
            (i < props.len()).then(|| fm::set_value(props, i, valeur))
        })
    }

    /// Ajoute `saisie` à la liste de la propriété `i` (`tags` : sans `#`). Rend aussi `true` si la
    /// saisie a été ajoutée.
    pub fn ajouter_element(
        &mut self,
        texte: &str,
        i: usize,
        type_: PropertyType,
        saisie: &str,
    ) -> Sortie {
        let raw = saisie.trim();
        if raw.is_empty() {
            return Sortie::vide();
        }
        let v = if type_ == PropertyType::Tags {
            fm::normaliser_tag(raw)
        } else {
            raw.to_owned()
        };
        let Some(bloc) = lire_bloc(texte) else {
            return Sortie::vide();
        };
        let Some(p) = bloc.props.get(i) else {
            return Sortie::vide();
        };
        let mut items = fm::as_list(&p.value);
        if v.is_empty() || items.contains(&v) {
            return Sortie::vide();
        }
        items.push(v);
        self.ecrire_liste(texte, i, items)
    }

    /// Retire l'élément `j` de la liste de la propriété `i` (la croix d'une pastille).
    pub fn retirer_element(&mut self, texte: &str, i: usize, j: usize) -> Sortie {
        let Some(bloc) = lire_bloc(texte) else {
            return Sortie::vide();
        };
        let Some(p) = bloc.props.get(i) else {
            return Sortie::vide();
        };
        let items: Vec<String> = fm::as_list(&p.value)
            .into_iter()
            .enumerate()
            .filter(|(k, _)| *k != j)
            .map(|(_, v)| v)
            .collect();
        self.ecrire_liste(texte, i, items)
    }

    /// Retour arrière dans un champ de liste vide : retire le DERNIER élément.
    pub fn retirer_dernier_element(&mut self, texte: &str, i: usize) -> Sortie {
        let Some(bloc) = lire_bloc(texte) else {
            return Sortie::vide();
        };
        let Some(p) = bloc.props.get(i) else {
            return Sortie::vide();
        };
        let n = fm::as_list(&p.value).len();
        if n == 0 {
            return Sortie::vide();
        }
        self.retirer_element(texte, i, n - 1)
    }

    fn ecrire_liste(&mut self, texte: &str, i: usize, items: Vec<String>) -> Sortie {
        let valeur = PropertyValue::List(items.into_iter().map(PropertyValue::Text).collect());
        let mut s = self.definir_valeur(texte, i, valeur);
        if s.edition.is_some() {
            s = s.effet(Effet::Focus(Focus::Valeur(i)));
        }
        s
    }

    // ── Suppression, glisser, presse-papiers ──────────────────────────────

    pub fn supprimer(&mut self, texte: &str, i: usize) -> Sortie {
        self.modifier(texte, |props| {
            (i < props.len()).then(|| fm::remove_at(props, i))
        })
    }

    /// « Clear properties » : tout le bloc, d'un coup.
    pub fn tout_effacer(&mut self, texte: &str) -> Sortie {
        if self.lecture_seule {
            return Sortie::vide();
        }
        let Some(bloc) = lire_bloc(texte) else {
            return Sortie::vide();
        };
        self.attente = None;
        self.effacer_bloc(texte, &bloc)
    }

    fn effacer_bloc(&self, texte: &str, bloc: &BlocProps) -> Sortie {
        Sortie::avec(Some(ecrire(texte, bloc, &[]))).effet(Effet::Focus(Focus::Texte))
    }

    /// Déplace la propriété `de` vers la position de dépôt `cible` (0 = avant la première,
    /// `n` = après la dernière), comme la barre de dépôt.
    pub fn deplacer(&mut self, texte: &str, de: usize, cible: usize) -> Sortie {
        let to = if cible > de { cible - 1 } else { cible };
        if to == de {
            return Sortie::vide();
        }
        self.modifier(texte, |props| {
            (de < props.len()).then(|| fm::move_prop(props, de, to))
        })
    }

    /// Le YAML d'une propriété, pour le presse-papiers.
    #[must_use]
    pub fn copier(texte: &str, i: usize) -> Option<String> {
        let bloc = lire_bloc(texte)?;
        Some(fm::serialize_properties(std::slice::from_ref(
            bloc.props.get(i)?,
        )))
    }

    /// Colle des propriétés après `i` (une clé déjà présente change de valeur sur place).
    pub fn coller(&mut self, texte: &str, i: usize, presse_papiers: &str) -> Sortie {
        let incoming = fm::parse_properties(presse_papiers).unwrap_or_default();
        if incoming.is_empty() {
            return Sortie::vide().effet(Effet::Notice(
                "Le presse-papiers ne contient pas de propriétés.".to_owned(),
            ));
        }
        self.modifier(texte, |props| Some(fm::merge_at(props, i, &incoming)))
    }

    fn modifier(
        &mut self,
        texte: &str,
        f: impl FnOnce(&[Property]) -> Option<Vec<Property>>,
    ) -> Sortie {
        if self.lecture_seule {
            return Sortie::vide();
        }
        let Some(bloc) = lire_bloc(texte) else {
            return Sortie::vide();
        };
        match f(&bloc.props) {
            Some(props) => Sortie::avec(Some(ecrire(texte, &bloc, &props))),
            None => Sortie::vide(),
        }
    }

    // ── Les menus ─────────────────────────────────────────────────────────

    /// Le menu d'une propriété (clic droit, ou clic sur son icône).
    #[must_use]
    pub fn menu_ligne(&self, texte: &str, reg: &dyn RegistreTypes, i: usize) -> Vec<EntreeMenu> {
        let Some(p) = lire_bloc(texte).and_then(|b| b.props.get(i).cloned()) else {
            return Vec::new();
        };
        let mut menu = Vec::new();
        if !self.lecture_seule {
            let assigne = reg.assigne(&p.key).map(|id| type_de_l_id(&id));
            let auto = fm::infer_type(&p.key, &p.value);
            let cle = p.key.clone();
            menu.push(EntreeMenu {
                titre: "Property type".to_owned(),
                icone: "info",
                coche: false,
                avertissement: false,
                section: "type",
                action: None,
                sous_menu: entrees_de_types(
                    format!("Automatic ({})", libelle_du_type(auto)),
                    assigne,
                    |t| Action::Type(cle.clone(), t),
                ),
            });
            menu.push(EntreeMenu::simple(
                "Cut",
                "scissors",
                "clipboard",
                Action::Couper(i),
            ));
        }
        menu.push(EntreeMenu::simple(
            "Copy",
            "copy",
            "clipboard",
            Action::Copier(i),
        ));
        if !self.lecture_seule {
            menu.push(EntreeMenu::simple(
                "Paste",
                "clipboard",
                "clipboard",
                Action::Coller(i),
            ));
            menu.push(EntreeMenu {
                avertissement: true,
                ..EntreeMenu::simple("Remove", "trash-2", "danger", Action::Supprimer(i))
            });
        }
        menu
    }

    /// Le menu de type de la ligne d'ajout (sa clé n'existe pas encore).
    #[must_use]
    pub fn menu_attente(&self) -> Vec<EntreeMenu> {
        let choisi = self.attente.as_ref().and_then(|a| a.type_choisi);
        vec![EntreeMenu {
            titre: "Property type".to_owned(),
            icone: "info",
            coche: false,
            avertissement: false,
            section: "",
            action: None,
            sous_menu: entrees_de_types("Automatic (Text)".to_owned(), choisi, Action::TypeAttente),
        }]
    }

    /// Le menu du bloc (clic droit sur l'en-tête ou une zone vide).
    #[must_use]
    pub fn menu_bloc(&self) -> Vec<EntreeMenu> {
        if self.lecture_seule {
            return Vec::new();
        }
        vec![EntreeMenu {
            avertissement: true,
            ..EntreeMenu::simple("Clear properties", "trash-2", "", Action::ToutEffacer)
        }]
    }

    /// Exécute l'action d'une entrée de menu. `presse_papiers` : le texte à coller.
    pub fn executer(
        &mut self,
        a: &Action,
        texte: &str,
        reg: &dyn RegistreTypes,
        presse_papiers: &str,
    ) -> Sortie {
        match a {
            Action::Type(cle, Some(t)) => {
                reg.definir(cle, id_du_type(*t));
                Sortie::vide()
            }
            Action::Type(cle, None) => {
                reg.retirer(cle);
                Sortie::vide()
            }
            Action::TypeAttente(t) => {
                if let Some(att) = &mut self.attente {
                    att.type_choisi = *t;
                }
                Sortie::vide().effet(Effet::Focus(Focus::CleAttente))
            }
            Action::Copier(i) => match Self::copier(texte, *i) {
                Some(y) => Sortie::vide().effet(Effet::Presse(y)),
                None => Sortie::vide(),
            },
            Action::Couper(i) => {
                let Some(y) = Self::copier(texte, *i) else {
                    return Sortie::vide();
                };
                let mut s = self.supprimer(texte, *i);
                s.effets.push(Effet::Presse(y));
                s
            }
            Action::Coller(i) => self.coller(texte, *i, presse_papiers),
            Action::Supprimer(i) => self.supprimer(texte, *i),
            Action::ToutEffacer => self.tout_effacer(texte),
        }
    }

    /// Le clic sur l'icône de la ligne d'ajout : sans clé, le menu de type (le type sera appliqué à
    /// la validation) ; avec une clé, on la valide, puis le menu s'ouvre sur la NOUVELLE propriété.
    /// Rend l'écriture éventuelle et l'indice de la propriété dont il faut ouvrir le menu.
    pub fn icone_de_l_attente(
        &mut self,
        texte: &str,
        reg: &dyn RegistreTypes,
        cle: &str,
    ) -> (Sortie, Option<usize>) {
        let cle = cle.trim();
        if cle.is_empty() {
            return (Sortie::vide(), None);
        }
        let Some(bloc) = lire_bloc(texte) else {
            return (Sortie::vide(), None);
        };
        if bloc.props.iter().any(|q| q.key == cle) {
            return (Sortie::vide(), None);
        }
        let n = bloc.props.len();
        (self.valider_cle_attente(texte, reg, cle, false), Some(n))
    }
}

/// Les registres de types ne servent pas à l'abandon d'une ligne : un registre sans effet.
struct SansTypes;

impl RegistreTypes for SansTypes {
    fn assigne(&self, _: &str) -> Option<String> {
        None
    }
    fn definir(&self, _: &str, _: &str) {}
    fn retirer(&self, _: &str) {}
}

fn doublon(cle: &str) -> String {
    format!("La propriété « {cle} » existe déjà.")
}

/// Le dépôt d'un glisser : l'index de la frontière (haut de chaque ligne, puis bas de la dernière)
/// la plus proche de `y`.
#[must_use]
pub fn cible_de_depot(hauts: &[f32], bas_derniere: f32, y: f32) -> usize {
    let bornes = hauts.iter().copied().chain(std::iter::once(bas_derniere));
    let mut meilleure = 0;
    let mut ecart = f32::INFINITY;
    for (i, b) in bornes.enumerate() {
        let e = (b - y).abs();
        if e < ecart {
            ecart = e;
            meilleure = i;
        }
    }
    meilleure
}

/// Le seuil (px) au-delà duquel un appui sur l'icône devient un glisser.
pub const SEUIL_GLISSER: f32 = 4.0;

/// Un appui sur l'icône est un glisser dès que le pointeur s'est éloigné de plus que le seuil
/// (somme des écarts, comme le TS).
#[must_use]
pub fn est_un_glisser(dx: f32, dy: f32) -> bool {
    dx.abs() + dy.abs() > SEUIL_GLISSER
}

fn nulle(cle: &str) -> Property {
    Property {
        key: cle.to_owned(),
        value: PropertyValue::Null,
    }
}
