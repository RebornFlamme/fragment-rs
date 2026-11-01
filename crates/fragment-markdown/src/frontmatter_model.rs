// Port de app/src/ui/views/markdown/properties/frontmatterModel.ts
//! Le modèle PUR des Properties (le frontmatter YAML d'une note, §7.6) :
//! repérer le bloc `---` … `---`, le lire en une liste ORDONNÉE de propriétés,
//! la ré-écrire, et deviner le type « Automatic » d'une valeur.
//!
//! ★ Une liste et non un objet : l'ordre des clés est celui du fichier — un
//!   objet JS remonterait les clés numériques (« 2023 ») en tête.
//! ★ Aucune conversion de valeur au changement de type (comme Obsidian) : le
//!   widget du type affiche ce qu'il sait lire, la valeur n'est réécrite que
//!   quand l'utilisateur l'édite.
//!
//! ★ `PropertyValue` n'est PAS un `serde_json::Value` : sans la feature
//!   `preserve_order` (que le workspace n'active pas), `serde_json::Map` trie
//!   ses clés — une propriété `{ b: 1, a: 2 }` serait réécrite `{ a, b }`, ce
//!   qui violerait « files over app ». Les objets imbriqués gardent donc leur
//!   ordre ici (liste de paires).
//!
//! ★ La sérialisation reproduit la lib `yaml` du TS (stringify avec `nullStr: ''`
//!   et `lineWidth: 0`) : mêmes règles de guillemets, mêmes blocs `|-`, mêmes
//!   `- ` des listes indentées. Voir `emit_*`.
//!
//! Positions (`FrontmatterRange`) : unités de code UTF-16 (ADR-004).

use fragment_text::utf16_len;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};
use yaml_rust2::{Yaml, YamlLoader};

/// Une valeur de propriété : ce qu'un YAML de frontmatter peut porter, ordre des clés gardé.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<PropertyValue>),
    /// Objet imbriqué : paires dans l'ordre du fichier.
    Map(Vec<(String, PropertyValue)>),
}

impl From<&str> for PropertyValue {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}
impl From<String> for PropertyValue {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl From<i64> for PropertyValue {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}
impl From<bool> for PropertyValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

/// Une propriété : clé + valeur.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub key: String,
    pub value: PropertyValue,
}

impl Property {
    pub fn new(key: &str, value: impl Into<PropertyValue>) -> Self {
        Self {
            key: key.to_owned(),
            value: value.into(),
        }
    }
}

/// Les ids d'Obsidian (`types.json`). `tags` n'est jamais proposé, il s'infère.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyType {
    Text,
    Multitext,
    Number,
    Checkbox,
    Date,
    Datetime,
    Tags,
}

/// Le bloc frontmatter dans le document : `from` = début du `---` d'ouverture,
/// `to` = fin du `---` de fermeture (UTF-16) ; `yaml` = les lignes entre les deux.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontmatterRange {
    pub from: usize,
    pub to: usize,
    pub yaml: String,
}

/// Une ligne du document : offsets UTF-16 absolus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocLine {
    pub from: usize,
    pub to: usize,
    pub text: String,
}

/// Ce qu'il faut du document : un texte découpable en lignes (numérotées depuis 1).
pub trait DocLike {
    fn lines(&self) -> usize;
    fn line(&self, n: usize) -> DocLine;
}

fn est_delim(text: &str) -> bool {
    text.strip_prefix("---")
        .is_some_and(|r| r.chars().all(char::is_whitespace))
}

/// Le frontmatter n'existe que si la ligne 1 est `---` ET qu'une ligne `---` le ferme.
pub fn frontmatter_range(doc: &dyn DocLike) -> Option<FrontmatterRange> {
    if doc.lines() < 2 || !est_delim(&doc.line(1).text) {
        return None;
    }
    for n in 2..=doc.lines() {
        let line = doc.line(n);
        if !est_delim(&line.text) {
            continue;
        }
        let yaml = if n == 2 {
            String::new()
        } else {
            text_between(doc, 2, n - 1)
        };
        return Some(FrontmatterRange {
            from: doc.line(1).from,
            to: line.to,
            yaml,
        });
    }
    None
}

fn text_between(doc: &dyn DocLike, a: usize, b: usize) -> String {
    (a..=b)
        .map(|n| doc.line(n).text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Un texte brut → [`DocLike`] (tests, et lecture hors éditeur).
pub struct TextDoc {
    lines: Vec<DocLine>,
}

/// Découpe `text` en lignes (offsets UTF-16).
pub fn doc_of(text: &str) -> TextDoc {
    let mut lines = Vec::new();
    let mut pos = 0;
    for l in text.split('\n') {
        let len = utf16_len(l);
        lines.push(DocLine {
            from: pos,
            to: pos + len,
            text: l.to_owned(),
        });
        pos += len + 1;
    }
    TextDoc { lines }
}

impl DocLike for TextDoc {
    fn lines(&self) -> usize {
        self.lines.len()
    }
    fn line(&self, n: usize) -> DocLine {
        self.lines[n - 1].clone()
    }
}

// ─── Lecture du YAML ──────────────────────────────────────────────────────

/// Le `String(x)` de JS pour un nombre (`1e+21`, `1.5`, `-0` → `0`).
fn js_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
    // Chiffres significatifs et exposant décimal du plus court aller-retour.
    let e = format!("{x:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let k = i32::try_from(digits.len()).unwrap_or(0);
    let n = exp + 1; // position de la virgule
    let corps = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{}", "0".repeat((-n) as usize), digits)
    } else {
        let signe = if n - 1 < 0 { '-' } else { '+' };
        let e10 = (n - 1).abs();
        if k == 1 {
            format!("{digits}e{signe}{e10}")
        } else {
            format!("{}.{}e{signe}{e10}", &digits[..1], &digits[1..])
        }
    };
    if neg { format!("-{corps}") } else { corps }
}

fn nombre(x: f64) -> PropertyValue {
    // ★ JS n'a qu'un type nombre : `1.0` ET `1` sont `1`.
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        PropertyValue::Int(x as i64)
    } else {
        PropertyValue::Float(x)
    }
}

fn reel(s: &str) -> f64 {
    match s {
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => f64::INFINITY,
        "-.inf" | "-.Inf" | "-.INF" => f64::NEG_INFINITY,
        ".nan" | ".NaN" | ".NAN" => f64::NAN,
        _ => s.parse().unwrap_or(f64::NAN),
    }
}

fn depuis_yaml(y: &Yaml) -> PropertyValue {
    match y {
        Yaml::Real(s) => nombre(reel(s)),
        Yaml::Integer(n) => PropertyValue::Int(*n),
        Yaml::String(s) => PropertyValue::Text(s.clone()),
        Yaml::Boolean(b) => PropertyValue::Bool(*b),
        Yaml::Array(a) => PropertyValue::List(a.iter().map(depuis_yaml).collect()),
        Yaml::Hash(h) => {
            PropertyValue::Map(h.iter().map(|(k, v)| (cle_de(k), depuis_yaml(v))).collect())
        }
        Yaml::Alias(_) | Yaml::Null | Yaml::BadValue => PropertyValue::Null,
    }
}

/// `String(k ?? '')` de `parseProperties`.
fn cle_de(k: &Yaml) -> String {
    match k {
        Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => String::new(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Integer(n) => n.to_string(),
        Yaml::Real(s) => js_number(reel(s)),
        Yaml::String(s) => s.clone(),
        Yaml::Array(_) | Yaml::Hash(_) => "[object Object]".into(),
    }
}

/// Les clés déjà vues d'un mapping, et « la prochaine entrée est-elle une clé ? ».
type Cadre = (HashSet<(bool, String)>, bool);

/// Repère les clés de mapping en double (la lib `yaml` refuse « Map keys must be unique » ;
/// yaml-rust2, lui, écrase en silence).
#[derive(Default)]
struct DetecteDoublons {
    /// Une pile : pour chaque mapping ouvert, ses clés déjà vues ; `None` = séquence.
    pile: Vec<Option<Cadre>>,
    doublon: bool,
}

impl DetecteDoublons {
    /// Un nœud (scalaire, alias, début de collection) vient d'être lu dans le parent.
    fn noeud(&mut self, cle: Option<(bool, String)>) {
        if let Some(Some((vues, attend_cle))) = self.pile.last_mut() {
            if *attend_cle
                && let Some(c) = cle
                && !vues.insert(c)
            {
                self.doublon = true;
            }
            *attend_cle = !*attend_cle;
        }
    }
}

impl MarkedEventReceiver for DetecteDoublons {
    fn on_event(&mut self, ev: Event, _mark: Marker) {
        match ev {
            Event::Scalar(s, style, _, _) => {
                self.noeud(Some((style == TScalarStyle::Plain, s)));
            }
            Event::Alias(_) => self.noeud(None),
            Event::SequenceStart(..) => {
                self.noeud(None);
                self.pile.push(None);
            }
            Event::MappingStart(..) => {
                self.noeud(None);
                self.pile.push(Some((HashSet::new(), true)));
            }
            Event::SequenceEnd | Event::MappingEnd => {
                self.pile.pop();
            }
            _ => {}
        }
    }
}

fn a_des_doublons(yaml: &str) -> bool {
    let mut d = DetecteDoublons::default();
    let mut p = Parser::new_from_str(yaml);
    p.load(&mut d, true).is_err() || d.doublon
}

/// Le YAML → propriétés ordonnées ; `None` si invalide ou si ce n'est pas une table clé/valeur.
pub fn parse_properties(yaml: &str) -> Option<Vec<Property>> {
    if yaml.trim().is_empty() {
        return Some(Vec::new());
    }
    let docs = YamlLoader::load_from_str(yaml).ok()?;
    if docs.len() > 1 || a_des_doublons(yaml) {
        return None;
    }
    // Aucun document (que des commentaires) : `doc.contents === null` → [].
    let Some(doc) = docs.first() else {
        return Some(Vec::new());
    };
    let Yaml::Hash(h) = doc else {
        return None;
    };
    Some(
        h.iter()
            .map(|(k, v)| Property {
                key: cle_de(k),
                value: depuis_yaml(v),
            })
            .collect(),
    )
}

// ─── Écriture du YAML (la lib `yaml`) ─────────────────────────────────────

/// Les caractères qui forcent un double-guillemet (`\x00-\x08\x0b-\x1f\x7f-\x9f`).
fn a_un_controle(s: &str) -> bool {
    s.chars().any(|c| {
        let n = c as u32;
        n <= 0x08 || (0x0b..=0x1f).contains(&n) || (0x7f..=0x9f).contains(&n)
    })
}

// ★ Le test de `plainString` de la lib `yaml` : un début/fin/milieu qui rendrait le scalaire ambigu.
static SPECIAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^[\n\t ,\[\]{}#&*!|>'"%@`]|^[?-]$|^[?-][ \t]|[\n:][ \t]|[ \t]\n|[\n\t ]#|[\n\t :]$"#,
    )
    .unwrap_or_else(|_| unreachable!("regex constante"))
});

// ★ Les tags du schéma « core » : une chaîne qui se lirait comme null/bool/nombre est guillemetée.
static TAGS_CORE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^(?:~|[Nn]ull|NULL)?$",
        r"^(?:[Tt]rue|TRUE|[Ff]alse|FALSE)$",
        r"^[-+]?[0-9]+$",
        r"^0o[0-7]+$",
        r"^0x[0-9a-fA-F]+$",
        r"^(?:[-+]?\.(?:inf|Inf|INF)|\.nan|\.NaN|\.NAN)$",
        r"^[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)[eE][-+]?[0-9]+$",
        r"^[-+]?(?:\.[0-9]+|[0-9]+\.[0-9]*)$",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

fn double_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\x0b' => out.push_str("\\v"),
            '\x0c' => out.push_str("\\f"),
            '\x1b' => out.push_str("\\e"),
            '\u{85}' => out.push_str("\\N"),
            '\u{a0}' => out.push_str("\\_"),
            '\u{2028}' => out.push_str("\\L"),
            '\u{2029}' => out.push_str("\\P"),
            c if (c as u32) < 0x20 || (0x7f..=0x9f).contains(&(c as u32)) => {
                out.push_str(&format!("\\x{:02X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// `quotedString` : double par défaut, simple si la chaîne contient `"` sans `'`.
fn quoted(s: &str, cle_implicite: bool) -> String {
    let a_double = s.contains('"');
    let a_simple = s.contains('\'');
    if a_double && !a_simple && !(cle_implicite && s.contains('\n')) && !s.contains('\n') {
        single_quote(s)
    } else {
        double_quote(s)
    }
}

/// Comment une chaîne s'écrit : sur la ligne, ou en bloc littéral `|`.
enum Rendu {
    EnLigne(String),
    Bloc(String),
}

fn rendre_texte(s: &str, cle_implicite: bool) -> Rendu {
    if a_un_controle(s) {
        return Rendu::EnLigne(double_quote(s));
    }
    let multiligne = s.contains('\n');
    if cle_implicite && multiligne {
        return Rendu::EnLigne(quoted(s, true));
    }
    if s.is_empty() || SPECIAL.is_match(s) {
        return if cle_implicite || !multiligne {
            Rendu::EnLigne(quoted(s, cle_implicite))
        } else {
            Rendu::Bloc(s.to_owned())
        };
    }
    if TAGS_CORE.iter().any(|r| r.is_match(s)) {
        return Rendu::EnLigne(quoted(s, cle_implicite));
    }
    if !cle_implicite && multiligne {
        return Rendu::Bloc(s.to_owned());
    }
    Rendu::EnLigne(s.to_owned())
}

/// Les lignes d'un bloc littéral : l'en-tête (`|-`, `|`, `|+`) puis le contenu indenté de `indent`.
fn bloc_litteral(s: &str, indent: usize) -> (String, Vec<String>) {
    let corps = s.trim_end_matches('\n');
    let finaux = s.len() - corps.len();
    let chomp = match finaux {
        0 => "-",
        1 => "",
        _ => "+",
    };
    let indicateur = if corps.trim_start_matches('\n').starts_with(' ') {
        "2"
    } else {
        ""
    };
    let pad = " ".repeat(indent);
    let mut lignes: Vec<String> = corps
        .split('\n')
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{pad}{l}")
            }
        })
        .collect();
    for _ in 1..finaux {
        lignes.push(String::new());
    }
    (format!("|{indicateur}{chomp}"), lignes)
}

fn est_scalaire(v: &PropertyValue) -> bool {
    !matches!(v, PropertyValue::List(_) | PropertyValue::Map(_))
}

/// Un scalaire (ou collection vide) sur une ligne, ou son bloc : (texte de tête, lignes de bloc).
fn scalaire(v: &PropertyValue, indent_contenu: usize) -> (String, Vec<String>) {
    match v {
        PropertyValue::Null => (String::new(), Vec::new()),
        PropertyValue::Bool(b) => (b.to_string(), Vec::new()),
        PropertyValue::Int(n) => (n.to_string(), Vec::new()),
        PropertyValue::Float(x) => (
            if x.is_nan() {
                ".nan".into()
            } else if x.is_infinite() {
                if *x < 0.0 { "-.inf" } else { ".inf" }.into()
            } else {
                js_number(*x)
            },
            Vec::new(),
        ),
        PropertyValue::Text(s) => match rendre_texte(s, false) {
            Rendu::EnLigne(t) => (t, Vec::new()),
            Rendu::Bloc(b) => bloc_litteral(&b, indent_contenu),
        },
        PropertyValue::List(_) => ("[]".into(), Vec::new()),
        PropertyValue::Map(_) => ("{}".into(), Vec::new()),
    }
}

fn cle_yaml(k: &str) -> String {
    match rendre_texte(k, true) {
        Rendu::EnLigne(t) | Rendu::Bloc(t) => t,
    }
}

/// Les lignes d'une entrée `clé: valeur` à l'indentation `indent`.
fn lignes_entree(key: &str, value: &PropertyValue, indent: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    let k = cle_yaml(key);
    match value {
        PropertyValue::List(items) if !items.is_empty() => {
            let mut out = vec![format!("{pad}{k}:")];
            out.extend(lignes_liste(items, indent + 2));
            out
        }
        PropertyValue::Map(entries) if !entries.is_empty() => {
            let mut out = vec![format!("{pad}{k}:")];
            out.extend(lignes_objet(entries, indent + 2));
            out
        }
        v => {
            let (tete, bloc) = scalaire(v, indent + 2);
            let ligne = if tete.is_empty() {
                format!("{pad}{k}:")
            } else {
                format!("{pad}{k}: {tete}")
            };
            std::iter::once(ligne).chain(bloc).collect()
        }
    }
}

fn lignes_objet(entries: &[(String, PropertyValue)], indent: usize) -> Vec<String> {
    entries
        .iter()
        .flat_map(|(k, v)| lignes_entree(k, v, indent))
        .collect()
}

fn lignes_liste(items: &[PropertyValue], indent: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    let mut out = Vec::new();
    for item in items {
        match item {
            PropertyValue::List(sub) if !sub.is_empty() => {
                out.extend(imbriquer(&pad, lignes_liste(sub, indent + 2), indent + 2));
            }
            PropertyValue::Map(sub) if !sub.is_empty() => {
                out.extend(imbriquer(&pad, lignes_objet(sub, indent + 2), indent + 2));
            }
            v => {
                debug_assert!(
                    est_scalaire(v) || matches!(v, PropertyValue::List(_) | PropertyValue::Map(_))
                );
                let (tete, bloc) = scalaire(v, indent + 2);
                // ★ `- ` suivi de rien (élément null) garde son espace, comme la lib `yaml`.
                out.push(format!("{pad}- {tete}"));
                out.extend(bloc);
            }
        }
    }
    out
}

/// `- ` collé devant la première ligne d'une collection imbriquée.
fn imbriquer(pad: &str, lignes: Vec<String>, indent: usize) -> Vec<String> {
    lignes
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                format!("{pad}- {}", &l[indent.min(l.len())..])
            } else {
                l
            }
        })
        .collect()
}

/// Propriétés → YAML (une propriété par bloc, listes en `- a`, `null` → `clé:`).
pub fn serialize_properties(props: &[Property]) -> String {
    props
        .iter()
        .flat_map(|p| lignes_entree(&p.key, &p.value, 0))
        .map(|l| l + "\n")
        .collect()
}

/// Le bloc complet `---\n…---`, prêt à remplacer `FrontmatterRange`.
pub fn serialize_frontmatter(props: &[Property]) -> String {
    format!("---\n{}---", serialize_properties(props))
}

// ─── Opérations (toujours une NOUVELLE liste) ─────────────────────────────

pub fn set_value(props: &[Property], index: usize, value: PropertyValue) -> Vec<Property> {
    let mut out = props.to_vec();
    if let Some(p) = out.get_mut(index) {
        p.value = value;
    }
    out
}

pub fn set_key(props: &[Property], index: usize, key: &str) -> Vec<Property> {
    let mut out = props.to_vec();
    if let Some(p) = out.get_mut(index) {
        p.key = key.to_owned();
    }
    out
}

pub fn insert_at(props: &[Property], index: usize, prop: Property) -> Vec<Property> {
    let mut out = props.to_vec();
    out.insert(index.min(out.len()), prop);
    out
}

pub fn remove_at(props: &[Property], index: usize) -> Vec<Property> {
    props
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, p)| p.clone())
        .collect()
}

/// Déplace la propriété `from` pour qu'elle occupe l'index `to` (dans la liste finale).
pub fn move_prop(props: &[Property], from: usize, to: usize) -> Vec<Property> {
    let mut out = props.to_vec();
    if from >= out.len() {
        return out;
    }
    let p = out.remove(from);
    out.insert(to.min(out.len()), p);
    out
}

/// Fusionne `incoming` (un collage) après `index` : une clé déjà présente est remplacée sur place.
pub fn merge_at(props: &[Property], index: usize, incoming: &[Property]) -> Vec<Property> {
    let mut out = props.to_vec();
    let mut at = index + 1;
    for p in incoming {
        if let Some(i) = out.iter().position(|q| q.key == p.key) {
            out = set_value(&out, i, p.value.clone());
        } else {
            out = insert_at(&out, at, p.clone());
            at += 1;
        }
    }
    out
}

// ─── Types ────────────────────────────────────────────────────────────────

static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$").unwrap_or_else(|_| unreachable!("regex constante"))
});
static DATETIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9]{2}:[0-9]{2}(:[0-9]{2})?$")
        .unwrap_or_else(|_| unreachable!("regex constante"))
});

/// Le type « Automatic » : celui que la clé ou la valeur suggère.
pub fn infer_type(key: &str, value: &PropertyValue) -> PropertyType {
    let k = key.to_lowercase();
    if k == "tags" {
        return PropertyType::Tags;
    }
    if k == "aliases" || k == "cssclasses" || matches!(value, PropertyValue::List(_)) {
        return PropertyType::Multitext;
    }
    match value {
        PropertyValue::Bool(_) => PropertyType::Checkbox,
        PropertyValue::Int(_) | PropertyValue::Float(_) => PropertyType::Number,
        PropertyValue::Text(s) if DATE.is_match(s) => PropertyType::Date,
        PropertyValue::Text(s) if DATETIME.is_match(s) => PropertyType::Datetime,
        _ => PropertyType::Text,
    }
}

/// Le `String(v)` de JS.
fn js_string(v: &PropertyValue) -> String {
    match v {
        PropertyValue::Null => "null".into(),
        PropertyValue::Bool(b) => b.to_string(),
        PropertyValue::Int(n) => n.to_string(),
        PropertyValue::Float(x) => js_number(*x),
        PropertyValue::Text(s) => s.clone(),
        // `String([1, null])` = « 1, » : null et undefined deviennent vides dans un join.
        PropertyValue::List(items) => items
            .iter()
            .map(|i| {
                if *i == PropertyValue::Null {
                    String::new()
                } else {
                    js_string(i)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        PropertyValue::Map(_) => "[object Object]".into(),
    }
}

/// Une valeur lue comme liste de chaînes (List, Tags).
pub fn as_list(value: &PropertyValue) -> Vec<String> {
    match value {
        PropertyValue::Null => Vec::new(),
        PropertyValue::Text(s) if s.is_empty() => Vec::new(),
        PropertyValue::List(items) => items
            .iter()
            .filter(|v| **v != PropertyValue::Null)
            .map(js_string)
            .collect(),
        autre => vec![js_string(autre)],
    }
}

/// Le `JSON.stringify` d'une valeur (objets imbriqués de `as_text`).
fn json(v: &PropertyValue) -> String {
    match v {
        PropertyValue::Null => "null".into(),
        PropertyValue::Bool(b) => b.to_string(),
        PropertyValue::Int(n) => n.to_string(),
        PropertyValue::Float(x) => {
            if x.is_finite() {
                js_number(*x)
            } else {
                "null".into()
            }
        }
        PropertyValue::Text(s) => serde_json::Value::String(s.clone()).to_string(),
        PropertyValue::List(items) => {
            format!("[{}]", items.iter().map(json).collect::<Vec<_>>().join(","))
        }
        PropertyValue::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{}:{}", serde_json::Value::String(k.clone()), json(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// Une valeur lue comme texte (Text).
pub fn as_text(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Null => String::new(),
        PropertyValue::List(items) => items.iter().map(js_string).collect::<Vec<_>>().join(", "),
        PropertyValue::Map(_) => json(value),
        autre => js_string(autre),
    }
}

/// Un tag s'écrit sans `#` dans le frontmatter.
pub fn normaliser_tag(tag: &str) -> String {
    let t = tag.trim().trim_start_matches('#');
    let mut out = String::new();
    let mut dans_blancs = false;
    for c in t.chars() {
        if c.is_whitespace() {
            if !dans_blancs {
                out.push('-');
            }
            dans_blancs = true;
        } else {
            out.push(c);
            dans_blancs = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_text::slice_utf16;

    fn t(s: &str) -> PropertyValue {
        s.into()
    }
    fn liste(a: &[PropertyValue]) -> PropertyValue {
        PropertyValue::List(a.to_vec())
    }
    fn cles(p: &[Property]) -> Vec<&str> {
        p.iter().map(|x| x.key.as_str()).collect()
    }

    // ── frontmatterRange ──
    #[test]
    fn frontmatter_range_repere_le_bloc_ferme_en_ligne_1() {
        let text = "---\ntags: a\n---\ncorps";
        let r = frontmatter_range(&doc_of(text));
        assert_eq!(
            r,
            Some(FrontmatterRange {
                from: 0,
                to: 15,
                yaml: "tags: a".into()
            })
        );
        let r = r.unwrap_or_else(|| unreachable!());
        assert_eq!(slice_utf16(text, r.from, r.to), Some("---\ntags: a\n---"));
    }
    #[test]
    fn frontmatter_range_bloc_vide() {
        assert_eq!(
            frontmatter_range(&doc_of("---\n---")),
            Some(FrontmatterRange {
                from: 0,
                to: 7,
                yaml: String::new()
            })
        );
    }
    #[test]
    fn frontmatter_range_pas_de_fermeture_ou_pas_en_ligne_1_null() {
        assert_eq!(frontmatter_range(&doc_of("---\ntags: a")), None);
        assert_eq!(frontmatter_range(&doc_of("x\n---\na: 1\n---")), None);
        assert_eq!(frontmatter_range(&doc_of("---")), None);
    }

    // ── parseProperties ──
    #[test]
    fn parse_properties_garde_l_ordre_du_fichier_meme_pour_les_cles_numeriques() {
        assert_eq!(
            parse_properties("b: 1\n2023: x\ntest:\ntags:\n  - a"),
            Some(vec![
                Property::new("b", 1),
                Property::new("2023", "x"),
                Property::new("test", PropertyValue::Null),
                Property::new("tags", liste(&[t("a")])),
            ])
        );
    }
    #[test]
    fn parse_properties_vide() {
        assert_eq!(parse_properties(""), Some(vec![]));
        assert_eq!(parse_properties("  \n"), Some(vec![]));
    }
    #[test]
    fn parse_properties_invalide_ou_pas_une_table_null() {
        assert_eq!(parse_properties("a: ["), None);
        assert_eq!(parse_properties("Bonjour le monde"), None);
        assert_eq!(parse_properties("- a\n- b"), None);
    }

    // ── serializeProperties ──
    #[test]
    fn serialize_properties_ecrit_a_la_maniere_d_obsidian() {
        assert_eq!(
            serialize_properties(&[
                Property::new("test", PropertyValue::Null),
                Property::new("tags", liste(&[t("a"), t("b")])),
                Property::new("n", 3),
                Property::new("ok", true),
                Property::new("d", "2024-01-02"),
            ]),
            "test:\ntags:\n  - a\n  - b\nn: 3\nok: true\nd: 2024-01-02\n"
        );
    }
    #[test]
    fn serialize_properties_aller_retour() {
        let props = vec![
            Property::new("s", "a: b"),
            Property::new("2023", "x"),
            Property::new("l", liste(&[])),
        ];
        assert_eq!(parse_properties(&serialize_properties(&props)), Some(props));
    }
    #[test]
    fn serialize_properties_bloc_complet() {
        assert_eq!(serialize_frontmatter(&[]), "---\n---");
        assert_eq!(
            serialize_frontmatter(&[Property::new("test", PropertyValue::Null)]),
            "---\ntest:\n---"
        );
    }

    // ── opérations ──
    fn trois() -> Vec<Property> {
        vec![
            Property::new("a", 1),
            Property::new("b", 2),
            Property::new("c", 3),
        ]
    }
    #[test]
    fn operations_move() {
        assert_eq!(cles(&move_prop(&trois(), 1, 0)), vec!["b", "a", "c"]);
        assert_eq!(cles(&move_prop(&trois(), 0, 2)), vec!["b", "c", "a"]);
    }
    #[test]
    fn operations_merge_at_remplace_sur_place_insere_le_reste_apres_l_index() {
        assert_eq!(
            merge_at(&trois(), 0, &[Property::new("c", 9), Property::new("z", 0)]),
            vec![
                Property::new("a", 1),
                Property::new("z", 0),
                Property::new("b", 2),
                Property::new("c", 9),
            ]
        );
    }

    // ── types ──
    #[test]
    fn types_infer_type() {
        use PropertyType::*;
        let n = PropertyValue::Null;
        assert_eq!(infer_type("tags", &n), Tags);
        assert_eq!(infer_type("aliases", &n), Multitext);
        assert_eq!(infer_type("x", &liste(&[t("a")])), Multitext);
        assert_eq!(infer_type("x", &true.into()), Checkbox);
        assert_eq!(infer_type("x", &2.into()), Number);
        assert_eq!(infer_type("x", &t("2024-01-02")), Date);
        assert_eq!(infer_type("x", &t("2024-01-02T10:30")), Datetime);
        assert_eq!(infer_type("x", &t("2023-2024")), Text);
        assert_eq!(infer_type("x", &n), Text);
    }
    #[test]
    fn types_lectures() {
        assert_eq!(as_list(&PropertyValue::Null), Vec::<String>::new());
        assert_eq!(as_list(&t("a")), vec!["a".to_owned()]);
        assert_eq!(
            as_list(&liste(&[1.into(), t("b")])),
            vec!["1".to_owned(), "b".to_owned()]
        );
        assert_eq!(as_text(&liste(&[t("a"), t("b")])), "a, b");
        assert_eq!(as_text(&PropertyValue::Null), "");
        assert_eq!(normaliser_tag("#mon tag"), "mon-tag");
    }

    // ── Ajouts du port : la sérialisation de la lib `yaml` (cas hors des tests TS) ──
    #[test]
    fn port_serialize_guillemets_comme_la_lib_yaml() {
        let ser = |v: PropertyValue| serialize_properties(&[Property::new("k", v)]);
        assert_eq!(ser(t("true")), "k: \"true\"\n");
        assert_eq!(ser(t("12")), "k: \"12\"\n");
        assert_eq!(ser(t("")), "k: \"\"\n");
        assert_eq!(ser(t("null")), "k: \"null\"\n");
        assert_eq!(ser(t("a #b")), "k: \"a #b\"\n");
        assert_eq!(ser(t("- x")), "k: \"- x\"\n");
        assert_eq!(ser(t("fin:")), "k: \"fin:\"\n");
        assert_eq!(ser(t("dit \"oui\"")), "k: dit \"oui\"\n");
        assert_eq!(ser(t("\"oui\" dit")), "k: '\"oui\" dit'\n");
        assert_eq!(ser(t("l'a")), "k: l'a\n");
        assert_eq!(ser(t("é et 😀")), "k: é et 😀\n");
        assert_eq!(ser(t("a\tb")), "k: a\tb\n");
        assert_eq!(ser(t("x\u{1}y")), "k: \"x\\x01y\"\n");
    }
    #[test]
    fn port_serialize_blocs_et_imbrications() {
        let ser = |v: PropertyValue| serialize_properties(&[Property::new("k", v)]);
        assert_eq!(ser(t("l1\nl2")), "k: |-\n  l1\n  l2\n");
        assert_eq!(ser(t("l1\nl2\n")), "k: |\n  l1\n  l2\n");
        assert_eq!(
            ser(PropertyValue::Map(vec![
                ("b".into(), 1.into()),
                (
                    "a".into(),
                    liste(&[t("x"), PropertyValue::Map(vec![("n".into(), 2.into())])])
                ),
            ])),
            "k:\n  b: 1\n  a:\n    - x\n    - n: 2\n"
        );
        assert_eq!(
            ser(liste(&[liste(&[t("a"), t("b")])])),
            "k:\n  - - a\n    - b\n"
        );
        assert_eq!(ser(PropertyValue::Float(1.5)), "k: 1.5\n");
        assert_eq!(ser(PropertyValue::Map(vec![])), "k: {}\n");
        assert_eq!(
            serialize_properties(&[Property::new("a: b", 1)]),
            "\"a: b\": 1\n"
        );
    }
    /// ★ Sorties relevées sur la VRAIE lib `yaml` (`stringify({ k: v }, { nullStr: '', lineWidth: 0 })`).
    #[test]
    fn port_serialize_conforme_a_la_lib_yaml() {
        let ser = |v: PropertyValue| serialize_properties(&[Property::new("k", v)]);
        for (texte, attendu) in [
            ("1e3", "k: \"1e3\"\n"),
            (".5", "k: \".5\"\n"),
            ("0x1F", "k: \"0x1F\"\n"),
            ("yes", "k: yes\n"),
            ("?", "k: \"?\"\n"),
            ("?x", "k: ?x\n"),
            ("@at", "k: \"@at\"\n"),
            ("[x]", "k: \"[x]\"\n"),
            ("a, b", "k: a, b\n"),
            (" lead", "k: \" lead\"\n"),
            ("trailing ", "k: \"trailing \"\n"),
            ("l1\n\nl3", "k: |-\n  l1\n\n  l3\n"),
            ("x\ny: z", "k: |-\n  x\n  y: z\n"),
            ("l1\nl2\n\n", "k: |+\n  l1\n  l2\n\n"),
            ("  a\nb", "k: |2-\n    a\n  b\n"),
            ("tab\u{a0}x", "k: tab\u{a0}x\n"),
        ] {
            assert_eq!(ser(t(texte)), attendu, "chaîne {texte:?}");
        }
        assert_eq!(
            ser(liste(&[PropertyValue::Null, t("x")])),
            "k:\n  - \n  - x\n"
        );
        assert_eq!(
            ser(liste(&[liste(&[]), PropertyValue::Map(vec![])])),
            "k:\n  - []\n  - {}\n"
        );
        assert_eq!(ser(PropertyValue::Float(1e21)), "k: 1e+21\n");
    }

    #[test]
    fn port_parse_doublons_et_ordre_des_objets_imbriques() {
        assert_eq!(parse_properties("a: 1\na: 2"), None);
        assert_eq!(
            parse_properties("k:\n  z: 1\n  a: 2"),
            Some(vec![Property::new(
                "k",
                PropertyValue::Map(vec![("z".into(), 1.into()), ("a".into(), 2.into())])
            )])
        );
        // Une date reste une chaîne (schéma core de la lib `yaml`).
        assert_eq!(
            parse_properties("d: 2024-01-02"),
            Some(vec![Property::new("d", "2024-01-02")])
        );
        assert_eq!(parse_properties("# rien"), Some(vec![]));
        assert_eq!(js_number(1e21), "1e+21");
        assert_eq!(js_number(0.000001), "0.000001");
        assert_eq!(js_number(1e-7), "1e-7");
        assert_eq!(js_number(123456.5), "123456.5");
    }
    #[test]
    fn port_frontmatter_range_positions_utf16() {
        // « é😀 » = 3 unités UTF-16 (1 + 2) : `to` compte en UTF-16, pas en octets.
        let text = "---\na: é😀\n---\nx";
        let r = frontmatter_range(&doc_of(text));
        assert_eq!(r.map(|r| r.to), Some(4 + 7 + 3));
    }
}
