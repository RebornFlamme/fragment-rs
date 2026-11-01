// Port de app/src/ui/views/markdown/editorTransforms.ts
//! Les transformations des commandes `editor:*` — PURES.
//!
//! Chacune prend (le document, la sélection) et rend une transaction — ou
//! `None` quand il n'y a rien à faire. Aucune ne touche un éditeur : ce module
//! n'importe que des types de `fragment-text`.
//!
//! ★ POURQUOI cette pureté n'est pas un caprice : c'est ce qui rend « Mod+B
//!   sur une sélection déjà en gras l'enlève » VÉRIFIABLE en une ligne, sans
//!   lancer d'application. Les cas limites d'une commande de formatage sont
//!   nombreux (curseur seul, sélection qui inclut les marqueurs, gras dans
//!   italique…) et c'est exactement le genre de chose qu'on ne teste jamais
//!   quand il faut une fenêtre pour le faire.
//!
//! Convention (celle d'`EditorTransactionSpec`) : les `changes` sont en offsets
//! du document D'AVANT ; la `selection` en offsets du document D'APRÈS.
//!
//! ★ Positions (ADR-004) : TOUS les offsets sont des unités de code UTF-16, comme
//!   `String.length` en TS. Le document est donc manipulé en `Vec<u16>` (`Doc`) :
//!   un émoji compte pour 2, et un `&str` n'est jamais indexé par un offset.

use fragment_text::{
    EditorChangeSpec, EditorRange, EditorSelectionSpec, EditorTransactionSpec, utf16_len,
};
use regex::Regex;
use std::sync::LazyLock;

/// Une transformation de commande : (document, sélection) → transaction ou rien.
pub type Transformation = fn(&str, EditorRange) -> Option<EditorTransactionSpec>;

fn re(motif: &str) -> Regex {
    // Les motifs sont des constantes de ce fichier, éprouvées par les tests.
    Regex::new(motif).unwrap_or_else(|_| unreachable!("motif constant invalide : {motif}"))
}

// ── Le document en unités UTF-16 ─────────────────────────────────────────

const LF: u16 = b'\n' as u16;

/// Le `\s` de JS, sur une unité de code.
fn est_blanc(u: u16) -> bool {
    matches!(u, 0x09..=0x0d | 0x20 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}

struct Doc(Vec<u16>);

impl Doc {
    fn new(s: &str) -> Self {
        Self(s.encode_utf16().collect())
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn at(&self, i: usize) -> Option<u16> {
        self.0.get(i).copied()
    }
    /// `doc.slice(a, b)`, bornes ramenées dans le document.
    fn slice(&self, a: usize, b: usize) -> String {
        let a = a.min(self.len());
        let b = b.min(self.len());
        if a >= b {
            return String::new();
        }
        String::from_utf16_lossy(&self.0[a..b])
    }
    /// `doc.lastIndexOf('\n', from)` — ★ un `from` négatif se comporte comme 0 : la
    /// recherche porte QUAND MÊME sur l'index 0 (piège de JS, voir `lignes_de`).
    fn last_lf(&self, from: isize) -> isize {
        if self.0.is_empty() {
            return -1;
        }
        let debut = from.clamp(0, self.len() as isize - 1) as usize;
        (0..=debut)
            .rev()
            .find(|&i| self.0[i] == LF)
            .map_or(-1, |i| i as isize)
    }
    /// `doc.indexOf('\n', from)`.
    fn index_lf(&self, from: usize) -> Option<usize> {
        (from..self.len()).find(|&i| self.0[i] == LF)
    }
}

fn change(from: usize, to: Option<usize>, insert: &str) -> EditorChangeSpec {
    EditorChangeSpec {
        from,
        to,
        insert: insert.to_owned(),
    }
}

fn selection(anchor: usize, head: Option<usize>) -> Option<EditorSelectionSpec> {
    Some(EditorSelectionSpec { anchor, head })
}

fn transaction(
    changes: Vec<EditorChangeSpec>,
    selection: Option<EditorSelectionSpec>,
) -> EditorTransactionSpec {
    EditorTransactionSpec {
        changes,
        selection,
        ..EditorTransactionSpec::default()
    }
}

// ── Marqueurs en ligne : gras, italique, surlignage, code ────────────────

/// Longueur de la suite de `ch` qui se TERMINE en `pos` (exclu).
fn suite_avant(doc: &Doc, pos: usize, ch: u16) -> usize {
    let mut n = 0;
    while pos > n && doc.at(pos - n - 1) == Some(ch) {
        n += 1;
    }
    n
}

/// Longueur de la suite de `ch` qui COMMENCE en `pos`.
fn suite_apres(doc: &Doc, pos: usize, ch: u16) -> usize {
    let mut n = 0;
    while pos + n < doc.len() && doc.at(pos + n) == Some(ch) {
        n += 1;
    }
    n
}

/// Bascule un marqueur symétrique (`**`, `*`, `==`, `` ` ``) autour de la sélection.
///
/// Quatre cas, dans l'ordre où on les teste :
///   1. la sélection est ENTOURÉE par le marqueur (`**|texte|**`) → on l'enlève ;
///   2. la sélection CONTIENT le marqueur (`|**texte**|`) → on l'enlève ;
///   3. sélection vide → on insère la paire, curseur au milieu ;
///   4. sinon → on enveloppe.
///
/// ★ Le cas de l'astérisque. `**gras**` n'est PAS de l'italique, et
///   `***les deux***` l'est. Pour `*`, « entouré » veut donc dire : une suite
///   d'astérisques de longueur IMPAIRE de chaque côté. Pour les autres
///   marqueurs, une suite au moins aussi longue que le marqueur suffit.
pub fn basculer_marqueur(doc: &str, sel: EditorRange, marqueur: &str) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let l = utf16_len(marqueur);
    let ch = marqueur.encode_utf16().next().unwrap_or(0);
    let (mut from, mut to) = (sel.from, sel.to);

    // Les blancs en bordure de sélection restent DEHORS : `**mot **` est cassé.
    while from < to && d.at(from).is_some_and(est_blanc) {
        from += 1;
    }
    while to > from && d.at(to - 1).is_some_and(est_blanc) {
        to -= 1;
    }

    let avant = suite_avant(&d, from, ch);
    let apres = suite_apres(&d, to, ch);
    let entoure = if marqueur == "*" {
        avant % 2 == 1 && apres % 2 == 1
    } else {
        avant >= l && apres >= l
    };

    // 1. Entouré — y compris le curseur seul posé entre `**|**`.
    if entoure {
        return transaction(
            vec![
                change(from - l, Some(from), ""),
                change(to, Some(to + l), ""),
            ],
            selection(from - l, Some(to - l)),
        );
    }

    // 2. La sélection contient ses propres marqueurs.
    if to - from >= 2 * l && d.slice(from, from + l) == marqueur && d.slice(to - l, to) == marqueur
    {
        return transaction(
            vec![
                change(from, Some(from + l), ""),
                change(to - l, Some(to), ""),
            ],
            selection(from, Some(to - 2 * l)),
        );
    }

    // 3. Curseur seul : la paire, et le curseur au milieu.
    if from == to {
        return transaction(
            vec![change(from, None, &format!("{marqueur}{marqueur}"))],
            selection(from + l, None),
        );
    }

    // 4. Envelopper, en gardant le texte sélectionné.
    transaction(
        vec![change(from, None, marqueur), change(to, None, marqueur)],
        selection(from + l, Some(to + l)),
    )
}

// ── Liens ────────────────────────────────────────────────────────────────

static RESSEMBLE_A_UNE_URL: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)^[a-z][a-z0-9+.-]*://\S+$"));

/// `[texte](url)`. Si la sélection EST une URL, elle va dans les parenthèses
/// et le curseur dans les crochets ; sinon l'inverse. Sans sélection : `[]()`,
/// curseur dans les crochets.
pub fn inserer_lien(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let (from, to) = (sel.from, sel.to);
    let texte = d.slice(from, to);

    if !texte.is_empty() && RESSEMBLE_A_UNE_URL.is_match(&texte) {
        return transaction(
            vec![change(from, Some(to), &format!("[]({texte})"))],
            selection(from + 1, None),
        );
    }
    transaction(
        vec![change(from, Some(to), &format!("[{texte}]()"))],
        // Avec un texte, le curseur va dans l'URL vide ; sans, dans les crochets.
        selection(
            if texte.is_empty() {
                from + 1
            } else {
                from + utf16_len(&texte) + 3
            },
            None,
        ),
    )
}

/// `[[texte]]`. Curseur avant les crochets fermants.
pub fn inserer_lien_interne(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let texte = d.slice(sel.from, sel.to);
    transaction(
        vec![change(sel.from, Some(sel.to), &format!("[[{texte}]]"))],
        selection(sel.from + 2 + utf16_len(&texte), None),
    )
}

static WIKILIEN: LazyLock<Regex> =
    LazyLock::new(|| re(r"\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|[^\]]*)?\]\]"));
static LIEN_MD: LazyLock<Regex> = LazyLock::new(|| re(r"\[[^\]]*\]\(([^)\s]+)\)"));

/// Le lien sous le curseur, s'il y en a un : `[[cible]]`, `[[cible|alias]]`,
/// `[[cible#titre]]`, ou `[texte](cible)`. Rend la CIBLE nue.
pub fn lien_sous_le_curseur(doc: &str, pos: usize) -> Option<String> {
    let d = Doc::new(doc);
    let debut_ligne = (d.last_lf(pos as isize - 1) + 1) as usize;
    let fin_ligne = d.index_lf(pos).unwrap_or(d.len());
    let ligne = d.slice(debut_ligne, fin_ligne);
    let col = pos as isize - debut_ligne as isize;

    // Les offsets d'un match sont en octets : on les ramène en UTF-16 pour comparer à `col`.
    let dans = |debut: usize, fin: usize| -> bool {
        let a = utf16_len(&ligne[..debut]) as isize;
        let b = utf16_len(&ligne[..fin]) as isize;
        col >= a && col <= b
    };
    for m in WIKILIEN.captures_iter(&ligne) {
        if let (Some(tout), Some(cible)) = (m.get(0), m.get(1))
            && dans(tout.start(), tout.end())
        {
            return Some(cible.as_str().trim().to_owned());
        }
    }
    for m in LIEN_MD.captures_iter(&ligne) {
        if let (Some(tout), Some(cible)) = (m.get(0), m.get(1))
            && dans(tout.start(), tout.end())
        {
            return Some(cible.as_str().to_owned());
        }
    }
    None
}

// ── Lignes ───────────────────────────────────────────────────────────────

struct Ligne {
    from: usize,
    to: usize,
    text: String,
}

impl Ligne {
    fn len(&self) -> usize {
        self.to - self.from
    }
}

/// Les lignes ENTIÈRES couvertes par la sélection.
fn lignes_de(d: &Doc, sel: EditorRange) -> Vec<Ligne> {
    // ★ sel.from === 0 à part : lastIndexOf(…, -1) cherche QUAND MÊME en 0 —
    //   sur un document qui commence par '\n', la ligne prise serait la 2e.
    let debut = if sel.from == 0 {
        0
    } else {
        (d.last_lf(sel.from as isize - 1) + 1) as usize
    };
    // Une sélection qui finit pile après un saut de ligne ne prend pas la ligne suivante.
    let fin_sel = if sel.to > sel.from && d.at(sel.to - 1) == Some(LF) {
        sel.to - 1
    } else {
        sel.to
    };
    let fin = d.index_lf(fin_sel).unwrap_or(d.len());

    let mut lignes = Vec::new();
    let mut from = debut;
    for text in d.slice(debut, fin).split('\n') {
        let len = utf16_len(text);
        lignes.push(Ligne {
            from,
            to: from + len,
            text: text.to_owned(),
        });
        from += len + 1;
    }
    lignes
}

/// Remappe un offset d'AVANT vers APRÈS, pour une liste de changements
/// disjoints et triés. Un offset à l'intérieur d'une plage remplacée est
/// ramené à la fin de l'insertion — assez pour garder une sélection cohérente.
fn remapper(changes: &[EditorChangeSpec], pos: usize) -> usize {
    let mut delta: isize = 0;
    for c in changes {
        let to = c.to.unwrap_or(c.from);
        if pos < c.from {
            break;
        }
        let ins = utf16_len(&c.insert) as isize;
        if pos <= to {
            return (c.from as isize + delta + ins) as usize;
        }
        delta += ins - (to - c.from) as isize;
    }
    (pos as isize + delta) as usize
}

/// Une transaction sur plusieurs lignes, qui GARDE la sélection en la remappant.
fn transaction_lignes(
    changes: Vec<EditorChangeSpec>,
    sel: EditorRange,
) -> Option<EditorTransactionSpec> {
    if changes.is_empty() {
        return None;
    }
    let anchor = remapper(&changes, sel.from);
    let head = remapper(&changes, sel.to);
    Some(transaction(changes, selection(anchor, Some(head))))
}

static PREFIXE_TITRE: LazyLock<Regex> = LazyLock::new(|| re(r"^#{1,6}\s+"));

/// Pose un titre de niveau `niveau` (0 = retirer) sur chaque ligne sélectionnée.
pub fn definir_titre(doc: &str, sel: EditorRange, niveau: usize) -> Option<EditorTransactionSpec> {
    let d = Doc::new(doc);
    let prefixe = if niveau > 0 {
        format!("{} ", "#".repeat(niveau))
    } else {
        String::new()
    };
    let mut changes = Vec::new();

    for ligne in lignes_de(&d, sel) {
        let actuel = PREFIXE_TITRE.find(&ligne.text).map_or("", |m| m.as_str());
        if actuel == prefixe {
            continue; // déjà bon : pas de changement vide
        }
        changes.push(change(
            ligne.from,
            Some(ligne.from + utf16_len(actuel)),
            &prefixe,
        ));
    }
    transaction_lignes(changes, sel)
}

static PUCE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*)([-*+]|[0-9]+[.)])\s+"));
static TOUTE_PUCE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*)[-*+]\s+"));
static INDENT: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*"));

/// (indentation, marqueur actuel) d'une ligne, selon l'expression de marqueur `motif`.
fn indent_et_marqueur<'a>(motif: &Regex, texte: &'a str) -> (&'a str, &'a str) {
    match motif.captures(texte) {
        Some(c) => {
            let tout = c.get(0).map_or("", |m| m.as_str());
            let indent = c.get(1).map_or("", |m| m.as_str());
            (indent, &tout[indent.len()..])
        }
        None => (INDENT.find(texte).map_or("", |m| m.as_str()), ""),
    }
}

/// Bascule la liste à puces sur les lignes sélectionnées.
///
/// Si TOUTES les lignes non vides sont déjà des puces → on les retire. Sinon
/// → on en met partout où il en manque, et une liste numérotée devient une
/// liste à puces (c'est ce que fait Obsidian : la commande dit « à puces »,
/// pas « liste »).
pub fn basculer_liste(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    let d = Doc::new(doc);
    let lignes: Vec<Ligne> = lignes_de(&d, sel)
        .into_iter()
        .filter(|l| !l.text.trim().is_empty())
        .collect();
    if lignes.is_empty() {
        return None;
    }

    let toutes_a_puces = lignes.iter().all(|l| TOUTE_PUCE.is_match(&l.text));
    let mut changes = Vec::new();

    for ligne in &lignes {
        let (indent, actuel) = indent_et_marqueur(&PUCE, &ligne.text);
        let debut = ligne.from + utf16_len(indent);
        let fin = debut + utf16_len(actuel);
        if toutes_a_puces {
            changes.push(change(debut, Some(fin), ""));
        } else if actuel != "- " {
            changes.push(change(debut, Some(fin), "- "));
        }
    }
    transaction_lignes(changes, sel)
}

/// Déplace le bloc de lignes sélectionné d'une ligne vers le haut ou le bas.
pub fn permuter_lignes(doc: &str, sel: EditorRange, haut: bool) -> Option<EditorTransactionSpec> {
    let d = Doc::new(doc);
    let bloc = lignes_de(&d, sel);
    let (premiere, derniere) = (bloc.first()?, bloc.last()?);
    let texte_bloc = d.slice(premiere.from, derniere.to);

    if haut {
        if premiere.from == 0 {
            return None;
        }
        let p = EditorRange {
            from: premiere.from - 1,
            to: premiere.from - 1,
        };
        let voisines = lignes_de(&d, p);
        let voisine = voisines.first()?;
        let decalage = voisine.len() + 1;
        return Some(transaction(
            vec![change(
                voisine.from,
                Some(derniere.to),
                &format!("{texte_bloc}\n{}", voisine.text),
            )],
            selection(
                sel.from.saturating_sub(decalage),
                Some(sel.to.saturating_sub(decalage)),
            ),
        ));
    }

    if derniere.to >= d.len() {
        return None;
    }
    let p = EditorRange {
        from: derniere.to + 1,
        to: derniere.to + 1,
    };
    let voisines = lignes_de(&d, p);
    let voisine = voisines.first()?;
    let decalage = voisine.len() + 1;
    Some(transaction(
        vec![change(
            premiere.from,
            Some(voisine.to),
            &format!("{}\n{texte_bloc}", voisine.text),
        )],
        selection(sel.from + decalage, Some(sel.to + decalage)),
    ))
}

// ── Blocs ────────────────────────────────────────────────────────────────

/// Un encadré `> [!note]`. Avec une sélection, ses lignes deviennent le corps ;
/// sans, un encadré vide avec le curseur dans le corps.
pub fn inserer_encadre(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    // ★ La ligne qui suit un `>` en serait la « continuation paresseuse » (elle
    //   entrerait dans l'encadré) : une ligne vide l'en sépare si elle a du texte.
    let fin_ligne = |pos: usize| d.index_lf(pos).unwrap_or(d.len());
    let suite_non_vide = |pos: usize| {
        let fin = fin_ligne(pos);
        fin < d.len() && !d.slice(fin + 1, fin_ligne(fin + 1)).trim().is_empty()
    };
    if sel.from == sel.to {
        let insert = "> [!note]\n> ";
        let sep = if suite_non_vide(sel.from)
            && d.slice(sel.from, fin_ligne(sel.from)).trim().is_empty()
        {
            "\n"
        } else {
            ""
        };
        return transaction(
            vec![change(sel.from, None, &format!("{insert}{sep}"))],
            selection(sel.from + utf16_len(insert), None),
        );
    }
    let lignes = lignes_de(&d, sel);
    let corps = lignes
        .iter()
        .map(|l| format!("> {}", l.text))
        .collect::<Vec<_>>()
        .join("\n");
    let (Some(premiere), Some(derniere)) = (lignes.first(), lignes.last()) else {
        return transaction(Vec::new(), None);
    };
    let insert = format!(
        "> [!note]\n{corps}{}",
        if suite_non_vide(derniere.from) {
            "\n"
        } else {
            ""
        }
    );
    transaction(
        vec![change(premiere.from, Some(derniere.to), &insert)],
        selection(premiere.from, Some(premiere.from + utf16_len(&insert))),
    )
}

/// Un tableau 2×2 VIDE (en-tête compris), sur sa propre ligne, le curseur dans
/// la première cellule. S'il termine le document, une ligne vide le suit : sans
/// elle, il n'y aurait aucun endroit où écrire après le tableau.
pub fn inserer_tableau(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let p = EditorRange {
        from: sel.from,
        to: sel.from,
    };
    let lignes = lignes_de(&d, p);
    let Some(ligne) = lignes.first() else {
        return transaction(Vec::new(), None);
    };
    let tableau = "|     |     |\n| --- | --- |\n|     |     |";

    // Ligne vide : on l'occupe. Sinon : à la ligne d'après.
    let vide = ligne.text.trim().is_empty();
    let from = if vide { ligne.from } else { ligne.to };
    let prefixe = if vide { "" } else { "\n" };
    // Une ligne vide APRÈS : en fin de document (sinon aucun endroit où écrire),
    // ou avant un texte collé (GFM en ferait une ligne du tableau).
    let suffixe = if ligne.to >= d.len() {
        "\n"
    } else {
        let q = EditorRange {
            from: ligne.to + 1,
            to: ligne.to + 1,
        };
        match lignes_de(&d, q).first() {
            Some(s) if s.text.trim().is_empty() => "",
            _ => "\n",
        }
    };

    transaction(
        vec![change(
            from,
            Some(ligne.to),
            &format!("{prefixe}{tableau}{suffixe}"),
        )],
        selection(from + utf16_len(prefixe) + 2, None),
    )
}

// ── Formatage : effacer ──────────────────────────────────────────────────

/// Les marqueurs en ligne que « Effacer la mise en forme » retire. `_` est
/// volontairement absent : il casserait les identifiants snake_case.
static MARQUEURS_EN_LIGNE: LazyLock<Regex> = LazyLock::new(|| re(r"\*\*|\*|==|~~|`|\$|%%"));

/// Retire les marqueurs en ligne de la sélection. Sans sélection : rien.
pub fn effacer_mise_en_forme(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    if sel.from == sel.to {
        return None;
    }
    let d = Doc::new(doc);
    let texte = d.slice(sel.from, sel.to);
    let propre = MARQUEURS_EN_LIGNE.replace_all(&texte, "");
    if propre == texte {
        return None;
    }
    Some(transaction(
        vec![change(sel.from, Some(sel.to), &propre)],
        selection(sel.from, Some(sel.from + utf16_len(&propre))),
    ))
}

// ── Préfixes de ligne : listes numérotées, tâches, citations ─────────────

/// Tout marqueur de liste existant (puce, numéro, case à cocher) — remplacé
/// quand on change de type de liste, comme le fait Obsidian.
static MARQUEUR_LISTE: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(\s*)(?:[-*+]\s+\[[ xX]\]|[-*+]|[0-9]+[.)])\s+"));
static DEJA_NUMEROTEE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*[0-9]+[.)]\s+"));
static DEJA_TACHE: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*[-*+]\s+\[[ xX]\]\s+"));

/// Bascule un type de liste sur les lignes sélectionnées : si TOUTES les lignes
/// non vides l'ont déjà (`deja`) → on retire leur marqueur ; sinon chaque ligne
/// reçoit `marqueur(i)` à la place de son marqueur de liste éventuel.
fn basculer_type_liste(
    doc: &str,
    sel: EditorRange,
    deja: &Regex,
    marqueur: impl Fn(usize) -> String,
) -> Option<EditorTransactionSpec> {
    let d = Doc::new(doc);
    let toutes = lignes_de(&d, sel);
    let lignes: Vec<&Ligne> = toutes
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .collect();
    if lignes.is_empty() {
        // Ligne vide : on pose le marqueur, curseur après.
        let l = toutes.first()?;
        let insert = marqueur(0);
        return Some(transaction(
            vec![change(l.from, Some(l.to), &format!("{}{insert}", l.text))],
            selection(l.to + utf16_len(&insert), None),
        ));
    }

    let tous = lignes.iter().all(|l| deja.is_match(&l.text));
    let mut changes = Vec::new();
    for (i, ligne) in lignes.iter().enumerate() {
        let (indent, actuel) = indent_et_marqueur(&MARQUEUR_LISTE, &ligne.text);
        let voulu = if tous { String::new() } else { marqueur(i) };
        if actuel == voulu {
            continue;
        }
        let debut = ligne.from + utf16_len(indent);
        changes.push(change(debut, Some(debut + utf16_len(actuel)), &voulu));
    }
    transaction_lignes(changes, sel)
}

/// `1. `, `2. `… sur les lignes sélectionnées (ou les retire).
pub fn basculer_liste_numerotee(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    basculer_type_liste(doc, sel, &DEJA_NUMEROTEE, |i| format!("{}. ", i + 1))
}

/// `- [ ] ` sur les lignes sélectionnées (ou les retire).
pub fn basculer_liste_taches(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    basculer_type_liste(doc, sel, &DEJA_TACHE, |_| "- [ ] ".to_owned())
}

static CITATION_AVEC_ESPACE: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*)>\s?"));
static CITATION: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*>"));

/// `> ` en tête des lignes sélectionnées (ou le retire si toutes l'ont).
pub fn basculer_citation(doc: &str, sel: EditorRange) -> Option<EditorTransactionSpec> {
    let d = Doc::new(doc);
    let lignes = lignes_de(&d, sel);
    let pleines: Vec<&Ligne> = lignes
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .collect();
    let toutes = !pleines.is_empty()
        && pleines
            .iter()
            .all(|l| CITATION_AVEC_ESPACE.is_match(&l.text));
    let mut changes = Vec::new();
    for ligne in &lignes {
        if toutes {
            if let Some(c) = CITATION_AVEC_ESPACE.captures(&ligne.text) {
                let tout = c.get(0).map_or("", |m| m.as_str());
                let indent = c.get(1).map_or("", |m| m.as_str());
                changes.push(change(
                    ligne.from + utf16_len(indent),
                    Some(ligne.from + utf16_len(tout)),
                    "",
                ));
            }
        } else if !CITATION.is_match(&ligne.text) {
            changes.push(change(ligne.from, None, "> "));
        }
    }
    transaction_lignes(changes, sel)
}

// ── Insertions ───────────────────────────────────────────────────────────

static NOTE_DE_BAS_DE_PAGE: LazyLock<Regex> = LazyLock::new(|| re(r"\[\^([0-9]+)\]"));

/// Une note de bas de page : `[^n]` au curseur (n = le premier numéro libre),
/// et sa définition `[^n]: ` en fin de document, où l'on place le curseur.
pub fn inserer_note_de_bas_de_page(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    let mut n: u64 = 1;
    for m in NOTE_DE_BAS_DE_PAGE.captures_iter(doc) {
        let v = m
            .get(1)
            .and_then(|g| g.as_str().parse::<u64>().ok())
            .unwrap_or(u64::MAX - 1);
        n = n.max(v.saturating_add(1));
    }
    let r = format!("[^{n}]");
    let sep = if doc.is_empty() || doc.ends_with("\n\n") {
        ""
    } else if doc.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let def = format!("{sep}[^{n}]: ");
    let len = utf16_len(doc);
    transaction(
        vec![change(sel.to, None, &r), change(len, None, &def)],
        selection(len + utf16_len(&r) + utf16_len(&def), None),
    )
}

/// `---` sur sa propre ligne, curseur sur la ligne d'après.
pub fn inserer_ligne_horizontale(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let p = EditorRange {
        from: sel.from,
        to: sel.from,
    };
    let lignes = lignes_de(&d, p);
    let Some(ligne) = lignes.first() else {
        return transaction(Vec::new(), None);
    };
    if ligne.text.trim().is_empty() {
        // ★ `---` collé sous un paragraphe en ferait un titre setext (« texte\n--- »
        //   = H2) : une ligne vide l'en sépare.
        let avant = if ligne.from > 0 {
            let debut = (d.last_lf(ligne.from as isize - 2) + 1) as usize;
            d.slice(debut, ligne.from - 1)
        } else {
            String::new()
        };
        let insert = format!("{}---\n", if avant.trim().is_empty() { "" } else { "\n" });
        return transaction(
            vec![change(ligne.from, Some(ligne.to), &insert)],
            selection(ligne.from + utf16_len(&insert), None),
        );
    }
    let insert = "\n---\n";
    transaction(
        vec![change(ligne.to, None, insert)],
        selection(ligne.to + utf16_len(insert), None),
    )
}

/// Un bloc délimité par `fence` (```` ``` ```` ou `$$`) : autour des lignes
/// sélectionnées (contenu resélectionné), ou vide avec le curseur dedans.
fn inserer_bloc(doc: &str, sel: EditorRange, fence: &str) -> EditorTransactionSpec {
    let d = Doc::new(doc);
    let f = utf16_len(fence);
    let lignes = lignes_de(&d, sel);
    if sel.from == sel.to {
        let Some(ligne) = lignes.first() else {
            return transaction(Vec::new(), None);
        };
        let vide = ligne.text.trim().is_empty();
        let avant = if vide { "" } else { "\n" };
        let insert = format!("{avant}{fence}\n\n{fence}");
        let from = if vide { ligne.from } else { ligne.to };
        return transaction(
            vec![change(from, Some(ligne.to), &insert)],
            selection(from + utf16_len(avant) + f + 1, None),
        );
    }
    let (Some(premiere), Some(derniere)) = (lignes.first(), lignes.last()) else {
        return transaction(Vec::new(), None);
    };
    let (debut, fin) = (premiere.from, derniere.to);
    let corps = d.slice(debut, fin);
    transaction(
        vec![change(
            debut,
            Some(fin),
            &format!("{fence}\n{corps}\n{fence}"),
        )],
        selection(debut + f + 1, Some(debut + f + 1 + utf16_len(&corps))),
    )
}

pub fn inserer_bloc_de_code(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    inserer_bloc(doc, sel, "```")
}

pub fn inserer_bloc_math(doc: &str, sel: EditorRange) -> EditorTransactionSpec {
    inserer_bloc(doc, sel, "$$")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ce qu'un utilisateur voit après une transaction.
    struct Applique {
        doc: String,
        sel: String,
        /// -1 quand il n'y a pas de transaction (comme le TS).
        curseur: isize,
    }

    /// Applique une transaction comme le ferait le moteur : tous les changements
    /// sont en offsets d'AVANT, donc on les applique de la fin vers le début.
    fn appliquer(doc: &str, spec: Option<EditorTransactionSpec>) -> Applique {
        let Some(spec) = spec else {
            return Applique {
                doc: doc.to_owned(),
                sel: String::new(),
                curseur: -1,
            };
        };
        let mut apres: Vec<u16> = doc.encode_utf16().collect();
        let mut changes = spec.changes.clone();
        // Tri STABLE décroissant : à égalité, l'ordre d'origine est gardé (comme `Array.sort`).
        changes.sort_by_key(|a| std::cmp::Reverse(a.from));
        for c in &changes {
            let to = c.to.unwrap_or(c.from);
            let ins: Vec<u16> = c.insert.encode_utf16().collect();
            apres.splice(c.from..to, ins);
        }
        let (anchor, head) = spec.selection.map_or((None, None), |s| {
            (Some(s.anchor), Some(s.head.unwrap_or(s.anchor)))
        });
        let sel = match (anchor, head) {
            (Some(a), Some(h)) => String::from_utf16_lossy(&apres[a.min(h)..a.max(h)]),
            _ => String::new(),
        };
        Applique {
            doc: String::from_utf16_lossy(&apres),
            sel,
            curseur: anchor.map_or(-1, |a| a as isize),
        }
    }

    /// Écrit la sélection avec des barres : « Toggle |bold| » → doc + plage.
    fn texte(avec_barres: &str) -> (String, EditorRange) {
        let u: Vec<u16> = avec_barres.encode_utf16().collect();
        let barre = u16::from(b'|');
        let from = u.iter().position(|&c| c == barre).unwrap_or(0);
        let to = u.iter().rposition(|&c| c == barre).unwrap_or(0);
        let doc = avec_barres.replace('|', "");
        (
            doc,
            EditorRange {
                from,
                to: if to == from { from } else { to - 1 },
            },
        )
    }

    fn r(from: usize, to: usize) -> EditorRange {
        EditorRange { from, to }
    }
    fn sp(s: EditorTransactionSpec) -> Option<EditorTransactionSpec> {
        Some(s)
    }

    // ── basculerMarqueur — gras ──
    #[test]
    fn basculer_marqueur_gras_enveloppe_une_selection_et_la_garde_selectionnee() {
        let (doc, sel) = texte("Toggle |bold| here");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "Toggle **bold** here");
        assert_eq!(a.sel, "bold");
    }

    #[test]
    fn basculer_marqueur_gras_enleve_les_marqueurs_quand_la_selection_est_entouree() {
        let (doc, sel) = texte("Toggle **|bold|** here");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "Toggle bold here");
        assert_eq!(a.sel, "bold");
    }

    #[test]
    fn basculer_marqueur_gras_enleve_les_marqueurs_quand_la_selection_les_contient() {
        let (doc, sel) = texte("Toggle |**bold**| here");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "Toggle bold here");
        assert_eq!(a.sel, "bold");
    }

    #[test]
    fn basculer_marqueur_gras_curseur_seul_insere_la_paire_curseur_au_milieu() {
        let (doc, sel) = texte("Toggle | here");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "Toggle **** here");
        assert_eq!(a.curseur, "Toggle **".len() as isize);
    }

    #[test]
    fn basculer_marqueur_gras_curseur_seul_entre_marqueurs_retire_la_paire_vide() {
        let (doc, sel) = texte("Toggle **|** here");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "Toggle  here");
    }

    #[test]
    fn basculer_marqueur_gras_laisse_les_blancs_de_bordure_dehors() {
        let (doc, sel) = texte("Toggle | bold | here");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "Toggle  **bold**  here");
    }

    // ── basculerMarqueur — italique, le cas de l'astérisque ──
    #[test]
    fn basculer_marqueur_italique_gras_n_est_pas_de_l_italique_on_ajoute_une_paire() {
        let (doc, sel) = texte("**|gras|**");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "*")));
        assert_eq!(a.doc, "***gras***");
    }

    #[test]
    fn basculer_marqueur_italique_retirer_l_italique_laisse_le_gras() {
        let (doc, sel) = texte("***|les deux|***");
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "*")));
        assert_eq!(a.doc, "**les deux**");
    }

    #[test]
    fn basculer_marqueur_italique_se_retire() {
        let (doc, sel) = texte("*|italique|*");
        assert_eq!(
            appliquer(&doc, sp(basculer_marqueur(&doc, sel, "*"))).doc,
            "italique"
        );
    }

    // ── basculerMarqueur — surlignage et code ──
    #[test]
    fn basculer_marqueur_surlignage_et_code() {
        let (doc, sel) = texte("un |mot| ici");
        assert_eq!(
            appliquer(&doc, sp(basculer_marqueur(&doc, sel, "=="))).doc,
            "un ==mot== ici"
        );
        assert_eq!(
            appliquer(&doc, sp(basculer_marqueur(&doc, sel, "`"))).doc,
            "un `mot` ici"
        );
    }

    // ── liens ──
    #[test]
    fn liens_inserer_lien_le_texte_va_dans_les_crochets_curseur_dans_les_parentheses() {
        let (doc, sel) = texte("voir |la doc|");
        let a = appliquer(&doc, sp(inserer_lien(&doc, sel)));
        assert_eq!(a.doc, "voir [la doc]()");
        assert_eq!(a.curseur, "voir [la doc](".len() as isize);
    }

    #[test]
    fn liens_inserer_lien_une_url_va_dans_les_parentheses_curseur_dans_les_crochets() {
        let (doc, sel) = texte("|https://example.org|");
        let a = appliquer(&doc, sp(inserer_lien(&doc, sel)));
        assert_eq!(a.doc, "[](https://example.org)");
        assert_eq!(a.curseur, 1);
    }

    #[test]
    fn liens_inserer_lien_interne() {
        let (doc, sel) = texte("voir |Note|");
        let a = appliquer(&doc, sp(inserer_lien_interne(&doc, sel)));
        assert_eq!(a.doc, "voir [[Note]]");
        assert_eq!(a.curseur, "voir [[Note".len() as isize);
    }

    #[test]
    fn liens_lien_sous_le_curseur_lit_la_cible_nue() {
        let doc = "a [[Note|alias]] b [[Autre#Titre]] c [txt](chemin.md) d";
        let pos = |s: &str| doc.find(s).unwrap_or(0);
        assert_eq!(lien_sous_le_curseur(doc, pos("alias")), Some("Note".into()));
        assert_eq!(
            lien_sous_le_curseur(doc, pos("Titre")),
            Some("Autre".into())
        );
        assert_eq!(
            lien_sous_le_curseur(doc, pos("txt")),
            Some("chemin.md".into())
        );
        assert_eq!(lien_sous_le_curseur(doc, 0), None);
    }

    // ── definirTitre ──
    #[test]
    fn definir_titre_pose_un_titre_et_le_remplace_s_il_y_en_a_un() {
        let (doc, sel) = texte("# Anci|en");
        assert_eq!(
            appliquer(&doc, definir_titre(&doc, sel, 3)).doc,
            "### Ancien"
        );
    }

    #[test]
    fn definir_titre_niveau_0_retire_le_titre() {
        let (doc, sel) = texte("## Ti|tre");
        assert_eq!(appliquer(&doc, definir_titre(&doc, sel, 0)).doc, "Titre");
    }

    #[test]
    fn definir_titre_agit_sur_chaque_ligne_de_la_selection_et_remappe_la_selection() {
        let (doc, sel) = texte("|un\ndeux|\ntrois");
        let a = appliquer(&doc, definir_titre(&doc, sel, 2));
        assert_eq!(a.doc, "## un\n## deux\ntrois");
        assert_eq!(a.sel, "un\n## deux");
    }

    #[test]
    fn definir_titre_rend_null_quand_rien_ne_change() {
        let (doc, sel) = texte("# Dé|jà");
        assert_eq!(definir_titre(&doc, sel, 1), None);
    }

    // ── basculerListe ──
    #[test]
    fn basculer_liste_ajoute_des_puces() {
        let (doc, sel) = texte("|a\nb|");
        assert_eq!(appliquer(&doc, basculer_liste(&doc, sel)).doc, "- a\n- b");
    }

    #[test]
    fn basculer_liste_retire_les_puces_quand_toutes_les_lignes_en_ont() {
        let (doc, sel) = texte("|- a\n- b|");
        assert_eq!(appliquer(&doc, basculer_liste(&doc, sel)).doc, "a\nb");
    }

    #[test]
    fn basculer_liste_complete_les_lignes_qui_manquent_sans_toucher_aux_autres() {
        let (doc, sel) = texte("|- a\nb|");
        assert_eq!(appliquer(&doc, basculer_liste(&doc, sel)).doc, "- a\n- b");
    }

    #[test]
    fn basculer_liste_convertit_une_liste_numerotee_en_puces_en_gardant_l_indentation() {
        let (doc, sel) = texte("|1. a\n  2. b|");
        assert_eq!(appliquer(&doc, basculer_liste(&doc, sel)).doc, "- a\n  - b");
    }

    // ── permuterLignes ──
    #[test]
    fn permuter_lignes_monte_la_ligne_courante_et_deplace_le_curseur_avec_elle() {
        let (doc, sel) = texte("un\nde|ux\ntrois");
        let a = appliquer(&doc, permuter_lignes(&doc, sel, true));
        assert_eq!(a.doc, "deux\nun\ntrois");
        assert_eq!(a.curseur, 2);
    }

    #[test]
    fn permuter_lignes_descend_un_bloc_de_plusieurs_lignes() {
        let (doc, sel) = texte("|un\ndeux|\ntrois\nquatre");
        let a = appliquer(&doc, permuter_lignes(&doc, sel, false));
        assert_eq!(a.doc, "trois\nun\ndeux\nquatre");
        assert_eq!(a.sel, "un\ndeux");
    }

    #[test]
    fn permuter_lignes_rend_null_aux_bords_du_document() {
        assert_eq!(permuter_lignes("a\nb", r(0, 0), true), None);
        assert_eq!(permuter_lignes("a\nb", r(2, 2), false), None);
    }

    // ── blocs ──
    #[test]
    fn blocs_inserer_encadre_enveloppe_les_lignes_selectionnees() {
        let (doc, sel) = texte("avant\n|a\nb|\naprès");
        assert_eq!(
            appliquer(&doc, sp(inserer_encadre(&doc, sel))).doc,
            "avant\n> [!note]\n> a\n> b\n\naprès"
        );
    }

    #[test]
    fn blocs_inserer_encadre_sans_selection_encadre_vide_curseur_dans_le_corps() {
        let (doc, sel) = texte("|");
        let a = appliquer(&doc, sp(inserer_encadre(&doc, sel)));
        assert_eq!(a.doc, "> [!note]\n> ");
        assert_eq!(a.curseur, a.doc.len() as isize);
    }

    #[test]
    fn blocs_inserer_encadre_sur_une_ligne_vide_suivie_de_texte_une_ligne_vide_les_separe() {
        let doc = "début\n\nfin";
        let a = appliquer(doc, sp(inserer_encadre(doc, r(6, 6))));
        assert_eq!(a.doc, "début\n> [!note]\n> \n\nfin");
        assert_eq!(
            a.curseur,
            "début\n> [!note]\n> ".encode_utf16().count() as isize
        );
    }

    #[test]
    fn blocs_inserer_tableau_un_2x2_vide_sur_la_ligne_vide_curseur_dans_la_1re_cellule() {
        let (doc, sel) = texte("|");
        let a = appliquer(&doc, sp(inserer_tableau(&doc, sel)));
        assert_eq!(a.doc, "|     |     |\n| --- | --- |\n|     |     |\n"); // + ligne de fin
        assert_eq!(a.curseur, 2);
    }

    #[test]
    fn blocs_inserer_tableau_passe_a_la_ligne_si_la_ligne_est_occupee() {
        let (doc, sel) = texte("tex|te");
        let a = appliquer(&doc, sp(inserer_tableau(&doc, sel)));
        assert!(a.doc.starts_with("texte\n|     |"));
    }

    #[test]
    fn blocs_definir_titre_en_debut_de_document_qui_commence_par_une_ligne_vide_vise_la_1re_ligne()
    {
        let doc = "\ntexte";
        assert_eq!(
            appliquer(doc, definir_titre(doc, r(0, 0), 1)).doc,
            "# \ntexte"
        );
    }

    #[test]
    fn blocs_inserer_tableau_au_milieu_du_document_pas_de_ligne_ajoutee_apres() {
        let doc = "\nsuite";
        let a = appliquer(doc, sp(inserer_tableau(doc, r(0, 0))));
        assert_eq!(
            a.doc,
            "|     |     |\n| --- | --- |\n|     |     |\n\nsuite"
        );
    }

    // ── menu contextuel — formatage ──
    #[test]
    fn menu_contextuel_formatage_barre_math_commentaire_meme_mecanique_que_le_gras() {
        let (doc, sel) = texte("un |mot| ici");
        assert_eq!(
            appliquer(&doc, sp(basculer_marqueur(&doc, sel, "~~"))).doc,
            "un ~~mot~~ ici"
        );
        assert_eq!(
            appliquer(&doc, sp(basculer_marqueur(&doc, sel, "$"))).doc,
            "un $mot$ ici"
        );
        assert_eq!(
            appliquer(&doc, sp(basculer_marqueur(&doc, sel, "%%"))).doc,
            "un %%mot%% ici"
        );
    }

    #[test]
    fn menu_contextuel_formatage_effacer_la_mise_en_forme_retire_les_marqueurs_de_la_selection() {
        let (doc, sel) = texte("|**gras** et ==surligné== `code`|");
        let a = appliquer(&doc, effacer_mise_en_forme(&doc, sel));
        assert_eq!(a.doc, "gras et surligné code");
        assert_eq!(a.sel, "gras et surligné code");
    }

    #[test]
    fn menu_contextuel_formatage_effacer_sans_selection_rien() {
        assert_eq!(effacer_mise_en_forme("**a**", r(2, 2)), None);
    }

    // ── menu contextuel — paragraphes ──
    #[test]
    fn menu_contextuel_paragraphes_liste_numerotee_numerote_remplace_une_puce_puis_retire() {
        let (doc, sel) = texte("|a\n- b|");
        let a = appliquer(&doc, basculer_liste_numerotee(&doc, sel));
        assert_eq!(a.doc, "1. a\n2. b");
        let len = a.doc.encode_utf16().count();
        let a2 = appliquer(&a.doc, basculer_liste_numerotee(&a.doc, r(0, len)));
        assert_eq!(a2.doc, "a\nb");
    }

    #[test]
    fn menu_contextuel_paragraphes_liste_de_taches() {
        let (doc, sel) = texte("|a\n1. b|");
        assert_eq!(
            appliquer(&doc, basculer_liste_taches(&doc, sel)).doc,
            "- [ ] a\n- [ ] b"
        );
    }

    #[test]
    fn menu_contextuel_paragraphes_citation_ajoute_puis_retire() {
        let (doc, sel) = texte("|a\nb|");
        let a = appliquer(&doc, basculer_citation(&doc, sel));
        assert_eq!(a.doc, "> a\n> b");
        let len = a.doc.encode_utf16().count();
        assert_eq!(
            appliquer(&a.doc, basculer_citation(&a.doc, r(0, len))).doc,
            "a\nb"
        );
    }

    // ── menu contextuel — insertions ──
    #[test]
    fn menu_contextuel_insertions_note_de_bas_de_page_reference_au_curseur_definition_en_fin() {
        let doc = "Texte ici.\nFin";
        let a = appliquer(doc, sp(inserer_note_de_bas_de_page(doc, r(5, 5))));
        assert_eq!(a.doc, "Texte[^1] ici.\nFin\n\n[^1]: ");
        assert_eq!(a.curseur, a.doc.encode_utf16().count() as isize);
    }

    #[test]
    fn menu_contextuel_insertions_note_de_bas_de_page_numero_suivant_le_dernier() {
        let doc = "a[^1] b[^3]\n\n[^1]: x\n[^3]: y\n";
        let a = appliquer(doc, sp(inserer_note_de_bas_de_page(doc, r(1, 1))));
        assert!(a.doc.contains("a[^4][^1]"));
        assert!(a.doc.ends_with("[^3]: y\n\n[^4]: "));
    }

    #[test]
    fn menu_contextuel_insertions_ligne_horizontale_sur_une_ligne_vide_ou_a_la_ligne_d_apres() {
        let h =
            |doc: &str, p: usize| appliquer(doc, sp(inserer_ligne_horizontale(doc, r(p, p)))).doc;
        assert_eq!(h("", 0), "---\n");
        assert_eq!(h("abc", 1), "abc\n---\n");
        // ligne vide collée sous un paragraphe : une ligne vide d'écart (sinon titre setext)
        assert_eq!(h("abc\n", 4), "abc\n\n---\n");
        assert_eq!(h("abc\n\n", 5), "abc\n\n---\n");
    }

    #[test]
    fn menu_contextuel_insertions_bloc_de_code_vide_curseur_dedans_ou_autour_de_la_selection() {
        let vide = appliquer("", sp(inserer_bloc_de_code("", r(0, 0))));
        assert_eq!(vide.doc, "```\n\n```");
        assert_eq!(vide.curseur, 4);
        let (doc, sel) = texte("|x = 1|");
        let a = appliquer(&doc, sp(inserer_bloc_de_code(&doc, sel)));
        assert_eq!(a.doc, "```\nx = 1\n```");
        assert_eq!(a.sel, "x = 1");
    }

    #[test]
    fn menu_contextuel_insertions_bloc_math() {
        assert_eq!(
            appliquer("", sp(inserer_bloc_math("", r(0, 0)))).doc,
            "$$\n\n$$"
        );
    }

    // ── Ajouts du port (absents du TS) : positions UTF-16 ──
    #[test]
    fn port_basculer_marqueur_apres_un_emoji_les_offsets_sont_en_utf16() {
        // « 😀 » = 2 unités UTF-16 : la sélection de « mot » commence à 3.
        let (doc, sel) = texte("😀 |mot|");
        assert_eq!(sel, r(3, 6));
        let a = appliquer(&doc, sp(basculer_marqueur(&doc, sel, "**")));
        assert_eq!(a.doc, "😀 **mot**");
        assert_eq!(a.sel, "mot");
    }

    #[test]
    fn port_definir_titre_remappe_la_selection_apres_un_emoji() {
        let doc = "😀 a\nb";
        let a = appliquer(doc, definir_titre(doc, r(0, 6), 1));
        assert_eq!(a.doc, "# 😀 a\n# b");
        assert_eq!(a.sel, "😀 a\n# b");
    }

    #[test]
    fn port_lien_sous_le_curseur_apres_un_emoji() {
        let doc = "😀 [[Note]]";
        assert_eq!(lien_sous_le_curseur(doc, 6), Some("Note".into()));
    }
}
