// Port de app/src/ui/core/artefacts/types.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Contrat des artefacts, modèle du journal (journal-artefacts.pdf, partie III).
//
//  L'état d'un artefact n'est plus stocké : seuls ses changements le sont. Chacun forme
//  une transaction (Tx), écrite en une ligne à la fin du segment de son écrivain.
//  L'index résulte du PLI de ces
//  lignes (fold.rs) ; le cache n'en garde qu'une copie jetable (§12, R1-R6).
//
//  ★ LA RÈGLE (inchangée) : on stocke le NOM, on enregistre le VERBE. Le cœur ne
//    lit jamais `state` (invariant §19.1-7) : le plugin possède son schéma.
//
//  ★ Sérialisation : ces types SONT le format du fichier (NDJSON, lu aussi par le
//    Fragment TypeScript). L'ordre des champs reproduit celui de `JSON.stringify`
//    côté TS ; un champ absent en TS (`undefined`) est un `None` ici et n'est pas
//    écrit ; un champ présent et `null` (`anchor: null`) est un `Some(None)`.
// ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

/// L'estampille : `2026-09-29T17:20:00.000Z-0003-k3f9-b71` — l'heure ISO à la
/// milliseconde, un compteur sur 4 chiffres hexadécimaux, l'écrivain. Elle se lit
/// dans le fichier et se trie comme une chaîne (§14.3, format d'Actual Budget).
pub type Stamp = String;

/// Ce qui a voulu la transaction (§13). `disk` n'est jamais écrit : c'est la marque
/// posée À LA LECTURE sur ce qui vient d'un autre écrivain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    User,
    Plugin,
    Follow,
    Vault,
    Migrate,
    Import,
    Disk,
}

/// Où vit un artefact. Trois formes : un document, une collection propre à un
/// plugin, l'application entière.
///
/// ★ `{kind:'file', path}` désigne un DOCUMENT : `path` est une CLÉ de document
///   (chemin du coffre OU `schéma:reste`, documents/types.ts). Le nom `file` est
///   gardé pour le journal (aucune migration, lisible par les clients anciens).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ArtefactScope {
    File { path: String },
    Plugin { collection: String },
    Global,
}

impl ArtefactScope {
    /// Le scope « sur un document » (`DocScope` côté TS).
    pub fn file(path: impl Into<String>) -> Self {
        ArtefactScope::File { path: path.into() }
    }

    pub fn plugin(collection: impl Into<String>) -> Self {
        ArtefactScope::Plugin {
            collection: collection.into(),
        }
    }

    pub fn kind(&self) -> ScopeKind {
        match self {
            ArtefactScope::File { .. } => ScopeKind::File,
            ArtefactScope::Plugin { .. } => ScopeKind::Plugin,
            ArtefactScope::Global => ScopeKind::Global,
        }
    }
}

/// Le discriminant d'un scope (`ArtefactScope['kind']`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeKind {
    File,
    Plugin,
    Global,
}

/// Un filtre de scope (façade `list_sync`) — chaque champ affine, aucun n'est requis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeFilter {
    pub kind: Option<ScopeKind>,
    pub path: Option<String>,
    pub collection: Option<String>,
}

impl ScopeFilter {
    pub fn path(path: impl Into<String>) -> Self {
        ScopeFilter {
            path: Some(path.into()),
            ..Default::default()
        }
    }

    pub fn collection(collection: impl Into<String>) -> Self {
        ScopeFilter {
            collection: Some(collection.into()),
            ..Default::default()
        }
    }
}

/// La citation (§13, §15.7) : 32 caractères avant l'offset, les 32 suivants, puis
/// 32 encore. Elle rend un offset VÉRIFIABLE, et le retrouve si le texte a changé
/// hors de l'app. Le PDF n'en a pas : ses offsets sont fixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorQuote {
    pub prefix: String,
    pub exact: String,
    pub suffix: String,
}

/// Le COIN du pane depuis lequel se mesure une ancre `viewport`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportCorner {
    #[serde(rename = "top-left")]
    TopLeft,
    #[serde(rename = "top-right")]
    TopRight,
    #[serde(rename = "bottom-left")]
    BottomLeft,
    #[serde(rename = "bottom-right")]
    BottomRight,
}

/// Le côté de la marge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GutterSide {
    Left,
    Right,
}

/// L'ancre vivante de l'API layers (`layers/WidgetLayer.ts`).
///
/// ★ Copie MINIMALE du type de `fragment-layers` (contrats.md : `enum WidgetAnchor`,
///   fragment-layers) : cette crate l'écrit dans le journal et doit en connaître la
///   forme exacte ; quand fragment-layers l'exposera, l'une des deux crates
///   réexportera l'autre (à trancher par le chef, voir la PR). `pos`/`to` sont des
///   `i64` comme le `number` TS (un offset négatif se borne, il ne lève pas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum WidgetAnchor {
    Viewport {
        #[serde(with = "js_number")]
        x: f64,
        #[serde(with = "js_number")]
        y: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<ViewportCorner>,
    },
    Document {
        pos: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<i64>,
        #[serde(with = "js_number")]
        dx: f64,
        #[serde(with = "js_number")]
        dy: f64,
    },
    Gutter {
        side: GutterSide,
        pos: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<i64>,
        #[serde(with = "js_number")]
        dy: f64,
    },
}

impl WidgetAnchor {
    /// Une ancre au point `pos` du document, sans décalage.
    pub fn document(pos: i64) -> Self {
        WidgetAnchor::Document {
            pos,
            to: None,
            dx: 0.0,
            dy: 0.0,
        }
    }

    /// Les ancres qui suivent le TEXTE (`suitLeTexte`) : celles dont le `pos` doit être remappé.
    pub fn suit_le_texte(&self) -> bool {
        !matches!(self, WidgetAnchor::Viewport { .. })
    }

    /// `pos` d'une ancre au texte (`None` pour une ancre viewport).
    pub fn pos(&self) -> Option<i64> {
        match self {
            WidgetAnchor::Viewport { .. } => None,
            WidgetAnchor::Document { pos, .. } | WidgetAnchor::Gutter { pos, .. } => Some(*pos),
        }
    }

    /// `to` d'une ancre au texte (une PLAGE), s'il est posé.
    pub fn to(&self) -> Option<i64> {
        match self {
            WidgetAnchor::Viewport { .. } => None,
            WidgetAnchor::Document { to, .. } | WidgetAnchor::Gutter { to, .. } => *to,
        }
    }

    /// La même ancre, `pos` et `to` remplacés (`to` est posé seulement s'il l'était).
    pub fn with_range(&self, pos: i64, to: Option<i64>) -> Self {
        match self {
            WidgetAnchor::Viewport { .. } => self.clone(),
            WidgetAnchor::Document { dx, dy, .. } => WidgetAnchor::Document {
                pos,
                to,
                dx: *dx,
                dy: *dy,
            },
            WidgetAnchor::Gutter { side, dy, .. } => WidgetAnchor::Gutter {
                side: *side,
                pos,
                to,
                dy: *dy,
            },
        }
    }
}

/// L'ancre durable : l'ancre de l'API layers, à laquelle s'ajoute une citation (§12).
///
/// ★ Un champ inconnu (l'ancien indice `char`) est ignoré à la lecture : il « ne
///   passe pas » (anchor.ts `toLive`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurableAnchor {
    #[serde(flatten)]
    pub anchor: WidgetAnchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<AnchorQuote>,
}

impl From<WidgetAnchor> for DurableAnchor {
    fn from(anchor: WidgetAnchor) -> Self {
        DurableAnchor {
            anchor,
            quote: None,
        }
    }
}

/// La plus petite unité de changement (§13, §14.4). `set` affecte des champs d'un
/// record, groupe par groupe ; `del` pose une tombe.
/// (`Del` est petit, `Set` est gros : une indirection par op coûterait plus que les
/// octets vides d'un `del`, rare dans un journal.)
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Op {
    Set(SetOp),
    Del { del: String },
}

impl Op {
    pub fn del(id: impl Into<String>) -> Self {
        Op::Del { del: id.into() }
    }

    /// L'id du record touché.
    pub fn id(&self) -> &str {
        match self {
            Op::Set(op) => &op.set,
            Op::Del { del } => del,
        }
    }

    pub fn as_set(&self) -> Option<&SetOp> {
        match self {
            Op::Set(op) => Some(op),
            Op::Del { .. } => None,
        }
    }

    pub fn is_del(&self) -> bool {
        matches!(self, Op::Del { .. })
    }
}

/// Un `set` : chaque groupe est présent (`Some`) ou absent (`None`) — c'est le
/// `'type' in op` du TS. `anchor: Some(None)` = `anchor: null` (ancre retirée).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SetOp {
    /// L'id du record.
    pub set: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
    /// Le state inline…
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
    /// … ou l'empreinte d'un objet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ArtefactScope>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option"
    )]
    pub anchor: Option<Option<DurableAnchor>>,
    /// À la création seulement.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub local: bool,
}

impl SetOp {
    pub fn new(id: impl Into<String>) -> Self {
        SetOp {
            set: id.into(),
            ..Default::default()
        }
    }

    /// Le groupe contenu (`type` + `v` + `state`) — pour les tests et les fixtures.
    pub fn content(mut self, type_: &str, v: u32, state: Value) -> Self {
        self.type_ = Some(type_.to_string());
        self.v = Some(v);
        self.state = Some(state);
        self
    }

    pub fn scope(mut self, scope: ArtefactScope) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn anchor(mut self, anchor: impl Into<DurableAnchor>) -> Self {
        self.anchor = Some(Some(anchor.into()));
        self
    }
}

impl From<SetOp> for Op {
    fn from(op: SetOp) -> Self {
        Op::Set(op)
    }
}

/// Un groupe d'opérations appliqué d'un bloc : une ligne du journal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub t: Stamp,
    pub o: Origin,
    pub ops: Vec<Op>,
}

/// Le record plié (§14.4). Quatre groupes de champs, chacun avec l'estampille de
/// sa valeur courante : contenu, scope, ancre, tombe.
///
/// `local` n'est pas un groupe : il dit dans quel journal (coffre ou appareil) le
/// record vit, pour que ses écritures suivantes y retournent (§14.1 : « ce choix ne
/// change plus »).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rec {
    pub id: String,
    // groupe contenu
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(rename = "contentT", default, skip_serializing_if = "Option::is_none")]
    pub content_t: Option<Stamp>,
    // groupe scope
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ArtefactScope>,
    #[serde(rename = "scopeT", default, skip_serializing_if = "Option::is_none")]
    pub scope_t: Option<Stamp>,
    // groupe ancre
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option"
    )]
    pub anchor: Option<Option<DurableAnchor>>,
    #[serde(rename = "anchorT", default, skip_serializing_if = "Option::is_none")]
    pub anchor_t: Option<Stamp>,
    // tombe
    #[serde(rename = "delT", default, skip_serializing_if = "Option::is_none")]
    pub del_t: Option<Stamp>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub local: bool,
}

impl Rec {
    pub fn new(id: impl Into<String>) -> Self {
        Rec {
            id: id.into(),
            ..Default::default()
        }
    }

    /// L'ancre courante (aplatie : `anchor: null` et « pas d'ancre » se lisent pareil).
    pub fn anchor(&self) -> Option<&DurableAnchor> {
        self.anchor.as_ref().and_then(|a| a.as_ref())
    }
}

/// Les champs qu'un `set` affecte. `state` est rangé inline ou en objet selon sa taille (§14.5).
/// Un champ `None` est ABSENT (`'state' in fields` est faux) ; `anchor: Some(None)` retire l'ancre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {
    pub type_: Option<String>,
    pub v: Option<u32>,
    pub state: Option<Value>,
    pub scope: Option<ArtefactScope>,
    pub anchor: Option<Option<DurableAnchor>>,
    /// Un record de l'appareil, jamais synchronisé — à la création seulement (§14.1).
    pub local: bool,
}

impl Fields {
    pub fn new() -> Self {
        Fields::default()
    }

    pub fn type_(mut self, type_: &str) -> Self {
        self.type_ = Some(type_.to_string());
        self
    }

    pub fn v(mut self, v: u32) -> Self {
        self.v = Some(v);
        self
    }

    pub fn state(mut self, state: Value) -> Self {
        self.state = Some(state);
        self
    }

    pub fn scope(mut self, scope: ArtefactScope) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn anchor(mut self, anchor: impl Into<DurableAnchor>) -> Self {
        self.anchor = Some(Some(anchor.into()));
        self
    }

    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }
}

/// Un filtre de lecture et d'abonnement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub id: Option<String>,
    pub type_: Option<String>,
    pub scope: Option<ScopeFilter>,
}

impl Filter {
    pub fn scope(scope: ScopeFilter) -> Self {
        Filter {
            scope: Some(scope),
            ..Default::default()
        }
    }

    pub fn path(path: impl Into<String>) -> Self {
        Filter::scope(ScopeFilter::path(path))
    }
}

/// Ce qu'une transaction a changé à un record : avant / après (`None` = pas vivant).
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub id: String,
    pub before: Option<Rec>,
    pub after: Option<Rec>,
}

/// Ce que rend `ArtefactKind::describe` (« Calques ▸ Artefacts », `artefacts:list`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    pub label: String,
    pub icon: Option<String>,
    /// Le calque auquel l'artefact APPARTIENT ; absent = le calque Artefacts.
    pub layer: Option<String>,
}

/// La déclaration d'un KIND — le comportement, contribué par un plugin via
/// `registerArtefactKind`. Un `type` sans kind enregistré → record INERTE.
///
/// ★ `rehydrate(state, ctx) → ArtefactInstance` et `ArtefactContext` (vue, éditeur,
///   fichier, handle de widget) sont la PRÉSENTATION : ils tiennent au calque et
///   vivent dans fragment-ui (invariant §19.1-9 : le DOM sort de `rehydrate`). Le
///   cœur n'a besoin ici que de ce qu'il lit : le type, la version, la description.
pub struct ArtefactKind {
    /// `<pluginId>:<nom>`.
    pub type_: String,
    /// Défaut 1.
    pub version: Option<u32>,
    /// Ce qu'est ce type d'objet, pour qui veut en poser (`fragment artefacts:kinds`, l'agent).
    pub description: Option<String>,
    /// Le NOM d'un artefact de ce kind, d'après son state — seul le kind sait lire
    /// son state. Le state peut manquer (objet pas encore lu).
    #[allow(clippy::type_complexity)]
    pub describe: Option<Box<dyn Fn(Option<&Value>) -> Option<Described>>>,
    /// Migre un state d'une version ancienne.
    #[allow(clippy::type_complexity)]
    pub migrate: Option<Box<dyn Fn(Value, u32) -> Value>>,
}

impl ArtefactKind {
    pub fn new(type_: &str) -> Self {
        ArtefactKind {
            type_: type_.to_string(),
            version: None,
            description: None,
            describe: None,
            migrate: None,
        }
    }
}

impl std::fmt::Debug for ArtefactKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtefactKind")
            .field("type", &self.type_)
            .field("version", &self.version)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

// ── la façade de l'ancienne API (§18), le temps de réécrire les plugins ─────────

/// Ce qu'on passe à `save()` — l'`id` est créé si absent.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveSpec {
    pub id: Option<String>,
    pub type_: String,
    pub version: Option<u32>,
    pub scope: ArtefactScope,
    pub anchor: Option<DurableAnchor>,
    pub state: Value,
}

/// Ce que `update()` reçoit : chaque champ est facultatif ; `anchor: Some(None)` retire l'ancre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SavePatch {
    pub type_: Option<String>,
    pub version: Option<u32>,
    pub scope: Option<ArtefactScope>,
    pub anchor: Option<Option<DurableAnchor>>,
    pub state: Option<Value>,
}

/// Ce que `save()` rend : l'identité stable de l'artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtefactRef {
    pub id: String,
}

/// Un index plié : id → record.
pub type Index = BTreeMap<String, Rec>;

// ── sérialisation ──────────────────────────────────────────────────────────────

/// `Option<Option<T>>` : champ absent → `None` ; `null` → `Some(None)` ; valeur → `Some(Some)`.
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Un `number` JS : écrit sans partie décimale quand il est entier (`0`, pas `0.0`),
/// pour que le journal écrit ici et celui écrit par Fragment aient les mêmes octets.
pub mod js_number {
    use super::*;

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.fract() == 0.0 && v.abs() < 9_007_199_254_740_992.0 {
            s.serialize_i64(*v as i64)
        } else {
            s.serialize_f64(*v)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        f64::deserialize(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn un_set_et_un_del_font_l_aller_retour_json_dans_l_ordre_du_ts() {
        let op = Op::Set(
            SetOp::new("a")
                .content("t", 1, json!({"n": 3}))
                .scope(ArtefactScope::file("a.md"))
                .anchor(WidgetAnchor::document(12)),
        );
        let s = serde_json::to_string(&op).unwrap();
        assert_eq!(
            s,
            r#"{"set":"a","type":"t","v":1,"state":{"n":3},"scope":{"kind":"file","path":"a.md"},"anchor":{"mode":"document","pos":12,"dx":0,"dy":0}}"#
        );
        assert_eq!(serde_json::from_str::<Op>(&s).unwrap(), op);
        let del: Op = serde_json::from_str(r#"{"del":"x"}"#).unwrap();
        assert_eq!(del, Op::del("x"));
        assert_eq!(serde_json::to_string(&del).unwrap(), r#"{"del":"x"}"#);
    }

    #[test]
    fn anchor_null_se_distingue_d_une_ancre_absente() {
        let nul: SetOp = serde_json::from_str(r#"{"set":"a","anchor":null}"#).unwrap();
        assert_eq!(nul.anchor, Some(None));
        let absente: SetOp = serde_json::from_str(r#"{"set":"a"}"#).unwrap();
        assert_eq!(absente.anchor, None);
        assert_eq!(
            serde_json::to_string(&nul).unwrap(),
            r#"{"set":"a","anchor":null}"#
        );
    }

    #[test]
    fn l_ancien_indice_char_est_ignore_a_la_lecture() {
        let a: DurableAnchor =
            serde_json::from_str(r#"{"mode":"document","pos":1,"dx":0,"dy":0,"char":"e"}"#)
                .unwrap();
        assert_eq!(a, DurableAnchor::from(WidgetAnchor::document(1)));
    }
}
