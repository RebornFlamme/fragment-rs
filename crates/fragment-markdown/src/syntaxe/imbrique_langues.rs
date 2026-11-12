// Port de la liste de langages de coloration des blocs de code de Fragment
// (`codeLanguages: languages` de app/src/ui/views/markdown/livePreview.ts, @codemirror/language-data).
// Tables de mots réécrites ; seuls les noms de langage et d'alias suivent la référence (ADR-007).
//! Les langages que l'on sait colorer, comme données : mots-clés, commentaires, chaînes.
//!
//! ★ Écart assumé : Fragment charge à la demande une centaine de langages (grammaires Lezer et modes
//!   hérités). Ici : les langages courants des notes, décrits par une table lue par UN analyseur
//!   générique (`imbrique_code.rs`). Un langage inconnu n'est pas coloré (comme un langage dont le
//!   chargement échoue). Ajouter un langage = ajouter une table, sans toucher à l'analyseur.

#[path = "imbrique_langues_plus.rs"]
mod plus;

/// La famille d'analyse : ce que la table seule ne dit pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Famille {
    Generique,
    /// JSON : une chaîne suivie de `:` est une clé.
    Json,
    /// YAML (`:`) et TOML (`=`) : une clé en début de ligne.
    Cles(char),
    Css,
    /// HTML et XML : balises, attributs, commentaires.
    Balises,
}

/// La description d'un langage.
#[derive(Debug)]
pub(super) struct Langue {
    pub mots: &'static [&'static str],
    pub valeurs: &'static [&'static str],
    pub types: &'static [&'static str],
    /// Mots-clés insensibles à la casse (SQL).
    pub insensible: bool,
    pub ligne: &'static [&'static str],
    pub bloc: Option<(&'static str, &'static str)>,
    /// Les guillemets qui ouvrent une chaîne.
    pub chaines: &'static str,
    /// `"""` et `'''` ouvrent une chaîne sur plusieurs lignes (Python).
    pub triple: bool,
    /// Les guillemets dont la chaîne peut couvrir plusieurs lignes (les accents graves).
    pub multiligne: &'static str,
    /// Préfixe de variable (`$` du shell).
    pub variable: Option<char>,
    /// Caractères d'identifiant en plus des lettres et chiffres.
    pub ident_extra: &'static str,
    /// `nom!` est une macro (Rust).
    pub macros: bool,
    /// `'a'` est un littéral de caractère et `'a` une durée de vie (Rust).
    pub apostrophe_rust: bool,
    /// `#mot` en début de ligne est une directive (préprocesseur C).
    pub directive: bool,
    pub famille: Famille,
}

const VIDE: &[&str] = &[];

const BASE: Langue = Langue {
    mots: VIDE,
    valeurs: VIDE,
    types: VIDE,
    insensible: false,
    ligne: VIDE,
    bloc: None,
    chaines: "\"'",
    triple: false,
    multiligne: "",
    variable: None,
    ident_extra: "_",
    macros: false,
    apostrophe_rust: false,
    directive: false,
    famille: Famille::Generique,
};

const TYPES_C: &[&str] = &[
    "int", "char", "float", "double", "void", "long", "short", "signed", "unsigned", "size_t",
    "bool", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t",
    "int64_t", "wchar_t",
];

static JS: Langue = Langue {
    mots: &[
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "let",
        "new",
        "of",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
        "async",
        "await",
        "as",
    ],
    valeurs: &["true", "false", "null", "undefined", "NaN", "Infinity"],
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    chaines: "\"'`",
    multiligne: "`",
    ident_extra: "_$",
    ..BASE
};

static TS: Langue = Langue {
    mots: &[
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "let",
        "new",
        "of",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
        "async",
        "await",
        "as",
        "interface",
        "type",
        "implements",
        "private",
        "public",
        "protected",
        "readonly",
        "abstract",
        "declare",
        "namespace",
        "keyof",
        "is",
        "satisfies",
        "infer",
        "override",
    ],
    types: &[
        "string", "number", "boolean", "any", "unknown", "never", "object", "symbol", "bigint",
    ],
    ..JS
};

static JSON: Langue = Langue {
    valeurs: &["true", "false", "null"],
    chaines: "\"",
    famille: Famille::Json,
    ..BASE
};

static JSONC: Langue = Langue {
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    ..JSON
};

static RUST: Langue = Langue {
    mots: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type",
        "unsafe", "use", "where", "while",
    ],
    valeurs: &["true", "false"],
    types: &[
        "bool", "char", "str", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32",
        "u64", "u128", "usize", "f32", "f64", "String", "Vec", "Option", "Result", "Box",
    ],
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    chaines: "\"",
    macros: true,
    apostrophe_rust: true,
    ..BASE
};

static PYTHON: Langue = Langue {
    mots: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is",
        "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with",
        "yield", "match", "case",
    ],
    valeurs: &["True", "False", "None"],
    types: &[
        "int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes",
    ],
    ligne: &["#"],
    triple: true,
    ..BASE
};

static SHELL: Langue = Langue {
    mots: &[
        "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
        "in", "function", "select", "time", "return", "break", "continue", "local", "export",
        "readonly", "declare", "unset", "set", "shift", "exit",
    ],
    valeurs: &["true", "false"],
    ligne: &["#"],
    chaines: "\"'`",
    variable: Some('$'),
    ..BASE
};

static CSS: Langue = Langue {
    ident_extra: "_-",
    bloc: Some(("/*", "*/")),
    famille: Famille::Css,
    ..BASE
};

static SCSS: Langue = Langue {
    ligne: &["//"],
    ..CSS
};

static HTML: Langue = Langue {
    ident_extra: "_-:.",
    famille: Famille::Balises,
    ..BASE
};

static YAML: Langue = Langue {
    valeurs: &["true", "false", "null", "yes", "no", "~"],
    ligne: &["#"],
    ident_extra: "_-",
    famille: Famille::Cles(':'),
    ..BASE
};

static TOML: Langue = Langue {
    valeurs: &["true", "false"],
    ligne: &["#"],
    ident_extra: "_-",
    triple: true,
    famille: Famille::Cles('='),
    ..BASE
};

static SQL: Langue = Langue {
    mots: &[
        "select",
        "from",
        "where",
        "insert",
        "into",
        "values",
        "update",
        "set",
        "delete",
        "create",
        "table",
        "alter",
        "drop",
        "index",
        "view",
        "join",
        "inner",
        "left",
        "right",
        "outer",
        "on",
        "group",
        "by",
        "order",
        "having",
        "limit",
        "offset",
        "union",
        "all",
        "distinct",
        "as",
        "and",
        "or",
        "not",
        "is",
        "in",
        "like",
        "between",
        "exists",
        "case",
        "when",
        "then",
        "else",
        "end",
        "primary",
        "key",
        "foreign",
        "references",
        "default",
        "unique",
        "constraint",
        "database",
        "begin",
        "commit",
        "rollback",
        "with",
        "asc",
        "desc",
    ],
    valeurs: &["null", "true", "false"],
    types: &[
        "int",
        "integer",
        "bigint",
        "smallint",
        "varchar",
        "char",
        "text",
        "boolean",
        "date",
        "timestamp",
        "float",
        "double",
        "numeric",
        "decimal",
        "real",
        "serial",
    ],
    insensible: true,
    ligne: &["--"],
    bloc: Some(("/*", "*/")),
    ..BASE
};

static C: Langue = Langue {
    mots: &[
        "auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern",
        "for", "goto", "if", "inline", "register", "restrict", "return", "sizeof", "static",
        "struct", "switch", "typedef", "union", "volatile", "while",
    ],
    valeurs: &["NULL", "true", "false"],
    types: TYPES_C,
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    directive: true,
    ..BASE
};

static CPP: Langue = Langue {
    mots: &[
        "auto",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "constexpr",
        "continue",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "explicit",
        "extern",
        "final",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "namespace",
        "new",
        "noexcept",
        "operator",
        "override",
        "private",
        "protected",
        "public",
        "return",
        "sizeof",
        "static",
        "struct",
        "switch",
        "template",
        "this",
        "throw",
        "try",
        "typedef",
        "typename",
        "union",
        "using",
        "virtual",
        "volatile",
        "while",
    ],
    valeurs: &["nullptr", "NULL", "true", "false"],
    ..C
};

static JAVA: Langue = Langue {
    mots: &[
        "abstract",
        "assert",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "interface",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "volatile",
        "while",
        "var",
        "record",
    ],
    valeurs: &["true", "false", "null"],
    types: &[
        "boolean", "byte", "char", "double", "float", "int", "long", "short", "void", "String",
    ],
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    ..BASE
};

static GO: Langue = Langue {
    mots: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
    ],
    valeurs: &["true", "false", "nil", "iota"],
    types: &[
        "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64",
        "string", "bool", "byte", "rune", "float32", "float64", "error", "any",
    ],
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    chaines: "\"'`",
    multiligne: "`",
    ..BASE
};

static CSHARP: Langue = Langue {
    mots: &[
        "abstract",
        "as",
        "base",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "event",
        "finally",
        "for",
        "foreach",
        "if",
        "in",
        "interface",
        "internal",
        "is",
        "lock",
        "namespace",
        "new",
        "operator",
        "out",
        "override",
        "private",
        "protected",
        "public",
        "readonly",
        "ref",
        "return",
        "sealed",
        "static",
        "struct",
        "switch",
        "this",
        "throw",
        "try",
        "using",
        "var",
        "virtual",
        "while",
        "async",
        "await",
    ],
    valeurs: &["true", "false", "null"],
    types: &[
        "bool", "byte", "char", "decimal", "double", "float", "int", "long", "object", "short",
        "string", "uint", "ulong", "void",
    ],
    ligne: &["//"],
    bloc: Some(("/*", "*/")),
    ..BASE
};

/// Le langage nommé par la première ligne d'un bloc de code (`` ```rust ``), alias compris.
#[must_use]
pub(super) fn langue(info: &str) -> Option<&'static Langue> {
    let nom = info.split_whitespace().next()?;
    // `{.python}` et `python{…}` : on ne garde que le mot.
    let nom: String = nom
        .trim_start_matches(['{', '.'])
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '+' | '#' | '-' | '_'))
        .collect::<String>()
        .to_lowercase();
    Some(match nom.as_str() {
        "js" | "javascript" | "mjs" | "cjs" | "jsx" | "node" => &JS,
        "ts" | "typescript" | "tsx" | "mts" => &TS,
        "json" | "json5" => &JSON,
        "jsonc" => &JSONC,
        "rust" | "rs" => &RUST,
        "py" | "python" | "python3" => &PYTHON,
        "sh" | "bash" | "zsh" | "shell" | "console" => &SHELL,
        "css" => &CSS,
        "scss" | "less" => &SCSS,
        "html" | "xml" | "svg" | "xhtml" => &HTML,
        "yaml" | "yml" => &YAML,
        "toml" => &TOML,
        "sql" | "mysql" | "pgsql" | "postgresql" | "sqlite" => &SQL,
        "c" | "h" => &C,
        "cpp" | "c++" | "cc" | "cxx" | "hpp" => &CPP,
        "java" => &JAVA,
        "go" | "golang" => &GO,
        "cs" | "csharp" | "c#" => &CSHARP,
        _ => return plus::langue(nom.as_str()),
    })
}

/// Un identifiant de la langue : mot-clé, valeur ou type (comparaison selon la casse du langage).
#[must_use]
pub(super) fn dans(table: &[&str], mot: &str, insensible: bool) -> bool {
    if insensible {
        table.iter().any(|m| m.eq_ignore_ascii_case(mot))
    } else {
        table.contains(&mot)
    }
}
