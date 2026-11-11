// Port de app/src/ui/views/markdown/language.ts (les nœuds des extensions de Fragment) et des types de
// nœuds de @lezer/markdown 1.7.2 (MIT, © Marijn Haverbeke et contributeurs, voir syntaxe/mod.rs).
//! Les genres de nœuds de l'arbre, nommés EXACTEMENT comme Lezer.
//!
//! ★ Le nom compte : `decorations`, `blocs` et `listes` raisonnent sur ces noms comme le TS raisonne sur
//!   `node.name` ; un relevé Lezer (`tests/syntaxe/*.lezer`) se compare nom pour nom. Un nœud ajouté par
//!   fragment-rs (`FrontMatter`) est marqué comme tel et n'existe pas chez Lezer.

/// Genre d'un nœud de l'arbre syntaxique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Genre {
    // ── Racine et blocs composites ──
    Document,
    Blockquote,
    BulletList,
    OrderedList,
    ListItem,
    // ── Blocs feuilles ──
    Paragraph,
    ATXHeading1,
    ATXHeading2,
    ATXHeading3,
    ATXHeading4,
    ATXHeading5,
    ATXHeading6,
    SetextHeading1,
    SetextHeading2,
    CodeBlock,
    FencedCode,
    HorizontalRule,
    HTMLBlock,
    CommentBlock,
    ProcessingInstructionBlock,
    LinkReference,
    /// GFM : un tableau ne s'arrête qu'à une ligne vide (une ligne collée dessous est une rangée).
    Table,
    TableHeader,
    TableRow,
    TableCell,
    TableDelimiter,
    /// GFM : un élément de liste qui commence par `[ ]`, `[x]` ou `[X]` SUIVI d'un espace.
    Task,
    TaskMarker,
    /// Extension de Fragment : `$$…$$` en début de ligne (interrompt un paragraphe).
    BlockMath,
    /// Ajouté par fragment-rs (absent de Lezer) : le bloc `---` … `---` de tête de note, qui
    /// ENVELOPPE les nœuds que Lezer y voit (règle, paragraphes…). Fragment le détecte à part
    /// (`frontmatterModel`) ; ici il est dans l'arbre pour que `blocs` le trouve comme les autres.
    FrontMatter,
    // ── En ligne ──
    Escape,
    Entity,
    HardBreak,
    Emphasis,
    StrongEmphasis,
    Link,
    Image,
    InlineCode,
    HTMLTag,
    Comment,
    ProcessingInstruction,
    Autolink,
    /// GFM : URL nue (`https://…`, `www.…`, courriel) ; aussi la destination d'un lien.
    URL,
    Strikethrough,
    Subscript,
    Superscript,
    Emoji,
    /// Extension de Fragment : `==surligné==`.
    Highlight,
    /// Extension de Fragment : `[[cible]]`.
    WikiLink,
    /// Extension de Fragment : `![[cible]]`.
    Embed,
    /// Extension de Fragment : `$…$`.
    InlineMath,
    /// Extension de Fragment : `$$…$$` AU MILIEU d'une ligne (rendu en display, dans le flux).
    InlineDisplayMath,
    // ── Marques (la syntaxe que le live preview cache hors curseur) ──
    HeaderMark,
    QuoteMark,
    ListMark,
    LinkMark,
    EmphasisMark,
    CodeMark,
    CodeText,
    CodeInfo,
    LinkTitle,
    LinkLabel,
    StrikethroughMark,
    SubscriptMark,
    SuperscriptMark,
    HighlightMark,
    InlineMathMark,
}

impl Genre {
    /// Le nom Lezer du genre (`node.name` en TS).
    #[must_use]
    pub fn nom(self) -> &'static str {
        use Genre::*;
        match self {
            Document => "Document",
            Blockquote => "Blockquote",
            BulletList => "BulletList",
            OrderedList => "OrderedList",
            ListItem => "ListItem",
            Paragraph => "Paragraph",
            ATXHeading1 => "ATXHeading1",
            ATXHeading2 => "ATXHeading2",
            ATXHeading3 => "ATXHeading3",
            ATXHeading4 => "ATXHeading4",
            ATXHeading5 => "ATXHeading5",
            ATXHeading6 => "ATXHeading6",
            SetextHeading1 => "SetextHeading1",
            SetextHeading2 => "SetextHeading2",
            CodeBlock => "CodeBlock",
            FencedCode => "FencedCode",
            HorizontalRule => "HorizontalRule",
            HTMLBlock => "HTMLBlock",
            CommentBlock => "CommentBlock",
            ProcessingInstructionBlock => "ProcessingInstructionBlock",
            LinkReference => "LinkReference",
            Table => "Table",
            TableHeader => "TableHeader",
            TableRow => "TableRow",
            TableCell => "TableCell",
            TableDelimiter => "TableDelimiter",
            Task => "Task",
            TaskMarker => "TaskMarker",
            BlockMath => "BlockMath",
            FrontMatter => "FrontMatter",
            Escape => "Escape",
            Entity => "Entity",
            HardBreak => "HardBreak",
            Emphasis => "Emphasis",
            StrongEmphasis => "StrongEmphasis",
            Link => "Link",
            Image => "Image",
            InlineCode => "InlineCode",
            HTMLTag => "HTMLTag",
            Comment => "Comment",
            ProcessingInstruction => "ProcessingInstruction",
            Autolink => "Autolink",
            URL => "URL",
            Strikethrough => "Strikethrough",
            Subscript => "Subscript",
            Superscript => "Superscript",
            Emoji => "Emoji",
            Highlight => "Highlight",
            WikiLink => "WikiLink",
            Embed => "Embed",
            InlineMath => "InlineMath",
            InlineDisplayMath => "InlineDisplayMath",
            HeaderMark => "HeaderMark",
            QuoteMark => "QuoteMark",
            ListMark => "ListMark",
            LinkMark => "LinkMark",
            EmphasisMark => "EmphasisMark",
            CodeMark => "CodeMark",
            CodeText => "CodeText",
            CodeInfo => "CodeInfo",
            LinkTitle => "LinkTitle",
            LinkLabel => "LinkLabel",
            StrikethroughMark => "StrikethroughMark",
            SubscriptMark => "SubscriptMark",
            SuperscriptMark => "SuperscriptMark",
            HighlightMark => "HighlightMark",
            InlineMathMark => "InlineMathMark",
        }
    }

    /// Le niveau d'un titre (1 à 6, ATX ou Setext), `None` pour un autre genre.
    #[must_use]
    pub fn niveau_titre(self) -> Option<u8> {
        use Genre::*;
        match self {
            ATXHeading1 | SetextHeading1 => Some(1),
            ATXHeading2 | SetextHeading2 => Some(2),
            ATXHeading3 => Some(3),
            ATXHeading4 => Some(4),
            ATXHeading5 => Some(5),
            ATXHeading6 => Some(6),
            _ => None,
        }
    }

    /// Le titre ATX de niveau `n` (1 à 6).
    pub(crate) fn atx(n: usize) -> Genre {
        match n {
            1 => Genre::ATXHeading1,
            2 => Genre::ATXHeading2,
            3 => Genre::ATXHeading3,
            4 => Genre::ATXHeading4,
            5 => Genre::ATXHeading5,
            _ => Genre::ATXHeading6,
        }
    }

    /// Vrai pour un bloc (composite ou feuille), faux pour l'en-ligne et les marques.
    #[must_use]
    pub fn est_bloc(self) -> bool {
        use Genre::*;
        matches!(
            self,
            Document
                | Blockquote
                | BulletList
                | OrderedList
                | ListItem
                | Paragraph
                | ATXHeading1
                | ATXHeading2
                | ATXHeading3
                | ATXHeading4
                | ATXHeading5
                | ATXHeading6
                | SetextHeading1
                | SetextHeading2
                | CodeBlock
                | FencedCode
                | HorizontalRule
                | HTMLBlock
                | CommentBlock
                | ProcessingInstructionBlock
                | LinkReference
                | Table
                | Task
                | BlockMath
                | FrontMatter
        )
    }
}

impl std::fmt::Display for Genre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}
