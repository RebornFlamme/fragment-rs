// Port de app/src/ui/core/artefacts/anchor.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'ancre durable ↔ l'ancre vivante (§15.7).
//
//  Une ancre au texte est un offset. Un offset n'est juste que pour une version
//  exacte du texte ; or le texte et le journal voyagent dans deux fichiers que
//  la synchronisation livre dans un ordre quelconque. La CITATION rend l'ancre
//  vérifiable, et la retrouve si le texte a changé hors de l'app.
//
//  À chaque écriture d'une ancre au texte, le cœur relève 32 caractères avant
//  l'offset, les 32 suivants, puis 32 encore : préfixe, texte exact, suffixe
//  (TextQuoteSelector du W3C). Le PDF n'a pas de citation : ses offsets sont fixes.
//
//  La résolution est la cascade de Hypothesis, avec sa notation des candidats et
//  une tolérance deux fois plus stricte (¼ d'erreurs au lieu de ½) :
//
//      surface textuelle ? ─non─▶ ancre gardée, pos borné
//      le texte à pos est celui de la citation ? ─oui─▶ ancre gardée
//      la citation est retrouvée à l'identique ? ─oui─▶ relocalisée
//      retrouvée à un quart d'erreurs près ? ─oui─▶ relocalisée
//      sinon ─▶ orpheline : widget rangé dans la marge, à ré-ancrer à la main
//
//  ★ La relocalisation n'écrit RIEN tant que l'utilisateur n'a rien fait (en
//    mémoire seulement) : le texte et le journal peuvent arriver dans n'importe
//    quel ordre.
//
//  ★ UNITÉ : les index dans le texte comptent des CARACTÈRES (valeurs scalaires
//    Unicode), pas des unités UTF-16 comme `String.prototype.slice` côté TS ni des
//    octets. Même compte pour tout ce qui n'est pas hors du plan multilingue de
//    base (émojis) ; l'unité de `pos` dans fragment-text est à trancher (voir la PR).
// ═══════════════════════════════════════════════════════════════════════════

pub mod approx;

use crate::types::{AnchorQuote, DurableAnchor, GutterSide, WidgetAnchor};

/// La longueur de chaque morceau de la citation (§15.7 ; le contexte de Hypothesis).
pub const QUOTE_LEN: usize = 32;

/// La tolérance de la recherche approchée : un quart d'erreurs au plus (§11.2).
pub const MAX_ERROR_RATIO: f64 = 1.0 / 4.0;

/// La notation des candidats (Hypothesis, match-quote.ts).
const QUOTE_WEIGHT: f64 = 50.0;
const PREFIX_WEIGHT: f64 = 20.0;
const SUFFIX_WEIGHT: f64 = 20.0;
const POS_WEIGHT: f64 = 2.0;

/// Le texte entier d'une surface, avec sa carte index ↔ position (`SurfaceText`
/// de core/editor/Editor.ts). Pour un éditeur, index = position ; pour une
/// surface à PAS FIXE (une conversation : une plage de positions par unité),
/// l'index dans le texte joint n'est pas la position.
pub trait SurfaceText {
    /// Le texte, en caractères.
    fn chars(&self) -> &[char];
    /// index dans le texte → position (un index entre deux unités : fin de la précédente).
    fn to_pos(&self, index: i64) -> i64;
    /// position → index dans le texte (bornée à son unité ; hors unité : l'unité suivante).
    fn to_index(&self, pos: i64) -> i64;
}

/// Une chaîne = un éditeur : la carte identité.
pub struct PlainText {
    chars: Vec<char>,
}

impl PlainText {
    pub fn new(text: &str) -> Self {
        PlainText {
            chars: text.chars().collect(),
        }
    }
}

impl SurfaceText for PlainText {
    fn chars(&self) -> &[char] {
        &self.chars
    }
    fn to_pos(&self, index: i64) -> i64 {
        index
    }
    fn to_index(&self, pos: i64) -> i64 {
        pos
    }
}

/// Ce qu'une surface sait de son texte (`fullText` de Editor.ts) : jamais (PDF,
/// image), pas encore (chargement), ou le texte.
pub enum FullText<'a> {
    /// Surface sans texte adressable : les offsets sont fixes.
    Never,
    /// La surface donnera son texte, mais pas encore : ne résoudre aucune ancre maintenant.
    Pending,
    Ready(&'a dyn SurfaceText),
}

/// La surface d'un document, vue par les ancres (le `DocumentSurface` de l'éditeur,
/// réduit à `fullText`).
pub trait TextSource {
    fn full_text(&self) -> FullText<'_>;
}

/// Retire la citation : l'ancre vivante de l'API layers.
pub fn to_live(anchor: &DurableAnchor) -> WidgetAnchor {
    anchor.anchor.clone()
}

fn clamp(i: i64, len: usize) -> usize {
    i.clamp(0, len as i64) as usize
}

fn slice(text: &[char], a: usize, b: usize) -> String {
    let b = b.min(text.len());
    let a = a.min(b);
    text[a..b].iter().collect()
}

/// La citation relevée autour de `pos` dans `text`.
pub fn quote_at(text: &[char], pos: i64) -> AnchorQuote {
    let p = clamp(pos, text.len());
    AnchorQuote {
        prefix: slice(text, p.saturating_sub(QUOTE_LEN), p),
        exact: slice(text, p, p + QUOTE_LEN),
        suffix: slice(text, p + QUOTE_LEN, p + 2 * QUOTE_LEN),
    }
}

/// La citation d'une PLAGE `[from, to)` : son texte même (W3C TextQuoteSelector),
/// entre les 32 caractères qui la précèdent et les 32 qui la suivent.
pub fn quote_of(text: &[char], from: i64, to: i64) -> AnchorQuote {
    let a = clamp(from, text.len());
    let b = clamp(to, text.len()).max(a);
    AnchorQuote {
        prefix: slice(text, a.saturating_sub(QUOTE_LEN), a),
        exact: slice(text, a, b),
        suffix: slice(text, b, b + QUOTE_LEN),
    }
}

/// L'ancre vivante → durable. Sur une surface textuelle, et pour une ancre au
/// texte, relève la citation : le texte de la PLAGE (`to`), sinon la fenêtre
/// autour du point. Sinon (viewport, surface sans texte), copie telle quelle.
pub fn to_durable(anchor: &WidgetAnchor, editor: Option<&dyn TextSource>) -> DurableAnchor {
    let live = anchor.clone();
    let Some(t) = surface_text(editor) else {
        return live.into();
    };
    let Some(pos) = live.pos() else {
        return live.into();
    };
    let i = t.to_index(pos);
    let quote = match live.to() {
        Some(to) if to > pos => quote_of(t.chars(), i, t.to_index(to)),
        _ => quote_at(t.chars(), i),
    };
    DurableAnchor {
        anchor: live,
        quote: Some(quote),
    }
}

/// Le texte entier de la surface (avec sa carte vers les positions), ou `None`
/// si elle n'en a pas d'adressable (PDF, image…) ou pas ENCORE (chargement —
/// cf. `surface_pending`).
pub fn surface_text(editor: Option<&dyn TextSource>) -> Option<&dyn SurfaceText> {
    match editor?.full_text() {
        FullText::Ready(t) => Some(t),
        FullText::Never | FullText::Pending => None,
    }
}

/// La surface donnera son texte, mais pas encore : ne résoudre aucune ancre maintenant.
pub fn surface_pending(editor: Option<&dyn TextSource>) -> bool {
    matches!(editor.map(|e| e.full_text()), Some(FullText::Pending))
}

/// L'ancre durable → vivante, en bornant `pos` aux limites du document courant
/// (sans quoi un `coords_at_pos` lève sur un document rétréci). Les ancres viewport
/// et les surfaces sans texte (PDF : offsets fixes) passent inchangées.
pub fn clamp_to_doc(anchor: &DurableAnchor, editor: Option<&dyn TextSource>) -> WidgetAnchor {
    let live = to_live(anchor);
    let (Some(pos), Some(t)) = (live.pos(), surface_text(editor)) else {
        return live;
    };
    let borne = |p: i64| t.to_pos(clamp(t.to_index(p), t.chars().len()) as i64);
    live.with_range(borne(pos), live.to().map(borne))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    /// Vérifiée.
    Kept,
    /// Retrouvée ailleurs (en mémoire).
    Relocated,
    /// Introuvable.
    Orphan,
}

/// Le résultat d'une résolution.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub anchor: WidgetAnchor,
    pub status: ResolutionStatus,
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    start: usize,
    end: usize,
    errors: usize,
}

fn find_chars(text: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > text.len() {
        return None;
    }
    (from..=text.len() - needle.len()).find(|&i| &text[i..i + needle.len()] == needle)
}

/// Les occurrences exactes de `needle` dans `text`.
fn exact_matches(text: &[char], needle: &[char]) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = find_chars(text, needle, from) {
        out.push(Candidate {
            start: i,
            end: i + needle.len(),
            errors: 0,
        });
        from = i + 1;
    }
    out
}

/// La ressemblance de `needle` à `text`, entre 0 et 1 (Hypothesis : textMatchScore).
fn text_match_score(text: &[char], needle: &[char]) -> f64 {
    if needle.is_empty() || text.is_empty() {
        return 0.0;
    }
    if !exact_matches(text, needle).is_empty() {
        return 1.0;
    }
    match approx::search(text, needle, needle.len()).first() {
        Some(m) => 1.0 - m.errors as f64 / needle.len() as f64,
        None => 0.0,
    }
}

/// La note d'un candidat : citation 50, préfixe 20, suffixe 20, proximité 2 — normalisée.
fn score(text: &[char], quote: &AnchorQuote, c: Candidate, hint: usize) -> f64 {
    let exact_len = quote.exact.chars().count().max(1);
    let quote_score = 1.0 - c.errors as f64 / exact_len as f64;
    let prefix: Vec<char> = quote.prefix.chars().collect();
    let suffix: Vec<char> = quote.suffix.chars().collect();
    let prefix_score = if prefix.is_empty() {
        1.0
    } else {
        text_match_score(
            &text[c.start.saturating_sub(prefix.len())..c.start],
            &prefix,
        )
    };
    let suffix_score = if suffix.is_empty() {
        1.0
    } else {
        let end = c.end.min(text.len());
        text_match_score(&text[end..(end + suffix.len()).min(text.len())], &suffix)
    };
    let pos_score = if text.is_empty() {
        1.0
    } else {
        1.0 - (c.start as f64 - hint as f64).abs() / text.len() as f64
    };
    let raw = QUOTE_WEIGHT * quote_score
        + PREFIX_WEIGHT * prefix_score
        + SUFFIX_WEIGHT * suffix_score
        + POS_WEIGHT * pos_score;
    raw / (QUOTE_WEIGHT + PREFIX_WEIGHT + SUFFIX_WEIGHT + POS_WEIGHT)
}

/// Cherche la citation dans `text` : à l'identique d'abord, puis à un quart
/// d'erreurs près. Plusieurs candidats : le mieux noté l'emporte. Rend l'offset
/// du début du texte exact, ou `None`.
pub fn find_quote(text: &[char], quote: &AnchorQuote, hint: usize) -> Option<usize> {
    let exact: Vec<char> = quote.exact.chars().collect();
    if exact.is_empty() {
        return None;
    }
    let mut candidates = exact_matches(text, &exact);
    if candidates.is_empty() {
        let max_errors = (exact.len() as f64 * MAX_ERROR_RATIO).floor() as usize;
        if max_errors < 1 {
            return None;
        }
        candidates = approx::search(text, &exact, max_errors)
            .into_iter()
            .filter(|m| m.errors as f64 <= exact.len() as f64 * MAX_ERROR_RATIO)
            .map(|m| Candidate {
                start: m.start,
                end: m.end,
                errors: m.errors,
            })
            .collect();
    }
    let mut best: Option<(f64, usize)> = None;
    for c in candidates {
        let s = score(text, quote, c, hint);
        if best.is_none_or(|(bs, _)| s > bs) {
            best = Some((s, c.start));
        }
    }
    best.map(|(_, start)| start)
}

/// Résout une ancre durable contre le texte courant (`None` = surface sans texte).
/// La cascade de la figure 35.
pub fn resolve_anchor(anchor: &DurableAnchor, text: Option<&dyn SurfaceText>) -> Resolution {
    let live = to_live(anchor);
    let Some(pos) = live.pos() else {
        return Resolution {
            anchor: live,
            status: ResolutionStatus::Kept,
        };
    };
    // Surface non textuelle (PDF) : offsets fixes, simplement bornés si on connaît la longueur.
    let Some(t) = text else {
        return Resolution {
            anchor: live,
            status: ResolutionStatus::Kept,
        };
    };

    // La citation se cherche dans le TEXTE ; le résultat se rend en POSITION
    // (identité sur un éditeur, carte d'une surface à pas fixe).
    let chars = t.chars();
    let i = clamp(t.to_index(pos), chars.len());
    let quote = anchor.quote.as_ref();
    // Une PLAGE garde sa longueur (dans le texte) quand elle est relocalisée. ★ Celle
    // de SA CITATION (son texte, cf. `to_durable`) : l'écart des anciennes positions
    // relu dans le NOUVEAU texte ne vaut rien quand les unités d'une surface à pas
    // fixe ont changé (conversation relue, parts regroupées → longueur 0, plus de
    // passage). Sans citation : l'écart des positions. Plage VIDE (`to == pos`, le
    // fil d'un cercle) : sa citation est celle d'un POINT (une fenêtre), longueur 0.
    let longueur: Option<usize> = match live.to() {
        None => None,
        Some(to) if to <= pos => Some(0),
        Some(to) => match quote {
            Some(q) if !q.exact.is_empty() => Some(q.exact.chars().count()),
            _ => Some((t.to_index(to) - t.to_index(pos)).max(0) as usize),
        },
    };
    let placer = |debut: usize| -> WidgetAnchor {
        let p = t.to_pos(debut as i64);
        match longueur {
            None => live.with_range(p, None),
            Some(l) => live.with_range(p, Some(t.to_pos((debut + l).min(chars.len()) as i64))),
        }
    };
    let clamped = placer(i);
    // Pas de citation (ancre importée), ou citation vide (ancre en fin de document) : on garde.
    let Some(q) = quote.filter(|q| !q.exact.is_empty()) else {
        return Resolution {
            anchor: clamped,
            status: ResolutionStatus::Kept,
        };
    };

    // 1. Le texte à pos est-il celui de la citation ?
    let exact: Vec<char> = q.exact.chars().collect();
    if chars.len() >= i + exact.len() && chars[i..i + exact.len()] == exact[..] {
        return Resolution {
            anchor: clamped,
            status: ResolutionStatus::Kept,
        };
    }

    // 2-3. Retrouvée à l'identique, ou à un quart d'erreurs près.
    if let Some(found) = find_quote(chars, q, i) {
        return Resolution {
            anchor: placer(found),
            status: ResolutionStatus::Relocated,
        };
    }

    // 4. Orpheline : rangée dans la marge, à ré-ancrer à la main.
    Resolution {
        anchor: WidgetAnchor::Gutter {
            side: GutterSide::Right,
            pos: clamped.pos().unwrap_or(0),
            to: None,
            dy: 0.0,
        },
        status: ResolutionStatus::Orphan,
    }
}

/// `resolve_anchor` contre une chaîne (la carte identité).
pub fn resolve_anchor_in(anchor: &DurableAnchor, text: &str) -> Resolution {
    resolve_anchor(anchor, Some(&PlainText::new(text)))
}

#[cfg(test)]
mod tests;
