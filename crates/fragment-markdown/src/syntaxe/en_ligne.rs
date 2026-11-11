// Port de app/src/ui/views/markdown/language.ts (extensions en ligne de Fragment : surligné, liens
// internes et embeds, maths en ligne) et de l'analyse en ligne de @lezer/markdown 1.7.2 (MIT,
// © Marijn Haverbeke et contributeurs ; GFM, indice, exposant, émoji). Traduction, voir syntaxe/mod.rs.
//! Analyse EN LIGNE : d'un texte de bloc feuille à la liste de ses éléments (emphase, liens, code…).
//!
//! L'algorithme est celui de Lezer : on avance caractère par caractère ; à chaque position, les
//! analyseurs sont essayés DANS L'ORDRE (`ANALYSEURS`) ; le premier qui répond pose un élément
//! (nœud fini) ou un DÉLIMITEUR (`*`, `==`, `$`, `[`…), et les délimiteurs sont appariés à la fin
//! (`resoudre`), du plus proche au plus lointain.
//!
//! ★ Positions en OCTETS ici (le texte est un `&str`) ; Lezer compte en unités UTF-16. Tous les
//!   caractères testés sont ASCII, donc les deux comptes donnent les mêmes décisions, À UNE EXCEPTION
//!   près : le caractère voisin d'un délimiteur d'emphase. JS en lit UNE unité de code : pour un
//!   caractère hors du plan de base (un émoji), c'est un demi-substitut, ni espace ni ponctuation.
//!   `voisin` reproduit ce cas (`Voisin::Substitut`), sinon `é😀 **gras**` changerait d'avis.

use std::sync::LazyLock;

use regex::Regex;

use super::genre::Genre;

/// Un élément produit par l'analyse (nœud et ses enfants), en octets absolus du document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Elt {
    pub genre: Genre,
    pub from: usize,
    pub to: usize,
    pub enfants: Vec<Elt>,
}

impl Elt {
    pub(crate) fn feuille(genre: Genre, from: usize, to: usize) -> Elt {
        Elt {
            genre,
            from,
            to,
            enfants: Vec::new(),
        }
    }
    pub(crate) fn avec(genre: Genre, from: usize, to: usize, enfants: Vec<Elt>) -> Elt {
        Elt {
            genre,
            from,
            to,
            enfants,
        }
    }
}

// ─── Classes de caractères de JS ─────────────────────────────────────────────

/// La classe `\s` de JS (le moteur `regex` n'a pas exactement la même).
const JS_S: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

fn re(motif: &str) -> Regex {
    Regex::new(motif).unwrap_or_else(|e| unreachable!("regex constante : {e}"))
}

fn est_espace_js(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

static PONCTUATION: LazyLock<Regex> = LazyLock::new(|| re(r"^[\p{S}\p{P}|]$"));

/// `[\p{S}|\p{P}]` de Lezer (ponctuation et symboles Unicode).
fn est_ponctuation(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_punctuation();
    }
    let mut tampon = [0u8; 4];
    PONCTUATION.is_match(c.encode_utf8(&mut tampon))
}

/// `space()` de Lezer : espace, tabulation, saut de ligne, retour chariot.
pub(crate) fn espace(c: i32) -> bool {
    c == 32 || c == 9 || c == 10 || c == 13
}

/// L'unité de code UTF-16 voisine d'une position, comme JS la voit.
#[derive(Clone, Copy)]
enum Voisin {
    /// Hors de la section (chaîne vide en JS).
    Vide,
    Car(char),
    /// Demi-substitut d'un caractère hors du plan de base : ni espace ni ponctuation.
    Substitut,
}

impl Voisin {
    fn espace(self) -> bool {
        match self {
            Voisin::Vide => true,
            Voisin::Car(c) => est_espace_js(c),
            Voisin::Substitut => false,
        }
    }
    fn ponctuation(self) -> bool {
        matches!(self, Voisin::Car(c) if est_ponctuation(c))
    }
}

// ─── Contexte ────────────────────────────────────────────────────────────────

/// Type de délimiteur (appariés à la fin par `resoudre`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeDelim {
    EmphaseSouligne,
    EmphaseEtoile,
    DebutLien,
    DebutImage,
    Surligne,
    Maths,
    MathsDisplay,
    Barre,
}

impl TypeDelim {
    /// Le nœud produit par une paire (`None` : le délimiteur n'est pas apparié par `resoudre`).
    fn resout(self) -> Option<Genre> {
        match self {
            TypeDelim::EmphaseSouligne | TypeDelim::EmphaseEtoile => Some(Genre::Emphasis),
            TypeDelim::DebutLien | TypeDelim::DebutImage => None,
            TypeDelim::Surligne => Some(Genre::Highlight),
            TypeDelim::Maths => Some(Genre::InlineMath),
            TypeDelim::MathsDisplay => Some(Genre::InlineDisplayMath),
            TypeDelim::Barre => Some(Genre::Strikethrough),
        }
    }
    fn marque(self) -> Option<Genre> {
        match self {
            TypeDelim::EmphaseSouligne | TypeDelim::EmphaseEtoile => Some(Genre::EmphasisMark),
            TypeDelim::DebutLien | TypeDelim::DebutImage => None,
            TypeDelim::Surligne => Some(Genre::HighlightMark),
            TypeDelim::Maths | TypeDelim::MathsDisplay => Some(Genre::InlineMathMark),
            TypeDelim::Barre => Some(Genre::StrikethroughMark),
        }
    }
    fn emphase(self) -> bool {
        matches!(self, TypeDelim::EmphaseSouligne | TypeDelim::EmphaseEtoile)
    }
}

const OUVRE: u8 = 1;
const FERME: u8 = 2;

#[derive(Debug, Clone, Copy)]
struct Delim {
    genre: TypeDelim,
    from: usize,
    to: usize,
    cote: u8,
}

#[derive(Debug)]
enum Part {
    Elt(Elt),
    Delim(Delim),
    /// Une case vidée (le `null` de Lezer).
    Vide,
}

/// Une section de texte en ligne : `texte` commence à la position absolue `offset`.
pub(crate) struct Ctx<'a> {
    texte: &'a str,
    offset: usize,
    parts: Vec<Part>,
}

/// Valeur de `char()` hors de la section, AVANT son début (`NaN` en JS : égal à rien, pas un espace).
const AVANT_DEBUT: i32 = -2;

impl<'a> Ctx<'a> {
    fn fin(&self) -> usize {
        self.offset + self.texte.len()
    }

    /// L'octet à la position absolue `pos` ; -1 au-delà de la fin.
    fn car(&self, pos: usize) -> i32 {
        if pos >= self.fin() {
            -1
        } else if pos < self.offset {
            AVANT_DEBUT
        } else {
            i32::from(self.texte.as_bytes()[pos - self.offset])
        }
    }

    /// `char(pos - 1)` sans débordement.
    fn car_avant(&self, pos: usize) -> i32 {
        if pos == 0 {
            AVANT_DEBUT
        } else {
            self.car(pos - 1)
        }
    }

    fn reste(&self, pos: usize) -> &'a str {
        let i = pos.saturating_sub(self.offset).min(self.texte.len());
        self.texte.get(i..).unwrap_or("")
    }

    /// L'unité de code juste AVANT `pos` (`slice(pos - 1, pos)`).
    fn voisin_avant(&self, pos: usize) -> Voisin {
        if pos <= self.offset {
            return Voisin::Vide;
        }
        let i = pos - self.offset;
        match self.texte.get(..i).and_then(|s| s.chars().next_back()) {
            Some(c) if c.len_utf16() == 2 => Voisin::Substitut,
            Some(c) => Voisin::Car(c),
            None => Voisin::Vide,
        }
    }

    /// L'unité de code À `pos` (`slice(pos, pos + 1)`).
    fn voisin_apres(&self, pos: usize) -> Voisin {
        match self.reste(pos).chars().next() {
            _ if pos >= self.fin() => Voisin::Vide,
            Some(c) if c.len_utf16() == 2 => Voisin::Substitut,
            Some(c) => Voisin::Car(c),
            None => Voisin::Vide,
        }
    }

    fn sauter_espaces(&self, from: usize) -> usize {
        let b = self.texte.as_bytes();
        let mut i = from.saturating_sub(self.offset);
        while i < b.len() && espace(i32::from(b[i])) {
            i += 1;
        }
        i + self.offset
    }

    fn ajouter(&mut self, e: Elt) -> Option<usize> {
        let to = e.to;
        self.parts.push(Part::Elt(e));
        Some(to)
    }

    fn delimiteur(
        &mut self,
        genre: TypeDelim,
        from: usize,
        to: usize,
        ouvre: bool,
        ferme: bool,
    ) -> Option<usize> {
        let cote = (if ouvre { OUVRE } else { 0 }) | (if ferme { FERME } else { 0 });
        self.parts.push(Part::Delim(Delim {
            genre,
            from,
            to,
            cote,
        }));
        Some(to)
    }

    fn a_un_lien_ouvert(&self) -> bool {
        self.parts.iter().rev().any(|p| {
            matches!(p, Part::Delim(d) if matches!(d.genre, TypeDelim::DebutLien | TypeDelim::DebutImage))
        })
    }

    /// Apparie les délimiteurs à partir de l'index `from` (fermant par fermant, vers l'ouvrant le plus
    /// proche) et rend les éléments restants. `resolveMarkers` de Lezer.
    fn resoudre(&mut self, from: usize) -> Vec<Elt> {
        let mut i = from;
        while i < self.parts.len() {
            let ferme = match &self.parts[i] {
                Part::Delim(d) if d.genre.resout().is_some() && d.cote & FERME != 0 => *d,
                _ => {
                    i += 1;
                    continue;
                }
            };
            let emp = ferme.genre.emphase();
            let taille_ferme = ferme.to - ferme.from;
            let mut ouvre = None;
            let mut j = i;
            while j > from {
                j -= 1;
                if let Part::Delim(p) = &self.parts[j] {
                    let taille = p.to - p.from;
                    // ★ Règle du « multiple de 3 » de CommonMark pour l'emphase.
                    let interdit = emp
                        && (ferme.cote & OUVRE != 0 || p.cote & FERME != 0)
                        && (taille + taille_ferme) % 3 == 0
                        && (taille % 3 != 0 || taille_ferme % 3 != 0);
                    if p.cote & OUVRE != 0 && p.genre == ferme.genre && !interdit {
                        ouvre = Some(*p);
                        break;
                    }
                }
            }
            let Some(ouvre) = ouvre else {
                i += 1;
                continue;
            };
            let mut genre = ferme.genre.resout().unwrap_or(Genre::Emphasis);
            let (mut start, mut end) = (ouvre.from, ferme.to);
            if emp {
                let taille = 2.min(ouvre.to - ouvre.from).min(taille_ferme);
                start = ouvre.to - taille;
                end = ferme.from + taille;
                genre = if taille == 1 {
                    Genre::Emphasis
                } else {
                    Genre::StrongEmphasis
                };
            }
            let mut contenu = Vec::new();
            if let Some(m) = ouvre.genre.marque() {
                contenu.push(Elt::feuille(m, start, ouvre.to));
            }
            for k in j + 1..i {
                if let Part::Elt(e) = std::mem::replace(&mut self.parts[k], Part::Vide) {
                    contenu.push(e);
                }
            }
            if let Some(m) = ferme.genre.marque() {
                contenu.push(Elt::feuille(m, ferme.from, end));
            }
            let element = Elt::avec(genre, start, end, contenu);
            // Des caractères d'emphase restent : on raccourcit l'ouvrant et le fermant au lieu de les vider.
            self.parts[j] = if emp && ouvre.from != start {
                Part::Delim(Delim { to: start, ..ouvre })
            } else {
                Part::Vide
            };
            if emp && ferme.to != end {
                self.parts[i] = Part::Delim(Delim { from: end, ..ferme });
                self.parts.insert(i, Part::Elt(element));
            } else {
                self.parts[i] = Part::Elt(element);
            }
            i += 1;
        }
        let mut res = Vec::new();
        for p in self.parts.drain(from..) {
            if let Part::Elt(e) = p {
                res.push(e);
            }
        }
        res
    }
}

// ─── Les analyseurs, dans l'ordre de Fragment ────────────────────────────────

/// Les caractères qu'une barre oblique inverse échappe (CommonMark).
const ECHAPPABLES: &[u8] = b"!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

fn echappement(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 92 || start + 1 == cx.fin() {
        return None;
    }
    let e = cx.car(start + 1);
    if e >= 0 && ECHAPPABLES.contains(&(e as u8)) {
        return cx.ajouter(Elt::feuille(Genre::Escape, start, start + 2));
    }
    None
}

fn entite(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 38 {
        return None;
    }
    // `^(?:#\d+|#x[a-f\d]+|\w+);` (insensible à la casse) dans les 30 unités suivantes.
    let s = cx.reste(start + 1).as_bytes();
    let s = &s[..s.len().min(30)];
    let n = if s.first() == Some(&b'#') {
        if matches!(s.get(1), Some(b'x' | b'X')) {
            let k = s[2..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
            if k > 0 {
                2 + k
            } else {
                1 + s[1..].iter().take_while(|b| b.is_ascii_digit()).count()
            }
        } else {
            1 + s[1..].iter().take_while(|b| b.is_ascii_digit()).count()
        }
    } else {
        s.iter()
            .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
            .count()
    };
    let corps_ok = if s.first() == Some(&b'#') {
        n > 1
    } else {
        n > 0
    };
    if corps_ok && s.get(n) == Some(&b';') {
        return cx.ajouter(Elt::feuille(Genre::Entity, start, start + 2 + n));
    }
    None
}

fn code_en_ligne(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 96 || (start != 0 && cx.car_avant(start) == 96) {
        return None;
    }
    let mut pos = start + 1;
    while pos < cx.fin() && cx.car(pos) == 96 {
        pos += 1;
    }
    let taille = pos - start;
    let mut courant = 0;
    while pos < cx.fin() {
        if cx.car(pos) == 96 {
            courant += 1;
            if courant == taille && cx.car(pos + 1) != 96 {
                return cx.ajouter(Elt::avec(
                    Genre::InlineCode,
                    start,
                    pos + 1,
                    vec![
                        Elt::feuille(Genre::CodeMark, start, start + taille),
                        Elt::feuille(Genre::CodeMark, pos + 1 - taille, pos + 1),
                    ],
                ));
            }
        } else {
            courant = 0;
        }
        pos += 1;
    }
    None
}

static AUTOLIEN_CHEVRONS: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"(?i)^(?:[a-z][-A-Za-z0-9_+.]+:[^{JS_S}>]+|[a-z0-9.!#$%&'*+/=?^_`{{|}}~-]+@[a-z0-9](?:[a-z0-9-]{{0,61}}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{{0,61}}[a-z0-9])?)*)>"
    ))
});
static COMMENTAIRE: LazyLock<Regex> = LazyLock::new(|| re(r"^!--[^>](?:-[^-]|[^-])*?-->"));
static INSTRUCTION: LazyLock<Regex> = LazyLock::new(|| re(r"^\?(?s:.)*?\?>"));
static BALISE: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r#"^(?:![A-Z](?s:.)*?>|!\[CDATA\[(?s:.)*?\]\]>|/[{JS_S}]*[a-zA-Z][A-Za-z0-9_-]*[{JS_S}]*>|[{JS_S}]*[a-zA-Z][A-Za-z0-9_-]*([{JS_S}]+[a-zA-Z:_][A-Za-z0-9_\-.:]*(?:[{JS_S}]*=[{JS_S}]*(?:[^{JS_S}"'=<>`]+|'[^']*'|"[^"]*"))?)*[{JS_S}]*(/[{JS_S}]*)?>)"#
    ))
});

fn balise_html(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 60 || start + 1 == cx.fin() {
        return None;
    }
    let apres = cx.reste(start + 1);
    if let Some(m) = AUTOLIEN_CHEVRONS.find(apres) {
        let n = m.end();
        return cx.ajouter(Elt::avec(
            Genre::Autolink,
            start,
            start + 1 + n,
            vec![
                Elt::feuille(Genre::LinkMark, start, start + 1),
                Elt::feuille(Genre::URL, start + 1, start + n),
                Elt::feuille(Genre::LinkMark, start + n, start + 1 + n),
            ],
        ));
    }
    if let Some(m) = COMMENTAIRE.find(apres) {
        return cx.ajouter(Elt::feuille(Genre::Comment, start, start + 1 + m.end()));
    }
    if let Some(m) = INSTRUCTION.find(apres) {
        return cx.ajouter(Elt::feuille(
            Genre::ProcessingInstruction,
            start,
            start + 1 + m.end(),
        ));
    }
    let m = BALISE.find(apres)?;
    cx.ajouter(Elt::feuille(Genre::HTMLTag, start, start + 1 + m.end()))
}

/// Fragment : `$…$` (InlineMath) et `$$…$$` au milieu d'une ligne (InlineDisplayMath), par délimiteurs.
/// Ouvre si le caractère suivant n'est pas un espace, ferme si le précédent n'en est pas un : ainsi
/// « 5 $ et 10 $ » ne fait pas de maths. Hors de la section, À LA FIN, compte comme un espace.
fn maths_en_ligne(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    if c != 36 {
        return None;
    }
    let est_espace = |x: i32| x == -1 || espace(x);
    if cx.car(pos + 1) == 36 {
        let ouvre = !est_espace(cx.car(pos + 2));
        let ferme = !est_espace(cx.car_avant(pos));
        if !ouvre && !ferme {
            return None;
        }
        return cx.delimiteur(TypeDelim::MathsDisplay, pos, pos + 2, ouvre, ferme);
    }
    let ouvre = !est_espace(cx.car(pos + 1));
    let ferme = !est_espace(cx.car_avant(pos));
    if !ouvre && !ferme {
        return None;
    }
    cx.delimiteur(TypeDelim::Maths, pos, pos + 1, ouvre, ferme)
}

fn emphase(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 95 && c != 42 {
        return None;
    }
    let mut pos = start + 1;
    while cx.car(pos) == c {
        pos += 1;
    }
    let (avant, apres) = (cx.voisin_avant(start), cx.voisin_apres(pos));
    let (p_avant, p_apres) = (avant.ponctuation(), apres.ponctuation());
    let (s_avant, s_apres) = (avant.espace(), apres.espace());
    let flanc_gauche = !s_apres && (!p_apres || s_avant || p_avant);
    let flanc_droit = !s_avant && (!p_avant || s_apres || p_apres);
    let ouvre = flanc_gauche && (c == 42 || !flanc_droit || p_avant);
    let ferme = flanc_droit && (c == 42 || !flanc_gauche || p_apres);
    let genre = if c == 95 {
        TypeDelim::EmphaseSouligne
    } else {
        TypeDelim::EmphaseEtoile
    };
    cx.delimiteur(genre, start, pos, ouvre, ferme)
}

/// Fragment : `==surligné==`, un délimiteur de PAIRE qui ouvre et ferme à la fois (comme `~~`).
fn surligne(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    if c != 61 || cx.car(pos + 1) != 61 {
        return None;
    }
    cx.delimiteur(TypeDelim::Surligne, pos, pos + 2, true, true)
}

fn barre(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    if c != 126 || cx.car(pos + 1) != 126 || cx.car(pos + 2) == 126 {
        return None;
    }
    let (avant, apres) = (cx.voisin_avant(pos), cx.voisin_apres(pos + 2));
    let (s_avant, s_apres) = (avant.espace(), apres.espace());
    let (p_avant, p_apres) = (avant.ponctuation(), apres.ponctuation());
    cx.delimiteur(
        TypeDelim::Barre,
        pos,
        pos + 2,
        !s_apres && (!p_apres || s_avant || p_avant),
        !s_avant && (!p_avant || s_apres || p_apres),
    )
}

fn retour_force(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c == 92 && cx.car(start + 1) == 10 {
        return cx.ajouter(Elt::feuille(Genre::HardBreak, start, start + 2));
    }
    if c == 32 {
        let mut pos = start + 1;
        while cx.car(pos) == 32 {
            pos += 1;
        }
        if cx.car(pos) == 10 && pos >= start + 2 {
            return cx.ajouter(Elt::feuille(Genre::HardBreak, start, pos + 1));
        }
    }
    None
}

/// Fragment : `[[cible]]` et `![[cible]]`, nœuds AUTONOMES posés d'un coup : on cherche le `]]` sur la
/// même ligne ; un `[[` imbriqué ou une cible vide font renoncer (le `[` redevient un lien ordinaire).
fn lien_interne(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    let embed = c == 33 && cx.car(pos + 1) == 91 && cx.car(pos + 2) == 91;
    let lien = c == 91 && cx.car(pos + 1) == 91;
    if !embed && !lien {
        return None;
    }
    let debut = pos + if embed { 3 } else { 2 };
    let mut i = debut;
    while i < cx.fin() {
        let x = cx.car(i);
        if x == 93 && cx.car(i + 1) == 93 {
            break;
        }
        if (x == 91 && cx.car(i + 1) == 91) || x == 10 {
            return None;
        }
        i += 1;
    }
    if i >= cx.fin() || i == debut {
        return None;
    }
    cx.ajouter(Elt::feuille(
        if embed { Genre::Embed } else { Genre::WikiLink },
        pos,
        i + 2,
    ))
}

fn debut_lien(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 91 {
        return None;
    }
    cx.delimiteur(TypeDelim::DebutLien, start, start + 1, true, false)
}

fn debut_image(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 33 || cx.car(start + 1) != 91 {
        return None;
    }
    cx.delimiteur(TypeDelim::DebutImage, start, start + 2, true, false)
}

static AUTOLIEN_DEBUT: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(?:(www\.)|(https?://)|([A-Za-z0-9_.+-]{1,100}@)|(mailto:|xmpp:))"));
static URL_GFM: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)+(:[0-9]+)?(/[^{JS_S}<]*)?"
    ))
});
static DEUX_DERNIERS_MOTS: LazyLock<Regex> =
    LazyLock::new(|| re(r"[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+($|[/:])"));
static COURRIEL: LazyLock<Regex> =
    LazyLock::new(|| re(r"^[A-Za-z0-9_.+-]+@[A-Za-z0-9_-]+(\.[A-Za-z0-9_.-]+)+"));
static RESSOURCE_XMPP: LazyLock<Regex> = LazyLock::new(|| re(r"^/[a-zA-Z0-9@.]+"));
static ENTITE_FINALE: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)&(?:#[0-9]+|#x[a-f0-9]+|[A-Za-z0-9_]+);$"));

/// Fin d'une URL GFM commençant à `from` (indices dans `t`), -1 si aucune.
fn fin_url(t: &str, from: usize) -> Option<usize> {
    let m = URL_GFM.find(t.get(from..)?)?;
    let s = m.as_str();
    if DEUX_DERNIERS_MOTS
        .find(s)
        .is_none_or(|d| d.as_str().contains('_'))
    {
        return None;
    }
    let mut end = from + m.end();
    let b = t.as_bytes();
    while end > from {
        let dernier = b[end - 1];
        let compte = |ch: u8| b[from..end].iter().filter(|&&x| x == ch).count();
        if b"?!.,:*_~".contains(&dernier) || (dernier == b')' && compte(b')') > compte(b'(')) {
            end -= 1;
        } else if dernier == b';' {
            match ENTITE_FINALE.find(&t[from..end]) {
                Some(e) => end = from + e.start(),
                None => break,
            }
        } else {
            break;
        }
    }
    Some(end)
}

fn fin_courriel(t: &str, from: usize) -> Option<usize> {
    let m = COURRIEL.find(t.get(from..)?)?;
    let dernier = m.as_str().as_bytes()[m.end() - 1];
    if dernier == b'_' || dernier == b'-' {
        return None;
    }
    Some(from + m.end() - usize::from(dernier == b'.'))
}

/// GFM : URL nue (`www.`, `http(s)://`, courriel, `mailto:`/`xmpp:`) → un nœud `URL`.
fn autolien(cx: &mut Ctx, _c: i32, abs: usize) -> Option<usize> {
    let pos = abs - cx.offset;
    let t = cx.texte;
    let b = t.as_bytes();
    if !t.is_char_boundary(pos)
        || (pos > 0 && (b[pos - 1].is_ascii_alphanumeric() || b[pos - 1] == b'_'))
    {
        return None;
    }
    let caps = AUTOLIEN_DEBUT.captures(&t[pos..])?;
    let n = caps.get(0).map_or(0, |m| m.end());
    let end = if caps.get(1).is_some() || caps.get(2).is_some() {
        let mut end = fin_url(t, pos + n)?;
        if cx.a_un_lien_ouvert() {
            // Pas de crochet non apparié dans l'URL quand un lien est ouvert : `([^\[\]]|\[[^\]]*\])*`.
            let s = &b[pos..end];
            let mut k = 0;
            while k < s.len() {
                match s[k] {
                    b'[' => match s[k + 1..].iter().position(|&x| x == b']') {
                        Some(f) => k += f + 2,
                        None => break,
                    },
                    b']' => break,
                    _ => k += 1,
                }
            }
            end = pos + k;
        }
        end
    } else if caps.get(3).is_some() {
        fin_courriel(t, pos)?
    } else {
        let mut end = fin_courriel(t, pos + n)?;
        if caps.get(4).is_some_and(|m| m.as_str() == "xmpp:")
            && let Some(r) = RESSOURCE_XMPP.find(&t[end..])
        {
            end += r.end();
        }
        end
    };
    cx.ajouter(Elt::feuille(Genre::URL, abs, end + cx.offset))
}

/// Indice `~x~` et exposant `^x^` (style Pandoc) : posés d'un coup, sans espace dedans.
fn indice_exposant(
    cx: &mut Ctx,
    c: i32,
    pos: usize,
    ch: i32,
    genre: Genre,
    marque: Genre,
) -> Option<usize> {
    if c != ch || cx.car(pos + 1) == ch {
        return None;
    }
    let mut elts = vec![Elt::feuille(marque, pos, pos + 1)];
    let mut i = pos + 1;
    while i < cx.fin() {
        let x = cx.car(i);
        if x == ch {
            elts.push(Elt::feuille(marque, i, i + 1));
            return cx.ajouter(Elt::avec(genre, pos, i + 1, elts));
        }
        if x == 92 {
            elts.push(Elt::feuille(Genre::Escape, i, i + 2));
            i += 1;
        }
        if espace(x) {
            break;
        }
        i += 1;
    }
    None
}

fn indice(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    indice_exposant(cx, c, pos, 126, Genre::Subscript, Genre::SubscriptMark)
}

fn exposant(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    indice_exposant(cx, c, pos, 94, Genre::Superscript, Genre::SuperscriptMark)
}

fn emoji(cx: &mut Ctx, c: i32, pos: usize) -> Option<usize> {
    if c != 58 {
        return None;
    }
    let s = cx.reste(pos + 1).as_bytes();
    let n = s
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .count();
    if n == 0 || s.get(n) != Some(&b':') {
        return None;
    }
    cx.ajouter(Elt::feuille(Genre::Emoji, pos, pos + n + 2))
}

fn fin_lien(cx: &mut Ctx, c: i32, start: usize) -> Option<usize> {
    if c != 93 {
        return None;
    }
    let mut i = cx.parts.len();
    while i > 0 {
        i -= 1;
        let Part::Delim(d) = &cx.parts[i] else {
            continue;
        };
        if !matches!(d.genre, TypeDelim::DebutLien | TypeDelim::DebutImage) {
            continue;
        }
        let d = *d;
        // Ouvrant invalidé (lien imbriqué), ou `[]` vide sans `(`/`[` derrière : on oublie les deux.
        let suivant = cx.car(start + 1);
        if d.cote == 0 || (cx.sauter_espaces(d.to) == start && suivant != 40 && suivant != 91) {
            cx.parts[i] = Part::Vide;
            return None;
        }
        let contenu = cx.resoudre(i);
        cx.parts.truncate(i);
        let genre = if d.genre == TypeDelim::DebutLien {
            Genre::Link
        } else {
            Genre::Image
        };
        let lien = finir_lien(cx, contenu, genre, d.from, start + 1);
        let to = lien.to;
        cx.parts.push(Part::Elt(lien));
        if d.genre == TypeDelim::DebutLien {
            for p in &mut cx.parts[..i] {
                if let Part::Delim(q) = p
                    && q.genre == TypeDelim::DebutLien
                {
                    q.cote = 0;
                }
            }
        }
        return Some(to);
    }
    None
}

fn finir_lien(
    cx: &Ctx,
    mut contenu: Vec<Elt>,
    genre: Genre,
    start: usize,
    pos_debut: usize,
) -> Elt {
    let suivant = cx.car(pos_debut);
    let mut fin = pos_debut;
    contenu.insert(
        0,
        Elt::feuille(
            Genre::LinkMark,
            start,
            start + if genre == Genre::Image { 2 } else { 1 },
        ),
    );
    contenu.push(Elt::feuille(Genre::LinkMark, pos_debut - 1, pos_debut));
    if suivant == 40 {
        let mut pos = cx.sauter_espaces(pos_debut + 1);
        let dest = analyser_url(cx.texte, pos - cx.offset, cx.offset);
        let mut titre = None;
        if let Some(Some(d)) = &dest {
            pos = cx.sauter_espaces(d.to);
            if pos != d.to {
                titre = analyser_titre(cx.texte, pos - cx.offset, cx.offset).flatten();
                if let Some(t) = &titre {
                    pos = cx.sauter_espaces(t.to);
                }
            }
        }
        if cx.car(pos) == 41 {
            contenu.push(Elt::feuille(Genre::LinkMark, pos_debut, pos_debut + 1));
            fin = pos + 1;
            if let Some(Some(d)) = dest {
                contenu.push(d);
            }
            if let Some(t) = titre {
                contenu.push(t);
            }
            contenu.push(Elt::feuille(Genre::LinkMark, pos, fin));
        }
    } else if suivant == 91
        && let Some(Some(l)) = analyser_etiquette(cx.texte, pos_debut - cx.offset, cx.offset, false)
    {
        fin = l.to;
        contenu.push(l);
    }
    Elt::avec(genre, start, fin, contenu)
}

// ★ Les trois fonctions suivantes rendent `None` quand elles tombent au bout du texte (le `null` de
//   Lezer : la définition de lien peut continuer à la ligne suivante) et `Some(None)` quand l'analyse
//   échoue (`false`).

pub(crate) fn analyser_url(texte: &str, start: usize, offset: usize) -> Option<Option<Elt>> {
    let b = texte.as_bytes();
    if start >= b.len() {
        return None;
    }
    if b[start] == b'<' {
        for (pos, &ch) in b.iter().enumerate().skip(start + 1) {
            if ch == b'>' {
                return Some(Some(Elt::feuille(
                    Genre::URL,
                    start + offset,
                    pos + 1 + offset,
                )));
            }
            if ch == b'<' || ch == b'\n' {
                return Some(None);
            }
        }
        return None;
    }
    let (mut profondeur, mut pos, mut echappe) = (0usize, start, false);
    while pos < b.len() {
        let ch = b[pos];
        if espace(i32::from(ch)) {
            break;
        } else if echappe {
            echappe = false;
        } else if ch == b'(' {
            profondeur += 1;
        } else if ch == b')' {
            if profondeur == 0 {
                break;
            }
            profondeur -= 1;
        } else if ch == b'\\' {
            echappe = true;
        }
        pos += 1;
    }
    if pos > start {
        Some(Some(Elt::feuille(Genre::URL, start + offset, pos + offset)))
    } else if pos == b.len() {
        None
    } else {
        Some(None)
    }
}

pub(crate) fn analyser_titre(texte: &str, start: usize, offset: usize) -> Option<Option<Elt>> {
    let b = texte.as_bytes();
    let Some(&premier) = b.get(start) else {
        return Some(None);
    };
    if premier != b'\'' && premier != b'"' && premier != b'(' {
        return Some(None);
    }
    let fin = if premier == b'(' { b')' } else { premier };
    let mut echappe = false;
    for (pos, &ch) in b.iter().enumerate().skip(start + 1) {
        if echappe {
            echappe = false;
        } else if ch == fin {
            return Some(Some(Elt::feuille(
                Genre::LinkTitle,
                start + offset,
                pos + 1 + offset,
            )));
        } else if ch == b'\\' {
            echappe = true;
        }
    }
    None
}

pub(crate) fn analyser_etiquette(
    texte: &str,
    start: usize,
    offset: usize,
    mut non_vide: bool,
) -> Option<Option<Elt>> {
    let b = texte.as_bytes();
    let fin = b.len().min(start + 1 + 999);
    let mut echappe = false;
    for (pos, &ch) in b.iter().enumerate().take(fin).skip(start + 1) {
        if echappe {
            echappe = false;
        } else if ch == b']' {
            return Some(if non_vide {
                None
            } else {
                Some(Elt::feuille(
                    Genre::LinkLabel,
                    start + offset,
                    pos + 1 + offset,
                ))
            });
        } else {
            if non_vide && !espace(i32::from(ch)) {
                non_vide = false;
            }
            if ch == b'[' {
                return Some(None);
            } else if ch == b'\\' {
                echappe = true;
            }
        }
    }
    None
}

type Analyseur = fn(&mut Ctx, i32, usize) -> Option<usize>;

/// ★ L'ordre de Fragment, recalculé à partir des `before`/`after` de Lezer (base CommonMark, puis
///   `markdownLanguage` : GFM, indice, exposant, émoji ; puis les extensions de `language.ts`). Un
///   analyseur sans position est inséré AVANT le dernier (`LinkEnd`), d'où sa place en fin de liste.
const ANALYSEURS: &[Analyseur] = &[
    echappement,
    entite,
    code_en_ligne,
    balise_html,
    maths_en_ligne,
    emphase,
    surligne,
    barre,
    retour_force,
    lien_interne,
    debut_lien,
    debut_image,
    autolien,
    indice,
    exposant,
    emoji,
    fin_lien,
];

/// Analyse en ligne de `texte`, qui commence à la position absolue `offset` du document.
pub(crate) fn analyser(texte: &str, offset: usize) -> Vec<Elt> {
    let mut cx = Ctx {
        texte,
        offset,
        parts: Vec::new(),
    };
    let mut pos = offset;
    'externe: while pos < cx.fin() {
        let c = cx.car(pos);
        for a in ANALYSEURS {
            if let Some(fin) = a(&mut cx, c, pos) {
                pos = fin;
                continue 'externe;
            }
        }
        pos += 1;
    }
    cx.resoudre(0)
}

/// Injecte les marques de conteneur (`>` des citations) dans les éléments, au plus profond de
/// celui qui les contient. `injectMarks` de Lezer.
pub(crate) fn injecter_marques(elements: Vec<Elt>, marques: &[Elt]) -> Vec<Elt> {
    if marques.is_empty() {
        return elements;
    }
    if elements.is_empty() {
        return marques.to_vec();
    }
    let mut elts = elements;
    let mut e_i = 0;
    for m in marques {
        while e_i < elts.len() && elts[e_i].to < m.to {
            e_i += 1;
        }
        if e_i < elts.len() && elts[e_i].from < m.from {
            let e = &mut elts[e_i];
            let enfants = std::mem::take(&mut e.enfants);
            e.enfants = injecter_marques(enfants, std::slice::from_ref(m));
        } else {
            elts.insert(e_i, m.clone());
            e_i += 1;
        }
    }
    elts
}
