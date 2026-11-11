// Port de app/src/ui/index.css (jetons, alias `var()`, `color-mix`, `calc`) et de
// app/src/ui/core/settings/theme.ts (l'accent CHOISI l'emporte sur celui du thème).
//
//! La cascade des jetons, comme un graphe.
//!
//! Chaque jeton est une valeur ou une EXPRESSION telle qu'`index.css` l'écrit : `var(--x)`,
//! `color-mix(in srgb, …)`, `calc(3 * var(--x))`, un littéral. Le [`Graphe`] les résout à la
//! demande ; surcharger un jeton (l'accent des réglages, un `theme.json`) puis résoudre de nouveau
//! recalcule donc ses dérivés, exactement comme le fait la cascade CSS dans Chromium.
//!
//! ★ Sans surcharge, `Theme::sombre()` et `Theme::clair()` sont construits par ce graphe et restent
//! identiques, au bit près, aux palettes écrites à la main qu'ils remplacent (instantané figé dans
//! `tests/figes/`). Une surcharge illisible, circulaire ou du mauvais type est REFUSÉE avec un
//! avertissement : elle ne casse jamais le thème.

use std::collections::BTreeMap;
use std::fmt;

use crate::couleur::{Couleur, Ombre};
use crate::jetons_css::{BODY, ROOT};
use crate::theme::{Forme, Mode, Opacites, Palette, Theme, Typo};
use crate::valeurs_css::{self, fonction, par_blancs, par_virgules};

/// Pourquoi une expression ou un jeton n'a pas pu être lu ou résolu.
#[derive(Debug, Clone, PartialEq)]
pub enum ErreurCascade {
    /// Le texte n'est pas une expression que la cascade sait lire.
    Syntaxe {
        /// L'expression fautive.
        expression: String,
        /// Ce qui ne va pas.
        raison: String,
    },
    /// `var(--x)` renvoie à un jeton qui n'existe pas (et sans valeur de repli).
    Inconnu {
        /// Le nom du jeton manquant.
        nom: String,
    },
    /// Des jetons se définissent les uns par les autres.
    Cycle {
        /// Le chemin parcouru, du premier jeton au jeton qui boucle.
        chemin: Vec<String>,
    },
    /// La valeur résolue n'a pas la forme que le jeton attend.
    Type {
        /// Le jeton.
        nom: String,
        /// La forme attendue (« couleur », « longueur »…).
        attendu: &'static str,
    },
}

impl fmt::Display for ErreurCascade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntaxe { expression, raison } => write!(f, "« {expression} » : {raison}"),
            Self::Inconnu { nom } => write!(f, "le jeton {nom} n'existe pas"),
            Self::Cycle { chemin } => write!(f, "cycle entre jetons : {}", chemin.join(" → ")),
            Self::Type { nom, attendu } => write!(f, "{nom} attend une valeur de type {attendu}"),
        }
    }
}

impl std::error::Error for ErreurCascade {}

fn syntaxe(expression: &str, raison: &str) -> ErreurCascade {
    ErreurCascade::Syntaxe {
        expression: expression.to_owned(),
        raison: raison.to_owned(),
    }
}

/// Une valeur résolue.
#[derive(Debug, Clone, PartialEq)]
pub enum Valeur {
    /// Une couleur 8 bits.
    Couleur(Couleur),
    /// Une longueur, en pixels.
    Longueur(f32),
    /// Un nombre sans unité.
    Nombre(f32),
    /// Une durée, en millisecondes.
    Duree(f32),
    /// Une liste d'ombres portées.
    Ombres(Vec<Ombre>),
    /// Une courbe `cubic-bezier`.
    Bezier([f32; 4]),
    /// Tout le reste, gardé tel quel (piles de polices).
    Texte(String),
}

/// Une expression de jeton.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Un littéral.
    Valeur(Valeur),
    /// `var(--nom)` ou `var(--nom, repli)`.
    Var {
        /// Le jeton visé, avec ses deux tirets.
        nom: String,
        /// La valeur si le jeton n'existe pas.
        repli: Option<Box<Expr>>,
    },
    /// `color-mix(in srgb, a, b)`, pourcentages normalisés : `a` pèse `pourcent_a`, `b` le reste.
    Melange {
        /// Première couleur.
        a: Box<Expr>,
        /// Seconde couleur.
        b: Box<Expr>,
        /// Part de `a`, de 0 à 100.
        pourcent_a: f32,
    },
    /// `calc(…)` : `+ - * /` sur des longueurs et des nombres.
    Calc(Box<Calc>),
}

/// L'arbre d'un `calc()`.
#[derive(Debug, Clone, PartialEq)]
pub enum Calc {
    /// Un opérande : littéral ou `var()`.
    Feuille(Expr),
    /// Une opération (`+`, `-`, `*`, `/`).
    Op(char, Box<Calc>, Box<Calc>),
}

impl Expr {
    /// Lit une expression CSS de jeton.
    ///
    /// # Errors
    /// [`ErreurCascade::Syntaxe`] si le texte est vide, si une fonction est inconnue ou mal
    /// formée.
    pub fn analyser(texte: &str) -> Result<Self, ErreurCascade> {
        let t = texte.trim();
        if t.is_empty() {
            return Err(syntaxe(texte, "valeur vide"));
        }
        if let Some((nom, args)) = fonction(t) {
            match nom.as_str() {
                "var" => return analyser_var(t, args),
                "color-mix" => return analyser_melange(t, args),
                "calc" => return analyser_calc(t, args),
                "rgb" | "rgba" | "hsl" | "hsla" | "cubic-bezier" => {}
                _ => return Err(syntaxe(t, "fonction CSS non gérée")),
            }
        }
        Ok(Self::Valeur(litteral(t)))
    }
}

/// Un littéral : la première forme reconnue, sinon du texte brut.
fn litteral(t: &str) -> Valeur {
    if let Some(c) = valeurs_css::couleur(t) {
        Valeur::Couleur(c)
    } else if let Some(b) = valeurs_css::bezier(t) {
        Valeur::Bezier(b)
    } else if let Some(d) = valeurs_css::duree_ms(t) {
        Valeur::Duree(d)
    } else if let Some(l) = valeurs_css::longueur_px(t) {
        Valeur::Longueur(l)
    } else if let Some(n) = valeurs_css::nombre(t) {
        Valeur::Nombre(n)
    } else if let Some(o) = valeurs_css::ombres(t) {
        Valeur::Ombres(o)
    } else {
        Valeur::Texte(t.to_owned())
    }
}

fn analyser_var(t: &str, args: &str) -> Result<Expr, ErreurCascade> {
    let morceaux = par_virgules(args);
    let nom = morceaux.first().copied().unwrap_or("");
    if !nom.starts_with("--") || nom.len() < 3 {
        return Err(syntaxe(t, "var() attend un nom en --"));
    }
    let repli = if morceaux.len() > 1 {
        Some(Box::new(Expr::analyser(&morceaux[1..].join(", "))?))
    } else {
        None
    };
    Ok(Expr::Var {
        nom: nom.to_owned(),
        repli,
    })
}

/// Un terme de `color-mix` : la couleur et son pourcentage éventuel (avant ou après).
fn terme_de_melange(terme: &str) -> Result<(Expr, Option<f32>), ErreurCascade> {
    let mut pourcent = None;
    let mut reste = Vec::new();
    for morceau in par_blancs(terme) {
        let p = morceau.strip_suffix('%').and_then(valeurs_css::nombre);
        match p {
            Some(p) if pourcent.is_none() => pourcent = Some(p),
            Some(_) => return Err(syntaxe(terme, "deux pourcentages pour une couleur")),
            None => reste.push(morceau),
        }
    }
    if reste.is_empty() {
        return Err(syntaxe(terme, "couleur manquante"));
    }
    Ok((Expr::analyser(&reste.join(" "))?, pourcent))
}

fn analyser_melange(t: &str, args: &str) -> Result<Expr, ErreurCascade> {
    let morceaux = par_virgules(args);
    let [espace, premier, second] = morceaux[..] else {
        return Err(syntaxe(t, "color-mix attend un espace et deux couleurs"));
    };
    if !espace.eq_ignore_ascii_case("in srgb") {
        return Err(syntaxe(t, "seul `in srgb` est géré"));
    }
    let ((a, pa), (b, pb)) = (terme_de_melange(premier)?, terme_de_melange(second)?);
    let pourcent_a = match (pa, pb) {
        (None, None) => 50.0,
        (Some(p), None) => p,
        (None, Some(q)) => 100.0 - q,
        // Deux pourcentages : on les ramène à une somme de 100 (la spec ajoute une transparence
        // quand la somme est inférieure ; Fragment n'écrit jamais ce cas).
        (Some(p), Some(q)) if p + q > 0.0 => p / (p + q) * 100.0,
        (Some(_), Some(_)) => return Err(syntaxe(t, "pourcentages nuls")),
    };
    if !(0.0..=100.0).contains(&pourcent_a) {
        return Err(syntaxe(t, "pourcentage hors de 0 à 100"));
    }
    Ok(Expr::Melange {
        a: Box::new(a),
        b: Box::new(b),
        pourcent_a,
    })
}

/// Les jetons d'un `calc()` : opérandes, opérateurs, parenthèses.
fn jetons_de_calc(texte: &str) -> Vec<String> {
    let caracteres: Vec<char> = texte.chars().collect();
    let mut jetons = Vec::new();
    let mut i = 0;
    while i < caracteres.len() {
        let c = caracteres[i];
        if c.is_whitespace() {
            i += 1;
        } else if matches!(c, '(' | ')' | '*' | '/') {
            jetons.push(c.to_string());
            i += 1;
        } else {
            // Un opérande : jusqu'au prochain blanc ou symbole ; `var(...)` va jusqu'à sa fermante.
            let debut = i;
            let mut profondeur = 0_i32;
            while i < caracteres.len() {
                let d = caracteres[i];
                if profondeur == 0 && (d.is_whitespace() || matches!(d, ')' | '*' | '/')) {
                    break;
                }
                if d == '(' {
                    if profondeur == 0 && i == debut {
                        break;
                    }
                    profondeur += 1;
                } else if d == ')' {
                    profondeur -= 1;
                }
                i += 1;
            }
            jetons.push(caracteres[debut..i].iter().collect());
        }
    }
    jetons
}

/// Lecteur récursif d'un `calc()` : somme, produit, facteur.
struct LecteurCalc<'a> {
    jetons: &'a [String],
    position: usize,
    source: &'a str,
}

impl LecteurCalc<'_> {
    fn courant(&self) -> Option<&str> {
        self.jetons.get(self.position).map(String::as_str)
    }

    fn somme(&mut self) -> Result<Calc, ErreurCascade> {
        let mut gauche = self.produit()?;
        while let Some(op @ ("+" | "-")) = self.courant() {
            let op = op.chars().next().unwrap_or('+');
            self.position += 1;
            let droite = self.produit()?;
            gauche = Calc::Op(op, Box::new(gauche), Box::new(droite));
        }
        Ok(gauche)
    }

    fn produit(&mut self) -> Result<Calc, ErreurCascade> {
        let mut gauche = self.facteur()?;
        while let Some(op @ ("*" | "/")) = self.courant() {
            let op = op.chars().next().unwrap_or('*');
            self.position += 1;
            let droite = self.facteur()?;
            gauche = Calc::Op(op, Box::new(gauche), Box::new(droite));
        }
        Ok(gauche)
    }

    fn facteur(&mut self) -> Result<Calc, ErreurCascade> {
        let Some(jeton) = self.courant().map(str::to_owned) else {
            return Err(syntaxe(self.source, "calc() s'arrête trop tôt"));
        };
        self.position += 1;
        if jeton == "(" {
            let interieur = self.somme()?;
            if self.courant() != Some(")") {
                return Err(syntaxe(self.source, "parenthèse non refermée dans calc()"));
            }
            self.position += 1;
            return Ok(interieur);
        }
        if matches!(jeton.as_str(), ")" | "*" | "/" | "+" | "-") {
            return Err(syntaxe(self.source, "opérande attendu dans calc()"));
        }
        Ok(Calc::Feuille(Expr::analyser(&jeton)?))
    }
}

fn analyser_calc(t: &str, args: &str) -> Result<Expr, ErreurCascade> {
    let jetons = jetons_de_calc(args);
    let mut lecteur = LecteurCalc {
        jetons: &jetons,
        position: 0,
        source: t,
    };
    let arbre = lecteur.somme()?;
    if lecteur.position != jetons.len() {
        return Err(syntaxe(t, "calc() : texte en trop"));
    }
    Ok(Expr::Calc(Box::new(arbre)))
}

/// Les jetons qu'on ne peut pas surcharger : leur champ dans `Typo` est une pile `'static`.
const JETONS_FIGES: &[&str] = &[
    "--font-interface",
    "--font-text",
    "--font-monospace",
    "--font-serif",
];

/// Le graphe des jetons : un nom (`--text-normal`) pour une expression.
#[derive(Debug, Clone, Default)]
pub struct Graphe {
    definitions: BTreeMap<String, Expr>,
}

/// Un nombre ou une longueur calculés, avec leur nature (pour `calc()`).
type Mesure = (f32, bool);

impl Graphe {
    /// Le graphe d'index.css pour un thème : `:root`, `body`, puis `.theme-dark` ou `.theme-light`.
    ///
    /// # Errors
    /// Une déclaration de la table n'est pas lisible (bug de table : un test le détecte).
    pub fn par_defaut(mode: Mode) -> Result<Self, ErreurCascade> {
        let propre = match mode {
            Mode::Sombre => crate::sombre::JETONS,
            Mode::Clair => crate::clair::JETONS,
        };
        let mut graphe = Self::default();
        for (nom, css, _ligne) in ROOT.iter().chain(BODY).chain(propre) {
            graphe.definir_css(nom, css)?;
        }
        Ok(graphe)
    }

    /// Définit (ou redéfinit) un jeton depuis une expression CSS.
    ///
    /// # Errors
    /// L'expression n'est pas lisible : le graphe n'est pas modifié.
    pub fn definir_css(&mut self, nom: &str, css: &str) -> Result<(), ErreurCascade> {
        let expr = Expr::analyser(css)?;
        self.definitions.insert(nom.to_owned(), expr);
        Ok(())
    }

    /// Remplace un jeton ; rend l'ancienne expression.
    pub fn definir(&mut self, nom: &str, expr: Expr) -> Option<Expr> {
        self.definitions.insert(nom.to_owned(), expr)
    }

    /// Le jeton est-il déclaré ?
    pub fn contient(&self, nom: &str) -> bool {
        self.definitions.contains_key(nom)
    }

    /// Les noms déclarés, triés.
    pub fn noms(&self) -> impl Iterator<Item = &str> {
        self.definitions.keys().map(String::as_str)
    }

    /// Résout un jeton en valeur finale.
    ///
    /// # Errors
    /// Jeton inconnu, cycle, ou expression mal typée.
    pub fn resoudre(&self, nom: &str) -> Result<Valeur, ErreurCascade> {
        self.resoudre_nom(nom, &mut Vec::new())
    }

    fn resoudre_nom(&self, nom: &str, pile: &mut Vec<String>) -> Result<Valeur, ErreurCascade> {
        if pile.iter().any(|n| n == nom) {
            let mut chemin = pile.clone();
            chemin.push(nom.to_owned());
            return Err(ErreurCascade::Cycle { chemin });
        }
        let expr = self
            .definitions
            .get(nom)
            .ok_or_else(|| ErreurCascade::Inconnu {
                nom: nom.to_owned(),
            })?;
        pile.push(nom.to_owned());
        let resultat = self.resoudre_expr(expr, nom, pile);
        pile.pop();
        resultat
    }

    fn resoudre_expr(
        &self,
        expr: &Expr,
        pour: &str,
        pile: &mut Vec<String>,
    ) -> Result<Valeur, ErreurCascade> {
        match expr {
            Expr::Valeur(v) => Ok(v.clone()),
            Expr::Var { nom, repli } => match (self.contient(nom), repli) {
                (true, _) => self.resoudre_nom(nom, pile),
                (false, Some(r)) => self.resoudre_expr(r, pour, pile),
                (false, None) => Err(ErreurCascade::Inconnu { nom: nom.clone() }),
            },
            Expr::Melange { a, b, pourcent_a } => {
                let couleur =
                    |e: &Expr, pile: &mut Vec<String>| match self.resoudre_expr(e, pour, pile)? {
                        Valeur::Couleur(c) => Ok(c),
                        _ => Err(ErreurCascade::Type {
                            nom: pour.to_owned(),
                            attendu: "couleur",
                        }),
                    };
                let (ca, cb) = (couleur(a, pile)?, couleur(b, pile)?);
                Ok(Valeur::Couleur(ca.melange(*pourcent_a, cb)))
            }
            Expr::Calc(arbre) => {
                let (x, longueur) = self.calculer(arbre, pour, pile)?;
                Ok(if longueur {
                    Valeur::Longueur(x)
                } else {
                    Valeur::Nombre(x)
                })
            }
        }
    }

    fn calculer(
        &self,
        arbre: &Calc,
        pour: &str,
        pile: &mut Vec<String>,
    ) -> Result<Mesure, ErreurCascade> {
        let erreur = || ErreurCascade::Type {
            nom: pour.to_owned(),
            attendu: "longueur ou nombre dans calc()",
        };
        match arbre {
            Calc::Feuille(e) => match self.resoudre_expr(e, pour, pile)? {
                Valeur::Longueur(x) => Ok((x, true)),
                Valeur::Nombre(x) => Ok((x, false)),
                _ => Err(erreur()),
            },
            Calc::Op(op, g, d) => {
                let ((x, lx), (y, ly)) =
                    (self.calculer(g, pour, pile)?, self.calculer(d, pour, pile)?);
                match op {
                    '+' | '-' if lx == ly => Ok((if *op == '+' { x + y } else { x - y }, lx)),
                    '*' if !(lx && ly) => Ok((x * y, lx || ly)),
                    '/' if !ly => Ok((x / y, lx)),
                    _ => Err(erreur()),
                }
            }
        }
    }

    /// L'opacité finale d'un jeton, en pourcents (0 à 100) : celle d'un `color-mix` avec
    /// `transparent`, ou l'octet d'opacité d'un littéral. Sert à [`Opacites`].
    ///
    /// # Errors
    /// Comme [`Graphe::resoudre`] ; le jeton doit être une couleur.
    pub fn opacite_pourcent(&self, nom: &str) -> Result<f64, ErreurCascade> {
        self.opacite_nom(nom, &mut Vec::new())
    }

    fn opacite_nom(&self, nom: &str, pile: &mut Vec<String>) -> Result<f64, ErreurCascade> {
        // La résolution complète détecte cycles et types ; ensuite on suit les expressions.
        self.resoudre_nom(nom, &mut Vec::new())?;
        let expr = self
            .definitions
            .get(nom)
            .ok_or_else(|| ErreurCascade::Inconnu {
                nom: nom.to_owned(),
            })?;
        pile.push(nom.to_owned());
        let r = self.opacite_expr(expr, nom, pile);
        pile.pop();
        r
    }

    fn opacite_expr(
        &self,
        expr: &Expr,
        pour: &str,
        pile: &mut Vec<String>,
    ) -> Result<f64, ErreurCascade> {
        match expr {
            Expr::Valeur(Valeur::Couleur(c)) => Ok(f64::from(c.a) * 100.0 / 255.0),
            Expr::Var { nom, repli } => match (self.contient(nom), repli) {
                (true, _) => self.opacite_nom(nom, pile),
                (false, Some(r)) => self.opacite_expr(r, pour, pile),
                (false, None) => Err(ErreurCascade::Inconnu { nom: nom.clone() }),
            },
            Expr::Melange { a, b, pourcent_a } => {
                let p = f64::from(*pourcent_a);
                Ok((p * self.opacite_expr(a, pour, pile)?
                    + (100.0 - p) * self.opacite_expr(b, pour, pile)?)
                    / 100.0)
            }
            _ => Err(ErreurCascade::Type {
                nom: pour.to_owned(),
                attendu: "couleur",
            }),
        }
    }

    fn type_attendu(nom: &str, attendu: &'static str) -> ErreurCascade {
        ErreurCascade::Type {
            nom: nom.to_owned(),
            attendu,
        }
    }

    fn couleur(&self, nom: &str) -> Result<Couleur, ErreurCascade> {
        match self.resoudre(nom)? {
            Valeur::Couleur(c) => Ok(c),
            _ => Err(Self::type_attendu(nom, "couleur")),
        }
    }

    fn ombres(&self, nom: &str) -> Result<Vec<Ombre>, ErreurCascade> {
        match self.resoudre(nom)? {
            Valeur::Ombres(o) => Ok(o),
            _ => Err(Self::type_attendu(nom, "ombres")),
        }
    }

    fn reel(&self, nom: &str) -> Result<f32, ErreurCascade> {
        match self.resoudre(nom)? {
            Valeur::Longueur(x) | Valeur::Nombre(x) => Ok(x),
            _ => Err(Self::type_attendu(nom, "longueur ou nombre")),
        }
    }

    fn entier(&self, nom: &str) -> Result<u32, ErreurCascade> {
        match self.resoudre(nom)? {
            Valeur::Nombre(x) | Valeur::Duree(x) if x >= 0.0 && x.fract() == 0.0 => {
                // Entier positif vérifié ci-dessus : la conversion est exacte.
                Ok(x as u32)
            }
            _ => Err(Self::type_attendu(nom, "entier positif")),
        }
    }

    fn courbe(&self, nom: &str) -> Result<[f32; 4], ErreurCascade> {
        match self.resoudre(nom)? {
            Valeur::Bezier(b) => Ok(b),
            _ => Err(Self::type_attendu(nom, "cubic-bezier")),
        }
    }
}

/// `radius_m` devient `--radius-m`.
fn nom_css(champ: &str) -> String {
    format!("--{}", champ.replace('_', "-"))
}

/// Construit une struct de jetons depuis le graphe : un champ par jeton, rangé par type.
macro_rules! depuis_le_graphe {
    ($g:expr, $struct:ident {
        $(couleurs: [$($c:ident),* $(,)?],)?
        $(ombres: [$($o:ident),* $(,)?],)?
        $(reels: [$($r:ident),* $(,)?],)?
        $(entiers: [$($e:ident),* $(,)?],)?
        $(courbes: [$($b:ident),* $(,)?],)?
        $(autres: { $($a:ident : $v:expr),* $(,)? },)?
    }) => {
        $struct {
            $($($c: $g.couleur(&nom_css(stringify!($c)))?,)*)?
            $($($o: $g.ombres(&nom_css(stringify!($o)))?,)*)?
            $($($r: $g.reel(&nom_css(stringify!($r)))?,)*)?
            $($($e: $g.entier(&nom_css(stringify!($e)))?,)*)?
            $($($b: $g.courbe(&nom_css(stringify!($b)))?,)*)?
            $($($a: $v,)*)?
        }
    };
}

impl Palette {
    /// Les couleurs et ombres d'un graphe résolu.
    ///
    /// # Errors
    /// Un jeton de la palette ne se résout pas, ou pas en la forme attendue.
    pub fn depuis_graphe(g: &Graphe) -> Result<Self, ErreurCascade> {
        let pct = |nom: &str| -> Result<f32, ErreurCascade> {
            // L'opacité d'un jeton translucide, en pourcents (9,0 pour 9 %), exacte en flottant.
            Ok(g.opacite_pourcent(nom)? as f32)
        };
        let opacites = Opacites {
            bordure: pct("--background-modifier-border")?,
            bordure_survol: pct("--background-modifier-border-hover")?,
            survol: pct("--background-modifier-hover")?,
            survol_actif: pct("--background-modifier-active-hover")?,
        };
        Ok(depuis_le_graphe!(g, Palette {
            couleurs: [
                window_close_bg, text_on_accent, toggle_thumb_color, background_modifier_cover,
                background_overlay, text_on_overlay, background_modifier_error,
                background_modifier_warning, text_on_warning, color_red, color_orange,
                color_yellow, color_green, color_cyan, color_blue, color_purple, color_pink,
                link_color, text_highlight_bg, code_normal, code_comment, code_keyword,
                code_function, code_property, code_value, code_tag, code_operator, code_string,
                code_punctuation, code_important, interactive_accent, interactive_accent_hover,
                color_accent_1, color_accent_2, text_accent, background_modifier_border,
                background_modifier_border_hover, background_modifier_hover,
                background_modifier_active_hover, scrollbar_thumb_bg, scrollbar_active_thumb_bg,
                workspace_bg, color_accent, tab_bar_bg, background_secondary, background_primary,
                background_primary_alt, menu_bg, background_secondary_alt, tab_outline_color,
                text_normal, text_muted, text_faint, text_error, text_warning, text_success,
                pdf_page_bg,
            ],
            ombres: [menu_shadow, shadow_xs, shadow_s, shadow_card, shadow_l],
            reels: [avatar_bg_l, avatar_bg_c, avatar_fg_l, avatar_fg_c],
            autres: { opacites: opacites },
        }))
    }
}

impl Forme {
    /// La géométrie et les durées d'un graphe résolu.
    ///
    /// # Errors
    /// Un jeton de forme ne se résout pas, ou pas en la forme attendue.
    pub fn depuis_graphe(g: &Graphe) -> Result<Self, ErreurCascade> {
        Ok(depuis_le_graphe!(
            g,
            Forme {
                reels: [
                    header_height,
                    frame_button_width,
                    frame_right_space,
                    border_width,
                    radius_hair,
                    radius_xxs,
                    radius_xs,
                    radius_s,
                    radius_sm,
                    radius_m,
                    radius_ml,
                    radius_l,
                    radius_xl,
                    input_radius,
                    button_radius,
                    checkbox_radius,
                    clickable_icon_radius,
                    menu_radius,
                    modal_radius,
                    tooltip_radius,
                    code_radius,
                    icon_xs,
                    icon_s,
                    icon_m,
                    icon_l,
                    icon_stroke,
                    size_2_1,
                    size_2_2,
                    size_2_3,
                    size_2_5,
                    size_2_7,
                    size_2_9,
                    size_2_11,
                    size_2_13,
                    size_2_15,
                    size_2_17,
                    size_2_19,
                    size_4_1,
                    size_4_2,
                    size_4_3,
                    size_4_4,
                    size_4_5,
                    size_4_6,
                    size_4_7,
                    size_4_8,
                    size_4_9,
                    size_4_10,
                    size_4_11,
                    size_4_12,
                    size_4_14,
                    size_4_16,
                    size_4_18,
                    size_4_20,
                ],
                entiers: [
                    anim_duration_none,
                    anim_duration_superfast,
                    anim_duration_quick,
                    anim_duration_fast,
                    anim_duration_moderate,
                    anim_duration_slow,
                ],
                courbes: [anim_motion_smooth, anim_motion_in_out],
            }
        ))
    }
}

impl Typo {
    /// Les tailles et graisses d'un graphe résolu ; les piles de polices restent celles de
    /// [`Typo::commune`] (champs `'static`, voir `JETONS_FIGES`).
    ///
    /// # Errors
    /// Un jeton de typographie ne se résout pas, ou pas en la forme attendue.
    pub fn depuis_graphe(g: &Graphe) -> Result<Self, ErreurCascade> {
        let piles = Self::commune();
        Ok(depuis_le_graphe!(g, Typo {
            reels: [
                font_ui_small, font_ui_smallest, font_ui_smaller, font_ui_medium, font_ui_normal,
                font_ui_large, font_ui_larger, font_ui_largest, line_height_normal,
            ],
            entiers: [font_normal, font_medium, font_semibold, font_bold],
            autres: {
                font_interface: piles.font_interface,
                font_text: piles.font_text,
                font_monospace: piles.font_monospace,
                font_serif: piles.font_serif,
            },
        }))
    }
}

impl Theme {
    /// Le thème d'un graphe résolu.
    ///
    /// # Errors
    /// Un jeton ne se résout pas, ou pas en la forme attendue.
    pub fn depuis_graphe(mode: Mode, g: &Graphe) -> Result<Self, ErreurCascade> {
        Ok(Self {
            mode,
            palette: Palette::depuis_graphe(g)?,
            forme: Forme::depuis_graphe(g)?,
            typo: Typo::depuis_graphe(g)?,
        })
    }

    /// Le thème de `index.css`, sans surcharge. Les tables sont statiques et testées : un échec
    /// ici est un bug du dépôt, pas une entrée utilisateur.
    pub(crate) fn de_reference(mode: Mode) -> Self {
        match Graphe::par_defaut(mode).and_then(|g| Self::depuis_graphe(mode, &g)) {
            Ok(theme) => theme,
            Err(e) => panic!("table de jetons de index.css incohérente : {e}"),
        }
    }

    /// Le thème de `mode` avec des surcharges `(jeton, valeur CSS)` par-dessus : l'accent choisi
    /// des réglages, les entrées d'un `theme.json`. Les DÉRIVÉS sont recalculés (une surcharge de
    /// `--color-accent` change `--text-accent`, `--color-accent-1`…).
    ///
    /// Le nom peut s'écrire avec ou sans `--`. Une valeur vide est ignorée (c'est le « vide = celui
    /// du thème » de l'accent). Les surcharges s'appliquent dans l'ordre : la dernière gagne
    /// (poser l'accent choisi en dernier). Un jeton inconnu, une valeur illisible, circulaire ou
    /// du mauvais type donne un AVERTISSEMENT et la surcharge est ignorée ; jamais une erreur.
    pub fn avec_surcharges<N: AsRef<str>, V: AsRef<str>>(
        mode: Mode,
        surcharges: &[(N, V)],
    ) -> (Self, Vec<String>) {
        let mut avertissements = Vec::new();
        let Ok(mut graphe) = Graphe::par_defaut(mode) else {
            // Table interne cassée : on ne peut pas surcharger, on rend la référence.
            return (Self::de_reference(mode), avertissements);
        };
        for (nom, valeur) in surcharges {
            let valeur = valeur.as_ref().trim();
            if valeur.is_empty() {
                continue;
            }
            let nom = nom.as_ref().trim();
            let nom = if nom.starts_with("--") {
                nom.to_owned()
            } else {
                format!("--{nom}")
            };
            if JETONS_FIGES.contains(&nom.as_str()) {
                avertissements.push(format!(
                    "{nom} : les piles de polices ne se surchargent pas encore"
                ));
                continue;
            }
            if !graphe.contient(&nom) {
                avertissements.push(format!("{nom} : jeton inconnu, ignoré"));
                continue;
            }
            let expr = match Expr::analyser(valeur) {
                Ok(expr) => expr,
                Err(e) => {
                    avertissements.push(format!("{nom} : {e}"));
                    continue;
                }
            };
            let Some(ancienne) = graphe.definir(&nom, expr) else {
                continue;
            };
            // La surcharge doit laisser TOUT le thème résoluble : sinon on la retire.
            if let Err(e) = Self::depuis_graphe(mode, &graphe) {
                graphe.definir(&nom, ancienne);
                avertissements.push(format!("{nom} : {e}, ignoré"));
            }
        }
        match Self::depuis_graphe(mode, &graphe) {
            Ok(theme) => (theme, avertissements),
            Err(_) => (Self::de_reference(mode), avertissements),
        }
    }
}

/// Les surcharges d'un `theme.json` (ADR-005 §4) : un objet plat `{ "<jeton>": "<valeur CSS>" }`.
///
/// Rend les paires (triées par nom : `serde_json` range les clés) et des avertissements pour ce qui n'est pas une paire
/// utilisable (valeur ni texte ni nombre). Un JSON illisible, ou qui n'est pas un objet, donne
/// aucune paire et un avertissement.
pub fn surcharges_depuis_json(texte: &str) -> (Vec<(String, String)>, Vec<String>) {
    let mut avertissements = Vec::new();
    let valeur: serde_json::Value = match serde_json::from_str(texte) {
        Ok(v) => v,
        Err(e) => return (Vec::new(), vec![format!("theme.json illisible : {e}")]),
    };
    let serde_json::Value::Object(objet) = valeur else {
        return (
            Vec::new(),
            vec!["theme.json doit être un objet { jeton: valeur }".to_owned()],
        );
    };
    let mut paires = Vec::new();
    for (nom, valeur) in objet {
        match valeur {
            serde_json::Value::String(s) => paires.push((nom, s)),
            serde_json::Value::Number(n) => paires.push((nom, n.to_string())),
            _ => avertissements.push(format!("{nom} : la valeur doit être un texte, ignorée")),
        }
    }
    (paires, avertissements)
}
