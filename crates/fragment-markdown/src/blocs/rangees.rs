// Port de app/src/ui/views/markdown/decorationPlugin.ts (mathBlockField, embedField), htmlDecorations.ts
// (htmlBlockField) : la traduction `Decoration.replace` / `Decoration.widget` de blocs vers le modèle
// de rangées du fork (ADR-006).
//! Les blocs et le repli, traduits en modèle de rangées.
//!
//! - `Remplace` → `Placement::ALaPlace` : la source est masquée ligne par ligne, la hauteur du widget
//!   est dictée à la première ligne (0 aux suivantes) ;
//! - `SourceEtRendu` → `Placement::Dessous` : de la hauteur AJOUTÉE sous la dernière ligne ;
//! - `SourceSeule` → rien ;
//! - un commentaire de bloc remplacé → ses lignes masquées, hauteur 0 (aucun widget) ;
//! - le repli → les lignes cachées : masquées, hauteur 0, et sans police ni bloc.
//!
//! ★ Une ligne cachée par un repli n'a pas de hauteur à elle : on RETIRE d'abord ce que les titres
//!   et les blocs y avaient dicté, car `ModeleRangees::normaliser` garde la PLUS GRANDE hauteur d'une
//!   ligne et ferait réapparaître la rangée.

use super::modele::{BlocCalcule, ModeBloc};
use super::repli::{AnalyseRepli, EtatRepli};
use crate::syntaxe::modele_rangees::{Bloc, HauteurLigne, ModeleRangees, Placement, Plage};

/// Ajoute à `modele` les blocs de `blocs` et le repli `repli`.
///
/// `lignes` = les lignes logiques (`lignes_de`) ; `mesure` donne la hauteur du rendu d'un bloc, en
/// pixels logiques (mesurée par le widget ; 0 tant qu'elle ne l'est pas). Un bloc dont la première
/// ligne est cachée par un repli disparaît du modèle.
pub fn integrer(
    modele: &mut ModeleRangees,
    lignes: &[Plage],
    blocs: &[BlocCalcule],
    repli: &EtatRepli,
    analyse: &AnalyseRepli,
    mesure: &dyn Fn(&BlocCalcule) -> f32,
) {
    let cachees = repli.lignes_cachees(analyse);
    let cachee = |l: usize| {
        let i = cachees.partition_point(|&(a, _)| a <= l);
        i > 0 && l <= cachees[i - 1].1
    };

    for b in blocs {
        if b.mode == ModeBloc::SourceSeule || cachee(b.premiere_ligne) || cachee(b.derniere_ligne) {
            continue;
        }
        match (b.genre_partage(), b.mode) {
            (Some(genre), mode) => modele.blocs.push(Bloc {
                genre,
                source: b.source,
                premiere_ligne: b.premiere_ligne,
                derniere_ligne: b.derniere_ligne,
                placement: if mode == ModeBloc::Remplace {
                    Placement::ALaPlace
                } else {
                    Placement::Dessous
                },
                hauteur: mesure(b),
            }),
            // Le commentaire de bloc : plus rien à l'écran.
            (None, ModeBloc::Remplace) if matches!(b.contenu, super::ContenuBloc::Commentaire) => {
                for l in b.premiere_ligne..=b.derniere_ligne {
                    if let Some(p) = lignes.get(l) {
                        modele.masques.push(*p);
                    }
                    modele.hauteurs.push(HauteurLigne {
                        ligne: l,
                        hauteur: 0.0,
                    });
                }
            }
            _ => {}
        }
    }

    if cachees.is_empty() {
        return;
    }
    modele.hauteurs.retain(|h| !cachee(h.ligne));
    modele.polices.retain(|p| !cachee(p.ligne));
    modele.blocs.retain(|b| !cachee(b.premiere_ligne));
    for &(a, z) in &cachees {
        for l in a..=z {
            if let Some(p) = lignes.get(l) {
                modele.masques.push(*p);
            }
            modele.hauteurs.push(HauteurLigne {
                ligne: l,
                hauteur: 0.0,
            });
        }
    }
}
