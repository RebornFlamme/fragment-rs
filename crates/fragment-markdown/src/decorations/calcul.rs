// Port de app/src/ui/views/markdown/decorationPlugin.ts (buildFnMarkdownRenderer, isInsideCode,
// listLevel).
//! Le calcul des décorations : un parcours de l'arbre LIMITÉ aux plages visibles, nœud par nœud,
//! puis les barres d'indentation ligne par ligne, comme le `ViewPlugin` de Fragment.
//!
//! ★ On calcule en OCTETS (découpe du texte) ; la sélection est convertie une fois. La conversion
//!   est monotone, donc `touches` donne le même verdict qu'en UTF-16.

use std::collections::HashMap;
use std::sync::LazyLock;

use fragment_core::parse_linktext;
use regex::Regex;

use super::index_lignes::IndexLignes;
use super::styles::{INTERLIGNE_TITRE, LARGEUR_INDENTATION_EM, TAILLE_CODE_EM, taille_titre};
use super::{
    Contexte, Curseur, DecoLigne, Decorations, GenreLigne, GenreMarque, GenreWidget, Largeur,
    Marque, WidgetEnLigne, curseur_actif, touches,
};
use crate::syntaxe::modele_rangees::{HauteurLigne, PoliceLigne};
use crate::syntaxe::{Arbre, Etendue, Genre, Noeud, Parcours, Plage};

fn re(motif: &str) -> Regex {
    Regex::new(motif).unwrap_or_else(|_| unreachable!("motif constant invalide : {motif}"))
}

/// Une ligne de tâche : puce, blancs, puis `[ ]`, `[x]` ou `[X]`.
static LIGNE_TACHE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*[-*+]\s+\[[ xX]\]"));
/// Une tâche SANS texte (`- [ ]` en fin de ligne) : la capture va jusqu'au `[`.
static TACHE_NUE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*[-*+]\s+)\[[ xX]\]$"));
/// L'ouverture d'un bloc de code (trois accents graves ou trois tildes, ou plus).
static OUVERTURE_CODE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*(`{3,}|~{3,})"));
/// Une ligne qui n'est QU'une clôture de bloc de code.
static CLOTURE_CODE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*(`{3,}|~{3,})\s*$"));
/// `![alt](chemin)` tout entier ; la capture est le chemin (chevrons et blancs ôtés).
static IMAGE: LazyLock<Regex> = LazyLock::new(|| re(r"^!\[[^\]]*\]\(\s*<?([^)>]+?)>?\s*\)$"));
/// Un chemin à schéma (`http://`, `file://`…) : externe, jamais rendu depuis l'éditeur.
static SCHEMA: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^[a-z][a-z0-9+.-]*://"));

/// Les décorations de `texte` (analysé en `arbre`) pour ce curseur et ce contexte.
#[must_use]
pub fn calculer(texte: &str, arbre: &Arbre, curseur: &Curseur, ctx: &Contexte) -> Decorations {
    calculer_dans(texte, IndexLignes::new(texte), arbre, curseur, ctx)
}

/// [`calculer`], avec les lignes logiques de `texte` déjà connues (`lignes_de(texte)`) : un éditeur
/// qui les tient à jour à chaque frappe évite de relire tout le texte (fr-t3.4).
#[must_use]
pub fn calculer_avec_lignes(
    texte: &str,
    lignes: &[Plage],
    arbre: &Arbre,
    curseur: &Curseur,
    ctx: &Contexte,
) -> Decorations {
    calculer_dans(texte, IndexLignes::avec(texte, lignes), arbre, curseur, ctx)
}

fn calculer_dans(
    texte: &str,
    idx: IndexLignes,
    arbre: &Arbre,
    curseur: &Curseur,
    ctx: &Contexte,
) -> Decorations {
    let principale = curseur.selections.first().copied().unwrap_or_default();
    let (a, b) = (
        principale.from.min(principale.to),
        principale.from.max(principale.to),
    );
    let mut c = Calcul {
        texte,
        arbre,
        sel: Etendue::new(idx.octet(a), idx.octet(b)),
        actif: !ctx.lecture_seule
            && curseur_actif(ctx.revelation, curseur.focus, curseur.a_eu_le_focus),
        unite: ctx.unite_indentation.max(1),
        idx,
        out: Decorations::default(),
        codes: Vec::new(),
        marques_liste: HashMap::new(),
    };
    let tout = [Etendue::new(0, c.idx.utf16(texte.len()))];
    let visibles = if ctx.visibles.is_empty() {
        &tout[..]
    } else {
        ctx.visibles
    };
    for v in visibles {
        c.codes.clear();
        c.marques_liste.clear();
        arbre.parcourir(v.from, v.to, |i, n| {
            c.retenir(i, n);
            c.noeud(n);
            Parcours::Entrer
        });
        c.indentation(c.idx.octet(v.from), c.idx.octet(v.to));
    }
    c.finir(ctx.taille_texte)
}

struct Calcul<'a> {
    texte: &'a str,
    arbre: &'a Arbre,
    idx: IndexLignes<'a>,
    /// La sélection principale, en octets, `from <= to`.
    sel: Etendue,
    /// Le curseur peut révéler (ni lecture seule, ni avant le 1er focus, ni cellule sans focus).
    actif: bool,
    unite: usize,
    out: Decorations,
    /// Les blocs de code vus dans la plage visible en cours (octets, ordre du document).
    codes: Vec<Etendue>,
    /// Les marques de liste vues dans la plage en cours : début (octet) → profondeur de l'élément.
    marques_liste: HashMap<usize, usize>,
}

impl Calcul<'_> {
    /// `touches` en octets, faux si le curseur ne peut pas révéler.
    fn touche(&self, from: usize, to: usize) -> bool {
        self.actif && touches(self.sel, from, to)
    }

    fn texte(&self, from: usize, to: usize) -> &str {
        self.texte.get(from..to).unwrap_or("")
    }

    fn masquer(&mut self, from: usize, to: usize) {
        let p = self.idx.plage(from, to);
        self.out.rangees.masques.push(p);
    }

    fn marquer(&mut self, from: usize, to: usize, genre: GenreMarque) {
        let plage = self.idx.plage(from, to);
        self.out.marques.push(Marque { plage, genre });
    }

    fn ligne(&mut self, ligne: usize, genre: GenreLigne) {
        self.out.lignes.push(DecoLigne { ligne, genre });
    }

    fn widget(&mut self, from: usize, to: usize, genre: GenreWidget, largeur: Largeur) {
        let source = self.idx.plage(from, to);
        self.out.widgets.push(WidgetEnLigne {
            source,
            genre,
            largeur,
        });
    }

    /// Le parent de `n` touche-t-il le curseur (le nœud lui-même s'il n'en a pas) ?
    fn parent_touche(&self, n: &Noeud) -> bool {
        match n.parent {
            Some(p) => {
                let p = &self.arbre.noeud(p).plage.octets;
                self.touche(p.from, p.to)
            }
            None => self.touche(n.plage.octets.from, n.plage.octets.to),
        }
    }

    /// La fin d'une marque, étendue à l'espace qui la suit s'il y en a une.
    fn avec_espace(&self, to: usize) -> usize {
        if self.texte(to, to + 1) == " " {
            to + 1
        } else {
            to
        }
    }

    /// Juste après le premier `]` de la ligne `n` (la zone « éditable » d'une tâche commence au
    /// début de la ligne et s'arrête là) ; le début de la ligne s'il n'y en a pas.
    fn fin_marqueur_tache(&self, n: usize) -> usize {
        let debut = self.idx.ligne(n).octets.from;
        debut + self.idx.texte_ligne(n).find(']').map_or(0, |k| k + 1)
    }

    fn noeud(&mut self, n: &Noeud) {
        let (nf, nt) = (n.plage.octets.from, n.plage.octets.to);
        match n.genre {
            g @ (Genre::ATXHeading1
            | Genre::ATXHeading2
            | Genre::ATXHeading3
            | Genre::ATXHeading4
            | Genre::ATXHeading5
            | Genre::ATXHeading6) => {
                // Toute la ligne du titre grandit (une décoration de ligne, pas de marque).
                let niveau = g.niveau_titre().unwrap_or(1);
                self.ligne(self.idx.ligne_octet(nf), GenreLigne::Titre(niveau));
            }
            Genre::StrongEmphasis => self.marquer(nf, nt, GenreMarque::Gras),
            Genre::Emphasis => self.marquer(nf, nt, GenreMarque::Italique),
            Genre::Strikethrough => self.marquer(nf, nt, GenreMarque::Barre),
            Genre::Highlight => self.marquer(nf, nt, GenreMarque::Surligne),
            Genre::InlineCode => self.marquer(nf, nt, GenreMarque::Code),
            Genre::InlineMath | Genre::InlineDisplayMath => {
                // Rendu hors curseur, source LaTeX dedans ; une formule vide reste en source.
                if !self.touche(nf, nt) {
                    let affichage = n.genre == Genre::InlineDisplayMath;
                    let d = if affichage { 2 } else { 1 };
                    let tex = self.texte(nf + d, nt.saturating_sub(d)).to_owned();
                    if !tex.trim().is_empty() {
                        self.widget(
                            nf,
                            nt,
                            GenreWidget::Maths { tex, affichage },
                            Largeur::Mesuree,
                        );
                    }
                }
            }
            Genre::WikiLink => {
                if !self.touche(nf, nt) {
                    let interieur = self.texte(nf + 2, nt.saturating_sub(2)).to_owned();
                    if !interieur.trim().is_empty() {
                        let lien = parse_linktext(&interieur);
                        let texte = lien
                            .alias
                            .unwrap_or_else(|| format!("{}{}", lien.path, lien.subpath));
                        self.widget(
                            nf,
                            nt,
                            GenreWidget::LienInterne { interieur, texte },
                            Largeur::Mesuree,
                        );
                    }
                }
            }
            Genre::Image => {
                if !self.touche(nf, nt) {
                    let chemin = IMAGE
                        .captures(self.texte(nf, nt))
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().trim().to_owned());
                    if let Some(chemin) = chemin {
                        let externe = SCHEMA.is_match(&chemin) || chemin.starts_with("data:");
                        if !externe {
                            let chemin = decoder_uri(&chemin).unwrap_or(chemin);
                            self.widget(nf, nt, GenreWidget::Image { chemin }, Largeur::Mesuree);
                        }
                    }
                }
            }
            Genre::FencedCode => self.code_delimite(nf, nt),
            Genre::Link => self.marquer(nf, nt, GenreMarque::Lien),
            Genre::Blockquote => {
                for l in self.idx.ligne_octet(nf)..=self.idx.ligne_octet(nt) {
                    self.ligne(l, GenreLigne::Citation);
                }
            }
            Genre::TaskMarker => {
                let l = self.idx.ligne_octet(nf);
                let debut = self.idx.ligne(l).octets.from;
                if !self.touche(debut, self.fin_marqueur_tache(l)) {
                    let cochee = self.texte(nf, nt).contains(['x', 'X']);
                    let marqueur = self.idx.utf16(nf);
                    self.widget(
                        nf,
                        nt,
                        GenreWidget::Case { cochee, marqueur },
                        Largeur::Em(LARGEUR_INDENTATION_EM),
                    );
                }
            }
            Genre::ListMark => self.marque_de_liste(nf, nt),
            Genre::HorizontalRule => {
                if !self.touche(nf, nt) {
                    let l = self.idx.ligne(self.idx.ligne_octet(nf)).octets;
                    self.widget(l.from, l.to, GenreWidget::Separateur, Largeur::Ligne);
                }
            }
            Genre::HeaderMark | Genre::QuoteMark => {
                // Révélé dès que le curseur est dans le titre ou la citation entière.
                if !self.parent_touche(n) {
                    let fin = self.avec_espace(nt);
                    let l = self.idx.ligne(self.idx.ligne_octet(nf)).octets;
                    // Un `#` ou un `>` seul sur sa ligne reste visible : masqué, la ligne paraîtrait
                    // vide et l'élément serait introuvable.
                    if !self.texte(fin, l.to.max(fin)).trim().is_empty() {
                        self.masquer(nf, fin);
                    }
                }
            }
            Genre::LinkMark | Genre::URL => {
                // Seulement dans un lien `[…](…)` : l'URL d'un lien automatique reste visible.
                if let Some(p) = n.parent {
                    let p = self.arbre.noeud(p);
                    if p.genre == Genre::Link
                        && !self.touche(p.plage.octets.from, p.plage.octets.to)
                    {
                        self.masquer(nf, nt);
                    }
                }
            }
            Genre::EmphasisMark
            | Genre::CodeMark
            | Genre::StrikethroughMark
            | Genre::HighlightMark
                if !self.parent_touche(n) =>
            {
                self.masquer(nf, nt);
            }
            _ => {}
        }
    }

    fn marque_de_liste(&mut self, nf: usize, nt: usize) {
        let l = self.idx.ligne_octet(nf);
        let ligne = self.idx.ligne(l).octets;
        let texte_ligne = self.idx.texte_ligne(l);
        if LIGNE_TACHE.is_match(texte_ligne) {
            // La puce d'une tâche disparaît (il ne reste que la case), même zone active que la case.
            if !self.touche(ligne.from, self.fin_marqueur_tache(l)) {
                let fin = self.avec_espace(nt);
                self.masquer(nf, fin);
                // ★ `- [ ]` SANS texte : GFM n'y voit pas de tâche (il veut une espace et du texte),
                //   donc pas de `TaskMarker`. On pose la case nous-mêmes, sinon `[ ]` resterait brut
                //   derrière une puce masquée.
                if let Some(m) = TACHE_NUE.captures(texte_ligne).and_then(|c| c.get(1)) {
                    let b = ligne.from + m.end();
                    let cochee = self.texte(b, b + 3).contains(['x', 'X']);
                    let marqueur = self.idx.utf16(b);
                    self.widget(
                        b,
                        b + 3,
                        GenreWidget::Case { cochee, marqueur },
                        Largeur::Em(LARGEUR_INDENTATION_EM),
                    );
                }
            }
        } else if matches!(self.texte(nf, nt), "-" | "*" | "+") && !self.touche(nf, nt) {
            // Liste à puces : la puce ronde, sauf curseur sur la marque. (Une liste numérotée garde
            // son numéro.)
            let debut_ligne = self.idx.utf16(ligne.from);
            self.widget(
                nf,
                nt,
                GenreWidget::Puce { debut_ligne },
                Largeur::Em(LARGEUR_INDENTATION_EM),
            );
        }
    }

    /// Un bloc ```` ``` ```` : fond sur chaque ligne, coins arrondis en haut et en bas ; hors curseur,
    /// le texte des clôtures est masqué (jamais leur saut de ligne : rien ne traverse une ligne) et
    /// l'ouverture porte le badge de la langue.
    fn code_delimite(&mut self, nf: usize, nt: usize) {
        let premiere = self.idx.ligne_octet(nf);
        let derniere = self.idx.ligne_octet(nt);
        let ouverture = self.idx.ligne(premiere).octets;
        let cloture = self.idx.ligne(derniere).octets;
        let texte_ouverture = self.idx.texte_ligne(premiere);
        let langue = OUVERTURE_CODE
            .find(texte_ouverture)
            .map_or(texte_ouverture, |m| &texte_ouverture[m.end()..])
            .trim()
            .to_owned();
        // Une vraie clôture ? Pas pendant la frappe d'un bloc encore ouvert.
        let ferme = derniere > premiere && CLOTURE_CODE.is_match(self.idx.texte_ligne(derniere));
        let contenu_fin = if ferme { derniere - 1 } else { derniere };
        let a_du_contenu = contenu_fin > premiere;
        if !self.touche(nf, nt) && a_du_contenu {
            if langue.is_empty() {
                self.masquer(ouverture.from, ouverture.to);
            } else {
                self.widget(
                    ouverture.from,
                    ouverture.to,
                    GenreWidget::BadgeLangue { langue },
                    Largeur::Nulle,
                );
            }
            if ferme {
                self.masquer(cloture.from, cloture.to);
            }
        }
        for l in premiere..=derniere {
            self.ligne(l, GenreLigne::Code);
            if l == premiere {
                self.ligne(l, GenreLigne::CodePremiere);
            }
            if l == derniere {
                self.ligne(l, GenreLigne::CodeDerniere);
            }
        }
    }

    /// Retient, pendant le parcours, ce que les barres d'indentation interrogent ensuite : les blocs
    /// de code et la profondeur de chaque marque de liste.
    ///
    /// ★ Le TS remonte l'arbre depuis la position (`resolveInner(pos, 1)`) à chaque ligne indentée ;
    ///   ici `noeud_a` parcourt les enfants de la racine un à un, d'où un coût quadratique mesuré
    ///   (≈ 90 ms pour 10 000 lignes). Tout nœud qui contient une ligne de la plage la touche, donc
    ///   il est déjà visité : on le note au passage.
    fn retenir(&mut self, i: usize, n: &Noeud) {
        match n.genre {
            Genre::FencedCode | Genre::CodeBlock => self.codes.push(n.plage.octets),
            Genre::ListMark => {
                let listes = self
                    .arbre
                    .ancetres(i)
                    .filter(|&k| {
                        matches!(
                            self.arbre.noeud(k).genre,
                            Genre::BulletList | Genre::OrderedList
                        )
                    })
                    .count();
                self.marques_liste
                    .insert(n.plage.octets.from, listes.saturating_sub(1));
            }
            _ => {}
        }
    }

    /// `pos` est-il dans un bloc de code (délimité ou indenté), `from <= pos < to` ?
    fn dans_du_code(&self, pos: usize) -> bool {
        // Les blocs de code ne s'imbriquent pas : triés par début, disjoints.
        let k = self.codes.partition_point(|c| c.from <= pos);
        k > 0 && pos < self.codes[k - 1].to
    }

    /// La profondeur de l'élément de liste dont la marque commence pile en `pos` (0 = premier
    /// niveau) ; `None` si `pos` n'est pas une marque de liste.
    fn niveau_liste(&self, pos: usize) -> Option<usize> {
        self.marques_liste.get(&pos).copied()
    }

    /// Les barres d'indentation des lignes de `[de, a]` (octets) : un espaceur de largeur fixe par
    /// niveau (les blancs répartis entre eux) et le style de ligne qui trace les traits.
    fn indentation(&mut self, de: usize, a: usize) {
        let (premiere, derniere) = (self.idx.ligne_octet(de), self.idx.ligne_octet(a));
        for l in premiere..=derniere.min(self.idx.nombre().saturating_sub(1)) {
            let debut = self.idx.ligne(l).octets.from;
            let texte_ligne = self.idx.texte_ligne(l);
            let blancs = texte_ligne
                .bytes()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count();
            // Dans du code, l'indentation est du texte : pas de barres.
            if blancs == 0 || self.dans_du_code(debut) {
                continue;
            }
            // Un élément de liste : sa profondeur (une sous-puce à deux espaces est un VRAI niveau) ;
            // sinon les colonnes (une tabulation compte jusqu'au taquet suivant) par unité.
            let niveaux = self
                .niveau_liste(debut + blancs)
                .unwrap_or_else(|| colonnes(&texte_ligne[..blancs], self.unite) / self.unite)
                .min(blancs);
            if niveaux == 0 {
                continue;
            }
            for i in 0..niveaux {
                let x = debut + i * blancs / niveaux;
                let y = debut + (i + 1) * blancs / niveaux;
                self.widget(
                    x,
                    y,
                    GenreWidget::Indentation,
                    Largeur::Em(LARGEUR_INDENTATION_EM),
                );
            }
            self.ligne(l, GenreLigne::Indentation { niveaux });
        }
    }

    fn finir(mut self, taille_texte: f32) -> Decorations {
        let out = &mut self.out;
        out.lignes.sort_by_key(|d| (d.ligne, d.genre));
        out.lignes.dedup();
        for d in &out.lignes {
            match d.genre {
                GenreLigne::Titre(n) => {
                    let taille = taille_titre(n, taille_texte);
                    out.rangees.polices.push(PoliceLigne {
                        ligne: d.ligne,
                        taille,
                    });
                    out.rangees.hauteurs.push(HauteurLigne {
                        ligne: d.ligne,
                        hauteur: taille * INTERLIGNE_TITRE,
                    });
                }
                GenreLigne::Code => out.rangees.polices.push(PoliceLigne {
                    ligne: d.ligne,
                    taille: taille_texte * TAILLE_CODE_EM,
                }),
                _ => {}
            }
        }
        out.widgets
            .sort_by_key(|w| (w.source.octets.from, w.source.octets.to));
        out.widgets.dedup();
        // ★ Un masque DANS la source d'un widget est déjà remplacé par lui (les `[`, `]` d'un `[x]`
        //   que Lezer lit aussi comme un lien, les accents graves sous un badge) : retiré, sinon la
        //   fusion des masques le collerait à un masque voisin et mangerait le début du widget.
        let mut fin_max = Vec::with_capacity(out.widgets.len());
        let mut m = 0;
        for w in &out.widgets {
            m = m.max(w.source.octets.to);
            fin_max.push(m);
        }
        let widgets = &out.widgets;
        out.rangees.masques.retain(|p| {
            let k = widgets.partition_point(|w| w.source.octets.from <= p.octets.from);
            k == 0 || fin_max[k - 1] < p.octets.to
        });
        out.rangees.normaliser();
        out.marques
            .sort_by_key(|m| (m.plage.octets.from, m.plage.octets.to, m.genre));
        out.marques.dedup();
        self.out
    }
}

/// La colonne atteinte au bout de `blancs` (`countColumn`) : une tabulation avance au prochain
/// multiple de `taquet`.
fn colonnes(blancs: &str, taquet: usize) -> usize {
    blancs.chars().fold(0, |n, c| {
        if c == '\t' {
            n + taquet - n % taquet
        } else {
            n + 1
        }
    })
}

/// `decodeURIComponent` : `%XX` → octet ; `None` si une séquence est invalide ou si le résultat
/// n'est pas de l'UTF-8 (Fragment garde alors le chemin tel quel).
fn decoder_uri(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = s.get(i + 1..i + 3)?;
            if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colonnes_avec_tabulations() {
        assert_eq!(colonnes("  ", 4), 2);
        assert_eq!(colonnes("\t", 4), 4);
        assert_eq!(colonnes(" \t ", 4), 5);
    }

    #[test]
    fn decoder_uri_comme_decode_uri_component() {
        assert_eq!(decoder_uri("a%20b.png").as_deref(), Some("a b.png"));
        assert_eq!(decoder_uri("%C3%A9t%C3%A9").as_deref(), Some("été"));
        assert_eq!(decoder_uri("100%"), None);
        assert_eq!(decoder_uri("%FF"), None);
    }
}
