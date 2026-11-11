//! L'arbre syntaxique Markdown de Fragment (ADR-010) : la base du live preview, des listes et des blocs.
//!
//! Fragment analyse ses notes avec Lezer (`@lezer/markdown`, via CodeMirror) : CommonMark, GFM
//! (tableaux, tâches, barré, URL nues), indice et exposant, émoji, plus ses propres extensions
//! (`app/src/ui/views/markdown/language.ts`) : surligné `==…==`, liens internes `[[…]]` et embeds
//! `![[…]]`, maths `$…$`, `$$…$$` en ligne et `$$…$$` en bloc. Ce module en est la TRADUCTION en
//! Rust, nœud pour nœud : mêmes noms (`Genre`), mêmes plages — vérifiées contre des relevés de Lezer
//! (`tests/syntaxe/*.lezer`, produits hors du dépôt sur les documents réécrits de `tests/syntaxe/`).
//!
//! Choix (ADR-010, par la mesure) : ni `tree-sitter-md` (70,6 % des nœuds de Lezer, ≈ 0,5 s pour
//! 10 000 lignes) ni `pulldown-cmark` (59,4 %) ; les deux ignorent les extensions de Fragment et
//! découpent autrement. Le port est fidèle par construction et sans dépendance nouvelle.
//!
//! - `analyser` : texte → [`Arbre`] (nœuds à plat, plages UTF-16 et octets, parcours limité à une
//!   plage, nœud à une position).
//! - [`modele_rangees`] : les types partagés par `decorations` et `blocs` (masques, hauteurs et
//!   polices par ligne, blocs).
//!
//! - [`Arbre::reanalyser`] : après un [`Changement`], le même arbre en ne réanalysant que les blocs
//!   de tête touchés (`incremental.rs`, fr-t3.1).
//!
//! Licence : `analyse_blocs.rs`, `en_ligne.rs`, `genre.rs` et `incremental.rs` traduisent
//! l'algorithme et les noms de nœuds de `@lezer/markdown` 1.7.2 (et, pour la réutilisation de
//! l'arbre précédent, le principe des fragments de `@lezer/common`), sous la licence MIT reproduite
//! ci-dessous.

// MIT License
//
// Copyright (C) 2020 by Marijn Haverbeke <marijn@haverbeke.berlin> and others
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

mod analyse_blocs;
pub mod arbre;
mod en_ligne;
pub mod genre;
mod incremental;
pub mod modele_rangees;

pub use arbre::{Arbre, Cote, Noeud, Parcours};
pub use genre::Genre;
pub use incremental::Changement;
pub use modele_rangees::{Etendue, Plage};

/// Analyse `texte` en entier (raccourci de [`Arbre::analyser`]).
#[must_use]
pub fn analyser(texte: &str) -> Arbre {
    Arbre::analyser(texte)
}
