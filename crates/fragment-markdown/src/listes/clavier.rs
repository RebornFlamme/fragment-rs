// Port de app/src/ui/views/markdown/listKeys.ts (sortirDeTacheVide, continuerListe, retirerCase,
// listKeymap) et du comportement de @codemirror/lang-markdown (insertNewlineContinueMarkup avec
// `nonTightLists: false`, deleteMarkupBackward) et de @codemirror/commands (indentMore, indentLess)
// qu'il enchaîne.
//! Entrée, Retour arrière, Tab et Maj+Tab dans les listes, à la manière d'Obsidian.
//!
//! Écarts d'Obsidian avec le comportement de base de CodeMirror, que ce module corrige :
//!   - une liste « lâche » (items séparés par une ligne vide) : CodeMirror insère une LIGNE VIDE à
//!     chaque Entrée ; Obsidian jamais ;
//!   - Entrée sur un 2ᵉ item vide : CodeMirror le « descend » d'une ligne vide ; Obsidian sort de
//!     la liste ;
//!   - une tâche VIDE (`- [ ]`) : l'arbre n'y voit pas de tâche (il exige du texte après `]`), on
//!     sort donc de la liste au lieu de continuer par une simple puce ;
//!   - Retour arrière juste après `- [ ] ` : la case part, la puce reste.
//!
//! ★ Écart assumé : à l'intérieur d'un bloc de code clôturé, ces touches rendent `None`. CodeMirror
//!   y passe la main au langage du bloc ; le comportement par défaut de la vue (nouvelle ligne
//!   indentée) en est le plus proche.
//! ★ La sélection est unique (`EditorRange`), comme celle de `fragment-text` : le cas « plusieurs
//!   curseurs » de CodeMirror n'existe pas ici.

use std::sync::LazyLock;

use fragment_text::{
    EditorChangeSpec, EditorRange, EditorSelectionSpec, EditorTransactionSpec, utf16_len,
};
use regex::Regex;

use super::contexte::{
    Contexte, Doc, colonne, contexte_de, dans_code_cloture, est_blanc, renumeroter,
};
use crate::syntaxe::{Arbre, Cote, Genre};

fn re(motif: &str) -> Regex {
    Regex::new(motif).unwrap_or_else(|_| unreachable!("motif constant invalide : {motif}"))
}

static RE_TACHE_VIDE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*[-*+]\s+\[[ xX]\]\s*$"));
static RE_CASE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*[-*+]\s+)\[[ xX]\] ?"));
static RE_PREFIXE: LazyLock<Regex> = LazyLock::new(|| re(r"^[\s\d.)\-+*>]*"));
static RE_CITATION_VIDE: LazyLock<Regex> = LazyLock::new(|| re(r">\s*$"));

fn changement(from: usize, to: usize, insert: &str) -> EditorChangeSpec {
    EditorChangeSpec {
        from,
        to: Some(to),
        insert: insert.to_owned(),
    }
}

fn curseur(pos: usize) -> Option<EditorSelectionSpec> {
    Some(EditorSelectionSpec {
        anchor: pos,
        head: None,
    })
}

fn contient_non_blanc(s: &str) -> bool {
    s.chars().any(|c| !c.is_whitespace())
}

// ── Entrée ───────────────────────────────────────────────────────────────

/// La touche Entrée : sortir d'une tâche vide, sinon continuer la liste ou la citation.
/// `None` = la vue applique son comportement par défaut.
#[must_use]
pub fn entree(doc: &str, arbre: &Arbre, sel: EditorRange) -> Option<EditorTransactionSpec> {
    sortir_de_tache_vide(doc, sel).or_else(|| continuer_liste(doc, arbre, sel))
}

/// Entrée sur `- [ ]` / `- [ ] ` sans texte (curseur en fin de ligne) : la ligne est vidée.
#[must_use]
pub fn sortir_de_tache_vide(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    if sel.from != sel.to {
        return None;
    }
    let d = Doc::new(doc);
    let ligne = d.ligne_a(sel.from);
    if sel.from != ligne.to || !RE_TACHE_VIDE.is_match(&d.slice(ligne.from, ligne.to)) {
        return None;
    }
    Some(EditorTransactionSpec {
        changes: vec![changement(ligne.from, ligne.to, "")],
        selection: curseur(ligne.from),
        ..EditorTransactionSpec::default()
    })
}

/// Entrée qui continue la liste / la citation, SANS jamais insérer de ligne vide.
///
/// ★ Trois cas, dans l'ordre : (1) un item vide → on SORT (un niveau de marqueur de moins, le
///   marqueur du niveau parent est remis s'il s'agit d'une liste) ; (2) une seconde ligne de
///   citation vide d'affilée → les deux disparaissent ; (3) sinon on continue : même marqueur
///   (numéro + 1, case VIDE), la liste numérotée qui suit est renumérotée.
#[must_use]
pub fn continuer_liste(
    doc: &str,
    arbre: &Arbre,
    sel: EditorRange,
) -> Option<EditorTransactionSpec> {
    if sel.from != sel.to {
        return None;
    }
    let d = Doc::new(doc);
    let pos = sel.from;
    let ligne = d.ligne_a(pos);
    let noeud = arbre.noeud_a(pos, Cote::Avant);
    if dans_code_cloture(arbre, noeud) {
        return None;
    }
    let rel = pos - ligne.from;
    let mut contexte = contexte_de(arbre, &d, noeud);
    while contexte.last().is_some_and(|c| c.from > rel) {
        contexte.pop();
    }
    let interieur = contexte.last()?;
    let fin_marqueur = interieur.to.saturating_sub(interieur.espace_apres.len());
    if fin_marqueur > rel {
        return None;
    }
    let texte = d.slice(ligne.from, ligne.to);
    let reste = d.slice(ligne.from + interieur.to, ligne.to);
    let ligne_vide = rel >= fin_marqueur && !contient_non_blanc(&reste);

    // Item vide : on sort de la liste (jamais « descendu » d'une ligne vide).
    if interieur.item.is_some() && ligne_vide {
        let suivant = (contexte.len() > 1).then(|| &contexte[contexte.len() - 2]);
        let (supprimer_jusqu_a, insere) = match suivant {
            Some(n) if n.item.is_some() => (ligne.from + n.from, n.marqueur(arbre, &d, 1)),
            Some(n) => (ligne.from + n.to, String::new()),
            None => (ligne.from, String::new()),
        };
        let debut = supprimer_jusqu_a.min(pos);
        let mut changes = vec![changement(debut, pos, &insere)];
        if interieur.est_ordonnee()
            && let Some(item) = interieur.item
        {
            renumeroter(arbre, &d, item, -2, &mut changes);
        }
        if let Some(n) = suivant
            && n.est_ordonnee()
            && let Some(item) = n.item
        {
            renumeroter(arbre, &d, item, 0, &mut changes);
        }
        changes.sort_by_key(|c| c.from);
        return Some(EditorTransactionSpec {
            changes,
            selection: curseur(debut + utf16_len(&insere)),
            scroll_into_view: true,
        });
    }

    // Deux lignes de citation vides alignées d'affilée : on retire les deux.
    if interieur.genre == Genre::Blockquote && ligne_vide && ligne.from > 0 {
        let precedente = d.ligne_a(ligne.from - 1);
        let texte_prec = d.slice(precedente.from, precedente.to);
        if let Some(m) = RE_CITATION_VIDE.find(&texte_prec) {
            let index = utf16_len(&texte_prec[..m.start()]);
            if index == interieur.from {
                let premier = precedente.from + index;
                let second = ligne.from + interieur.from;
                return Some(EditorTransactionSpec {
                    changes: vec![
                        changement(premier, precedente.to, ""),
                        changement(second, ligne.to, ""),
                    ],
                    selection: curseur(second - (precedente.to - premier)),
                    scroll_into_view: true,
                });
            }
        }
    }

    // Continuer.
    let mut changes = Vec::new();
    if interieur.est_ordonnee()
        && let Some(item) = interieur.item
    {
        renumeroter(arbre, &d, item, 0, &mut changes);
    }
    let continuee = interieur
        .item
        .is_some_and(|i| arbre.noeud(i).from() < ligne.from);
    let longueur_prefixe = RE_PREFIXE.find(&texte).map_or(0, |m| utf16_len(m.as_str()));
    let mut insere = String::new();
    if !continuee || longueur_prefixe >= interieur.to {
        let dernier = contexte.len() - 1;
        for (i, c) in contexte.iter().enumerate() {
            if i == dernier && !continuee {
                insere.push_str(&c.marqueur(arbre, &d, 1));
            } else {
                let largeur = (i < dernier).then(|| {
                    let cible = colonne(&texte, contexte[i + 1].from) as isize;
                    (cible - insere.len() as isize).max(0) as usize
                });
                insere.push_str(&c.blanc(largeur, true));
            }
        }
    }
    // Les blancs qui précèdent le curseur ne restent pas en fin de la ligne coupée.
    let mut debut = pos;
    while debut > ligne.from && est_blanc(d.unite(debut - 1)) {
        debut -= 1;
    }
    changes.push(changement(debut, pos, &format!("\n{insere}")));
    changes.sort_by_key(|c| c.from);
    Some(EditorTransactionSpec {
        changes,
        selection: curseur(debut + utf16_len(&insere) + 1),
        scroll_into_view: true,
    })
}

// ── Retour arrière ───────────────────────────────────────────────────────

/// La touche Retour arrière : retirer la case d'une tâche, sinon retirer un marqueur de liste.
#[must_use]
pub fn retour_arriere(doc: &str, arbre: &Arbre, sel: EditorRange) -> Option<EditorTransactionSpec> {
    retirer_case(doc, sel).or_else(|| supprimer_marqueur_arriere(doc, arbre, sel))
}

/// Retour arrière juste après la case d'une tâche (`- [ ] |`) : retire la case, garde la puce.
#[must_use]
pub fn retirer_case(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    if sel.from != sel.to {
        return None;
    }
    let d = Doc::new(doc);
    let ligne = d.ligne_a(sel.from);
    let texte = d.slice(ligne.from, ligne.to);
    let m = RE_CASE.captures(&texte)?;
    if sel.from != ligne.from + utf16_len(&m[0]) {
        return None;
    }
    let from = ligne.from + utf16_len(&m[1]);
    Some(EditorTransactionSpec {
        changes: vec![changement(from, sel.from, "")],
        selection: curseur(from),
        ..EditorTransactionSpec::default()
    })
}

/// Retour arrière sur un marqueur de liste ou de citation (`deleteMarkupBackward`) : retire les
/// blancs en trop après le marqueur, ou le marqueur lui-même (remplacé par des blancs s'il n'est
/// pas le premier de sa liste, supprimé sinon).
#[must_use]
pub fn supprimer_marqueur_arriere(
    doc: &str,
    arbre: &Arbre,
    sel: EditorRange,
) -> Option<EditorTransactionSpec> {
    if sel.from != sel.to {
        return None;
    }
    let d = Doc::new(doc);
    let pos = sel.from;
    let ligne = d.ligne_a(pos);
    let noeud = arbre.noeud_a(pos, Cote::Avant);
    if dans_code_cloture(arbre, noeud) {
        return None;
    }
    let contexte = contexte_de(arbre, &d, noeud);
    let interieur: &Contexte = contexte.last()?;
    let rel = pos - ligne.from;
    let texte = d.slice(ligne.from, ligne.to);
    let fin_espace = interieur.to - interieur.espace_apres.len()
        + usize::from(!interieur.espace_apres.is_empty());

    // Blancs en trop après le marqueur.
    if rel > fin_espace && !contient_non_blanc(&d.slice(ligne.from + fin_espace, pos)) {
        let a = ligne.from + fin_espace;
        return Some(EditorTransactionSpec {
            changes: vec![changement(a, pos, "")],
            selection: curseur(a),
            scroll_into_view: true,
        });
    }
    let sur_la_ligne_du_marqueur = interieur
        .item
        .is_none_or(|i| ligne.from <= arbre.noeud(i).from())
        || !contient_non_blanc(&d.slice(ligne.from, ligne.from + interieur.to));
    if rel != fin_espace || !sur_la_ligne_du_marqueur {
        return None;
    }
    let debut = ligne.from + interieur.from;
    // Remplacer le marqueur d'un item qui n'ouvre pas sa liste par des blancs.
    if let Some(item) = interieur.item
        && interieur_liste_commence_avant(arbre, interieur, item)
        && contient_non_blanc(&d.slice(ligne.from + interieur.from, ligne.from + interieur.to))
    {
        let largeur = colonne(&texte, interieur.to).saturating_sub(colonne(&texte, interieur.from));
        let insere = interieur.blanc(Some(largeur), true);
        return Some(EditorTransactionSpec {
            changes: vec![changement(debut, ligne.from + interieur.to, &insere)],
            selection: curseur(debut + utf16_len(&insere)),
            scroll_into_view: true,
        });
    }
    // Retirer un niveau d'indentation.
    if debut < pos {
        return Some(EditorTransactionSpec {
            changes: vec![changement(debut, pos, "")],
            selection: curseur(debut),
            scroll_into_view: true,
        });
    }
    None
}

/// `inner.node.from < inner.item.from` : la liste (ou son parent d'item) commence AVANT cet item.
fn interieur_liste_commence_avant(arbre: &Arbre, c: &Contexte, item: usize) -> bool {
    arbre
        .noeud(item)
        .parent
        .is_some_and(|liste| arbre.noeud(liste).from() < arbre.noeud(item).from())
        && matches!(c.genre, Genre::BulletList | Genre::OrderedList)
}

// ── Tab et Maj+Tab ───────────────────────────────────────────────────────

/// Les lignes que touche la sélection (`changeBySelectedLine`) : une sélection non vide qui finit
/// au tout début d'une ligne ne compte pas cette ligne.
fn lignes_selectionnees(d: &Doc, sel: EditorRange) -> Vec<super::contexte::Ligne> {
    let mut lignes: Vec<super::contexte::Ligne> = Vec::new();
    let mut pos = sel.from;
    while pos <= sel.to {
        let l = d.ligne_a(pos);
        let nouvelle = lignes.last().is_none_or(|p| l.from > p.from);
        if nouvelle && (sel.from == sel.to || sel.to > l.from) {
            lignes.push(l);
        }
        pos = l.to + 1;
    }
    lignes
}

/// `mapPos` de CodeMirror à travers des changements croissants non chevauchants.
/// `assoc > 0` : une position collée à une insertion passe APRÈS elle.
fn reporter(pos: usize, changes: &[EditorChangeSpec], assoc: i8) -> usize {
    let mut decalage: isize = 0;
    for c in changes {
        let to = c.to.unwrap_or(c.from);
        let ins = utf16_len(&c.insert);
        if pos < c.from || (pos == c.from && assoc < 0) {
            break;
        }
        if pos > to || (pos == to && c.from < to) {
            decalage += ins as isize - (to - c.from) as isize;
        } else {
            let base = (c.from as isize + decalage) as usize;
            return base + if assoc > 0 { ins } else { 0 };
        }
    }
    (pos as isize + decalage) as usize
}

fn selection_reportee(
    sel: EditorRange,
    changes: &[EditorChangeSpec],
) -> Option<EditorSelectionSpec> {
    if sel.from == sel.to {
        return curseur(reporter(sel.from, changes, -1));
    }
    Some(EditorSelectionSpec {
        anchor: reporter(sel.from, changes, 1),
        head: Some(reporter(sel.to, changes, -1)),
    })
}

/// Tab : ajoute `unite` espaces au début de chaque ligne sélectionnée (`indentMore`). Les lignes
/// vides sont indentées aussi. Rend toujours une transaction : la touche est consommée.
#[must_use]
pub fn indenter(doc: &str, sel: EditorRange, unite: usize) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let blanc = " ".repeat(unite.max(1));
    let changes: Vec<EditorChangeSpec> = lignes_selectionnees(&d, sel)
        .iter()
        .map(|l| changement(l.from, l.from, &blanc))
        .collect();
    EditorTransactionSpec {
        selection: selection_reportee(sel, &changes),
        changes,
        ..EditorTransactionSpec::default()
    }
}

/// Maj+Tab : retire au plus `unite` colonnes d'indentation à chaque ligne sélectionnée
/// (`indentLess`). Sans indentation, la transaction est vide mais la touche reste consommée.
#[must_use]
pub fn desindenter(doc: &str, sel: EditorRange, unite: usize) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let unite = unite.max(1);
    let mut changes = Vec::new();
    for l in lignes_selectionnees(&d, sel) {
        let texte = d.slice(l.from, l.to);
        let blancs: String = texte.chars().take_while(|c| c.is_whitespace()).collect();
        if blancs.is_empty() {
            continue;
        }
        let n = utf16_len(&blancs);
        let cible = colonne(&blancs, n).saturating_sub(unite);
        let nouveau = " ".repeat(cible);
        let garde = blancs
            .chars()
            .zip(nouveau.chars())
            .take_while(|(a, b)| a == b)
            .count();
        changes.push(changement(l.from + garde, l.from + n, &nouveau[garde..]));
    }
    EditorTransactionSpec {
        selection: selection_reportee(sel, &changes),
        changes,
        ..EditorTransactionSpec::default()
    }
}
