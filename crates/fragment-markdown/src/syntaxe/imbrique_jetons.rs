// Port de app/src/ui/views/markdown/language.ts (`codeHighlightStyle`, tags Lezer → jetons `--code-*`).
//! Les jetons de coloration du code : des NOMS de jetons du thème, jamais une couleur.
//!
//! `codeHighlightStyle` range les tags Lezer en onze couleurs `--code-*` (les noms d'Obsidian) ;
//! chaque variante ci-dessous est l'un de ces onze jetons, et [`JetonCode::nom`] est le nom du champ
//! de `Palette` (fragment-theme) que la vue résout. Un thème les redéfinit : aucune valeur ici.

/// Un jeton de coloration du code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JetonCode {
    /// `--code-normal` : une définition.
    Normal,
    /// `--code-comment` (en italique, voir [`JetonCode::italique`]).
    Commentaire,
    /// `--code-keyword`.
    MotCle,
    /// `--code-function` : un nom de fonction appelée.
    Fonction,
    /// `--code-property` : un nom de propriété, de clé, de macro, d'attribut.
    Propriete,
    /// `--code-value` : une constante, un atome, un booléen, une variable spéciale.
    Valeur,
    /// `--code-tag` : un type, une classe, un nombre, une balise.
    Type,
    /// `--code-operator` : un opérateur, une URL, un échappement, une expression régulière.
    Operateur,
    /// `--code-string`.
    Chaine,
    /// `--code-punctuation` : les accolades et séparateurs.
    Ponctuation,
    /// `--code-important` : une erreur de syntaxe.
    Important,
}

impl JetonCode {
    /// Le nom du champ de `Palette` (fragment-theme).
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            JetonCode::Normal => "code_normal",
            JetonCode::Commentaire => "code_comment",
            JetonCode::MotCle => "code_keyword",
            JetonCode::Fonction => "code_function",
            JetonCode::Propriete => "code_property",
            JetonCode::Valeur => "code_value",
            JetonCode::Type => "code_tag",
            JetonCode::Operateur => "code_operator",
            JetonCode::Chaine => "code_string",
            JetonCode::Ponctuation => "code_punctuation",
            JetonCode::Important => "code_important",
        }
    }

    /// Les commentaires sont en italique (`fontStyle: "italic"` de `codeHighlightStyle`).
    #[must_use]
    pub fn italique(self) -> bool {
        self == JetonCode::Commentaire
    }

    /// Tous les jetons.
    pub const TOUS: [JetonCode; 11] = [
        JetonCode::Normal,
        JetonCode::Commentaire,
        JetonCode::MotCle,
        JetonCode::Fonction,
        JetonCode::Propriete,
        JetonCode::Valeur,
        JetonCode::Type,
        JetonCode::Operateur,
        JetonCode::Chaine,
        JetonCode::Ponctuation,
        JetonCode::Important,
    ];
}
