// Port de app/src/ui/core/editor/revealPassage.ts (le modèle pur `locatePassage` seulement)
//
// ═══════════════════════════════════════════════════════════════════════════
//  RÉVÉLER UN PASSAGE sur n'importe quelle surface de document — une note, un
//  PDF, une conversation, la vue d'un plugin tiers : rien n'est à coder par vue.
//
//  Un passage = des positions (souvent périmées : le document a pu changer
//  depuis) + sa CITATION (le texte du passage). On le relocalise par la citation
//  (la cascade des ancres, `findQuote`), on le SURLIGNE (la table commune des
//  surfaces, `range_highlights`, classe `is-passage`) et on le fait défiler à
//  vue : ces deux derniers temps sont de la présentation (fragment-ui) ; ici ne
//  vit que la question pure « où est le passage dans ce texte ? ».
//
//  ★ `findQuote` (artefacts/anchor.ts : à l'identique, puis à un quart d'erreurs
//    près, au plus près de l'indice) appartient à fragment-artefacts, qui dépend
//    de fragment-text : la fonction est INJECTÉE (`QuoteFinder`). La crate fournit
//    le chercheur à l'identique (`find_quote_exact`) ; les surfaces branchent la
//    cascade complète quand elles l'ont.
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::{EditorRange, SurfaceText};
use crate::find_passage::{est_blanc, replier, trim_js};
use crate::utf16::{byte_to_utf16, utf16_len};

/// Un passage à révéler.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PassageRef {
    /// Positions du passage (texte : unités de code UTF-16, ADR-004).
    pub from: usize,
    pub to: usize,
    /// Le texte du passage (blancs éventuellement repliés) : relocalise des positions périmées.
    pub quote: Option<String>,
}

impl PassageRef {
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            quote: None,
        }
    }

    pub fn with_quote(from: usize, to: usize, quote: &str) -> Self {
        Self {
            from,
            to,
            quote: Some(quote.to_owned()),
        }
    }
}

/// La clef du surlignage d'un passage révélé dans la table commune de la surface.
pub const PASSAGE_KEY: &str = "reveal:passage";

/// Cherche une citation `exact` dans `text`, au plus près de `hint` (un index
/// UTF-16, ADR-004) ; rend l'index UTF-16 de son début. La signature de `findQuote`
/// (artefacts/anchor.ts), moins le préfixe et le suffixe que `locate_passage`
/// laisse vides.
pub trait QuoteFinder {
    fn find_quote(&self, text: &str, exact: &str, hint: usize) -> Option<usize>;
}

impl<F: Fn(&str, &str, usize) -> Option<usize>> QuoteFinder for F {
    fn find_quote(&self, text: &str, exact: &str, hint: usize) -> Option<usize> {
        self(text, exact, hint)
    }
}

/// Le chercheur à l'IDENTIQUE : parmi les occurrences exactes, la plus proche de
/// l'indice (à égalité, la première) — le premier étage de la cascade de
/// `findQuote`, sans la recherche approchée.
pub fn find_quote_exact(text: &str, exact: &str, hint: usize) -> Option<usize> {
    if exact.is_empty() {
        return None;
    }
    let mut best: Option<usize> = None;
    let mut depuis = 0;
    // L'index UTF-16 de `depuis`, tenu à jour pour ne pas reparcourir le texte.
    let mut depuis16 = 0;
    while let Some(rel) = text[depuis..].find(exact) {
        let k_octet = depuis + rel;
        let k = depuis16 + byte_to_utf16(&text[depuis..], rel);
        if best.is_none_or(|b| k.abs_diff(hint) < b.abs_diff(hint)) {
            best = Some(k);
        }
        // Les occurrences qui se chevauchent comptent aussi (`exactMatches` avance d'un caractère).
        let suivant = text[k_octet..].chars().next().map_or(1, char::len_utf8);
        depuis16 = k + text[k_octet..k_octet + suivant].encode_utf16().count();
        depuis = k_octet + suivant;
    }
    best
}

/// Où est le passage dans ce texte ? Par la citation d'abord (relocalisée par
/// `find_quote_exact`, au plus près des positions données) ; sans citation, les
/// positions bornées. Pur.
pub fn locate_passage(t: &SurfaceText, p: &PassageRef) -> Option<EditorRange> {
    locate_passage_with(t, p, &find_quote_exact)
}

/// [`locate_passage`] avec le chercheur de citation de son choix (la cascade
/// complète de fragment-artefacts, par exemple).
pub fn locate_passage_with(
    t: &SurfaceText,
    p: &PassageRef,
    finder: &dyn QuoteFinder,
) -> Option<EditorRange> {
    let len = utf16_len(&t.text);
    let borne = |pos: usize| t.to_pos(t.to_index(pos).min(len));
    let quote = p.quote.as_deref().map(trim_js).unwrap_or("");
    if quote.is_empty() {
        return (p.to > p.from).then(|| EditorRange {
            from: borne(p.from),
            to: borne(p.to),
        });
    }
    let hint = t.to_index(p.from).min(len);
    // On cherche dans le texte aux BLANCS REPLIÉS (la citation a pu l'être : une
    // sélection sur plusieurs lignes) — la fin se relit alors exactement dans le
    // texte d'origine, même quand ses blancs diffèrent de ceux de la citation.
    let replie = replier_blancs(&t.text);
    let q = replier(quote);
    let j = finder.find_quote(&replie.text, &q, replie.vers_replie(hint))?;
    let a = replie.vers_texte(j);
    let b = replie.vers_texte((j + utf16_len(&q) - 1).min(replie.len.saturating_sub(1))) + 1;
    Some(EditorRange {
        from: t.to_pos(a),
        to: t.to_pos(b),
    })
}

/// Le texte aux blancs repliés, et les deux cartes d'indices (en unités UTF-16).
struct Replie {
    text: String,
    /// Longueur du texte replié, en unités UTF-16.
    len: usize,
    /// Pour chaque UNITÉ repliée, l'unité d'origine.
    indices: Vec<usize>,
    /// Longueur du texte d'origine, en unités UTF-16.
    origine_len: usize,
}

impl Replie {
    /// index replié → index d'origine (au-delà : la fin du texte).
    fn vers_texte(&self, i: usize) -> usize {
        self.indices.get(i).copied().unwrap_or(self.origine_len)
    }

    /// index d'origine → index replié (la première unité repliée dont l'origine est ≥ i).
    fn vers_replie(&self, i: usize) -> usize {
        self.indices.partition_point(|&x| x < i)
    }
}

fn replier_blancs(text: &str) -> Replie {
    let mut out = String::with_capacity(text.len());
    let mut indices = Vec::with_capacity(text.len());
    let mut i = 0;
    for c in text.chars() {
        let blanc = est_blanc(c);
        let l = c.len_utf16();
        if blanc && out.ends_with(' ') {
            i += l;
            continue;
        }
        if blanc {
            out.push(' ');
            indices.push(i);
        } else {
            out.push(c);
            indices.extend(i..i + l);
        }
        i += l;
    }
    Replie {
        len: indices.len(),
        text: out,
        indices,
        origine_len: i,
    }
}
