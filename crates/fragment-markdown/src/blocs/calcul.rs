// Port de app/src/ui/views/markdown/decorationPlugin.ts (buildMathBlockDecos, buildEmbedDecos),
// htmlDecorations.ts (buildHtmlBlockDecos), table/tableField.ts (buildTableDecos) et
// properties/frontmatterField.ts (buildDecos).
//! Le calcul des blocs : (texte, arbre, curseur, contexte) → [`BlocCalcule`].
//!
//! La règle du curseur est celle de `decorations` : la source se révèle quand la sélection PRINCIPALE
//! touche la plage du bloc, bornes comprises, que le curseur a le droit de révéler et que l'éditeur
//! n'est pas en lecture seule.

use super::modele::{BlocCalcule, ContenuBloc, ModeBloc};
use crate::decorations::{Contexte, Curseur, curseur_actif, touches};
use crate::frontmatter_model::{doc_of, frontmatter_range, parse_properties};
use crate::syntaxe::modele_rangees::{Etendue, Plage, lignes_de};
use crate::syntaxe::{Arbre, Genre, Parcours};

/// Le frontmatter VALIDE de tête (`---` fermé, YAML clé / valeur), ou `None` : invalide, la source
/// reste visible et rien n'est perdu.
#[must_use]
pub fn plage_proprietes(texte: &str) -> Option<Plage> {
    let r = frontmatter_range(&doc_of(texte))?;
    parse_properties(&r.yaml)?;
    Plage::depuis_utf16(texte, r.from, r.to)
}

struct Lignes {
    lignes: Vec<Plage>,
}

impl Lignes {
    /// La ligne qui contient l'octet `b` (un octet de fin de document est sur la dernière ligne).
    fn de_octet(&self, b: usize) -> usize {
        self.lignes
            .partition_point(|l| l.octets.from <= b)
            .saturating_sub(1)
    }

    /// Les lignes `[premiere, derniere]` entières, comme une plage.
    fn source(&self, premiere: usize, derniere: usize) -> Plage {
        let (a, b) = (self.lignes[premiere], self.lignes[derniere]);
        Plage {
            utf16: Etendue::new(a.utf16.from, b.utf16.to),
            octets: Etendue::new(a.octets.from, b.octets.to),
        }
    }

    fn bloc(&self, contenu: ContenuBloc, p: usize, d: usize, mode: ModeBloc) -> BlocCalcule {
        BlocCalcule {
            contenu,
            source: self.source(p, d),
            premiere_ligne: p,
            derniere_ligne: d,
            mode,
        }
    }
}

/// Les blocs du document, triés par première ligne.
///
/// `proprietes_repliees` : l'état de repli du bloc Properties (porté par [`super::EtatRepli`]).
#[must_use]
pub fn calculer(
    texte: &str,
    arbre: &Arbre,
    curseur: &Curseur,
    ctx: &Contexte,
    proprietes_repliees: bool,
) -> Vec<BlocCalcule> {
    let selection = curseur.selections.first().copied();
    let actif =
        !ctx.lecture_seule && curseur_actif(ctx.revelation, curseur.focus, curseur.a_eu_le_focus);
    let touche = |from: usize, to: usize| actif && selection.is_some_and(|s| touches(s, from, to));
    let l = Lignes {
        lignes: lignes_de(texte),
    };
    let mut blocs: Vec<BlocCalcule> = Vec::new();

    // Properties : le YAML valide est TOUJOURS remplacé (on l'édite dans le widget).
    if let Some(p) = plage_proprietes(texte) {
        let derniere = l.de_octet(p.octets.to);
        let contenu = ContenuBloc::Proprietes {
            replie: proprietes_repliees,
        };
        blocs.push(l.bloc(contenu, 0, derniere, ModeBloc::Remplace));
    }
    let front = arbre.front_matter().map(|p| p.octets);

    arbre.parcourir(0, usize::MAX, |i, n| {
        let (nf, nt) = (n.from(), n.to());
        let (bf, bt) = (n.plage.octets.from, n.plage.octets.to);
        let sous_le_curseur = |dedans: bool| {
            if dedans {
                ModeBloc::SourceEtRendu
            } else {
                ModeBloc::Remplace
            }
        };
        match n.genre {
            // Ce que Lezer voit dans le frontmatter n'est pas un bloc du document.
            Genre::FrontMatter => return Parcours::Sauter,
            Genre::BlockMath => {
                let tex = tex_du_bloc(&texte[bf..bt]);
                if tex.trim().is_empty() {
                    return Parcours::Sauter;
                }
                let (p, d) = (l.de_octet(bf), l.de_octet(bt));
                let mode = sous_le_curseur(touche(nf, nt));
                blocs.push(l.bloc(ContenuBloc::Maths { tex }, p, d, mode));
            }
            Genre::Embed => {
                let brut = &texte[bf..bt];
                let interieur = brut.get(3..brut.len().saturating_sub(2)).unwrap_or("");
                if interieur.trim().is_empty() {
                    return Parcours::Sauter;
                }
                let (p, d) = (l.de_octet(bf), l.de_octet(bt));
                let avant = texte.get(l.lignes[p].octets.from..bf).unwrap_or("");
                let apres = texte.get(bt..l.lignes[d].octets.to).unwrap_or("");
                // Au milieu d'une phrase : un widget en ligne (decorations), pas un bloc.
                if !avant.trim().is_empty() || !apres.trim().is_empty() {
                    return Parcours::Sauter;
                }
                let contenu = ContenuBloc::Embed {
                    interieur: interieur.to_string(),
                };
                blocs.push(l.bloc(contenu, p, d, sous_le_curseur(touche(nf, nt))));
            }
            Genre::Table => {
                let (p, d) = (l.de_octet(bf), l.de_octet(bt));
                let source = l.source(p, d).texte(texte).to_string();
                // Jamais révélé (on édite les cellules) ; jamais de tableau dans un tableau.
                let contenu = ContenuBloc::Tableau { source };
                blocs.push(l.bloc(contenu, p, d, ModeBloc::Remplace));
                return Parcours::Sauter;
            }
            Genre::HTMLBlock | Genre::CommentBlock => {
                // Le saut de ligne final ne fait pas partie du bloc.
                let fin = if bt > bf && texte.as_bytes().get(bt - 1) == Some(&b'\n') {
                    bt - 1
                } else {
                    bt
                };
                let (p, d) = (l.de_octet(bf), l.de_octet(fin.max(bf)));
                let src = l.source(p, d);
                let mode = if touche(src.utf16.from, src.utf16.to) {
                    ModeBloc::SourceSeule
                } else {
                    ModeBloc::Remplace
                };
                if n.genre == Genre::CommentBlock {
                    blocs.push(l.bloc(ContenuBloc::Commentaire, p, d, mode));
                } else {
                    let source = src.texte(texte).to_string();
                    if !source.trim().is_empty() {
                        blocs.push(l.bloc(ContenuBloc::Html { source }, p, d, mode));
                    }
                }
                return Parcours::Sauter;
            }
            Genre::FencedCode => {
                let langue = arbre
                    .enfants(i)
                    .find(|&e| arbre.noeud(e).genre == Genre::CodeInfo)
                    .map(|e| arbre.noeud(e).plage.texte(texte).trim().to_string())
                    .unwrap_or_default();
                let (p, d) = (l.de_octet(bf), l.de_octet(bt));
                let contenu = ContenuBloc::Code { langue };
                blocs.push(l.bloc(contenu, p, d, ModeBloc::SourceSeule));
                return Parcours::Sauter;
            }
            Genre::HorizontalRule => {
                // Une règle `---` du frontmatter n'en est pas une.
                if front.is_some_and(|f| bf < f.to) {
                    return Parcours::Sauter;
                }
                let p = l.de_octet(bf);
                let mode = if touche(nf, nt) {
                    ModeBloc::SourceSeule
                } else {
                    ModeBloc::Remplace
                };
                blocs.push(l.bloc(ContenuBloc::Separateur, p, p, mode));
            }
            _ => {}
        }
        Parcours::Entrer
    });
    blocs.sort_by_key(|b| (b.premiere_ligne, b.derniere_ligne));
    blocs
}

/// Le LaTeX d'un `$$…$$` : sans le `$$` d'ouverture (précédé de blancs) ni celui de fermeture (suivi de
/// blancs) ; un bloc non fermé n'a pas de `$$` final.
fn tex_du_bloc(brut: &str) -> String {
    let corps = brut.trim_start().strip_prefix("$$").unwrap_or(brut);
    corps
        .trim_end()
        .strip_suffix("$$")
        .unwrap_or(corps)
        .to_string()
}
