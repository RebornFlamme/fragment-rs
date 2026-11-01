// Port de app/src/ui/views/markdown/table/tableModel.ts
//! Le modèle d'un tableau GFM — la partie PURE du composant tableau.
//!
//! Le texte source est la vérité (files over app) : le widget ne garde qu'un
//! modèle DÉRIVÉ, le modifie par ces opérations, puis le RE-SÉRIALISE dans la
//! note. Aucun DOM ici : tout se teste en unitaire.
//!
//! Indexation « grille » : la ligne 0 est l'EN-TÊTE, les lignes 1… sont le
//! corps. La ligne de délimiteurs (`| --- |`) n'est pas une ligne de la grille :
//! elle ne porte que les alignements.
//!
//! ★ Positions (ADR-004) : `cell_at_offset` prend et rend des index en unités de
//!   code UTF-16 ; la largeur de colonne du `padEnd` TS est elle aussi en
//!   UTF-16 (`utf16_len`), pas en caractères.

use fragment_text::utf16_len;
use std::cmp::Ordering;
use unicode_normalization::UnicodeNormalization;

/// L'alignement d'une colonne (`None` = pas d'alignement, `---`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Un tableau : en-tête, alignements (un par colonne) et corps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableModel {
    pub header: Vec<String>,
    pub aligns: Vec<Option<Align>>,
    pub rows: Vec<Vec<String>>,
}

/// Le sens d'un tri.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

/// La cellule sous un offset du source (voir [`cell_at_offset`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellAt {
    pub row: usize,
    pub col: usize,
    /// Offset DANS le texte de la cellule (blancs de tête retirés), en UTF-16.
    pub in_cell: usize,
}

/// Découpe une ligne `| a | b\|c |` en cellules (`\|` dé-échappé).
pub fn split_row(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(r) = s.strip_prefix('|') {
        s = r;
    }
    // Un `|` final n'est un séparateur que s'il n'est pas échappé.
    if s.ends_with('|')
        && !s.ends_with("\\|")
        && let Some(r) = s.strip_suffix('|')
    {
        s = r;
    }

    let mut cells = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && chars.get(i + 1) == Some(&'|') {
            cur.push('|');
            i += 2;
            continue;
        }
        if ch == '|' {
            cells.push(cur.trim().to_owned());
            cur.clear();
            i += 1;
            continue;
        }
        cur.push(ch);
        i += 1;
    }
    cells.push(cur.trim().to_owned());
    cells
}

fn align_of(delim: &str) -> Option<Align> {
    let d = delim.trim();
    let left = d.starts_with(':');
    let right = d.ends_with(':');
    match (left, right) {
        (true, true) => Some(Align::Center),
        (false, true) => Some(Align::Right),
        (true, false) => Some(Align::Left),
        _ => None,
    }
}

/// Complète (ou tronque) une ligne à `n` cellules.
fn fit<T: Clone>(cells: Vec<T>, n: usize, fill: T) -> Vec<T> {
    let mut out: Vec<T> = cells.into_iter().take(n).collect();
    while out.len() < n {
        out.push(fill.clone());
    }
    out
}

/// Lit un tableau GFM (en-tête, ligne de délimiteurs, corps).
pub fn parse_table(src: &str) -> TableModel {
    let lines: Vec<&str> = src.split('\n').filter(|l| !l.trim().is_empty()).collect();
    let header = split_row(lines.first().copied().unwrap_or(""));
    let n = header.len().max(1);
    let aligns = fit(
        split_row(lines.get(1).copied().unwrap_or(""))
            .iter()
            .map(|d| align_of(d))
            .collect(),
        n,
        None,
    );
    let rows = lines
        .iter()
        .skip(2)
        .map(|l| fit(split_row(l), n, String::new()))
        .collect();
    TableModel {
        header: fit(header, n, String::new()),
        aligns,
        rows,
    }
}

fn escape_cell(text: &str) -> String {
    text.replace("\r\n", " ")
        .replace('\n', " ")
        .replace('|', "\\|")
}

fn delimiter(align: Option<Align>, width: usize) -> String {
    match align {
        Some(Align::Left) => format!(":{}", "-".repeat(width - 1)),
        Some(Align::Right) => format!("{}:", "-".repeat(width - 1)),
        Some(Align::Center) => format!(":{}:", "-".repeat(width - 2)),
        None => "-".repeat(width),
    }
}

/// `padEnd` de JS : complète avec des espaces jusqu'à `width` unités UTF-16.
fn pad_end(s: &str, width: usize) -> String {
    let mut out = s.to_owned();
    for _ in utf16_len(s)..width {
        out.push(' ');
    }
    out
}

/// `| a   | b   |` — colonnes alignées par du padding, pour que la SOURCE reste
/// lisible dans n'importe quel autre éditeur (le rendu, lui, n'en a pas besoin).
pub fn serialize_table(model: &TableModel) -> String {
    let grid: Vec<Vec<String>> = std::iter::once(&model.header)
        .chain(model.rows.iter())
        .map(|r| r.iter().map(|c| escape_cell(c)).collect())
        .collect();
    let n = model.header.len();
    // ★ Largeur minimale 3 : `---` est le plus court délimiteur GFM valide (et `:-:` tient dedans).
    let widths: Vec<usize> = (0..n)
        .map(|c| {
            grid.iter()
                .map(|r| r.get(c).map_or(0, |t| utf16_len(t)))
                .fold(3, usize::max)
        })
        .collect();
    let line = |cells: &[String]| -> String {
        let padded: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(c, t)| pad_end(t, widths.get(c).copied().unwrap_or(0)))
            .collect();
        format!("| {} |", padded.join(" | "))
    };
    let delims: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(c, w)| delimiter(model.aligns.get(c).copied().flatten(), *w))
        .collect();
    let mut out = vec![line(&grid[0]), format!("| {} |", delims.join(" | "))];
    out.extend(grid.iter().skip(1).map(|r| line(r)));
    out.join("\n")
}

// ── Opérations (toutes rendent un NOUVEAU modèle) ──────────────────────────

fn grille_de(m: &TableModel) -> Vec<Vec<String>> {
    std::iter::once(m.header.clone())
        .chain(m.rows.iter().cloned())
        .collect()
}

fn depuis_grille(mut g: Vec<Vec<String>>, aligns: Vec<Option<Align>>) -> TableModel {
    let header = if g.is_empty() {
        Vec::new()
    } else {
        g.remove(0)
    };
    TableModel {
        header,
        aligns,
        rows: g,
    }
}

/// Nombre de lignes de la grille (en-tête comprise).
pub fn row_count(m: &TableModel) -> usize {
    1 + m.rows.len()
}

/// Nombre de colonnes.
pub fn col_count(m: &TableModel) -> usize {
    m.header.len()
}

/// Le texte de la cellule `(r, c)` de la grille ; vide hors du tableau.
pub fn get_cell(m: &TableModel, r: usize, c: usize) -> String {
    grille_de(m)
        .get(r)
        .and_then(|row| row.get(c))
        .cloned()
        .unwrap_or_default()
}

/// Remplace le texte de la cellule `(r, c)` (ligne 0 = en-tête).
pub fn set_cell(m: &TableModel, r: usize, c: usize, text: &str) -> TableModel {
    let g = grille_de(m)
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == r {
                row.into_iter()
                    .enumerate()
                    .map(|(j, t)| if j == c { text.to_owned() } else { t })
                    .collect()
            } else {
                row
            }
        })
        .collect();
    depuis_grille(g, m.aligns.clone())
}

/// Insère une ligne vide à l'index de grille `at` (≥ 1 : jamais avant l'en-tête).
/// `None` = à la fin.
pub fn add_row(m: &TableModel, at: Option<usize>) -> TableModel {
    let mut g = grille_de(m);
    let i = at.unwrap_or(row_count(m)).max(1).min(g.len());
    g.insert(i, m.header.iter().map(|_| String::new()).collect());
    depuis_grille(g, m.aligns.clone())
}

/// Insère une colonne vide à l'index `at` (`None` = à droite).
pub fn add_column(m: &TableModel, at: Option<usize>) -> TableModel {
    let i = at.unwrap_or(col_count(m)).min(col_count(m));
    let g = grille_de(m)
        .into_iter()
        .map(|mut row| {
            row.insert(i.min(row.len()), String::new());
            row
        })
        .collect();
    let mut aligns = m.aligns.clone();
    aligns.insert(i.min(aligns.len()), None);
    depuis_grille(g, aligns)
}

fn move_item<T>(list: &mut Vec<T>, from: usize, to: usize) {
    let x = list.remove(from);
    list.insert(to.min(list.len()), x);
}

/// Déplace la ligne de grille `from` à l'index `to` (l'en-tête peut bouger : son contenu suit).
pub fn move_row(m: &TableModel, from: usize, to: usize) -> TableModel {
    let n = row_count(m);
    if from == to || from >= n || to >= n {
        return m.clone();
    }
    let mut g = grille_de(m);
    move_item(&mut g, from, to);
    depuis_grille(g, m.aligns.clone())
}

/// Déplace la colonne `from` à l'index `to` (son alignement la suit).
pub fn move_column(m: &TableModel, from: usize, to: usize) -> TableModel {
    let n = col_count(m);
    if from == to || from >= n || to >= n {
        return m.clone();
    }
    let g = grille_de(m)
        .into_iter()
        .map(|mut row| {
            move_item(&mut row, from, to);
            row
        })
        .collect();
    let mut aligns = m.aligns.clone();
    move_item(&mut aligns, from, to);
    depuis_grille(g, aligns)
}

/// Le rang d'un caractère dans l'ordre d'ICU : ponctuation < chiffres < lettres.
fn rang(c: char) -> u8 {
    if c.is_alphabetic() {
        2
    } else if c.is_numeric() {
        1
    } else {
        0
    }
}

/// `sensitivity: 'base'` : ni casse ni accents ne comptent.
fn plier(s: &str) -> Vec<char> {
    s.nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Compare deux entiers décimaux écrits en chiffres (sans signe, taille quelconque).
fn comparer_nombres(a: &[char], b: &[char]) -> Ordering {
    let a: Vec<char> = a.iter().copied().skip_while(|c| *c == '0').collect();
    let b: Vec<char> = b.iter().copied().skip_while(|c| *c == '0').collect();
    a.len().cmp(&b.len()).then_with(|| a.cmp(&b))
}

/// ★ `a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' })` :
///   les suites de chiffres se comparent comme des nombres (`2` avant `10`),
///   casse et accents ignorés. Approximation de l'ordre d'ICU : ponctuation,
///   puis chiffres, puis lettres, chacune dans l'ordre des points de code.
fn locale_compare_numeric(a: &str, b: &str) -> Ordering {
    let a = plier(a);
    let b = plier(b);
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let si = i;
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let o = comparer_nombres(&a[si..i], &b[sj..j]);
            if o != Ordering::Equal {
                return o;
            }
            continue;
        }
        let o = rang(a[i]).cmp(&rang(b[j])).then_with(|| a[i].cmp(&b[j]));
        if o != Ordering::Equal {
            return o;
        }
        i += 1;
        j += 1;
    }
    (a.len() - i).cmp(&(b.len() - j))
}

/// Trie le CORPS sur la colonne `c` (l'en-tête ne bouge pas). Comparaison
/// numérique (`2` avant `10`) ; les cellules vides vont toujours en dernier.
pub fn sort_by_column(m: &TableModel, c: usize, dir: SortDir) -> TableModel {
    let signe = if dir == SortDir::Asc { 1 } else { -1 };
    let mut rows = m.rows.clone();
    // `sort_by` de Rust est stable, comme `Array.prototype.sort` de JS.
    rows.sort_by(|a, b| {
        let x = a.get(c).map_or("", String::as_str);
        let y = b.get(c).map_or("", String::as_str);
        if x.is_empty() || y.is_empty() {
            return match (x.is_empty(), y.is_empty()) {
                (false, true) => Ordering::Less,
                (true, false) => Ordering::Greater,
                _ => Ordering::Equal,
            };
        }
        let o = locale_compare_numeric(x, y);
        if signe == 1 { o } else { o.reverse() }
    });
    TableModel { rows, ..m.clone() }
}

/// Supprime la ligne de grille `r` — jamais l'en-tête (r ≥ 1).
pub fn delete_row(m: &TableModel, r: usize) -> TableModel {
    if r < 1 || r >= row_count(m) {
        return m.clone();
    }
    let mut out = m.clone();
    out.rows.remove(r - 1);
    out
}

/// Supprime la colonne `c` — jamais la dernière (un tableau a au moins une colonne).
pub fn delete_column(m: &TableModel, c: usize) -> TableModel {
    if col_count(m) <= 1 || c >= col_count(m) {
        return m.clone();
    }
    fn sans<T: Clone>(row: &[T], c: usize) -> Vec<T> {
        row.iter()
            .enumerate()
            .filter(|(j, _)| *j != c)
            .map(|(_, x)| x.clone())
            .collect()
    }
    TableModel {
        header: sans(&m.header, c),
        aligns: sans(&m.aligns, c),
        rows: m.rows.iter().map(|r| sans(r, c)).collect(),
    }
}

/// Copie la ligne de grille `r` juste en dessous — pas l'en-tête (r ≥ 1).
pub fn duplicate_row(m: &TableModel, r: usize) -> TableModel {
    if r < 1 || r >= row_count(m) {
        return m.clone();
    }
    let mut out = m.clone();
    out.rows.insert(r, m.rows[r - 1].clone());
    out
}

/// Copie la colonne `c` (contenu + alignement) juste à sa droite.
pub fn duplicate_column(m: &TableModel, c: usize) -> TableModel {
    if c >= col_count(m) {
        return m.clone();
    }
    fn dup<T: Clone>(row: &[T], c: usize) -> Vec<T> {
        let mut r = row.to_vec();
        if let Some(x) = row.get(c) {
            r.insert(c + 1, x.clone());
        }
        r
    }
    TableModel {
        header: dup(&m.header, c),
        aligns: dup(&m.aligns, c),
        rows: m.rows.iter().map(|r| dup(r, c)).collect(),
    }
}

/// L'alignement de la colonne `c` (écrit dans la ligne de délimiteurs).
pub fn set_align(m: &TableModel, c: usize, align: Option<Align>) -> TableModel {
    if c >= col_count(m) {
        return m.clone();
    }
    let mut out = m.clone();
    if let Some(a) = out.aligns.get_mut(c) {
        *a = align;
    }
    out
}

/// Même nombre de lignes et de colonnes ? (le widget peut alors se mettre à jour EN PLACE).
pub fn same_shape(a: &TableModel, b: &TableModel) -> bool {
    row_count(a) == row_count(b) && col_count(a) == col_count(b)
}

/// La cellule sous un offset du SOURCE du tableau (+ l'offset dans la cellule).
/// Ligne 0 = en-tête ; la ligne de délimiteurs se rabat sur l'en-tête.
///
/// `offset` et `in_cell` sont en unités de code UTF-16 (ADR-004).
pub fn cell_at_offset(src: &str, offset: usize) -> CellAt {
    let lines: Vec<Vec<u16>> = src
        .split('\n')
        .map(|l| l.encode_utf16().collect())
        .collect();
    let mut pos = 0usize;
    let mut li = 0usize;
    while li + 1 < lines.len() && offset > pos + lines[li].len() {
        pos += lines[li].len() + 1;
        li += 1;
    }
    let empty: Vec<u16> = Vec::new();
    let line = lines.get(li).unwrap_or(&empty);
    let local = offset.saturating_sub(pos);
    // Compte les `|` NON échappés avant l'offset → l'index de colonne.
    let pipe = u16::from(b'|');
    let bslash = u16::from(b'\\');
    let mut pipes = 0usize;
    let mut cell_start = 0usize;
    let mut i = 0;
    let fin = local.min(line.len());
    while i < fin {
        if line[i] == bslash && line.get(i + 1) == Some(&pipe) {
            i += 2;
            continue;
        }
        if line[i] == pipe {
            pipes += 1;
            cell_start = i + 1;
        }
        i += 1;
    }
    let texte = String::from_utf16_lossy(line);
    let lead = usize::from(texte.trim_start().starts_with('|'));
    let col = pipes.saturating_sub(lead);
    let row = li.saturating_sub(1);
    // Offset DANS le texte de la cellule (blancs de tête retirés).
    let fin_brut = local.min(line.len()).max(cell_start);
    let brut = String::from_utf16_lossy(&line[cell_start.min(line.len())..fin_brut]);
    let in_cell = utf16_len(brut.trim_start());
    CellAt { row, col, in_cell }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "| A | B | C |\n| --- | :-: | --: |\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |";

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }
    fn vv(a: &[&[&str]]) -> Vec<Vec<String>> {
        a.iter().map(|r| v(r)).collect()
    }

    // ── splitRow ──
    #[test]
    fn split_row_decoupe_et_de_echappe_les_pipes() {
        assert_eq!(split_row("| a | b\\|c |"), v(&["a", "b|c"]));
    }
    #[test]
    fn split_row_tolere_l_absence_de_pipes_de_bord() {
        assert_eq!(split_row("a | b"), v(&["a", "b"]));
    }

    // ── parseTable / serializeTable ──
    #[test]
    fn parse_table_lit_en_tete_alignements_et_corps() {
        let m = parse_table(SRC);
        assert_eq!(m.header, v(&["A", "B", "C"]));
        assert_eq!(
            m.aligns,
            vec![None, Some(Align::Center), Some(Align::Right)]
        );
        assert_eq!(m.rows, vv(&[&["1", "2", "3"], &["4", "5", "6"]]));
    }

    #[test]
    fn parse_table_complete_les_lignes_trop_courtes() {
        let m = parse_table("| a | b |\n| - | - |\n| x |");
        assert_eq!(m.rows, vv(&[&["x", ""]]));
    }

    #[test]
    fn parse_table_serialize_table_aller_retour_stable_colonnes_alignees_alignements_gardes() {
        let out = serialize_table(&parse_table(SRC));
        assert_eq!(
            out,
            "| A   | B   | C   |\n\
             | --- | :-: | --: |\n\
             | 1   | 2   | 3   |\n\
             | 4   | 5   | 6   |"
        );
        assert_eq!(serialize_table(&parse_table(&out)), out);
    }

    #[test]
    fn serialize_table_re_echappe_les_pipes_et_aplatit_les_retours_a_la_ligne() {
        let m = set_cell(&parse_table(SRC), 1, 0, "x|y\nz");
        assert!(
            serialize_table(&m)
                .split('\n')
                .nth(2)
                .is_some_and(|l| l.contains("x\\|y z"))
        );
    }

    // ── opérations ──
    #[test]
    fn operations_set_cell_ligne_0_en_tete() {
        let m = parse_table(SRC);
        assert_eq!(get_cell(&set_cell(&m, 0, 1, "Bé"), 0, 1), "Bé");
        assert_eq!(get_cell(&set_cell(&m, 2, 2, "9"), 2, 2), "9");
    }

    #[test]
    fn operations_add_row_add_column_a_la_fin_par_defaut() {
        let m = parse_table(SRC);
        assert_eq!(add_row(&m, None).rows.len(), 3);
        let c = add_column(&m, None);
        assert_eq!(c.header, v(&["A", "B", "C", ""]));
        assert_eq!(
            c.aligns,
            vec![None, Some(Align::Center), Some(Align::Right), None]
        );
    }

    #[test]
    fn operations_add_row_ne_passe_jamais_avant_l_en_tete() {
        let m = parse_table(SRC);
        assert_eq!(add_row(&m, Some(0)).header, v(&["A", "B", "C"]));
    }

    #[test]
    fn operations_move_column_emporte_contenu_et_alignement() {
        let m = parse_table(SRC);
        let r = move_column(&m, 2, 0);
        assert_eq!(r.header, v(&["C", "A", "B"]));
        assert_eq!(
            r.aligns,
            vec![Some(Align::Right), None, Some(Align::Center)]
        );
        assert_eq!(r.rows[0], v(&["3", "1", "2"]));
    }

    #[test]
    fn operations_move_row_sur_la_grille_en_tete_compris() {
        let m = parse_table(SRC);
        let r = move_row(&m, 2, 1);
        assert_eq!(r.rows, vv(&[&["4", "5", "6"], &["1", "2", "3"]]));
        assert_eq!(move_row(&m, 1, 0).header, v(&["1", "2", "3"]));
    }

    #[test]
    fn operations_same_shape() {
        let m = parse_table(SRC);
        assert!(same_shape(&m, &set_cell(&m, 1, 1, "z")));
        assert!(!same_shape(&m, &add_row(&m, None)));
    }

    // ── cellAtOffset ──
    const SRC2: &str =
        "| Colonne 1 | Colonne 2 |\n| --------- | --------- |\n| Cellule 1 | Cellule 2 |";

    #[test]
    fn cell_at_offset_en_tete_premiere_cellule() {
        assert_eq!(
            cell_at_offset(SRC2, 2),
            CellAt {
                row: 0,
                col: 0,
                in_cell: 0
            }
        );
    }
    #[test]
    fn cell_at_offset_corps_deuxieme_colonne() {
        let off = SRC2.find("Cellule 2").unwrap_or(0) + 3;
        assert_eq!(
            cell_at_offset(SRC2, off),
            CellAt {
                row: 1,
                col: 1,
                in_cell: 3
            }
        );
    }

    // ── opérations des menus ──
    fn menus() -> TableModel {
        parse_table("| N | X |\n| --- | --- |\n| 10 | a |\n| 2 | b |\n|  | c |")
    }

    #[test]
    fn operations_des_menus_tri_numerique_en_tete_fixe_vides_en_dernier() {
        let m = menus();
        let col0 = |t: &TableModel| t.rows.iter().map(|r| r[0].clone()).collect::<Vec<_>>();
        assert_eq!(
            col0(&sort_by_column(&m, 0, SortDir::Asc)),
            v(&["2", "10", ""])
        );
        assert_eq!(
            col0(&sort_by_column(&m, 0, SortDir::Desc)),
            v(&["10", "2", ""])
        );
        assert_eq!(sort_by_column(&m, 0, SortDir::Asc).header, v(&["N", "X"]));
    }

    #[test]
    fn operations_des_menus_supprimer_jamais_l_en_tete_jamais_la_derniere_colonne() {
        let m = menus();
        assert_eq!(
            delete_row(&m, 1)
                .rows
                .iter()
                .map(|r| r[1].clone())
                .collect::<Vec<_>>(),
            v(&["b", "c"])
        );
        assert_eq!(delete_row(&m, 0), m);
        let une = delete_column(&m, 0);
        assert_eq!(une.header, v(&["X"]));
        assert_eq!(delete_column(&une, 0), une);
    }

    #[test]
    fn operations_des_menus_dupliquer_une_ligne_une_colonne_la_copie_juste_apres() {
        let m = menus();
        assert_eq!(
            duplicate_row(&m, 2)
                .rows
                .iter()
                .map(|r| r[1].clone())
                .collect::<Vec<_>>(),
            v(&["a", "b", "b", "c"])
        );
        let d = duplicate_column(&set_align(&m, 0, Some(Align::Right)), 0);
        assert_eq!(d.header, v(&["N", "N", "X"]));
        assert_eq!(d.aligns, vec![Some(Align::Right), Some(Align::Right), None]);
    }

    #[test]
    fn operations_des_menus_aligner_ecrit_le_delimiteur() {
        let m = menus();
        let s = serialize_table(&set_align(&m, 1, Some(Align::Center)));
        assert_eq!(s.split('\n').nth(1), Some("| --- | :-: |"));
    }

    // ── Ajouts du port (absents du TS) : UTF-16 et tri accentué ──
    #[test]
    fn port_serialize_table_pad_end_compte_en_unites_utf16() {
        // « 😀 » vaut 2 unités UTF-16 : le `padEnd` TS ajoute 1 espace pour atteindre 3.
        let m = parse_table("| a |\n| - |\n| 😀 |");
        assert_eq!(serialize_table(&m), "| a   |\n| --- |\n| 😀  |");
    }

    #[test]
    fn port_cell_at_offset_apres_un_emoji_en_utf16() {
        let src = "| 😀 | b |\n| - | - |";
        // `| 😀 | ` = 1+1+2+1+1+1 = 7 unités UTF-16 avant `b`.
        assert_eq!(
            cell_at_offset(src, 7),
            CellAt {
                row: 0,
                col: 1,
                in_cell: 0
            }
        );
    }

    #[test]
    fn port_tri_ignore_casse_et_accents() {
        let m = parse_table("| N |\n| - |\n| é |\n| a |\n| B |");
        let col: Vec<String> = sort_by_column(&m, 0, SortDir::Asc)
            .rows
            .iter()
            .map(|r| r[0].clone())
            .collect();
        assert_eq!(col, v(&["a", "B", "é"]));
    }
}
