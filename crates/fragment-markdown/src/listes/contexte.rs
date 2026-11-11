// Port de app/src/ui/views/markdown/listKeys.ts (le contexte de marqueurs de @codemirror/lang-markdown
// dont il dépend : getContext, Context, renumberList, countColumn).
//! Le « contexte de marqueurs » d'une position : la pile des listes et citations qui l'enveloppent,
//! avec, pour chacune, où commence et finit son marqueur sur sa ligne.
//!
//! ★ Les offsets d'un contexte (`from`, `to`) sont RELATIFS au début de la ligne où le nœud commence,
//!   pas au document : une puce imbriquée est sur sa propre ligne, son parent sur une ligne plus haut.

use std::sync::LazyLock;

use fragment_text::{EditorChangeSpec, utf16_len};
use regex::Regex;

use crate::syntaxe::{Arbre, Genre};

/// Largeur d'une tabulation pour le calcul des colonnes (celle de CodeMirror par défaut).
pub(super) const TAB: usize = 4;

fn re(motif: &str) -> Regex {
    Regex::new(motif).unwrap_or_else(|_| unreachable!("motif constant invalide : {motif}"))
}

static RE_CITATION: LazyLock<Regex> = LazyLock::new(|| re(r"^ *>( ?)"));
static RE_NUMEROTEE: LazyLock<Regex> = LazyLock::new(|| re(r"^( *)\d+([.)])( *)"));
static RE_PUCE: LazyLock<Regex> = LazyLock::new(|| re(r"^( *)([-+*])( {1,4}\[[ xX]\])?( +)"));
static RE_NUMERO: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*)(\d+)[.)]"));

// ── Le document en unités UTF-16, ligne par ligne ────────────────────────

/// Une ligne : `[from, to)` en UTF-16, sans le saut de ligne.
#[derive(Debug, Clone, Copy)]
pub(super) struct Ligne {
    pub from: usize,
    pub to: usize,
}

pub(super) struct Doc(Vec<u16>);

impl Doc {
    pub fn new(s: &str) -> Self {
        Self(s.encode_utf16().collect())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `doc.sliceString(a, b)`, bornes ramenées dans le document.
    pub fn slice(&self, a: usize, b: usize) -> String {
        let a = a.min(self.len());
        let b = b.min(self.len());
        if a >= b {
            return String::new();
        }
        String::from_utf16_lossy(&self.0[a..b])
    }

    /// L'unité UTF-16 en `i` (0 hors du document).
    pub fn unite(&self, i: usize) -> u16 {
        self.0.get(i).copied().unwrap_or(0)
    }

    /// `doc.lineAt(pos)`.
    pub fn ligne_a(&self, pos: usize) -> Ligne {
        let pos = pos.min(self.len());
        let from = self.0[..pos]
            .iter()
            .rposition(|&u| u == 10)
            .map_or(0, |i| i + 1);
        let to = self.0[pos..]
            .iter()
            .position(|&u| u == 10)
            .map_or(self.len(), |i| pos + i);
        Ligne { from, to }
    }
}

/// `countColumn(texte, TAB, jusqu_a)` : la colonne atteinte après `jusqu_a` unités UTF-16, une
/// tabulation avançant jusqu'au multiple de `TAB` suivant.
pub(super) fn colonne(texte: &str, jusqu_a: usize) -> usize {
    let mut n = 0;
    for u in texte.encode_utf16().take(jusqu_a) {
        if u == 9 {
            n += TAB - n % TAB;
        } else {
            n += 1;
        }
    }
    n
}

/// Le `\s` de JS sur une unité de code UTF-16 (assez large pour le texte de lignes).
pub(super) fn est_blanc(u: u16) -> bool {
    char::from_u32(u32::from(u)).is_some_and(char::is_whitespace) || u == 0xfeff
}

// ── Le contexte ──────────────────────────────────────────────────────────

pub(super) struct Contexte {
    /// Le genre du nœud englobant : `BulletList`, `OrderedList` ou `Blockquote`.
    pub genre: Genre,
    pub from: usize,
    pub to: usize,
    pub espace_avant: String,
    pub espace_apres: String,
    /// `-`, `-` suivi de ` [ ]` (une tâche reprend toujours une case VIDE), `1.`-style (`.`/`)`), `>`.
    pub type_: String,
    pub item: Option<usize>,
}

impl Contexte {
    pub fn est_ordonnee(&self) -> bool {
        self.genre == Genre::OrderedList
    }

    /// Le marqueur de la ligne suivante : numéro + `add` pour une numérotée.
    pub fn marqueur(&self, arbre: &Arbre, doc: &Doc, add: i64) -> String {
        let numero = if self.est_ordonnee() {
            self.item
                .and_then(|i| numero_item(arbre, doc, i))
                .map_or_else(String::new, |(_, n, _)| (n + add).to_string())
        } else {
            String::new()
        };
        format!(
            "{}{}{}{}",
            self.espace_avant, numero, self.type_, self.espace_apres
        )
    }

    /// Une ligne de continuation : de quoi aligner le texte sous celui du marqueur.
    pub fn blanc(&self, largeur_max: Option<usize>, final_: bool) -> String {
        let mut r = self.espace_avant.clone();
        if self.genre == Genre::Blockquote {
            r.push('>');
        }
        match largeur_max {
            Some(l) => {
                while r.len() < l {
                    r.push(' ');
                }
                r
            }
            None => {
                let n = (self.to as isize)
                    - (self.from as isize)
                    - r.len() as isize
                    - self.espace_apres.len() as isize;
                for _ in 0..n.max(0) {
                    r.push(' ');
                }
                if final_ {
                    r.push_str(&self.espace_apres);
                }
                r
            }
        }
    }
}

/// Le numéro d'un item numéroté : (largeur des blancs, numéro, largeur totale « blancs + chiffres »).
fn numero_item(arbre: &Arbre, doc: &Doc, item: usize) -> Option<(usize, i64, usize)> {
    let n = arbre.noeud(item);
    let texte = doc.slice(n.from(), n.to());
    let c = RE_NUMERO.captures(&texte)?;
    let blancs = c.get(1).map_or("", |g| g.as_str());
    let chiffres = c.get(2).map_or("", |g| g.as_str());
    Some((
        utf16_len(blancs),
        chiffres.parse::<i64>().unwrap_or(0),
        utf16_len(blancs) + utf16_len(chiffres),
    ))
}

/// Les contextes de `noeud` (le nœud le plus profond à la position), du plus EXTERNE au plus interne.
pub(super) fn contexte_de(arbre: &Arbre, doc: &Doc, noeud: usize) -> Vec<Contexte> {
    let mut noeuds = Vec::new();
    for i in std::iter::once(noeud).chain(arbre.ancetres(noeud)) {
        if matches!(arbre.noeud(i).genre, Genre::ListItem | Genre::Blockquote) {
            noeuds.push(i);
        }
    }
    let mut contexte = Vec::new();
    for &i in noeuds.iter().rev() {
        let n = arbre.noeud(i);
        let ligne = doc.ligne_a(n.from());
        let debut = n.from() - ligne.from;
        let texte = doc.slice(n.from(), ligne.to);
        let parent = n.parent.map(|p| arbre.noeud(p).genre);
        if n.genre == Genre::Blockquote {
            if let Some(c) = RE_CITATION.captures(&texte) {
                let long = utf16_len(&c[0]);
                contexte.push(Contexte {
                    genre: Genre::Blockquote,
                    from: debut,
                    to: debut + long,
                    espace_avant: String::new(),
                    espace_apres: c[1].to_owned(),
                    type_: ">".to_owned(),
                    item: None,
                });
            }
        } else if parent == Some(Genre::OrderedList)
            && let Some(c) = RE_NUMEROTEE.captures(&texte)
        {
            let mut apres = c[3].to_owned();
            let mut long = utf16_len(&c[0]);
            if apres.len() >= 4 {
                apres.truncate(apres.len() - 4);
                long -= 4;
            }
            contexte.push(Contexte {
                genre: Genre::OrderedList,
                from: debut,
                to: debut + long,
                espace_avant: c[1].to_owned(),
                espace_apres: apres,
                type_: c[2].to_owned(),
                item: Some(i),
            });
        } else if parent == Some(Genre::BulletList)
            && let Some(c) = RE_PUCE.captures(&texte)
        {
            let mut apres = c[4].to_owned();
            let mut long = utf16_len(&c[0]);
            if apres.len() > 4 {
                apres.truncate(apres.len() - 4);
                long -= 4;
            }
            let mut type_ = c[2].to_owned();
            if let Some(case) = c.get(3) {
                // Une tâche continue avec une case VIDE, même depuis une cochée.
                type_.push_str(&case.as_str().replacen(['x', 'X'], " ", 1));
            }
            contexte.push(Contexte {
                genre: Genre::BulletList,
                from: debut,
                to: debut + long,
                espace_avant: c[1].to_owned(),
                espace_apres: apres,
                type_,
                item: Some(i),
            });
        }
    }
    contexte
}

/// Les items qui SUIVENT `apres` dans sa liste, tant que leurs numéros se suivent : chacun est
/// renuméroté de `+1 + decalage`. S'arrête au premier numéro qui ne suit pas (liste volontairement
/// désordonnée : on n'y touche pas).
pub(super) fn renumeroter(
    arbre: &Arbre,
    doc: &Doc,
    apres: usize,
    decalage: i64,
    changes: &mut Vec<EditorChangeSpec>,
) {
    let mut precedent: Option<i64> = None;
    let mut courant = Some(apres);
    while let Some(i) = courant {
        let n = arbre.noeud(i);
        if n.genre == Genre::ListItem
            && let Some((blancs, numero, total)) = numero_item(arbre, doc, i)
        {
            if let Some(p) = precedent {
                if numero != p + 1 {
                    return;
                }
                changes.push(EditorChangeSpec {
                    from: n.from() + blancs,
                    to: Some(n.from() + total),
                    insert: (p + 2 + decalage).to_string(),
                });
            }
            precedent = Some(numero);
        }
        courant = frere_suivant(arbre, i);
    }
}

fn frere_suivant(arbre: &Arbre, i: usize) -> Option<usize> {
    let parent = arbre.noeud(i).parent?;
    let fin = arbre.noeud(i).fin_sous_arbre;
    arbre.enfants(parent).find(|&e| e == fin)
}

/// Vrai si la position est dans un bloc de code clôturé : CodeMirror y confie la touche au langage
/// du bloc, pas au Markdown.
pub(super) fn dans_code_cloture(arbre: &Arbre, noeud: usize) -> bool {
    arbre.ancetre_de_genre(noeud, Genre::FencedCode).is_some()
}
