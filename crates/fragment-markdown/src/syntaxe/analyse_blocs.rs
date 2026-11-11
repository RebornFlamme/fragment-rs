// Port de app/src/ui/views/markdown/language.ts (BlockMath : `$$` en début de ligne, qui interrompt
// un paragraphe) et de l'analyse de blocs de @lezer/markdown 1.7.2 (MIT, © Marijn Haverbeke et
// contributeurs ; CommonMark, tableaux et tâches GFM). Traduction, voir syntaxe/mod.rs.
//! Analyse de BLOCS, ligne par ligne, comme Lezer.
//!
//! Une pile de blocs composites (document, citation, liste, élément de liste) ; chaque ligne lue
//! commence par « sauter » le balisage des composites ouverts (`>` d'une citation, indentation d'un
//! élément) ; ce qui reste est essayé par les analyseurs de blocs (`BLOCS`, dans l'ordre de Fragment),
//! sinon commence un bloc FEUILLE (paragraphe, tableau, titre Setext, définition de lien, tâche) qui
//! court jusqu'à une ligne vide ou un bloc qui l'interrompt (`interrompt_feuille`).
//!
//! ★ Positions en octets. Les lignes « nettoyées » (`Ligne::nettoyer`, le `scrub` de Lezer)
//!   remplacent le balisage des conteneurs par des espaces : même longueur, donc les positions de
//!   l'analyse en ligne restent celles du document.

use std::borrow::Cow;
use std::cell::Cell;
use std::sync::LazyLock;

use regex::Regex;

use super::en_ligne::{
    self, Elt, analyser_etiquette, analyser_titre, analyser_url, espace, injecter_marques,
};
use super::genre::Genre;

fn re(motif: &str) -> Regex {
    Regex::new(motif).unwrap_or_else(|e| unreachable!("regex constante : {e}"))
}

/// La classe `\s` de JS.
const JS_S: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

// ─── Ligne ───────────────────────────────────────────────────────────────────

/// La ligne en cours d'analyse (le `Line` de Lezer). Positions relatives au début de la ligne.
struct Ligne<'a> {
    texte: &'a str,
    /// Indentation (en colonnes) déjà prise par les composites.
    base_indent: usize,
    /// Position correspondant à `base_indent`.
    base_pos: usize,
    /// Nombre de composites traversés.
    profondeur: usize,
    /// Marques de composites (`>`) lues sur cette ligne.
    marqueurs: Vec<Elt>,
    /// Prochain caractère non blanc au-delà du balisage.
    pos: usize,
    /// Sa colonne.
    indent: usize,
    /// Le caractère à `pos` (-1 en fin de ligne).
    suivant: i32,
}

impl<'a> Ligne<'a> {
    fn car(&self, i: usize) -> i32 {
        self.texte.as_bytes().get(i).map_or(-1, |&b| i32::from(b))
    }
    fn len(&self) -> usize {
        self.texte.len()
    }
    fn sauter_espaces(&self, from: usize) -> usize {
        sauter_espaces(self.texte, from)
    }
    fn avancer(&mut self) {
        if self.base_pos > self.pos {
            self.avancer_dedans();
        }
    }
    fn avancer_dedans(&mut self) {
        let nouveau = self.sauter_espaces(self.base_pos);
        self.indent = self.compter_indent(nouveau, self.pos, self.indent);
        self.pos = nouveau;
        self.suivant = if nouveau == self.len() {
            -1
        } else {
            self.car(nouveau)
        };
    }
    fn reinitialiser(&mut self, texte: &'a str) {
        self.texte = texte;
        self.base_indent = 0;
        self.base_pos = 0;
        self.pos = 0;
        self.indent = 0;
        self.avancer_dedans();
        self.profondeur = 1;
        self.marqueurs.clear();
    }
    fn deplacer_base(&mut self, to: usize) {
        self.base_pos = to;
        self.base_indent = self.compter_indent(to, self.pos, self.indent);
    }
    fn deplacer_base_colonne(&mut self, indent: usize) {
        self.base_indent = indent;
        self.base_pos = self.trouver_colonne(indent);
    }
    /// ★ Une tabulation avance à la prochaine colonne multiple de 4.
    fn compter_indent(&self, to: usize, from: usize, mut indent: usize) -> usize {
        for &b in self.texte.as_bytes().get(from..to).unwrap_or(&[]) {
            indent += if b == 9 { 4 - indent % 4 } else { 1 };
        }
        indent
    }
    fn trouver_colonne(&self, but: usize) -> usize {
        let b = self.texte.as_bytes();
        let (mut i, mut indent) = (0, 0);
        while i < b.len() && indent < but {
            indent += if b[i] == 9 { 4 - indent % 4 } else { 1 };
            i += 1;
        }
        i
    }
    /// Le texte de la ligne, balisage des composites remplacé par des espaces (même longueur).
    fn nettoyer(&self) -> Cow<'a, str> {
        if self.base_indent == 0 {
            return Cow::Borrowed(self.texte);
        }
        let n = self.base_pos.min(self.len());
        let mut s = " ".repeat(n);
        s.push_str(self.texte.get(n..).unwrap_or(""));
        Cow::Owned(s)
    }
}

pub(crate) fn sauter_espaces(t: &str, mut i: usize) -> usize {
    let b = t.as_bytes();
    while i < b.len() && espace(i32::from(b[i])) {
        i += 1;
    }
    i
}

fn sauter_espaces_arriere(t: &str, mut i: usize, to: usize) -> usize {
    let b = t.as_bytes();
    while i > to && espace(i32::from(b[i - 1])) {
        i -= 1;
    }
    i
}

// ─── Reconnaissance des débuts de blocs ─────────────────────────────────────

fn est_code_cloture(l: &Ligne) -> Option<usize> {
    if l.suivant != 96 && l.suivant != 126 {
        return None;
    }
    let mut pos = l.pos + 1;
    while pos < l.len() && l.car(pos) == l.suivant {
        pos += 1;
    }
    if pos < l.pos + 3 {
        return None;
    }
    if l.suivant == 96 && l.texte.as_bytes()[pos..].contains(&b'`') {
        return None;
    }
    Some(pos)
}

fn est_citation(l: &Ligne) -> Option<usize> {
    if l.suivant != 62 {
        return None;
    }
    Some(if l.car(l.pos + 1) == 32 { 2 } else { 1 })
}

fn est_titre_atx(l: &Ligne) -> Option<usize> {
    if l.suivant != 35 {
        return None;
    }
    let mut pos = l.pos + 1;
    while pos < l.len() && l.car(pos) == 35 {
        pos += 1;
    }
    if pos < l.len() && l.car(pos) != 32 {
        return None;
    }
    let taille = pos - l.pos;
    (taille <= 6).then_some(taille)
}

fn est_soulignement_setext(l: &Ligne) -> Option<usize> {
    if (l.suivant != 45 && l.suivant != 61) || l.indent >= l.base_indent + 4 {
        return None;
    }
    let mut pos = l.pos + 1;
    while pos < l.len() && l.car(pos) == l.suivant {
        pos += 1;
    }
    let fin = pos;
    while pos < l.len() && espace(l.car(pos)) {
        pos += 1;
    }
    (pos == l.len()).then_some(fin)
}

static FIN_LIGNE_VIDE: LazyLock<Regex> = LazyLock::new(|| re(r"^[ \t]*$"));
static FIN_COMMENTAIRE: LazyLock<Regex> = LazyLock::new(|| re(r"-->"));
static FIN_INSTRUCTION: LazyLock<Regex> = LazyLock::new(|| re(r"\?>"));

/// Les sept styles de blocs HTML de CommonMark : (début, fin).
static STYLES_HTML: LazyLock<Vec<(Regex, &'static LazyLock<Regex>)>> = LazyLock::new(|| {
    static FIN_SCRIPT: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)</(?:script|pre|style)>"));
    static FIN_DECLARATION: LazyLock<Regex> = LazyLock::new(|| re(r">"));
    static FIN_CDATA: LazyLock<Regex> = LazyLock::new(|| re(r"\]\]>"));
    vec![
        (
            re(&format!(r"(?i)^<(?:script|pre|style)(?:[{JS_S}]|>|$)")),
            &FIN_SCRIPT,
        ),
        (re(&format!(r"^[{JS_S}]*<!--")), &FIN_COMMENTAIRE),
        (re(&format!(r"^[{JS_S}]*<\?")), &FIN_INSTRUCTION),
        (re(&format!(r"(?i)^[{JS_S}]*<![A-Z]")), &FIN_DECLARATION),
        (re(&format!(r"^[{JS_S}]*<!\[CDATA\[")), &FIN_CDATA),
        (
            re(&format!(
                r"(?i)^[{JS_S}]*</?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h1|h2|h3|h4|h5|h6|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|section|source|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?:[{JS_S}]|/?>|$)"
            )),
            &FIN_LIGNE_VIDE,
        ),
        (
            re(&format!(
                r#"(?i)^[{JS_S}]*(?:</[a-z][A-Za-z0-9_-]*[{JS_S}]*>|<[a-z][A-Za-z0-9_-]*([{JS_S}]+[a-z:_][A-Za-z0-9_\-.]*(?:[{JS_S}]*=[{JS_S}]*(?:[^{JS_S}"'=<>`]+|'[^']*'|"[^"]*"))?)*[{JS_S}]*>)[{JS_S}]*$"#
            )),
            &FIN_LIGNE_VIDE,
        ),
    ]
});

static LIGNE_DELIMITEUR: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"^[>{JS_S}]*\|?([{JS_S}]*:?-+:?[{JS_S}]*\|)+([{JS_S}]*:?-+:?[{JS_S}]*)?$"
    ))
});

fn indent_de_liste(l: &Ligne, pos: usize) -> usize {
    let apres = l.compter_indent(pos, l.pos, l.indent);
    let saute = l.sauter_espaces(pos);
    let indente = l.compter_indent(saute, pos, apres);
    if indente >= apres + 5 || saute == l.len() {
        apres + 1
    } else {
        indente
    }
}

fn ajouter_texte_code(marques: &mut Vec<Elt>, from: usize, to: usize) {
    if let Some(m) = marques.last_mut()
        && m.to == from
        && m.genre == Genre::CodeText
    {
        m.to = to;
        return;
    }
    marques.push(Elt::feuille(Genre::CodeText, from, to));
}

/// `hasPipe` : une barre verticale non échappée à partir de `start`.
fn a_une_barre(s: &str, start: usize) -> bool {
    let b = s.as_bytes();
    let mut i = start;
    while i < b.len() {
        if b[i] == b'|' {
            return true;
        }
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    false
}

/// Une rangée de tableau : nombre de cellules ; pousse cellules et délimiteurs dans `elts` si donné.
fn analyser_rangee(ligne: &str, debut: usize, elts: Option<&mut Vec<Elt>>, offset: usize) -> usize {
    let b = ligne.as_bytes();
    let (mut compte, mut premier, mut echappe) = (0, true, false);
    let (mut debut_cellule, mut fin_cellule): (Option<usize>, usize) = (None, 0);
    let mut elts = elts;
    let cellule = |elts: &mut Vec<Elt>, a: usize, z: usize| {
        let enfants = en_ligne::analyser(&ligne[a..z], offset + a);
        elts.push(Elt::avec(Genre::TableCell, offset + a, offset + z, enfants));
    };
    for (i, &c) in b.iter().enumerate().skip(debut) {
        if c == b'|' && !echappe {
            if !premier || debut_cellule.is_some() {
                compte += 1;
            }
            premier = false;
            if let Some(e) = elts.as_deref_mut() {
                if let Some(a) = debut_cellule {
                    cellule(e, a, fin_cellule);
                }
                e.push(Elt::feuille(
                    Genre::TableDelimiter,
                    i + offset,
                    i + offset + 1,
                ));
            }
            debut_cellule = None;
        } else if echappe || (c != 32 && c != 9) {
            if debut_cellule.is_none() {
                debut_cellule = Some(i);
            }
            fin_cellule = i + 1;
        }
        echappe = !echappe && c == 92;
    }
    if let Some(a) = debut_cellule {
        compte += 1;
        if let Some(e) = elts {
            cellule(e, a, fin_cellule);
        }
    }
    compte
}

// ─── Blocs composites et feuilles ───────────────────────────────────────────

struct Composite {
    genre: Genre,
    /// Indentation pour un élément de liste ; caractère de balisage pour une liste.
    valeur: usize,
    from: usize,
    end: usize,
    enfants: Vec<Elt>,
}

impl Composite {
    fn en_elt(self, end: usize) -> Elt {
        let end = self
            .enfants
            .last()
            .map_or(end.max(self.end), |e| end.max(self.end).max(e.to));
        Elt::avec(self.genre, self.from, end, self.enfants)
    }
}

/// État de l'analyse d'une définition de lien `[étiquette]: url "titre"`.
struct RefLien {
    etape: i8,
    elts: Vec<Elt>,
    pos: usize,
    start: usize,
}

impl RefLien {
    fn suivante(&mut self, e: Option<Option<Elt>>) -> bool {
        match e {
            Some(Some(e)) => {
                self.pos = e.to - self.start;
                self.elts.push(e);
                self.etape += 1;
                true
            }
            Some(None) => {
                self.etape = -1;
                false
            }
            None => false,
        }
    }

    /// Avance sur `contenu` ; rend la longueur d'une définition complète, -1 sinon.
    fn avancer(&mut self, contenu: &str) -> isize {
        loop {
            match self.etape {
                0 => {
                    if !self.suivante(analyser_etiquette(contenu, self.pos, self.start, true)) {
                        return -1;
                    }
                    if contenu.as_bytes().get(self.pos) != Some(&b':') {
                        self.etape = -1;
                        return -1;
                    }
                    self.elts.push(Elt::feuille(
                        Genre::LinkMark,
                        self.pos + self.start,
                        self.pos + self.start + 1,
                    ));
                    self.pos += 1;
                }
                1 => {
                    let p = sauter_espaces(contenu, self.pos);
                    if !self.suivante(analyser_url(contenu, p, self.start)) {
                        return -1;
                    }
                }
                2 => {
                    let saut = sauter_espaces(contenu, self.pos);
                    let mut fin = 0isize;
                    if saut > self.pos
                        && let Some(Some(t)) = analyser_titre(contenu, saut, self.start)
                    {
                        let fin_titre = fin_de_ligne(contenu, t.to - self.start);
                        if fin_titre > 0 {
                            self.suivante(Some(Some(t)));
                            fin = fin_titre;
                        }
                    }
                    if fin == 0 {
                        fin = fin_de_ligne(contenu, self.pos);
                    }
                    return if fin > 0 && (fin as usize) < contenu.len() {
                        fin
                    } else {
                        -1
                    };
                }
                3 => return fin_de_ligne(contenu, self.pos),
                _ => return -1,
            }
        }
    }
}

fn fin_de_ligne(t: &str, mut pos: usize) -> isize {
    let b = t.as_bytes();
    while pos < b.len() {
        if b[pos] == 10 {
            break;
        }
        if !espace(i32::from(b[pos])) {
            return -1;
        }
        pos += 1;
    }
    pos as isize
}

enum ParseurFeuille {
    Reference(RefLien),
    /// `None` : deuxième ligne pas encore vue ; `Some(None)` : pas un tableau ; `Some(Some(r))` : rangées.
    Tableau(Option<Option<Vec<Elt>>>),
    Setext,
    Tache,
}

struct Feuille {
    start: usize,
    contenu: String,
    marques: Vec<Elt>,
    parseurs: Vec<ParseurFeuille>,
}

/// Résultat d'un analyseur de bloc.
enum Res {
    /// Ne s'applique pas.
    Non,
    /// A consommé un bloc feuille.
    Feuille,
    /// A ouvert un composite : la suite de la ligne est réessayée.
    Contexte,
}

struct Analyse<'a> {
    doc: &'a str,
    ligne: Ligne<'a>,
    debut_ligne: usize,
    fin_ligne_abs: usize,
    au_bout: bool,
    pile: Vec<Composite>,
    /// Borne (exclue) de tout ce que l'analyse a LU du document jusqu'ici : la fin de la dernière
    /// ligne lue, plus son saut de ligne (ou la fin du document, qui est aussi une lecture).
    /// ★ Sert à l'incrémental : un repère dont `lu` précède la modification ne dépend pas d'elle.
    lu: Cell<usize>,
}

/// Un point de reprise de l'analyse (incrémental, `incremental.rs`) : au début d'un pas, seule la
/// racine `Document` est ouverte et la ligne en cours commence en `ligne`. De là, la suite de
/// l'analyse ne dépend QUE de `doc[ligne..]` (aucune lecture en arrière, aucun état hérité).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Repere {
    /// Début (octets) de la ligne en cours.
    pub ligne: usize,
    /// Nombre d'enfants de `Document` déjà produits par cette analyse.
    pub enfants: usize,
    /// `Analyse::lu` à ce moment.
    pub lu: usize,
}

/// Comment s'est terminée une analyse partielle.
pub(crate) enum Fin {
    /// Au bout du document : la fin (octets) de la racine `Document`.
    Document(usize),
    /// Arrêtée au dernier repère, que `arret` a accepté.
    Arret,
}

/// Le produit de [`analyser_depuis`].
pub(crate) struct Resultat {
    /// Les enfants de `Document` produits depuis la reprise.
    pub enfants: Vec<Elt>,
    pub reperes: Vec<Repere>,
    pub fin: Fin,
}

impl<'a> Analyse<'a> {
    /// Une analyse qui reprend au début de ligne `ligne`, racine seule ouverte, avec `lu` déjà lu.
    fn depuis(doc: &'a str, ligne: usize, lu: usize) -> Self {
        let mut a = Analyse {
            doc,
            ligne: Ligne {
                texte: "",
                base_indent: 0,
                base_pos: 0,
                profondeur: 0,
                marqueurs: Vec::new(),
                pos: 0,
                indent: 0,
                suivant: -1,
            },
            debut_ligne: ligne,
            fin_ligne_abs: 0,
            au_bout: false,
            pile: vec![Composite {
                genre: Genre::Document,
                valeur: 0,
                from: 0,
                end: 0,
                enfants: Vec::new(),
            }],
            lu: Cell::new(lu),
        };
        a.lire_ligne();
        a
    }

    fn bloc(&mut self) -> &mut Composite {
        let n = self.pile.len() - 1;
        &mut self.pile[n]
    }

    fn dans_liste(&self, genre: Genre) -> bool {
        self.pile.iter().any(|c| c.genre == genre)
    }

    fn ajouter(&mut self, e: Elt) {
        self.bloc().enfants.push(e);
    }

    fn fin_ligne_prec(&self) -> usize {
        if self.au_bout {
            self.debut_ligne
        } else {
            self.debut_ligne - 1
        }
    }

    /// ★ La SEULE lecture du document : tient `lu` à jour.
    fn texte_ligne(&self, start: usize) -> &'a str {
        let t = if start >= self.doc.len() {
            ""
        } else {
            let reste = &self.doc[start..];
            reste.find('\n').map_or(reste, |i| &reste[..i])
        };
        self.lu.set(self.lu.get().max(start + t.len() + 1));
        t
    }

    fn ligne_suivante_brute(&self) -> &'a str {
        self.texte_ligne(self.fin_ligne_abs + 1)
    }

    /// Passe à la ligne suivante ; faux au bout du document.
    fn ligne_suivante(&mut self) -> bool {
        self.debut_ligne += self.ligne.len();
        if self.fin_ligne_abs >= self.doc.len() {
            self.au_bout = true;
            self.lire_ligne();
            false
        } else {
            self.debut_ligne += 1;
            self.lire_ligne();
            true
        }
    }

    fn lire_ligne(&mut self) {
        let texte = if self.au_bout {
            ""
        } else {
            self.texte_ligne(self.debut_ligne)
        };
        self.fin_ligne_abs = self.debut_ligne + texte.len();
        self.ligne.reinitialiser(texte);
        while self.ligne.profondeur < self.pile.len() {
            let d = self.ligne.profondeur;
            let avant = self.ligne.marqueurs.len();
            if !self.sauter_balisage(d) {
                if self.ligne.marqueurs.len() > avant
                    && let Some(m) = self.ligne.marqueurs.last()
                {
                    self.pile[d].end = m.to;
                }
                self.ligne.avancer();
                break;
            }
            self.ligne.avancer();
            self.ligne.profondeur += 1;
        }
    }

    /// Le balisage du composite à la profondeur `d` continue-t-il sur cette ligne ?
    fn sauter_balisage(&mut self, d: usize) -> bool {
        match self.pile[d].genre {
            Genre::Blockquote => {
                let l = &mut self.ligne;
                if l.suivant != 62 {
                    return false;
                }
                let p = self.debut_ligne + l.pos;
                l.marqueurs.push(Elt::feuille(Genre::QuoteMark, p, p + 1));
                let saut = if espace(l.car(l.pos + 1)) { 2 } else { 1 };
                l.deplacer_base(l.pos + saut);
                self.pile[d].end = self.debut_ligne + l.len();
                true
            }
            Genre::ListItem => {
                let v = self.pile[d].valeur;
                let l = &mut self.ligne;
                if l.indent < l.base_indent + v && l.suivant > -1 {
                    return false;
                }
                l.deplacer_base_colonne(l.base_indent + v);
                true
            }
            Genre::BulletList | Genre::OrderedList => self.sauter_pour_liste(d),
            _ => true,
        }
    }

    fn sauter_pour_liste(&self, d: usize) -> bool {
        let l = &self.ligne;
        if l.pos == l.len()
            || (d != self.pile.len() - 1
                && l.indent >= self.pile[l.profondeur + 1].valeur + l.base_indent)
        {
            return true;
        }
        if l.indent >= l.base_indent + 4 {
            return false;
        }
        let bl = &self.pile[d];
        let taille = if bl.genre == Genre::OrderedList {
            self.est_liste_ordonnee(false)
        } else {
            self.est_liste_puces(false)
        };
        let Some(taille) = taille else { return false };
        (bl.genre != Genre::BulletList || self.est_regle(false).is_none())
            && l.car(l.pos + taille - 1) == i32::try_from(bl.valeur).unwrap_or(-1)
    }

    fn est_regle(&self, interrompt: bool) -> Option<()> {
        let l = &self.ligne;
        if l.suivant != 42 && l.suivant != 45 && l.suivant != 95 {
            return None;
        }
        let mut compte = 1;
        for pos in l.pos + 1..l.len() {
            let c = l.car(pos);
            if c == l.suivant {
                compte += 1;
            } else if !espace(c) {
                return None;
            }
        }
        // ★ Un titre Setext l'emporte sur une règle `---` sous un paragraphe.
        if interrompt
            && l.suivant == 45
            && est_soulignement_setext(l).is_some()
            && l.profondeur == self.pile.len()
        {
            return None;
        }
        (compte >= 3).then_some(())
    }

    fn est_liste_puces(&self, interrompt: bool) -> Option<usize> {
        let l = &self.ligne;
        let ok = (l.suivant == 45 || l.suivant == 43 || l.suivant == 42)
            && (l.pos + 1 == l.len() || espace(l.car(l.pos + 1)))
            && (!interrompt
                || self.dans_liste(Genre::BulletList)
                || l.sauter_espaces(l.pos + 2) < l.len());
        ok.then_some(1)
    }

    fn est_liste_ordonnee(&self, interrompt: bool) -> Option<usize> {
        let l = &self.ligne;
        let (mut pos, mut c) = (l.pos, l.suivant);
        loop {
            if (48..=57).contains(&c) {
                pos += 1;
            } else {
                break;
            }
            if pos == l.len() {
                return None;
            }
            c = l.car(pos);
        }
        if pos == l.pos
            || pos > l.pos + 9
            || (c != 46 && c != 41)
            || (pos + 1 < l.len() && !espace(l.car(pos + 1)))
            || (interrompt
                && !self.dans_liste(Genre::OrderedList)
                && (l.sauter_espaces(pos + 1) == l.len() || pos > l.pos + 1 || l.suivant != 49))
        {
            return None;
        }
        Some(pos + 1 - l.pos)
    }

    fn est_bloc_html(&self, interrompt: bool) -> Option<usize> {
        let l = &self.ligne;
        if l.suivant != 60 {
            return None;
        }
        let reste = &l.texte[l.pos..];
        let n = STYLES_HTML.len() - usize::from(interrompt);
        STYLES_HTML[..n]
            .iter()
            .position(|(debut, _)| debut.is_match(reste))
    }

    fn est_maths_bloc(&self) -> bool {
        self.ligne.suivant == 36 && self.ligne.car(self.ligne.pos + 1) == 36
    }

    fn debut_contexte(&mut self, genre: Genre, start: usize, valeur: usize) {
        let from = self.debut_ligne + start;
        let end = self.debut_ligne + self.ligne.len();
        self.pile.push(Composite {
            genre,
            valeur,
            from,
            end,
            enfants: Vec::new(),
        });
    }

    fn finir_contexte(&mut self) {
        if let Some(c) = self.pile.pop() {
            let e = c.en_elt(0);
            self.ajouter(e);
        }
    }

    fn finir(&mut self) -> Elt {
        while self.pile.len() > 1 {
            self.finir_contexte();
        }
        let fin = self.debut_ligne;
        match self.pile.pop() {
            Some(doc) => doc.en_elt(fin),
            None => Elt::feuille(Genre::Document, 0, fin),
        }
    }

    // ─── Les analyseurs de blocs ─────────────────────────────────────────

    fn code_indente(&mut self) -> Res {
        let base = self.ligne.base_indent + 4;
        if self.ligne.indent < base {
            return Res::Non;
        }
        let start = self.ligne.trouver_colonne(base);
        let from = self.debut_ligne + start;
        let mut to = self.debut_ligne + self.ligne.len();
        let (mut marques, mut en_attente) = (Vec::new(), Vec::new());
        ajouter_texte_code(&mut marques, from, to);
        while self.ligne_suivante() && self.ligne.profondeur >= self.pile.len() {
            if self.ligne.pos == self.ligne.len() {
                ajouter_texte_code(&mut en_attente, self.debut_ligne - 1, self.debut_ligne);
                en_attente.extend(self.ligne.marqueurs.iter().cloned());
            } else if self.ligne.indent < base {
                break;
            } else {
                for m in en_attente.drain(..) {
                    if m.genre == Genre::CodeText {
                        ajouter_texte_code(&mut marques, m.from, m.to);
                    } else {
                        marques.push(m);
                    }
                }
                ajouter_texte_code(&mut marques, self.debut_ligne - 1, self.debut_ligne);
                marques.extend(self.ligne.marqueurs.iter().cloned());
                to = self.debut_ligne + self.ligne.len();
                let debut_code =
                    self.debut_ligne + self.ligne.trouver_colonne(self.ligne.base_indent + 4);
                if debut_code < to {
                    ajouter_texte_code(&mut marques, debut_code, to);
                }
            }
        }
        en_attente.retain(|m| m.genre != Genre::CodeText);
        if !en_attente.is_empty() {
            en_attente.append(&mut self.ligne.marqueurs);
            self.ligne.marqueurs = en_attente;
        }
        self.ajouter(Elt::avec(Genre::CodeBlock, from, to, marques));
        Res::Feuille
    }

    /// Fragment : `$$…$$` en début de ligne. Sur une ligne (`$$x$$`, le reste de la ligne est
    /// ignoré), sur plusieurs, ou sans fermeture : alors jusqu'au bout du document (bloc en cours de
    /// frappe : le nœud existe, la source reste visible).
    fn maths_bloc(&mut self) -> Res {
        if !self.est_maths_bloc() {
            return Res::Non;
        }
        let start = self.debut_ligne + self.ligne.pos;
        if let Some(i) = self
            .ligne
            .texte
            .get(self.ligne.pos + 2..)
            .and_then(|s| s.find("$$"))
        {
            let end = self.debut_ligne + self.ligne.pos + 2 + i + 2;
            self.ligne_suivante();
            self.ajouter(Elt::feuille(Genre::BlockMath, start, end));
            return Res::Feuille;
        }
        while self.ligne_suivante() {
            if let Some(i) = self.ligne.texte.find("$$") {
                let end = self.debut_ligne + i + 2;
                self.ligne_suivante();
                self.ajouter(Elt::feuille(Genre::BlockMath, start, end));
                return Res::Feuille;
            }
        }
        let end = self.fin_ligne_prec();
        self.ajouter(Elt::feuille(Genre::BlockMath, start, end));
        Res::Feuille
    }

    fn code_cloture(&mut self) -> Res {
        let Some(fin_cloture) = est_code_cloture(&self.ligne) else {
            return Res::Non;
        };
        let from = self.debut_ligne + self.ligne.pos;
        let ch = self.ligne.suivant;
        let len = fin_cloture - self.ligne.pos;
        let info_de = self.ligne.sauter_espaces(fin_cloture);
        let info_a = sauter_espaces_arriere(self.ligne.texte, self.ligne.len(), info_de);
        let mut marques = vec![Elt::feuille(Genre::CodeMark, from, from + len)];
        if info_de < info_a {
            marques.push(Elt::feuille(
                Genre::CodeInfo,
                self.debut_ligne + info_de,
                self.debut_ligne + info_a,
            ));
        }
        let (mut premier, mut vide, mut a_ligne) = (true, true, false);
        while self.ligne_suivante() && self.ligne.profondeur >= self.pile.len() {
            let l = &self.ligne;
            let mut i = l.pos;
            if l.indent.saturating_sub(l.base_indent) < 4 {
                while i < l.len() && l.car(i) == ch {
                    i += 1;
                }
            }
            if i - l.pos >= len && l.sauter_espaces(i) == l.len() {
                marques.extend(l.marqueurs.iter().cloned());
                if vide && a_ligne {
                    ajouter_texte_code(&mut marques, self.debut_ligne - 1, self.debut_ligne);
                }
                marques.push(Elt::feuille(
                    Genre::CodeMark,
                    self.debut_ligne + l.pos,
                    self.debut_ligne + i,
                ));
                self.ligne_suivante();
                break;
            }
            a_ligne = true;
            if !premier {
                ajouter_texte_code(&mut marques, self.debut_ligne - 1, self.debut_ligne);
                vide = false;
            }
            let l = &self.ligne;
            marques.extend(l.marqueurs.iter().cloned());
            let (debut_texte, fin_texte) =
                (self.debut_ligne + l.base_pos, self.debut_ligne + l.len());
            if debut_texte < fin_texte {
                ajouter_texte_code(&mut marques, debut_texte, fin_texte);
                vide = false;
            }
            premier = false;
        }
        let to = self.fin_ligne_prec();
        self.ajouter(Elt::avec(Genre::FencedCode, from, to, marques));
        Res::Feuille
    }

    fn citation(&mut self) -> Res {
        let Some(taille) = est_citation(&self.ligne) else {
            return Res::Non;
        };
        let pos = self.ligne.pos;
        self.debut_contexte(Genre::Blockquote, pos, 0);
        let p = self.debut_ligne + pos;
        self.ajouter(Elt::feuille(Genre::QuoteMark, p, p + 1));
        self.ligne.deplacer_base(pos + taille);
        Res::Contexte
    }

    fn regle(&mut self) -> Res {
        if self.est_regle(false).is_none() {
            return Res::Non;
        }
        let from = self.debut_ligne + self.ligne.pos;
        self.ligne_suivante();
        let to = self.fin_ligne_prec();
        self.ajouter(Elt::feuille(Genre::HorizontalRule, from, to));
        Res::Feuille
    }

    fn liste(&mut self, genre: Genre) -> Res {
        let taille = if genre == Genre::BulletList {
            self.est_liste_puces(false)
        } else {
            self.est_liste_ordonnee(false)
        };
        let Some(taille) = taille else {
            return Res::Non;
        };
        let (base_pos, pos) = (self.ligne.base_pos, self.ligne.pos);
        if self.bloc().genre != genre {
            let valeur = if genre == Genre::BulletList {
                self.ligne.suivant
            } else {
                self.ligne.car(pos + taille - 1)
            };
            self.debut_contexte(genre, base_pos, usize::try_from(valeur).unwrap_or(0));
        }
        let nouvelle_base = indent_de_liste(&self.ligne, pos + taille);
        let v = nouvelle_base - self.ligne.base_indent;
        self.debut_contexte(Genre::ListItem, base_pos, v);
        let p = self.debut_ligne + pos;
        self.ajouter(Elt::feuille(Genre::ListMark, p, p + taille));
        self.ligne.deplacer_base_colonne(nouvelle_base);
        Res::Contexte
    }

    fn titre_atx(&mut self) -> Res {
        let Some(taille) = est_titre_atx(&self.ligne) else {
            return Res::Non;
        };
        let l = &self.ligne;
        let off = l.pos;
        let from = self.debut_ligne + off;
        let fin_espace = sauter_espaces_arriere(l.texte, l.len(), off);
        let mut apres = fin_espace;
        while apres > off && l.car(apres - 1) == l.suivant {
            apres -= 1;
        }
        if apres == fin_espace || apres == off || !espace(l.car(apres - 1)) {
            apres = l.len();
        }
        let mut enfants = vec![Elt::feuille(Genre::HeaderMark, from, from + taille)];
        let a = off + taille + 1;
        if a < apres {
            enfants.extend(en_ligne::analyser(&l.texte[a..apres], from + taille + 1));
        }
        if apres < l.len() {
            enfants.push(Elt::feuille(
                Genre::HeaderMark,
                self.debut_ligne + apres,
                self.debut_ligne + fin_espace,
            ));
        }
        let to = self.debut_ligne + l.len();
        self.ligne_suivante();
        self.ajouter(Elt::avec(Genre::atx(taille), from, to, enfants));
        Res::Feuille
    }

    fn bloc_html(&mut self) -> Res {
        let Some(style) = self.est_bloc_html(false) else {
            return Res::Non;
        };
        let from = self.debut_ligne + self.ligne.pos;
        let fin: &Regex = STYLES_HTML[style].1;
        let ligne_vide = std::ptr::eq(fin, &*FIN_LIGNE_VIDE);
        let mut marques = Vec::new();
        let mut finale = !ligne_vide;
        while !fin.is_match(self.ligne.texte) && self.ligne_suivante() {
            if self.ligne.profondeur < self.pile.len() {
                finale = false;
                break;
            }
            marques.extend(self.ligne.marqueurs.iter().cloned());
        }
        if finale {
            self.ligne_suivante();
        }
        let genre = if std::ptr::eq(fin, &*FIN_COMMENTAIRE) {
            Genre::CommentBlock
        } else if std::ptr::eq(fin, &*FIN_INSTRUCTION) {
            Genre::ProcessingInstructionBlock
        } else {
            Genre::HTMLBlock
        };
        let to = self.fin_ligne_prec();
        self.ajouter(Elt::avec(genre, from, to, marques));
        Res::Feuille
    }

    /// ★ L'ordre de Fragment : `BlockMath` passe AVANT `FencedCode` (`before: "FencedCode"`).
    fn essayer_blocs(&mut self) -> Res {
        for k in 0..9 {
            let r = match k {
                0 => self.code_indente(),
                1 => self.maths_bloc(),
                2 => self.code_cloture(),
                3 => self.citation(),
                4 => self.regle(),
                5 => self.liste(Genre::BulletList),
                6 => self.liste(Genre::OrderedList),
                7 => self.titre_atx(),
                _ => self.bloc_html(),
            };
            if !matches!(r, Res::Non) {
                return r;
            }
        }
        Res::Non
    }

    /// Une ligne qui interrompt un bloc feuille (paragraphe…) sans ligne vide.
    fn interrompt_feuille(&self, feuille: &Feuille) -> bool {
        let l = &self.ligne;
        if est_titre_atx(l).is_some()
            || est_code_cloture(l).is_some()
            || est_citation(l).is_some()
            || self.est_liste_puces(true).is_some()
            || self.est_liste_ordonnee(true).is_some()
            || self.est_regle(true).is_some()
            || self.est_bloc_html(true).is_some()
        {
            return true;
        }
        // GFM : une ligne à barres suivie d'une ligne de délimiteurs commence un tableau.
        if !feuille
            .parseurs
            .iter()
            .any(|p| matches!(p, ParseurFeuille::Tableau(_)))
            && a_une_barre(l.texte, l.base_pos)
        {
            let suivante = self.ligne_suivante_brute();
            if LIGNE_DELIMITEUR.is_match(suivante)
                && analyser_rangee(l.texte, l.base_pos, None, 0)
                    == analyser_rangee(suivante, l.base_pos, None, 0)
            {
                return true;
            }
        }
        // ★ Fragment : `$$` en début de ligne interrompt un paragraphe (comme ```), sinon le `$$`
        //   fermant ouvrirait un bloc sans fin qui avalerait la suite de la note.
        self.est_maths_bloc()
    }

    fn ajouter_feuille(&mut self, feuille: &Feuille, e: Elt) {
        let enfants = injecter_marques(e.enfants, &feuille.marques);
        self.ajouter(Elt::avec(e.genre, e.from, e.to, enfants));
    }

    /// `nextLine` d'un parseur de feuille ; vrai s'il a fini la feuille.
    fn feuille_ligne_suivante(&mut self, p: &mut ParseurFeuille, feuille: &Feuille) -> bool {
        match p {
            ParseurFeuille::Reference(r) => {
                if r.etape == -1 {
                    return false;
                }
                let contenu = format!("{}\n{}", feuille.contenu, self.ligne.nettoyer());
                let fin = r.avancer(&contenu);
                if fin > -1 && (fin as usize) < contenu.len() {
                    let e = Elt::avec(
                        Genre::LinkReference,
                        r.start,
                        r.start + fin as usize,
                        std::mem::take(&mut r.elts),
                    );
                    self.ajouter_feuille(feuille, e);
                    return true;
                }
                false
            }
            ParseurFeuille::Tableau(rangees) => {
                let l = &self.ligne;
                match rangees {
                    None => {
                        *rangees = Some(None);
                        let texte = &l.texte[l.pos..];
                        if (l.suivant == 45 || l.suivant == 58 || l.suivant == 124)
                            && LIGNE_DELIMITEUR.is_match(texte)
                        {
                            let mut premiere = Vec::new();
                            let n = analyser_rangee(
                                &feuille.contenu,
                                0,
                                Some(&mut premiere),
                                feuille.start,
                            );
                            if n == analyser_rangee(texte, 0, None, 0) {
                                let fin_tete = feuille.start + feuille.contenu.len();
                                *rangees = Some(Some(vec![
                                    Elt::avec(
                                        Genre::TableHeader,
                                        feuille.start,
                                        fin_tete,
                                        premiere,
                                    ),
                                    Elt::feuille(
                                        Genre::TableDelimiter,
                                        self.debut_ligne + l.pos,
                                        self.debut_ligne + l.len(),
                                    ),
                                ]));
                            }
                        }
                    }
                    Some(Some(r)) => {
                        let mut contenu = Vec::new();
                        analyser_rangee(l.texte, l.pos, Some(&mut contenu), self.debut_ligne);
                        r.push(Elt::avec(
                            Genre::TableRow,
                            self.debut_ligne + l.pos,
                            self.debut_ligne + l.len(),
                            contenu,
                        ));
                    }
                    Some(None) => {}
                }
                false
            }
            ParseurFeuille::Setext => {
                let l = &self.ligne;
                let soulign = if l.profondeur < self.pile.len() {
                    None
                } else {
                    est_soulignement_setext(l)
                };
                let Some(soulign) = soulign else { return false };
                let suivant = l.suivant;
                let marque = Elt::feuille(
                    Genre::HeaderMark,
                    self.debut_ligne + l.pos,
                    self.debut_ligne + soulign,
                );
                self.ligne_suivante();
                let mut enfants = en_ligne::analyser(&feuille.contenu, feuille.start);
                enfants.push(marque);
                let genre = if suivant == 61 {
                    Genre::SetextHeading1
                } else {
                    Genre::SetextHeading2
                };
                let e = Elt::avec(genre, feuille.start, self.fin_ligne_prec(), enfants);
                self.ajouter_feuille(feuille, e);
                true
            }
            ParseurFeuille::Tache => false,
        }
    }

    fn finir_feuille(&mut self, mut feuille: Feuille) {
        let parseurs = std::mem::take(&mut feuille.parseurs);
        let fin = feuille.start + feuille.contenu.len();
        for p in parseurs {
            match p {
                ParseurFeuille::Reference(r) => {
                    if (r.etape == 2 || r.etape == 3)
                        && sauter_espaces(&feuille.contenu, r.pos) == feuille.contenu.len()
                    {
                        let e = Elt::avec(Genre::LinkReference, r.start, fin, r.elts);
                        self.ajouter_feuille(&feuille, e);
                        return;
                    }
                }
                ParseurFeuille::Tableau(Some(Some(rangees))) => {
                    let e = Elt::avec(Genre::Table, feuille.start, fin, rangees);
                    self.ajouter_feuille(&feuille, e);
                    return;
                }
                ParseurFeuille::Tache => {
                    let s = feuille.start;
                    let mut enfants = vec![Elt::feuille(Genre::TaskMarker, s, s + 3)];
                    enfants.extend(en_ligne::analyser(&feuille.contenu[3..], s + 3));
                    self.ajouter_feuille(&feuille, Elt::avec(Genre::Task, s, fin, enfants));
                    return;
                }
                _ => {}
            }
        }
        let enfants = injecter_marques(
            en_ligne::analyser(&feuille.contenu, feuille.start),
            &feuille.marques,
        );
        self.ajouter(Elt::avec(Genre::Paragraph, feuille.start, fin, enfants));
    }

    /// Un pas d'analyse ; rend l'arbre quand le document est fini.
    fn avancer(&mut self) -> Option<Elt> {
        loop {
            let mut i_marque = 0;
            loop {
                let suivant_end = (self.ligne.profondeur < self.pile.len())
                    .then(|| self.pile[self.pile.len() - 1].end);
                while i_marque < self.ligne.marqueurs.len()
                    && suivant_end.is_none_or(|end| self.ligne.marqueurs[i_marque].from < end)
                {
                    let m = self.ligne.marqueurs[i_marque].clone();
                    i_marque += 1;
                    self.ajouter(m);
                }
                if suivant_end.is_none() {
                    break;
                }
                self.finir_contexte();
            }
            if self.ligne.pos < self.ligne.len() {
                break;
            }
            // Ligne vide.
            if !self.ligne_suivante() {
                return Some(self.finir());
            }
        }
        loop {
            match self.essayer_blocs() {
                Res::Feuille => return None,
                Res::Contexte => self.ligne.avancer(),
                Res::Non => break,
            }
        }
        if self.ligne.pos == self.ligne.len() {
            return if self.ligne_suivante() {
                None
            } else {
                Some(self.finir())
            };
        }
        let start = self.debut_ligne + self.ligne.pos;
        let contenu = self.ligne.texte[self.ligne.pos..].to_owned();
        let mut parseurs = Vec::new();
        if contenu.starts_with('[') {
            let mut r = RefLien {
                etape: 0,
                elts: Vec::new(),
                pos: 0,
                start,
            };
            r.avancer(&contenu);
            parseurs.push(ParseurFeuille::Reference(r));
        }
        if a_une_barre(&contenu, 0) {
            parseurs.push(ParseurFeuille::Tableau(None));
        }
        parseurs.push(ParseurFeuille::Setext);
        let b = contenu.as_bytes();
        // GFM : `[ ]`, `[x]` ou `[X]` SUIVI d'un espace ou d'une tabulation, dans un élément de liste.
        // ★ `- [ ]` sans rien derrière n'est donc PAS une tâche (Fragment le rend à part).
        if b.len() >= 4
            && b[0] == b'['
            && matches!(b[1], b' ' | b'x' | b'X')
            && b[2] == b']'
            && matches!(b[3], b' ' | b'\t')
            && self.bloc().genre == Genre::ListItem
        {
            parseurs.push(ParseurFeuille::Tache);
        }
        let mut feuille = Feuille {
            start,
            contenu,
            marques: Vec::new(),
            parseurs,
        };
        while self.ligne_suivante() {
            if self.ligne.pos == self.ligne.len() {
                break;
            }
            if self.ligne.indent < self.ligne.base_indent + 4 && self.interrompt_feuille(&feuille) {
                break;
            }
            let mut parseurs = std::mem::take(&mut feuille.parseurs);
            let mut fini = false;
            for p in &mut parseurs {
                if self.feuille_ligne_suivante(p, &feuille) {
                    fini = true;
                    break;
                }
            }
            feuille.parseurs = parseurs;
            if fini {
                return None;
            }
            feuille.contenu.push('\n');
            feuille.contenu.push_str(&self.ligne.nettoyer());
            feuille.marques.extend(self.ligne.marqueurs.iter().cloned());
        }
        self.finir_feuille(feuille);
        None
    }
}

/// Analyse `doc` à partir du début de ligne `ligne` (0, ou un repère d'une analyse précédente dont
/// `lu` est repris), en relevant un [`Repere`] à chaque pas où seule la racine est ouverte. Après
/// chaque repère, `arret(ligne)` peut arrêter l'analyse (la suite est connue : reprise de l'ancien
/// arbre).
///
/// ★ Les repères de « bout de document » (`au_bout`) ne sont pas relevés : il n'y a plus rien à
///   reprendre.
pub(crate) fn analyser_depuis(
    doc: &str,
    ligne: usize,
    lu: usize,
    mut arret: impl FnMut(usize) -> bool,
) -> Resultat {
    let mut a = Analyse::depuis(doc, ligne, lu);
    let mut reperes = Vec::new();
    loop {
        if a.pile.len() == 1 && !a.au_bout && a.ligne.marqueurs.is_empty() {
            reperes.push(Repere {
                ligne: a.debut_ligne,
                enfants: a.pile[0].enfants.len(),
                lu: a.lu.get(),
            });
            if arret(a.debut_ligne) {
                let enfants = a.pile.pop().map(|d| d.enfants).unwrap_or_default();
                return Resultat {
                    enfants,
                    reperes,
                    fin: Fin::Arret,
                };
            }
        }
        if let Some(racine) = a.avancer() {
            return Resultat {
                enfants: racine.enfants,
                reperes,
                fin: Fin::Document(racine.to),
            };
        }
    }
}
