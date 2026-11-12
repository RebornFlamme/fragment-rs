// Port de la part « rangées » des StateField de bloc d'app/src/ui/views/markdown/decorationPlugin.ts
// (mathBlockField, embedField), du `codeFolding` branché par `markdownFolding`, et des hauteurs de
// lignes de `.cm-ob-codeblock*` (editor.css).
//! Ce que l'hôte des blocs AJOUTE au modèle de rangées des décorations avant qu'il parte au fork.
//!
//! - les blocs de `fragment_markdown::blocs` (maths, embed, tableau, Properties, HTML, commentaire),
//!   à la hauteur que leur widget a mesurée ;
//! - le repli : lignes cachées masquées, hauteur 0 ;
//! - les hauteurs des lignes d'un bloc de code (corps à 0,9 em, padding des clôtures).
//!
//! Le calcul est pur : (texte, arbre, curseur, état de repli, mesures) → `ModeleRangees` complété.

use fragment_markdown::blocs::{self, AnalyseRepli, BlocCalcule, ContenuBloc, EtatRepli};
use fragment_markdown::decorations::{Contexte, Curseur};
use fragment_markdown::syntaxe::Arbre;
use fragment_markdown::syntaxe::modele_rangees::{HauteurLigne, ModeleRangees, Plage};

use super::geometrie::hauteur_ligne_code;

/// Ce qu'il faut pour compléter le modèle d'un document.
pub struct Entree<'a> {
    pub texte: &'a str,
    pub arbre: &'a Arbre,
    pub lignes: &'a [Plage],
    pub curseur: &'a Curseur<'a>,
    pub ctx: &'a Contexte<'a>,
    pub repli: &'a EtatRepli,
    pub analyse: &'a AnalyseRepli,
}

/// Complète `modele` (les rangées des décorations) et rend les blocs calculés, que l'hôte dessine.
///
/// `mesure` donne la hauteur du rendu d'un bloc (0 tant qu'il n'est pas mesuré).
pub fn completer(
    e: &Entree<'_>,
    mesure: &dyn Fn(&BlocCalcule) -> f32,
    modele: &mut ModeleRangees,
) -> Vec<BlocCalcule> {
    let liste = blocs::calculer(
        e.texte,
        e.arbre,
        e.curseur,
        e.ctx,
        e.repli.proprietes_repliees,
    );
    for b in &liste {
        if matches!(b.contenu, ContenuBloc::Code { .. }) {
            for l in b.premiere_ligne..=b.derniere_ligne {
                modele.hauteurs.push(HauteurLigne {
                    ligne: l,
                    hauteur: hauteur_ligne_code(
                        e.ctx.taille_texte,
                        l == b.premiere_ligne,
                        l == b.derniere_ligne,
                    ),
                });
            }
        }
    }
    // ★ `integrer` retire ce que les lignes repliées avaient dicté : les hauteurs de code sont donc
    //   posées AVANT lui.
    blocs::integrer(modele, e.lignes, &liste, e.repli, e.analyse, mesure);
    liste
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::decorations::{self, Revelation};
    use fragment_markdown::syntaxe::modele_rangees::lignes_de;
    use fragment_markdown::syntaxe::{Etendue, analyser};

    fn completer_texte(
        texte: &str,
        curseur: usize,
        repli: &EtatRepli,
        mesure: &dyn Fn(&BlocCalcule) -> f32,
    ) -> (ModeleRangees, Vec<BlocCalcule>) {
        let arbre = analyser(texte);
        let sel = [Etendue::new(curseur, curseur)];
        let c = Curseur {
            selections: &sel,
            focus: true,
            a_eu_le_focus: true,
        };
        let ctx = Contexte {
            revelation: Revelation::ApresPremierFocus,
            ..Contexte::default()
        };
        let mut modele = decorations::calculer(texte, &arbre, &c, &ctx).rangees;
        let lignes = lignes_de(texte);
        let analyse = AnalyseRepli::new(texte, &arbre);
        let e = Entree {
            texte,
            arbre: &arbre,
            lignes: &lignes,
            curseur: &c,
            ctx: &ctx,
            repli,
            analyse: &analyse,
        };
        let liste = completer(&e, mesure, &mut modele);
        modele.normaliser();
        (modele, liste)
    }

    #[test]
    fn les_lignes_d_un_bloc_de_code_ont_leur_hauteur() {
        let t = "avant\n```py\nx = 1\n```\napres";
        let (m, _) = completer_texte(t, t.len(), &EtatRepli::default(), &|_| 0.0);
        let h: Vec<_> = m.hauteurs.iter().map(|h| (h.ligne, h.hauteur)).collect();
        assert_eq!(h.len(), 3, "{h:?}");
        assert!((h[0].1 - 30.24).abs() < 1e-3 && h[0].0 == 1);
        assert!((h[1].1 - 20.16).abs() < 1e-3 && h[1].0 == 2);
        assert!((h[2].1 - 30.24).abs() < 1e-3 && h[2].0 == 3);
    }

    #[test]
    fn un_bloc_de_maths_hors_curseur_prend_sa_place() {
        let t = "a\n$$\nx^2\n$$\nb";
        let (m, liste) = completer_texte(t, t.len(), &EtatRepli::default(), &|_| 80.0);
        assert!(
            liste
                .iter()
                .any(|b| matches!(b.contenu, ContenuBloc::Maths { .. }))
        );
        let f = m.vers_fork(&lignes_de(t));
        // Les trois lignes de la source masquées ; 80 px à la première, 0 aux suivantes.
        assert_eq!(f.hauteurs, [(1, 80.0), (2, 0.0), (3, 0.0)]);
        assert!(f.blocs.is_empty());
    }

    #[test]
    fn replier_un_titre_cache_ses_lignes() {
        let t = "## A\ntexte A\n### A1\ntexte A1\n## B\ntexte B";
        let arbre = analyser(t);
        let analyse = AnalyseRepli::new(t, &arbre);
        let mut repli = EtatRepli::default();
        assert!(repli.basculer(&analyse, 0));
        let (m, _) = completer_texte(t, t.len(), &repli, &|_| 0.0);
        let f = m.vers_fork(&lignes_de(t));
        let nulles: Vec<_> = f
            .hauteurs
            .iter()
            .filter(|(_, h)| *h == 0.0)
            .map(|(l, _)| *l)
            .collect();
        assert_eq!(nulles, [1, 2, 3]);
    }
}
