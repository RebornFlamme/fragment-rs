// Port de app/src/ui/views/markdown/decorationPlugin.ts (mathBlockField, embedField), htmlDecorations.ts
// (htmlBlockField), table/tableField.ts (tableField) et properties/frontmatterField.ts (propertiesField).
//! Les types d'un bloc calculé : sa source, son mode, ce qu'il rend.

use crate::syntaxe::modele_rangees::{GenreBloc, Plage};

/// Où en est la source d'un bloc selon le curseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeBloc {
    /// Le rendu REMPLACE la source : curseur dehors.
    Remplace,
    /// La source reste visible et le rendu est AJOUTÉ dessous : curseur dedans.
    SourceEtRendu,
    /// La source seule : pas de rendu en bloc (bloc HTML ou commentaire sous le curseur, code,
    /// séparateur sous le curseur).
    SourceSeule,
}

/// Ce qu'un bloc représente, avec ce dont son widget a besoin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContenuBloc {
    /// `$$…$$` : le LaTeX, délimiteurs retirés.
    Maths { tex: String },
    /// `![[…]]` seul sur sa ligne : ce qui est entre les crochets.
    Embed { interieur: String },
    /// Tableau GFM : sa source (lignes entières).
    Tableau { source: String },
    /// Front matter VALIDE : `replie` = replié par le chevron de gouttière.
    Proprietes { replie: bool },
    /// Bloc HTML : sa source (lignes entières), que le widget assainit.
    Html { source: String },
    /// `<!-- … -->` en bloc : caché hors curseur, sans widget.
    Commentaire,
    /// Bloc de code délimité : sa langue (vide s'il n'y en a pas).
    Code { langue: String },
    /// Règle horizontale.
    Separateur,
}

/// Un bloc du document pour un curseur donné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlocCalcule {
    pub contenu: ContenuBloc,
    /// La source, en LIGNES ENTIÈRES (sans le saut de ligne final).
    pub source: Plage,
    /// Première et dernière lignes logiques de la source (bornes comprises, à partir de 0).
    pub premiere_ligne: usize,
    pub derniere_ligne: usize,
    pub mode: ModeBloc,
}

impl BlocCalcule {
    /// Le genre de bloc que connaît le modèle de rangées, ou `None` quand le bloc ne change pas les
    /// rangées (commentaire : lignes masquées de hauteur 0 ; code et séparateur : rendus par
    /// `decorations`).
    #[must_use]
    pub fn genre_partage(&self) -> Option<GenreBloc> {
        match self.contenu {
            ContenuBloc::Maths { .. } => Some(GenreBloc::Maths),
            ContenuBloc::Embed { .. } => Some(GenreBloc::Embed),
            ContenuBloc::Tableau { .. } => Some(GenreBloc::Tableau),
            ContenuBloc::Proprietes { .. } => Some(GenreBloc::Proprietes),
            ContenuBloc::Html { .. } => Some(GenreBloc::Html),
            ContenuBloc::Commentaire | ContenuBloc::Code { .. } | ContenuBloc::Separateur => None,
        }
    }
}
