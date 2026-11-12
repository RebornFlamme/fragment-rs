// Port de la traduction décorations → vue de app/src/ui/views/markdown/decorationPlugin.ts
// (`Decoration.replace`, `.mark`, `.line`), réécrite pour le fork de l'éditeur (ADR-006).
//! La PROJECTION du modèle du live preview sur le fork : ce que l'élément éditeur pose sur son
//! `EditorState` après chaque changement de texte, de sélection ou de focus.
//!
//! - `fork` : masques sans trou, hauteurs et tailles de police des lignes (`set_row_model`,
//!   `set_line_font_sizes`) ;
//! - `marques` : les styles de texte des marques en ligne et la coloration du code (jetons
//!   `--code-*` de `syntaxe::imbrique`), en octets ;
//! - `lignes` : le style du texte des lignes entières (titres en gras, citation…).
//!
//! Le calcul est pur (texte, arbres, curseur, thème) : testé sans fenêtre.
//!
//! [`projeter`] calcule le document entier (l'ouverture, un changement de thème, et l'ORACLE des
//! tests) ; [`Projeteur`] tient la même projection à jour par RÉGIONS (`fr-t3.4`) : après une
//! frappe ou un déplacement du curseur, seuls les blocs de tête touchés sont recalculés, le reste est
//! décalé. Le résultat est IDENTIQUE à `projeter` (oracle de `tests::incremental_egal_au_complet`).

use std::ops::Range;
use std::rc::Rc;

use fragment_markdown::decorations::{
    self, Contexte, Curseur, Decorations, GenreWidget, curseur_actif,
};
use fragment_markdown::syntaxe::imbrique::Imbrique;
use fragment_markdown::syntaxe::modele_rangees::{
    ArgumentsFork, HauteurLigne, ModeleRangees, PoliceLigne, lignes_de,
};
use fragment_markdown::syntaxe::{Arbre, Changement, Etendue, Genre, Plage};
use fragment_text::utf16::utf16_len;
use fragment_theme::Theme;
use gpui_kit::HighlightStyle;

use super::jetons;

/// Une plage stylée, en octets.
type Style = (Range<usize>, HighlightStyle);

/// Ce que l'éditeur pose sur le fork pour un état donné.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Projection {
    pub fork: ArgumentsFork,
    pub marques: Vec<Style>,
    pub lignes: Vec<Style>,
    /// Le modèle d'où viennent les trois premiers champs (pour les tests et les widgets aval).
    pub decorations: Decorations,
}

/// Ce que lit un complément de rangées : l'état du document au moment de la projection.
pub struct EntreeComplement<'a> {
    pub texte: &'a str,
    pub arbre: &'a Arbre,
    /// Les lignes logiques de `texte`.
    pub lignes: &'a [Plage],
    pub curseur: &'a Curseur<'a>,
    /// La configuration de l'éditeur (`visibles` vide : le document entier).
    pub ctx: &'a Contexte<'a>,
}

/// Un complément des rangées des décorations (les blocs de `fr-t3-widgets-blocs`, le repli…),
/// appliqué sur une COPIE du modèle de rangées avant sa traduction pour le fork. Il voit toujours le
/// modèle du document ENTIER.
pub type ComplementRangees = Rc<dyn Fn(&EntreeComplement<'_>, &mut ModeleRangees)>;

/// Les arguments du fork : les rangées des décorations, complétées s'il y a lieu, normalisées.
fn arguments_fork(
    rangees: &ModeleRangees,
    complement: Option<&ComplementRangees>,
    entree: &EntreeComplement<'_>,
) -> ArgumentsFork {
    let Some(f) = complement else {
        return rangees.vers_fork(entree.lignes);
    };
    let mut r = rangees.clone();
    f(entree, &mut r);
    r.normaliser();
    r.vers_fork(entree.lignes)
}

/// Calcule la projection de `texte` (arbre et régions imbriquées à jour) pour ce curseur, sur le
/// document ENTIER.
///
/// ★ Le document entier, pas le viewport : un masque ou une hauteur de titre hors écran change la
///   hauteur du contenu (barre de défilement, `pos_at_coords` hors écran). C'est le [`Projeteur`]
///   qui rend la mise à jour bon marché, en gardant ce qui n'a pas changé.
#[must_use]
pub fn projeter(
    texte: &str,
    arbre: &Arbre,
    imbrique: &Imbrique,
    curseur: &Curseur,
    ctx: &Contexte,
    theme: &Theme,
    complement: Option<&ComplementRangees>,
) -> Projection {
    let decorations = decorations::calculer(texte, arbre, curseur, ctx);
    let lignes_logiques = lignes_de(texte);
    let fork = arguments_fork(
        &decorations.rangees,
        complement,
        &EntreeComplement {
            texte,
            arbre,
            lignes: &lignes_logiques,
            curseur,
            ctx,
        },
    );
    let mut marques = styler_marques(&decorations, theme);
    marques.extend(styler_jetons(imbrique, texte, 0, texte.len(), theme));
    marques.sort_by_key(|(r, _)| (r.start, r.end));
    let lignes = styler_lignes(&decorations, &lignes_logiques, theme);
    Projection {
        fork,
        marques,
        lignes,
        decorations,
    }
}

/// Les marques en ligne du modèle, en styles de texte (dans l'ordre du modèle).
fn styler_marques(d: &Decorations, theme: &Theme) -> Vec<Style> {
    d.marques
        .iter()
        .filter(|m| !m.plage.octets.is_empty())
        .map(|m| {
            (
                m.plage.octets.range(),
                jetons::style_marque(theme, &m.genre.style()),
            )
        })
        .collect()
}

/// Les jetons de coloration du code qui rencontrent `[de, a)` (octets).
fn styler_jetons(
    imbrique: &Imbrique,
    texte: &str,
    de: usize,
    a: usize,
    theme: &Theme,
) -> Vec<Style> {
    imbrique
        .jetons_dans(texte, de, a)
        .into_iter()
        .filter(|j| !j.plage.octets.is_empty())
        .map(|j| (j.plage.octets.range(), jetons::style_code(theme, j.jeton)))
        .collect()
}

/// Les styles de lignes entières (réduits à ce qui se pose sur leur texte).
fn styler_lignes(d: &Decorations, lignes: &[Plage], theme: &Theme) -> Vec<Style> {
    d.lignes
        .iter()
        .filter_map(|d| {
            let plage = lignes.get(d.ligne)?;
            let style = jetons::style_ligne(theme, &d.genre.style());
            (!plage.octets.is_empty() && style != HighlightStyle::default())
                .then(|| (plage.octets.range(), style))
        })
        .collect()
}

/// La ligne logique qui contient l'octet `o` (l'octet du saut de ligne compte dans sa ligne).
fn ligne_de(lignes: &[Plage], o: usize) -> usize {
    lignes
        .partition_point(|l| l.octets.to < o)
        .min(lignes.len().saturating_sub(1))
}

/// L'octet `o` en UTF-16, par la ligne qui le contient (sans reparcourir le texte depuis 0).
#[must_use]
pub fn vers_utf16(texte: &str, lignes: &[Plage], o: usize) -> usize {
    let Some(p) = lignes.get(ligne_de(lignes, o)) else {
        return 0;
    };
    let fin = o.clamp(p.octets.from, p.octets.to);
    p.utf16.from + utf16_len(texte.get(p.octets.from..fin).unwrap_or("")) + (o - fin)
}

/// Les lignes logiques du nouveau texte, déduites de celles de l'ancien : seules les lignes touchées
/// par `c` sont relues, les suivantes sont décalées. Identique à `lignes_de(nouveau)`.
///
/// ★ `lignes_de` relit tout le texte (et compte l'UTF-16 de chaque caractère) : ≈ 1 ms par frappe
///   sur 10 000 lignes en release, pour des lignes qui n'ont presque toutes pas changé.
#[must_use]
pub fn lignes_apres(anciennes: &[Plage], nouveau: &str, c: Changement) -> Vec<Plage> {
    let (l0, l1) = (ligne_de(anciennes, c.de), ligne_de(anciennes, c.fin_ancien));
    let (Some(p0), Some(p1)) = (anciennes.get(l0), anciennes.get(l1)) else {
        return lignes_de(nouveau);
    };
    // La zone touchée : du début de `l0` à la fin de `l1`, décalée de l'édition.
    let (debut, fin) = (p0.octets.from, p1.octets.to - c.fin_ancien + c.fin_nouveau);
    let mut res = Vec::with_capacity(anciennes.len() + 1);
    res.extend_from_slice(&anciennes[..l0]);
    let (mut octet, mut u16) = (debut, p0.utf16.from);
    for l in nouveau.get(debut..fin).unwrap_or("").split('\n') {
        let n16 = utf16_len(l);
        res.push(Plage {
            utf16: Etendue::new(u16, u16 + n16),
            octets: Etendue::new(octet, octet + l.len()),
        });
        octet += l.len() + 1;
        u16 += n16 + 1;
    }
    // Les suivantes : décalées, sans arithmétique signée (elles sont toutes au-delà de `l1`).
    let (fin16, ancien_fin16) = (u16 - 1, p1.utf16.to);
    for p in &anciennes[l1 + 1..] {
        res.push(Plage {
            utf16: Etendue::new(
                p.utf16.from - ancien_fin16 + fin16,
                p.utf16.to - ancien_fin16 + fin16,
            ),
            octets: Etendue::new(
                p.octets.from - p1.octets.to + fin,
                p.octets.to - p1.octets.to + fin,
            ),
        });
    }
    res
}

/// Ce que devient une position de l'ancien texte après une édition : ce qui précède les lignes
/// touchées reste, ce qui les suit est décalé, ce qui est DESSUS est retiré (recalculé ensuite).
///
/// ★ Pas d'arithmétique signée : une position décalée est toujours au-delà de la fin de
///   l'édition, `p - fin_ancien + fin_nouveau` ne déborde pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decalage {
    pub changement: Changement,
    /// La fin de l'édition en UTF-16 : (ancien texte, nouveau texte).
    fin_utf16: (usize, usize),
    /// Lignes logiques touchées de l'ancien texte (`L0..=L1`), et `L1` dans le nouveau.
    lignes: (usize, usize),
    derniere_nouvelle: usize,
    /// Octets de l'ancien texte : début de `L0`, fin de `L1` (sans son saut de ligne).
    zone: (usize, usize),
}

impl Decalage {
    /// Le décalage de `c` (ancien → nouveau), avec les lignes logiques des deux textes.
    #[must_use]
    pub fn new(
        ancien: &str,
        anciennes: &[Plage],
        nouveau: &str,
        nouvelles: &[Plage],
        c: Changement,
    ) -> Decalage {
        let (l0, l1) = (ligne_de(anciennes, c.de), ligne_de(anciennes, c.fin_ancien));
        Decalage {
            changement: c,
            fin_utf16: (
                vers_utf16(ancien, anciennes, c.fin_ancien),
                vers_utf16(nouveau, nouvelles, c.fin_nouveau),
            ),
            lignes: (l0, l1),
            derniere_nouvelle: ligne_de(nouvelles, c.fin_nouveau),
            zone: (
                anciennes.get(l0).map_or(0, |p| p.octets.from),
                anciennes.get(l1).map_or(0, |p| p.octets.to),
            ),
        }
    }

    /// Un élément qui COMMENCE à l'octet `o` : gardé, décalé (`Some`) ou retiré (`None`).
    fn octet(&self, o: usize) -> Option<usize> {
        if o < self.zone.0 {
            Some(o)
        } else if o > self.zone.1 {
            Some(o - self.changement.fin_ancien + self.changement.fin_nouveau)
        } else {
            None
        }
    }

    /// Une position UTF-16 d'un élément décalé (au-delà de l'édition) ou gardé (avant elle).
    fn utf16(&self, u: usize) -> usize {
        if u >= self.fin_utf16.0 {
            u - self.fin_utf16.0 + self.fin_utf16.1
        } else {
            u
        }
    }

    fn plage(&self, p: Plage) -> Option<Plage> {
        let de = self.octet(p.octets.from)?;
        if p.octets.from < self.zone.0 {
            return Some(p);
        }
        Some(Plage {
            utf16: Etendue::new(self.utf16(p.utf16.from), self.utf16(p.utf16.to)),
            octets: Etendue::new(de, p.octets.to - p.octets.from + de),
        })
    }

    fn ligne(&self, l: usize) -> Option<usize> {
        if l < self.lignes.0 {
            Some(l)
        } else if l > self.lignes.1 {
            Some(l - self.lignes.1 + self.derniere_nouvelle)
        } else {
            None
        }
    }

    fn style(&self, (r, s): Style) -> Option<Style> {
        let de = self.octet(r.start)?;
        Some((de..r.end - r.start + de, s))
    }

    /// `mapPos` d'une borne : dans l'édition, son début (`apres` faux) ou sa fin (`apres` vrai).
    fn borne(&self, o: usize, apres: bool) -> usize {
        let c = self.changement;
        if o < c.de {
            o
        } else if o >= c.fin_ancien {
            o - c.fin_ancien + c.fin_nouveau
        } else if apres {
            c.fin_nouveau
        } else {
            c.de
        }
    }

    /// Décale les éléments d'une projection (le `fork` se recalcule à part).
    fn appliquer(&self, p: &mut Projection) {
        let d = &mut p.decorations;
        d.rangees.masques = d
            .rangees
            .masques
            .iter()
            .filter_map(|m| self.plage(*m))
            .collect();
        for h in std::mem::take(&mut d.rangees.hauteurs) {
            if let Some(ligne) = self.ligne(h.ligne) {
                d.rangees.hauteurs.push(HauteurLigne { ligne, ..h });
            }
        }
        for po in std::mem::take(&mut d.rangees.polices) {
            if let Some(ligne) = self.ligne(po.ligne) {
                d.rangees.polices.push(PoliceLigne { ligne, ..po });
            }
        }
        for m in std::mem::take(&mut d.marques) {
            if let Some(plage) = self.plage(m.plage) {
                d.marques.push(decorations::Marque { plage, ..m });
            }
        }
        for l in std::mem::take(&mut d.lignes) {
            if let Some(ligne) = self.ligne(l.ligne) {
                d.lignes.push(decorations::DecoLigne { ligne, ..l });
            }
        }
        for mut w in std::mem::take(&mut d.widgets) {
            let Some(source) = self.plage(w.source) else {
                continue;
            };
            if w.source.octets.from > self.zone.1 {
                match &mut w.genre {
                    GenreWidget::Case { marqueur, .. } => *marqueur = self.utf16(*marqueur),
                    GenreWidget::Puce { debut_ligne } => *debut_ligne = self.utf16(*debut_ligne),
                    _ => {}
                }
            }
            w.source = source;
            d.widgets.push(w);
        }
        p.marques = std::mem::take(&mut p.marques)
            .into_iter()
            .filter_map(|s| self.style(s))
            .collect();
        p.lignes = std::mem::take(&mut p.lignes)
            .into_iter()
            .filter_map(|s| self.style(s))
            .collect();
    }
}

/// Ce que lit le projeteur à chaque mise à jour.
#[derive(Clone, Copy)]
pub struct Entree<'a> {
    pub texte: &'a str,
    /// Les lignes logiques de `texte` (`lignes_de`).
    pub lignes: &'a [Plage],
    pub arbre: &'a Arbre,
    pub imbrique: &'a Imbrique,
    /// La sélection principale, en OCTETS, `from <= to`.
    pub selection: Etendue,
    pub focus: bool,
    pub a_eu_le_focus: bool,
    /// La configuration de l'éditeur (`visibles` est ignoré : le projeteur choisit ses régions).
    pub ctx: Contexte<'a>,
    pub theme: &'a Theme,
    /// Le complément des rangées (`None` : les rangées des décorations seules).
    pub complement: Option<&'a ComplementRangees>,
}

impl Entree<'_> {
    fn actif(&self) -> bool {
        !self.ctx.lecture_seule
            && curseur_actif(self.ctx.revelation, self.focus, self.a_eu_le_focus)
    }

    fn complement_de<'b>(
        &'b self,
        curseur: &'b Curseur<'b>,
        ctx: &'b Contexte<'b>,
    ) -> EntreeComplement<'b> {
        EntreeComplement {
            texte: self.texte,
            arbre: self.arbre,
            lignes: self.lignes,
            curseur,
            ctx,
        }
    }

    fn selection_utf16(&self) -> [Etendue; 1] {
        [Etendue::new(
            vers_utf16(self.texte, self.lignes, self.selection.from),
            vers_utf16(self.texte, self.lignes, self.selection.to),
        )]
    }
}

/// Ce qu'une mise à jour a changé, pour ne reposer sur le fork que ce qui doit l'être.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changements {
    pub marques: bool,
    pub lignes: bool,
}

impl Changements {
    const TOUT: Changements = Changements {
        marques: true,
        lignes: true,
    };
}

/// Les blocs de tête (enfants de `Document`) : genre et plage en octets.
fn blocs_de_tete(arbre: &Arbre) -> Vec<(Genre, Etendue)> {
    arbre
        .enfants(0)
        .map(|i| {
            let n = arbre.noeud(i);
            (n.genre, n.plage.octets)
        })
        .collect()
}

/// Les régions (lignes logiques `[premiere, derniere]`, triées, disjointes) qui couvrent ces
/// intervalles d'octets, chacun étendu aux blocs de tête qu'il touche (bornes comprises).
///
/// ★ Une région est faite de blocs de tête ENTIERS : les décorations d'un nœud ne dépendent que de
///   lui, de ses ancêtres et du curseur, et tout ancêtre (hors la racine) est dans le même bloc de
///   tête. Recalculer un bloc entier rend donc exactement ce que donnerait le document entier.
fn regions(
    intervalles: &[(usize, usize)],
    blocs: &[(Genre, Etendue)],
    lignes: &[Plage],
) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = intervalles
        .iter()
        .map(|&(a, b)| {
            let (mut a, mut b) = (a.min(b), a.max(b));
            let mut i = blocs.partition_point(|(_, e)| e.to < a);
            while let Some((_, e)) = blocs.get(i) {
                if e.from > b {
                    break;
                }
                a = a.min(e.from);
                b = b.max(e.to);
                i += 1;
            }
            (ligne_de(lignes, a), ligne_de(lignes, b))
        })
        .collect();
    v.sort_unstable();
    let mut fusion: Vec<(usize, usize)> = Vec::with_capacity(v.len());
    for (a, b) in v {
        match fusion.last_mut() {
            Some(d) if a <= d.1 + 1 => d.1 = d.1.max(b),
            _ => fusion.push((a, b)),
        }
    }
    fusion
}

/// Remplace dans `liste` les éléments dont la position (`pos`) tombe dans l'une des `bornes`
/// (triées, disjointes, bornes comprises) par `nouveaux`, et rend vrai si ce qui a été retiré
/// diffère de ce qui est posé. `liste` et `nouveaux` sont triés d'abord par `pos`.
///
/// ★ Un `splice` par région, pas un tri : les éléments d'une région sont CONTIGUS dans la liste
///   triée, et l'ordre de `nouveaux` (à position égale) est celui du calcul complet.
fn remplacer<T: PartialEq>(
    liste: &mut Vec<T>,
    mut nouveaux: Vec<T>,
    pos: impl Fn(&T) -> usize,
    bornes: &[(usize, usize)],
) -> bool {
    let mut change = false;
    for &(a, b) in bornes.iter().rev() {
        let debut = nouveaux.partition_point(|x| pos(x) < a);
        let part: Vec<T> = nouveaux.drain(debut..).collect();
        let lo = liste.partition_point(|x| pos(x) < a);
        let hi = lo + liste[lo..].partition_point(|x| pos(x) <= b);
        change |= liste[lo..hi] != part[..];
        liste.splice(lo..hi, part);
    }
    change
}

/// La projection d'un éditeur, tenue à jour par régions.
#[derive(Debug, Clone, Default)]
pub struct Projeteur {
    projection: Projection,
    /// Les blocs de tête de l'arbre de la dernière mise à jour.
    blocs: Vec<(Genre, Etendue)>,
    /// Le curseur pouvait-il révéler, à la dernière mise à jour (`None` : jamais projeté) ?
    actif: Option<bool>,
    /// La sélection de la dernière mise à jour (octets).
    selection: Etendue,
}

impl Projeteur {
    /// La projection courante.
    #[must_use]
    pub fn projection(&self) -> &Projection {
        &self.projection
    }

    /// Recalcule tout (ouverture, thème, droit de révéler).
    pub fn complet(&mut self, e: &Entree) -> Changements {
        let selections = e.selection_utf16();
        let curseur = Curseur {
            selections: &selections,
            focus: e.focus,
            a_eu_le_focus: e.a_eu_le_focus,
        };
        let ctx = Contexte {
            visibles: &[],
            ..e.ctx
        };
        self.projection = projeter(
            e.texte,
            e.arbre,
            e.imbrique,
            &curseur,
            &ctx,
            e.theme,
            e.complement,
        );
        self.blocs = blocs_de_tete(e.arbre);
        self.actif = Some(e.actif());
        self.selection = e.selection;
        Changements::TOUT
    }

    /// Après une édition `d` : décale ce qui suit, recalcule les blocs touchés par l'édition, ceux
    /// dont la structure a changé, et ceux de l'ancienne et de la nouvelle sélection.
    pub fn apres_edition(&mut self, e: &Entree, d: &Decalage) -> Changements {
        if self.actif != Some(e.actif()) {
            return self.complet(e);
        }
        d.appliquer(&mut self.projection);
        let anciens: Vec<(Genre, Etendue)> = self
            .blocs
            .iter()
            .map(|&(g, x)| (g, Etendue::new(d.borne(x.from, false), d.borne(x.to, true))))
            .collect();
        let nouveaux = blocs_de_tete(e.arbre);
        // Préfixe et suffixe communs : entre les deux, la structure a changé (une clôture de code
        // ouverte qui avale la suite, un paragraphe devenu titre…).
        let prefixe = anciens
            .iter()
            .zip(&nouveaux)
            .take_while(|(a, n)| a == n)
            .count();
        let reste = anciens.len().min(nouveaux.len()) - prefixe;
        let suffixe = anciens
            .iter()
            .rev()
            .zip(nouveaux.iter().rev())
            .take(reste)
            .take_while(|(a, n)| a == n)
            .count();
        let c = d.changement;
        let s = self.selection;
        let mut intervalles = vec![
            (c.de, c.fin_nouveau),
            (d.borne(s.from, false), d.borne(s.to, true)),
            (e.selection.from, e.selection.to),
        ];
        let differents = anciens[prefixe..anciens.len() - suffixe]
            .iter()
            .chain(&nouveaux[prefixe..nouveaux.len() - suffixe]);
        for (_, x) in differents {
            intervalles.push((x.from, x.to));
        }
        self.blocs = nouveaux;
        self.partiel(e, &intervalles);
        Changements::TOUT
    }

    /// Après un déplacement de la sélection (texte inchangé).
    pub fn apres_selection(&mut self, e: &Entree) -> Changements {
        if self.actif != Some(e.actif()) {
            return self.complet(e);
        }
        if e.selection == self.selection {
            return Changements::default();
        }
        let s = self.selection;
        self.partiel(e, &[(s.from, s.to), (e.selection.from, e.selection.to)])
    }

    /// Recalcule les régions qui couvrent `intervalles` (octets du texte courant).
    fn partiel(&mut self, e: &Entree, intervalles: &[(usize, usize)]) -> Changements {
        self.selection = e.selection;
        let regions = regions(intervalles, &self.blocs, e.lignes);
        let bornes = |&(a, b): &(usize, usize)| -> Option<(Plage, Plage)> {
            Some((*e.lignes.get(a)?, *e.lignes.get(b)?))
        };
        let plages: Vec<(Plage, Plage)> = regions.iter().filter_map(bornes).collect();
        let visibles: Vec<Etendue> = plages
            .iter()
            .map(|(a, b)| Etendue::new(a.utf16.from, b.utf16.to))
            .collect();
        let octets: Vec<(usize, usize)> = plages
            .iter()
            .map(|(a, b)| (a.octets.from, b.octets.to))
            .collect();
        let dans_octets = |o: usize| {
            let k = octets.partition_point(|r| r.1 < o);
            octets.get(k).is_some_and(|r| r.0 <= o)
        };
        let dans_lignes = |l: usize| {
            let k = regions.partition_point(|r| r.1 < l);
            regions.get(k).is_some_and(|r| r.0 <= l)
        };

        let selections = e.selection_utf16();
        let curseur = Curseur {
            selections: &selections,
            focus: e.focus,
            a_eu_le_focus: e.a_eu_le_focus,
        };
        let ctx = Contexte {
            visibles: &visibles,
            ..e.ctx
        };
        // ★ Sans région, `visibles` vide voudrait dire « tout le texte ».
        let mut neuves = if visibles.is_empty() {
            Decorations::default()
        } else {
            decorations::calculer_avec_lignes(e.texte, e.lignes, e.arbre, &curseur, &ctx)
        };
        // Le parcours visite aussi les nœuds qui TOUCHENT une région par sa borne : on ne garde que
        // ce qui commence dedans.
        neuves
            .rangees
            .masques
            .retain(|m| dans_octets(m.octets.from));
        neuves.rangees.hauteurs.retain(|h| dans_lignes(h.ligne));
        neuves.rangees.polices.retain(|p| dans_lignes(p.ligne));
        neuves.marques.retain(|m| dans_octets(m.plage.octets.from));
        neuves.lignes.retain(|l| dans_lignes(l.ligne));
        neuves.widgets.retain(|w| dans_octets(w.source.octets.from));

        let mut marques = styler_marques(&neuves, e.theme);
        // ★ `jetons()` plutôt que `jetons_dans` : celui-ci recompte l'UTF-16 depuis le début du
        //   texte, dont on n'a pas besoin (le fork lit des octets).
        let mut jetons = e.imbrique.jetons().peekable();
        for &(a, b) in &octets {
            while jetons.next_if(|(x, _)| x.from < a).is_some() {}
            while let Some((x, j)) = jetons.next_if(|(x, _)| x.from <= b) {
                if !x.is_empty() {
                    marques.push((x.range(), jetons::style_code(e.theme, j)));
                }
            }
        }
        marques.sort_by_key(|(r, _)| (r.start, r.end));
        let lignes = styler_lignes(&neuves, e.lignes, e.theme);

        let p = &mut self.projection;
        let d = &mut p.decorations;
        remplacer(
            &mut d.rangees.masques,
            neuves.rangees.masques,
            |m| m.octets.from,
            &octets,
        );
        remplacer(
            &mut d.rangees.hauteurs,
            neuves.rangees.hauteurs,
            |h| h.ligne,
            &regions,
        );
        remplacer(
            &mut d.rangees.polices,
            neuves.rangees.polices,
            |p| p.ligne,
            &regions,
        );
        remplacer(
            &mut d.marques,
            neuves.marques,
            |m| m.plage.octets.from,
            &octets,
        );
        remplacer(&mut d.lignes, neuves.lignes, |l| l.ligne, &regions);
        remplacer(
            &mut d.widgets,
            neuves.widgets,
            |w| w.source.octets.from,
            &octets,
        );
        let changements = Changements {
            marques: remplacer(&mut p.marques, marques, |(r, _)| r.start, &octets),
            lignes: remplacer(&mut p.lignes, lignes, |(r, _)| r.start, &octets),
        };
        let ctx = Contexte {
            visibles: &[],
            ..e.ctx
        };
        p.fork = arguments_fork(
            &p.decorations.rangees,
            e.complement,
            &e.complement_de(&curseur, &ctx),
        );
        changements
    }

    /// Recalcule seulement les arguments du fork (le complément a changé : un repli, une hauteur
    /// mesurée), sans toucher aux décorations.
    pub fn rangees(&mut self, e: &Entree) {
        let selections = e.selection_utf16();
        let curseur = Curseur {
            selections: &selections,
            focus: e.focus,
            a_eu_le_focus: e.a_eu_le_focus,
        };
        let ctx = Contexte {
            visibles: &[],
            ..e.ctx
        };
        self.projection.fork = arguments_fork(
            &self.projection.decorations.rangees,
            e.complement,
            &e.complement_de(&curseur, &ctx),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::decorations::Revelation;
    use fragment_markdown::syntaxe::analyser;

    fn projection(texte: &str, curseur: usize, focus: bool) -> Projection {
        let arbre = analyser(texte);
        let imbrique = Imbrique::analyser(texte, &arbre);
        let sel = [Etendue::new(curseur, curseur)];
        let c = Curseur {
            selections: &sel,
            focus,
            a_eu_le_focus: focus,
        };
        let ctx = Contexte {
            revelation: Revelation::ApresPremierFocus,
            ..Contexte::default()
        };
        projeter(texte, &arbre, &imbrique, &c, &ctx, &Theme::sombre(), None)
    }

    #[test]
    fn titre_masque_hors_curseur_et_plus_grand() {
        let t = "# Titre\n\ncorps";
        let p = projection(t, t.len(), true);
        assert_eq!(p.fork.masques, vec![(0..2)]);
        // 1,9 em × 16 px, interligne 1,25.
        assert_eq!(p.fork.polices, [(0, 1.9 * 16.0)]);
        assert_eq!(p.fork.hauteurs, [(0, 1.9 * 16.0 * 1.25)]);
        // Le titre est en gras sur toute sa ligne.
        assert!(
            p.lignes
                .iter()
                .any(|(r, s)| *r == (0..7) && s.font_weight.is_some())
        );
    }

    #[test]
    fn avant_le_premier_focus_rien_n_est_revele() {
        let t = "# Titre en **gras**\n\nsuite";
        // Curseur en 0 (la 1re ligne), jamais focalisé : tout est rendu.
        let p = projection(t, 0, false);
        assert!(p.fork.masques.contains(&(0..2)));
        // Focalisé dans le titre : `# ` se révèle ; `**` seulement si le curseur touche le gras.
        let p = projection(t, 3, true);
        assert!(!p.fork.masques.contains(&(0..2)));
        assert!(p.fork.masques.contains(&(11..13)));
        let p = projection(t, 14, true);
        assert!(p.fork.masques.is_empty(), "{:?}", p.fork.masques);
    }

    #[test]
    fn la_coloration_du_code_arrive_par_jetons() {
        let t = "```rust\nfn main() {}\n```\n";
        let p = projection(t, t.len(), true);
        // Au moins un jeton coloré dans la ligne `fn main() {}` (octets 8 à 20).
        assert!(
            p.marques
                .iter()
                .any(|(r, s)| r.start >= 8 && r.end <= 20 && s.color.is_some()),
            "{:?}",
            p.marques
        );
    }

    // ─── Incrémental (fr-t3.4) ───

    /// Un document et son état d'éditeur, réanalysé comme le fait l'élément.
    struct Doc {
        texte: String,
        lignes: Vec<Plage>,
        arbre: Arbre,
        imbrique: Imbrique,
        selection: Etendue,
        focus: bool,
        theme: Theme,
        complement: Option<ComplementRangees>,
    }

    impl Doc {
        fn new(texte: &str) -> Doc {
            let arbre = analyser(texte);
            let imbrique = Imbrique::analyser(texte, &arbre);
            Doc {
                texte: texte.to_owned(),
                lignes: lignes_de(texte),
                arbre,
                imbrique,
                selection: Etendue::new(0, 0),
                focus: true,
                theme: Theme::sombre(),
                complement: None,
            }
        }

        fn entree(&self) -> Entree<'_> {
            Entree {
                texte: &self.texte,
                lignes: &self.lignes,
                arbre: &self.arbre,
                imbrique: &self.imbrique,
                selection: self.selection,
                focus: self.focus,
                a_eu_le_focus: true,
                ctx: Contexte {
                    revelation: Revelation::ApresPremierFocus,
                    ..Contexte::default()
                },
                theme: &self.theme,
                complement: self.complement.as_ref(),
            }
        }

        /// Remplace `[de, a)` par `insere`, curseur après l'insertion ; rend le décalage.
        fn editer(&mut self, de: usize, a: usize, insere: &str) -> Decalage {
            let mut nouveau = self.texte.clone();
            nouveau.replace_range(de..a, insere);
            let c = Changement::entre(&self.texte, &nouveau);
            let ancien = std::mem::replace(&mut self.arbre, analyser(""));
            self.arbre = ancien.reanalyser(&nouveau, c);
            let vide = Imbrique::analyser("", &analyser(""));
            let ancienne = std::mem::replace(&mut self.imbrique, vide);
            self.imbrique = ancienne.reanalyser(&nouveau, &self.arbre, c);
            let lignes = lignes_apres(&self.lignes, &nouveau, c);
            assert_eq!(lignes, lignes_de(&nouveau), "lignes décalées");
            let d = Decalage::new(&self.texte, &self.lignes, &nouveau, &lignes, c);
            self.texte = nouveau;
            self.lignes = lignes;
            let fin = de + insere.len();
            self.selection = Etendue::new(fin, fin);
            d
        }

        fn complet(&self) -> Projection {
            let mut p = Projeteur::default();
            p.complet(&self.entree());
            p.projection
        }
    }

    const DOC: &str = "---\ntitre: x\n---\n# Titre **gras**\n\nUn paragraphe avec `code`, ==surligné==, \
        *italique* et [lien](https://e.org).\nsuite du paragraphe **sur\ndeux lignes**\n\n\
        ```rust\nfn f(x: u32) -> u32 { x + 1 } // c\n```\n\n> citation **forte**\n> suite\n\n\
        - [ ] tâche\n- puce\n  - sous-puce `x`\n    texte indenté\n\n1. un\n2. deux\n\n---\n\n\
        ## Section $x^2$ et [[note|alias]]\n\n    code indenté\n\n![img](a%20b.png)\nfin 😀 é";

    /// Petit générateur pseudo-aléatoire déterministe (pas de dépendance nouvelle).
    struct Alea(u64);
    impl Alea {
        fn suivant(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(self.0 >> 33).unwrap_or(0) % n.max(1)
        }
    }

    fn frontiere(t: &str, mut o: usize) -> usize {
        o = o.min(t.len());
        while !t.is_char_boundary(o) {
            o -= 1;
        }
        o
    }

    /// ★ L'oracle : après chaque frappe, suppression, collage ou déplacement, la projection tenue
    /// par régions est IDENTIQUE au calcul complet (fork, styles et modèle).
    #[test]
    fn incremental_egal_au_complet() {
        oracle(None);
    }

    /// Un complément (ici : un bloc sous la ligne du curseur, une hauteur sous chaque citation) est
    /// appliqué à chaque mise à jour, par régions comme au complet.
    #[test]
    fn incremental_egal_au_complet_avec_un_complement() {
        use fragment_markdown::syntaxe::modele_rangees::{Bloc, GenreBloc, Placement};
        let complement: ComplementRangees = Rc::new(|e, r| {
            let u = e.curseur.selections.first().map_or(0, |s| s.from);
            let l = e.lignes.partition_point(|p| p.utf16.to < u);
            if let Some(source) = e.lignes.get(l) {
                r.blocs.push(Bloc {
                    genre: GenreBloc::Maths,
                    source: *source,
                    premiere_ligne: l,
                    derniere_ligne: l,
                    placement: Placement::Dessous,
                    hauteur: 10.0,
                });
            }
            for (n, p) in e.lignes.iter().enumerate() {
                if p.texte(e.texte).starts_with("> ") {
                    r.hauteurs.push(HauteurLigne {
                        ligne: n,
                        hauteur: 30.0,
                    });
                }
            }
        });
        oracle(Some(complement));
    }

    fn oracle(complement: Option<ComplementRangees>) {
        let fragments = [
            "x", "\n", "**", "`", "```", "\n```\n", "# ", "> ", "- ", "  ", "==", "[", "](u)", "$",
            "---\n", "é", "😀", "\n\n", "~~", "[[a]]", "    ",
        ];
        let mut doc = Doc::new(DOC);
        doc.complement = complement;
        let mut p = Projeteur::default();
        p.complet(&doc.entree());
        if doc.complement.is_some() {
            assert!(
                !p.projection.fork.blocs.is_empty(),
                "le complément est appliqué"
            );
        }
        let mut alea = Alea(7);
        for pas in 0..600 {
            let n = doc.texte.len();
            match alea.suivant(4) {
                0 | 1 => {
                    let de = frontiere(&doc.texte, alea.suivant(n + 1));
                    let f = fragments[alea.suivant(fragments.len())];
                    let d = doc.editer(de, de, f);
                    p.apres_edition(&doc.entree(), &d);
                }
                2 => {
                    let de = frontiere(&doc.texte, alea.suivant(n + 1));
                    let a = frontiere(&doc.texte, de + alea.suivant(12));
                    let d = doc.editer(de, a, "");
                    p.apres_edition(&doc.entree(), &d);
                }
                _ => {
                    let a = frontiere(&doc.texte, alea.suivant(n + 1));
                    let b = frontiere(&doc.texte, a + alea.suivant(3) * alea.suivant(8));
                    doc.selection = Etendue::new(a, b);
                    p.apres_selection(&doc.entree());
                }
            }
            let attendu = doc.complet();
            assert_eq!(
                p.projection.decorations, attendu.decorations,
                "pas {pas} : {:?}",
                doc.texte
            );
            assert_eq!(p.projection, attendu, "pas {pas} : {:?}", doc.texte);
        }
    }

    /// Le premier focus (droit de révéler) recalcule tout ; un déplacement sans effet ne change rien.
    #[test]
    fn premier_focus_et_deplacement_sans_effet() {
        let mut doc = Doc::new("# T\n\npara\n\npara2");
        doc.focus = false;
        let mut p = Projeteur::default();
        let mut e = doc.entree();
        e.a_eu_le_focus = false;
        p.complet(&e);
        assert!(p.projection.fork.masques.contains(&(0..2)));
        doc.focus = true;
        doc.selection = Etendue::new(1, 1);
        assert_eq!(p.apres_selection(&doc.entree()), Changements::TOUT);
        assert_eq!(p.projection, doc.complet());
        doc.selection = Etendue::new(8, 8);
        p.apres_selection(&doc.entree());
        doc.selection = Etendue::new(7, 7);
        assert_eq!(p.apres_selection(&doc.entree()), Changements::default());
    }
}
