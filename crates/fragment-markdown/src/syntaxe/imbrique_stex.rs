// Port du mode `stexMath` de @codemirror/legacy-modes 6.5.3 (stex.js, MIT, © Marijn Haverbeke et
// contributeurs), tel que l'enrobe app/src/ui/views/markdown/language.ts (`MathHighlight`).
// Traduction de l'automate, pas du code : noms, commentaires et structure réécrits (ADR-007).
//! Coloration du LaTeX dans les formules `$…$`, `$$…$$` : un automate à pile de commandes, appliqué
//! ligne par ligne comme le fait `StreamLanguage` de CodeMirror.
//!
//! ★ La table de jetons de `language.ts` est TROMPEUSE : elle dit remplacer celle de CodeMirror, mais
//!   `TokenTable` part de sa table par défaut, qui gagne pour `tag`, `builtin` et `error`. Mesuré sur
//!   le vrai parseur : `\frac` est un `tagName` (sous-type de `typeName`, donc `code_tag` et non
//!   `code_keyword`), un `builtin` est `variableName.standard` (`code_value`), une erreur est `invalid`
//!   (`code_important`). Les cinq autres (`keyword`, `bracket`, `atom`, `number`, `string`,
//!   `comment`) suivent la table de `language.ts`.
//!
//! ★ Autre piège d'origine : `blankLine` remet l'automate en mode TEXTE (hors formule) et vide la
//!   pile. Une formule en bloc `$$\n…\n$$` commence par une ligne vide (l'intérieur est pris après
//!   `$$`) : elle est donc colorée en mode texte, pas en mode maths. On le reproduit tel quel.

use super::jetons::JetonCode;

/// Les jetons de l'automate (les noms de style de `stex.js`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Commande,
    MotCle,
    Predefini,
    Crochet,
    Atome,
    Nombre,
    Chaine,
    Commentaire,
    Erreur,
    VariableSpeciale,
}

impl Style {
    fn jeton(self) -> JetonCode {
        match self {
            Style::Commande | Style::Nombre => JetonCode::Type,
            Style::MotCle => JetonCode::MotCle,
            Style::Predefini | Style::Atome | Style::VariableSpeciale => JetonCode::Valeur,
            Style::Crochet => JetonCode::Ponctuation,
            Style::Chaine => JetonCode::Chaine,
            Style::Commentaire => JetonCode::Commentaire,
            Style::Erreur => JetonCode::Important,
        }
    }
}

/// Les commandes dont les arguments reçoivent un style propre (`plugins` de `stex.js`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commande {
    ImportModule,
    DocumentClass,
    UsePackage,
    Begin,
    End,
    Label,
    Autre,
}

impl Commande {
    fn de_nom(nom: &str) -> Commande {
        match nom {
            "importmodule" => Commande::ImportModule,
            "documentclass" => Commande::DocumentClass,
            "usepackage" => Commande::UsePackage,
            "begin" => Commande::Begin,
            "end" => Commande::End,
            "label" | "ref" | "eqref" | "cite" | "bibitem" | "Bibitem" | "RBibitem" => {
                Commande::Label
            }
            _ => Commande::Autre,
        }
    }

    /// Le style du `n`-ième argument entre accolades (1 pour le premier).
    fn style_argument(self, n: usize) -> Option<Style> {
        let styles: &[Option<Style>] = match self {
            Commande::ImportModule => &[Some(Style::Chaine), Some(Style::Predefini)],
            Commande::DocumentClass => &[None, Some(Style::Atome)],
            Commande::UsePackage | Commande::Begin | Commande::End | Commande::Label => {
                &[Some(Style::Atome)]
            }
            Commande::Autre => return None,
        };
        n.checked_sub(1)
            .and_then(|i| styles.get(i))
            .copied()
            .flatten()
    }
}

/// Une commande ouverte : son genre et le nombre d'arguments déjà ouverts.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ouverte {
    commande: Commande,
    arguments: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Formule sans fin attendue : l'état initial de `stexMath`.
    MathSansFin,
    /// Dans une formule qu'une séquence ferme.
    Math(&'static str),
    Texte,
    /// Juste après une commande : ses arguments.
    Arguments,
}

/// Le flux d'une ligne (le `StringStream` de CodeMirror).
struct Flux<'a> {
    ligne: &'a str,
    pos: usize,
}

impl<'a> Flux<'a> {
    fn reste(&self) -> &'a str {
        self.ligne.get(self.pos..).unwrap_or("")
    }
    fn peek(&self) -> Option<char> {
        self.reste().chars().next()
    }
    fn suivant(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    fn motif(&mut self, m: &str) -> bool {
        if self.reste().starts_with(m) {
            self.pos += m.len();
            true
        } else {
            false
        }
    }
    fn manger_tant_que(&mut self, f: impl Fn(char) -> bool) {
        while let Some(c) = self.peek() {
            if !f(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
    }
    fn fin_de_ligne(&mut self) {
        self.pos = self.ligne.len();
    }
}

fn est_blanc(c: char) -> bool {
    c.is_whitespace() || c == '\u{a0}'
}

/// `[a-zA-Z@\xc0-῿⁠-￿]` : les lettres d'un nom de commande.
fn lettre_commande(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '@' || ('\u{c0}'..='\u{1fff}').contains(&c) || c >= '\u{2060}'
}

fn mot(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// L'automate : un mode et la pile des commandes ouvertes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stex {
    mode: Mode,
    pile: Vec<Ouverte>,
}

impl Stex {
    /// L'état de départ de `stexMath` (formule, pas de fin attendue).
    pub(super) fn maths() -> Stex {
        Stex {
            mode: Mode::MathSansFin,
            pile: Vec::new(),
        }
    }

    /// Colore `texte` (l'intérieur d'une formule) ; rend (début, fin, jeton) en octets de `texte`,
    /// triés, les jetons voisins de même nom fusionnés.
    pub(super) fn colorer(texte: &str) -> Vec<(usize, usize, JetonCode)> {
        let mut etat = Stex::maths();
        let mut sortie: Vec<(usize, usize, Style)> = Vec::new();
        let mut debut_ligne = 0;
        for ligne in texte.split('\n') {
            if ligne.is_empty() {
                etat.ligne_vide();
            } else {
                let mut f = Flux { ligne, pos: 0 };
                while f.pos < ligne.len() {
                    let avant = f.pos;
                    let style = etat.jeton(&mut f);
                    if f.pos == avant {
                        // ★ Garde-fou : un jeton qui n'avance pas (impossible ici) boucle à l'infini.
                        f.suivant();
                    }
                    if let Some(s) = style {
                        // ★ Fusion par NOM DE STYLE (comme `mergeTokens`), pas par jeton : `^` et
                        //   `2` sont deux jetons de même couleur, mais deux styles.
                        let (de, a) = (debut_ligne + avant, debut_ligne + f.pos);
                        match sortie.last_mut() {
                            Some(d) if d.1 == de && d.2 == s => d.1 = a,
                            _ => sortie.push((de, a, s)),
                        }
                    }
                }
            }
            debut_ligne += ligne.len() + 1;
        }
        sortie
            .into_iter()
            .map(|(d, a, s)| (d, a, s.jeton()))
            .collect()
    }

    fn ligne_vide(&mut self) {
        self.mode = Mode::Texte;
        self.pile.clear();
    }

    fn jeton(&mut self, f: &mut Flux<'_>) -> Option<Style> {
        match self.mode {
            Mode::MathSansFin => self.en_maths(f, None),
            Mode::Math(fin) => self.en_maths(f, Some(fin)),
            Mode::Texte => self.texte(f),
            Mode::Arguments => self.arguments(f),
        }
    }

    /// Le style de l'argument courant de la commande la plus récente qui en a un.
    fn style_identifiant(&self) -> Option<Style> {
        self.pile
            .iter()
            .rev()
            .find(|o| o.commande != Commande::Autre)
            .and_then(|o| o.commande.style_argument(o.arguments))
    }

    fn texte(&mut self, f: &mut Flux<'_>) -> Option<Style> {
        // `\commande` : on l'empile et on attend ses arguments.
        let reste = f.reste();
        if let Some(sans) = reste.strip_prefix('\\') {
            let n: usize = sans
                .chars()
                .take_while(|&c| lettre_commande(c))
                .map(char::len_utf8)
                .sum();
            if n > 0 {
                let nom = &sans[..n];
                f.pos += 1 + n;
                self.pile.push(Ouverte {
                    commande: Commande::de_nom(nom),
                    arguments: 0,
                });
                self.mode = Mode::Arguments;
                return Some(Style::Commande);
            }
            // Échappements et espaces fins : `\$ \& \% \# \{ \} \_`, puis `\, \; \! \/ \\`.
            if let Some(c) = sans.chars().next()
                && ("$&%#{}_,;!/\\").contains(c)
            {
                f.pos += 1 + c.len_utf8();
                return Some(Style::Commande);
            }
        }
        for (ouvre, fin) in [("\\[", "\\]"), ("\\(", "\\)"), ("$$", "$$"), ("$", "$")] {
            if f.motif(ouvre) {
                self.mode = Mode::Math(fin);
                return Some(Style::MotCle);
            }
        }
        let c = f.suivant()?;
        match c {
            '%' => {
                f.fin_de_ligne();
                Some(Style::Commentaire)
            }
            '}' | ']' => {
                // ★ `}` ne dépile PAS : il consulte la commande en cours et rend la main à
                //   `arguments`, qui dépilera si aucun autre argument ne suit.
                if self.pile.is_empty() {
                    Some(Style::Erreur)
                } else {
                    self.mode = Mode::Arguments;
                    Some(Style::Crochet)
                }
            }
            '{' | '[' => {
                self.pile.push(Ouverte {
                    commande: Commande::Autre,
                    arguments: 0,
                });
                Some(Style::Crochet)
            }
            c if c.is_ascii_digit() => {
                f.manger_tant_que(|c| mot(c) || c == '.' || c == '%');
                Some(Style::Atome)
            }
            _ => {
                f.manger_tant_que(|c| mot(c) || c == '-');
                self.style_identifiant()
            }
        }
    }

    fn arguments(&mut self, f: &mut Flux<'_>) -> Option<Style> {
        match f.peek() {
            Some(c @ ('{' | '[')) => {
                if let Some(o) = self.pile.last_mut() {
                    o.arguments += 1;
                }
                f.pos += c.len_utf8();
                self.mode = Mode::Texte;
                Some(Style::Crochet)
            }
            Some(c @ (' ' | '\t' | '\r')) => {
                f.pos += c.len_utf8();
                None
            }
            _ => {
                self.mode = Mode::Texte;
                self.pile.pop();
                self.texte(f)
            }
        }
    }

    fn en_maths(&mut self, f: &mut Flux<'_>, fin: Option<&'static str>) -> Option<Style> {
        let avant = f.pos;
        f.manger_tant_que(est_blanc);
        if f.pos > avant {
            return None;
        }
        if let Some(fin) = fin
            && f.motif(fin)
        {
            self.mode = Mode::Texte;
            return Some(Style::MotCle);
        }
        let reste = f.reste();
        if let Some(sans) = reste.strip_prefix('\\') {
            let n = sans
                .bytes()
                .take_while(|b| b.is_ascii_alphabetic() || *b == b'@')
                .count();
            if n > 0 {
                f.pos += 1 + n;
                return Some(Style::Commande);
            }
            if let Some(c) = sans.chars().next()
                && ("$&%#{}_,;!/").contains(c)
            {
                f.pos += 1 + c.len_utf8();
                return Some(Style::Commande);
            }
        }
        let lettres = reste.bytes().take_while(u8::is_ascii_alphabetic).count();
        if lettres > 0 {
            f.pos += lettres;
            return Some(Style::VariableSpeciale);
        }
        let c = reste.chars().next()?;
        if "^_&".contains(c) {
            f.pos += 1;
            return Some(Style::Commande);
        }
        if "+-<>|=,/@!*:;'\"`~#?".contains(c) {
            f.pos += 1;
            return None;
        }
        if let Some(n) = longueur_nombre(reste) {
            f.pos += n;
            return Some(Style::Nombre);
        }
        f.suivant();
        match c {
            '{' | '}' | '[' | ']' | '(' | ')' => Some(Style::Crochet),
            '%' => {
                f.fin_de_ligne();
                Some(Style::Commentaire)
            }
            _ => Some(Style::Erreur),
        }
    }
}

/// `^(\d+\.\d*|\d*\.\d+|\d+)` : la longueur du nombre en tête de `s`.
fn longueur_nombre(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let entiers = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if b.get(entiers) == Some(&b'.') {
        let decimales = b[entiers + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if entiers > 0 || decimales > 0 {
            return Some(entiers + 1 + decimales);
        }
        return None;
    }
    (entiers > 0).then_some(entiers)
}

/// Ajoute un jeton en fusionnant avec le précédent s'il le touche et porte le même nom.
pub(super) fn poser(
    sortie: &mut Vec<(usize, usize, JetonCode)>,
    de: usize,
    a: usize,
    j: JetonCode,
) {
    if let Some(dernier) = sortie.last_mut()
        && dernier.1 == de
        && dernier.2 == j
    {
        dernier.1 = a;
        return;
    }
    sortie.push((de, a, j));
}
