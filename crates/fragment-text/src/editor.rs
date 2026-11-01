// Port de app/src/ui/core/editor/Editor.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  La façade de document — la couche anti-corruption de l'éditeur.
//
//  Elle a la FORME de CodeMirror 6 mais ne connaît aucun moteur : c'est le
//  contrat que toute surface de document implémente (la note, le PDF, la page
//  HTML, une conversation). Un calque écrit contre ce contrat tourne sur toutes.
//
//  TROIS niveaux, et chaque frontière compte :
//    - `DocumentSurface` : géométrie et virtualisation. Le socle, et TOUT ce que
//      les calques consomment.
//    - `TextSurface : DocumentSurface` : ce qu'un document TEXTUEL ajoute — lire
//      des lignes, des plages, une sélection. Le PDF en est une.
//    - `Editor : TextSurface` : ce qui S'ÉCRIT. La note en est un, le PDF non.
//
//  ★ Transposition Rust des gardes TS (`hasText`, `isEditable` rétrécissent le
//    type) : pas de `instanceof` possible sur un `dyn Trait`. Chaque surface
//    textuelle REDÉFINIT `as_text` (et chaque éditeur `as_editor`) pour se
//    rendre : la question « qu'y a-t-il là ? » reste posée à l'objet, jamais à
//    un drapeau tenu à côté.
//
//  ★ Unité des positions : des `usize`, OPAQUES (la surface décide ce qu'elles
//    désignent : offset d'une note, `(page−1)·1e6+1+car.` d'un PDF, index + 1
//    d'une page HTML). Quand une position EST un index dans un texte, c'est un
//    index en UNITÉS DE CODE UTF-16, comme `String.length` en TS (ADR-004) : les
//    fichiers, les ancres persistées et l'API des plugins parlent cette unité.
//    Pour découper un `&str`, passer par `crate::utf16`.
//
//  Le DOM de la surface (`contentEl`, `scrollEl`, `cm`, `Marker`, `addLayer`)
//  est de la présentation : il n'entre pas dans ce contrat, fragment-ui le porte.
// ═══════════════════════════════════════════════════════════════════════════

/// Une position ligne/colonne (0-indexée), comme l'`Editor` d'Obsidian.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct EditorPosition {
    pub line: usize,
    /// Colonne dans la ligne, en unités de code UTF-16 (ADR-004).
    pub ch: usize,
}

/// Une plage de positions absolues dans le document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct EditorRange {
    /// Positions absolues ; quand la surface est un texte, en unités de code
    /// UTF-16 (ADR-004).
    pub from: usize,
    pub to: usize,
}

/// Un rectangle, MÊME forme que le `Rect` de CM6.
///
/// ★ Système de coordonnées : DOCUMENT-relatif — l'origine est le coin
///   haut-gauche du contenu, pas la fenêtre. C'est le repère des markers et des
///   widgets ancrés : un objet placé avec ces coordonnées suit le défilement
///   gratuitement.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Le résultat d'un remappage de position. Les deux questions sont séparées à
/// dessein : « où est-ce maintenant ? » a TOUJOURS une réponse valide (sinon
/// l'appelant garderait une position hors bornes), « le texte a-t-il été
/// supprimé ? » est une information EN PLUS, dont chacun fait ce qu'il veut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MappedPos {
    /// La position dans le NOUVEAU document — toujours dans les bornes (texte :
    /// unités de code UTF-16, ADR-004).
    pub pos: usize,
    /// Le texte ancré a-t-il disparu ? `pos` est alors le point d'effondrement.
    pub deleted: bool,
}

/// De quel côté une position colle quand on insère pile dessus (CM6 : `assoc`).
///
/// `After` (défaut, TS `1`) : au caractère qui SUIT — une insertion sur l'ancre
/// la pousse avec son texte ; `Before` (TS `-1`) : au caractère qui précède,
/// l'ancre reste avant l'insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Assoc {
    Before,
    #[default]
    After,
}

impl Assoc {
    /// La valeur numérique du TS (`-1 | 1`).
    pub fn signe(self) -> i8 {
        match self {
            Assoc::Before => -1,
            Assoc::After => 1,
        }
    }
}

/// D'où vient un changement. `External` : il a été REÇU, pas écrit ici (une
/// resynchronisation depuis le disque, le texte d'une page HTML republié).
///
/// ★ Ce qui en dépend : un calque remappe ses ancres dans les deux cas, mais
///   n'ÉMET une opération (`follow`…) que pour un changement local — sinon chaque
///   vue ouverte sur le fichier publierait le même suivi, une fois par vue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChangeOrigin {
    External,
}

/// La fonction de remappage d'un changement : ancienne position (+ côté) → nouvelle.
pub type MapPos = Box<dyn Fn(usize, Assoc) -> MappedPos>;

/// Ce qui a bougé dans l'éditeur.
pub struct EditorChange {
    pub doc_changed: bool,
    pub selection_changed: bool,
    pub viewport_changed: bool,
    /// Remappe une position de l'ANCIEN document vers le NOUVEAU.
    ///
    /// C'est LA pièce sans laquelle rien ne peut être ancré au texte : une
    /// position brute désigne un caractère différent dès qu'on édite au-dessus
    /// d'elle. Tout ce qui garde une position entre deux changements DOIT passer
    /// par ici. Identité quand `doc_changed` est faux.
    pub map_pos: MapPos,
    /// Absent : frappe ou commande dans CETTE vue.
    pub origin: Option<ChangeOrigin>,
}

impl EditorChange {
    /// Un changement de document local (le cas courant), à partir de sa fonction de remappage.
    pub fn doc(map_pos: impl Fn(usize, Assoc) -> MappedPos + 'static) -> Self {
        Self {
            doc_changed: true,
            selection_changed: false,
            viewport_changed: false,
            map_pos: Box::new(map_pos),
            origin: None,
        }
    }

    /// Remappe une position (raccourci sur `map_pos`).
    pub fn map_pos(&self, pos: usize, assoc: Assoc) -> MappedPos {
        (self.map_pos)(pos, assoc)
    }
}

impl std::fmt::Debug for EditorChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorChange")
            .field("doc_changed", &self.doc_changed)
            .field("selection_changed", &self.selection_changed)
            .field("viewport_changed", &self.viewport_changed)
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

/// Une plage remappée par [`map_range`]. `deleted` : le DÉBUT a disparu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MappedRange {
    /// Positions de la surface (texte : unités de code UTF-16, ADR-004).
    pub from: usize,
    pub to: usize,
    pub deleted: bool,
}

/// LA règle des PLAGES, en un seul exemplaire (CM6 : une décoration non
/// inclusive, `RangeSet.map`) : le début colle au texte qui SUIT (assoc 1), la
/// fin au texte qui PRÉCÈDE (assoc −1) — taper juste avant ou juste après le
/// passage ne l'agrandit pas, taper DEDANS l'allonge ; jamais à l'envers. Les
/// positions sont celles de la surface (opaques) : la règle ne sait pas ce
/// qu'elles désignent. `deleted` : le DÉBUT a disparu.
pub fn map_range(
    map_pos: &dyn Fn(usize, Assoc) -> MappedPos,
    from: usize,
    to: usize,
) -> MappedRange {
    let debut = map_pos(from, Assoc::After);
    MappedRange {
        from: debut.pos,
        to: debut.pos.max(map_pos(to, Assoc::Before).pos),
        deleted: debut.deleted,
    }
}

/// LE SOCLE — ce dont les calques vivent réellement, et RIEN de plus.
///
/// ★ POURQUOI ce trait existe séparément d'`Editor` : « document affichable » et
///   « éditeur de texte » ne sont pas la même chose. Une vue PDF a une géométrie
///   et une virtualisation, mais peut n'avoir aucun texte adressable (un scan).
///   Un calque reçoit un `DocumentSurface` : il ne PEUT pas dépendre d'un texte
///   qui n'existera pas forcément.
///
/// Les coordonnées sont DOCUMENT-relatives (cf. [`Rect`]), sauf `pos_at_coords`.
pub trait DocumentSurface {
    fn coords_at_pos(&self, offset: usize) -> Option<Rect>;
    /// Coords CLIENT (issues d'un événement pointeur) → position. Pour la saisie.
    fn pos_at_coords(&self, x: f64, y: f64) -> Option<usize>;

    /// La fenêtre de VIRTUALISATION : ce que l'éditeur a monté. Elle inclut ce
    /// qui est masqué (replié) — une position peut y être sans être dessinée.
    fn viewport_range(&self) -> EditorRange;

    /// Les portions RÉELLEMENT dessinées : le viewport, MOINS ce qui est masqué.
    /// Une plage repliée y laisse un TROU.
    ///
    /// ★ POURQUOI c'est distinct de `viewport_range` : « pas visible » recouvre
    ///   deux situations aux réactions OPPOSÉES. Hors viewport, l'objet existe,
    ///   on GARDE sa dernière position. Dans un trou (replié), le contenu est
    ///   CACHÉ : on MASQUE l'objet — le repositionner est un contresens,
    ///   `coords_at_pos` répondrait le point dessiné le plus proche (le
    ///   placeholder du repli) et toutes les annotations d'un bloc replié
    ///   s'empileraient sur son titre.
    ///
    /// ⚠️ Un trou ne dit PAS « replié par l'utilisateur », il dit « non dessiné ».
    fn rendered_ranges(&self) -> Vec<EditorRange>;

    /// Force un recalcul des calques — le « poke » quand une donnée EXTERNE change.
    fn request_update(&self);

    /// Cette surface a-t-elle du texte adressable ? Une surface textuelle
    /// redéfinit cette méthode et se rend ; le défaut dit non.
    fn as_text(&self) -> Option<&dyn TextSurface> {
        None
    }
}

/// Le texte entier d'une surface, s'il est connaissable (cf. [`TextSurface::get_text`]).
pub enum FullText {
    /// La surface ne l'a JAMAIS (PDF : pages chargées à la demande, offsets fixes).
    Never,
    /// Pas ENCORE (chargement) ; quand il devient lisible, la surface émet un
    /// `doc_changed` d'origine `External` et `map_pos` identité.
    NotYet,
    Text(SurfaceText),
}

impl FullText {
    /// Le texte, s'il est là.
    pub fn text(&self) -> Option<&SurfaceText> {
        match self {
            FullText::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// Le contrat d'une surface de TEXTE : le socle, plus ce qu'un document textuel
/// sait faire en propre. Ce qui vit ICI a un point commun : ça n'a de sens que
/// s'il existe un espace de positions de caractères. `coords_for_range` en fait
/// partie — un rect par rangée VISUELLE d'une plage de texte.
pub trait TextSurface: DocumentSurface {
    fn get_line(&self, n: usize) -> String;
    fn line_count(&self) -> usize;
    fn offset_to_pos(&self, offset: usize) -> EditorPosition;
    fn pos_to_offset(&self, pos: EditorPosition) -> usize;
    fn get_selection(&self) -> EditorRange;
    fn coords_for_range(&self, from: usize, to: usize) -> Vec<Rect>;
    /// Le texte ENTIER, quand la surface le connaît — ce qui rend une ancre
    /// vérifiable par sa CITATION. Défaut : jamais.
    fn get_text(&self) -> FullText {
        FullText::Never
    }
    /// Ce texte est un RENDU du fichier (une page HTML : son texte vient du DOM
    /// que son JS construit) — pour l'agent, la vérité est le FICHIER (la source).
    fn file_is_source(&self) -> bool {
        false
    }
    /// Cette surface s'écrit-elle ? Un éditeur redéfinit cette méthode et se rend.
    fn as_editor(&self) -> Option<&dyn Editor> {
        None
    }
}

/// Le texte entier d'une surface et sa carte vers les POSITIONS. Sur un éditeur,
/// l'identité ; sur une surface à pas fixe (une conversation : une plage par
/// message), l'index dans `text` n'est pas la position.
pub struct SurfaceText {
    /// Le texte ; ses index (`to_pos`, `to_index`) sont en unités de code UTF-16
    /// (ADR-004) : le découper passe par `crate::utf16`.
    pub text: String,
    to_pos: Box<dyn Fn(usize) -> usize>,
    to_index: Box<dyn Fn(usize) -> usize>,
}

impl SurfaceText {
    /// Un texte dont les positions SONT les index (un éditeur).
    pub fn identity(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            to_pos: Box::new(|i| i),
            to_index: Box::new(|p| p),
        }
    }

    /// Un texte et ses deux cartes.
    pub fn new(
        text: impl Into<String>,
        to_pos: impl Fn(usize) -> usize + 'static,
        to_index: impl Fn(usize) -> usize + 'static,
    ) -> Self {
        Self {
            text: text.into(),
            to_pos: Box::new(to_pos),
            to_index: Box::new(to_index),
        }
    }

    /// index UTF-16 dans `text` → position (un index entre deux unités : fin de la précédente).
    pub fn to_pos(&self, index: usize) -> usize {
        (self.to_pos)(index)
    }

    /// position → index UTF-16 dans `text` (bornée à son unité ; hors unité : l'unité suivante).
    pub fn to_index(&self, pos: usize) -> usize {
        (self.to_index)(pos)
    }
}

impl std::fmt::Debug for SurfaceText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceText")
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

/// Un changement : remplace `[from, to)` par `insert`. `to` absent = insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorChangeSpec {
    /// Positions dans le document d'avant, en unités de code UTF-16 (ADR-004).
    pub from: usize,
    pub to: Option<usize>,
    pub insert: String,
}

/// La sélection voulue APRÈS une transaction. `head` absent = curseur à `anchor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorSelectionSpec {
    /// Positions dans le document d'après, en unités de code UTF-16 (ADR-004).
    pub anchor: usize,
    pub head: Option<usize>,
}

/// Ce qu'une commande d'éditeur PRODUIT — et ce que `Editor::transaction` applique.
///
/// ★ Les `changes` sont TOUS exprimés dans le document d'avant la transaction ;
///   c'est le moteur qui les compose. Une commande n'a donc jamais à recalculer
///   ses positions après son propre premier changement.
/// ★ La `selection`, elle, est dans le document d'APRÈS.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EditorTransactionSpec {
    pub changes: Vec<EditorChangeSpec>,
    pub selection: Option<EditorSelectionSpec>,
    /// Amène la sélection à l'écran.
    pub scroll_into_view: bool,
}

/// Le contrat d'un éditeur : une surface textuelle QUI S'ÉCRIT.
///
/// ★ POURQUOI un troisième étage : le PDF a du texte adressable mais ne s'édite
///   pas. Le mettre au même étage que la note aurait forcé un choix entre deux
///   mensonges : des méthodes d'écriture qui ne font rien, ou « Mettre en gras »
///   proposé sur un PDF. C'est `Editor` — et lui seul — que les commandes
///   `editor:*` reçoivent.
pub trait Editor: TextSurface {
    fn get_value(&self) -> String;
    fn get_range(&self, from: usize, to: usize) -> String;
    /// LA primitive d'écriture. Tout le reste est du sucre par-dessus.
    fn transaction(&self, spec: EditorTransactionSpec);
    /// Sucre : `transaction({ changes: [{ from, to, insert }] })`.
    fn replace_range(&self, from: usize, to: usize, text: &str) {
        self.transaction(EditorTransactionSpec {
            changes: vec![EditorChangeSpec {
                from,
                to: Some(to),
                insert: text.to_owned(),
            }],
            ..EditorTransactionSpec::default()
        });
    }
    /// Sucre : `transaction({ selection: { anchor, head } })`.
    fn set_selection(&self, anchor: usize, head: Option<usize>) {
        self.transaction(EditorTransactionSpec {
            selection: Some(EditorSelectionSpec { anchor, head }),
            ..EditorTransactionSpec::default()
        });
    }
    fn focus(&self);
    fn has_focus(&self) -> bool;
}

/// Cette surface a-t-elle du texte adressable ? (TS : le garde `hasText`.)
pub fn has_text(surface: &dyn DocumentSurface) -> bool {
    surface.as_text().is_some()
}

/// Cette surface s'écrit-elle ? (TS : le garde `isEditable`.)
pub fn is_editable(surface: &dyn DocumentSurface) -> bool {
    surface.as_text().is_some_and(|t| t.as_editor().is_some())
}

/// Le texte entier d'une surface : un éditeur le donne toujours (identité),
/// une `TextSurface` par `get_text` ; `Never` = jamais, `NotYet` = pas encore.
pub fn full_text(surface: &dyn DocumentSurface) -> FullText {
    let Some(texte) = surface.as_text() else {
        return FullText::Never;
    };
    match texte.as_editor() {
        Some(editeur) => FullText::Text(SurfaceText::identity(editeur.get_value())),
        None => texte.get_text(),
    }
}
