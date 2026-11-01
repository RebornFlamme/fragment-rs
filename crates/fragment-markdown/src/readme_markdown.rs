// Port de app/src/ui/settings/readmeMarkdown.ts
//! Le README d'un plugin, RENDU dans le navigateur de plugins (comme Obsidian) —
//! ici sous forme d'ARBRE de nœuds, que la vue (`fragment-ui`) transforme en
//! éléments.
//!
//! La fenêtre du navigateur tient une App LÉGÈRE : le service `markdown:render@1`
//! n'y est pas, et son pipeline (live preview) résout les liens/embeds dans le
//! COFFRE — pas dans un dépôt GitHub. D'où ce rendu minimal et SÛR : titres,
//! paragraphes, listes (tâches), citations, code, tableaux, règles, emphase,
//! liens, images.
//!
//! ★ Aucun HTML n'est produit : l'arbre ne contient que des éléments connus ; le
//!   HTML BRUT d'un README (badges, `<p align>`…) sort en [`Node::Html`], que la
//!   vue DOIT assainir avec la politique de [`crate::sanitize_html`] avant de
//!   l'insérer (c'était `sanitizeHtml` + `fixHtml` dans le TS).
//! ★ Liens et images RELATIFS : résolus contre
//!   `https://raw.githubusercontent.com/<repo>/HEAD/`. Un lien s'ouvre dans le
//!   navigateur du système (la vue branche le clic sur l'attribut `href`) ; une
//!   image n'a JAMAIS de `src` : son URL résolue est dans `data-src`, la vue la
//!   fait charger (la CSP refuse `img-src https:`) et pose le `blob:` reçu.

use regex::Regex;
use std::sync::LazyLock;

/// Un nœud de l'arbre rendu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Element(Element),
    /// HTML brut, NON assaini : à passer dans `sanitize_html` avant l'insertion.
    Html(String),
}

/// Un élément : balise, attributs (dans l'ordre), enfants.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Element {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_owned(),
            ..Self::default()
        }
    }
    fn with_children(tag: &str, children: Vec<Node>) -> Self {
        Self {
            tag: tag.to_owned(),
            attrs: Vec::new(),
            children,
        }
    }
    fn set(&mut self, nom: &str, valeur: &str) {
        self.attrs.push((nom.to_owned(), valeur.to_owned()));
    }
    /// La valeur de l'attribut `nom`.
    pub fn attr(&self, nom: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == nom)
            .map(|(_, v)| v.as_str())
    }
    /// Le `textContent` de l'élément (le HTML brut n'y compte pas : il n'est pas encore un DOM).
    pub fn text_content(&self) -> String {
        text_content(&self.children)
    }
}

/// Le `textContent` d'une suite de nœuds.
pub fn text_content(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(t) => t.clone(),
            Node::Element(e) => e.text_content(),
            Node::Html(_) => String::new(),
        })
        .collect()
}

/// Le contexte de rendu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadmeContext {
    /// Base des URL relatives (finit par `/`).
    pub base: String,
    /// La vue sait-elle charger une image distante (`loadImage`) ? Sinon, pas de `data-src`.
    pub load_images: bool,
}

/// La base raw d'un dépôt GitHub (`owner/repo`) ; `branch` vaut `HEAD` par défaut dans le TS.
pub fn readme_base(repo: &str, branch: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo}/{branch}/")
}

// ─── URL ──────────────────────────────────────────────────────────────────

static SCHEMA: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^[a-z][a-z0-9+.-]*:"));
static SCHEMA_PERMIS: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(https?|mailto):"));

fn re(motif: &str) -> Regex {
    // Les motifs sont des constantes de ce fichier, éprouvées par les tests.
    Regex::new(motif).unwrap_or_else(|_| unreachable!("motif constant invalide : {motif}"))
}

/// Le `trim()` de JS (le BOM compte comme un blanc, pas en Rust).
fn trim_js(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

fn trim_start_js(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// Encode comme le fait `new URL(…).href` pour un chemin : octets hors ASCII,
/// espace et `"<>`{}` en `%XX`.
fn encoder_chemin(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii() && !matches!(c, ' ' | '"' | '<' | '>' | '`' | '{' | '}') && !c.is_control()
        {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// `new URL(rel, base).href` pour une base http(s) qui se termine par `/`.
fn joindre(base: &str, rel: &str) -> Option<String> {
    let fin_schema = base.find("://")? + 3;
    let apres = &base[fin_schema..];
    let (hote, chemin_base) = match apres.find('/') {
        Some(i) => (&apres[..i], &apres[i..]),
        None => (apres, "/"),
    };
    let origine = &base[..fin_schema + hote.len()];
    // Sépare chemin / requête / ancre.
    let (avant_ancre, ancre) = match rel.find('#') {
        Some(i) => (&rel[..i], &rel[i..]),
        None => (rel, ""),
    };
    let (chemin_rel, requete) = match avant_ancre.find('?') {
        Some(i) => (&avant_ancre[..i], &avant_ancre[i..]),
        None => (avant_ancre, ""),
    };
    // Le dossier de la base + le chemin relatif, points résolus.
    let dossier = &chemin_base[..=chemin_base.rfind('/').unwrap_or(0)];
    let mut segments: Vec<&str> = dossier.split('/').filter(|s| !s.is_empty()).collect();
    let rel_segments: Vec<&str> = chemin_rel.split('/').collect();
    let n = rel_segments.len();
    for (i, seg) in rel_segments.iter().enumerate() {
        match *seg {
            "." => {}
            ".." => {
                segments.pop();
            }
            "" if i + 1 != n => {}
            s => segments.push(s),
        }
    }
    // Un chemin qui se termine par `.` / `..` / `/` garde un `/` final.
    let finit_par_dossier = matches!(rel_segments.last(), Some(&"" | &"." | &".."));
    let mut chemin = format!("/{}", segments.join("/"));
    if finit_par_dossier && !chemin.ends_with('/') {
        chemin.push('/');
    }
    Some(format!(
        "{origine}{}{}{}",
        encoder_chemin(&chemin),
        encoder_chemin(requete),
        ancre
    ))
}

/// Résout une URL du README : absolue http(s)/mailto gardée, `//hôte` → https,
/// relative (`./x`, `x`, `/x`) → dans le dépôt. Ancre `#…` ou schéma refusé
/// (`javascript:`…) → `None`.
pub fn resolve_readme_url(url: &str, base: &str) -> Option<String> {
    let u = trim_js(url);
    if u.is_empty() || u.starts_with('#') {
        return None;
    }
    if u.starts_with("//") {
        return Some(format!("https:{u}"));
    }
    if SCHEMA.is_match(u) {
        return SCHEMA_PERMIS.is_match(u).then(|| u.to_owned());
    }
    joindre(base, u.trim_start_matches('/'))
}

// ─── Blocs ────────────────────────────────────────────────────────────────

static FENCE: LazyLock<Regex> = LazyLock::new(|| re(r"^ {0,3}(`{3,}|~{3,})\s*([A-Za-z0-9_+-]*)"));
static HEADING: LazyLock<Regex> = LazyLock::new(|| re(r"^ {0,3}(#{1,6})\s+(.*?)\s*#*\s*$"));
static QUOTE: LazyLock<Regex> = LazyLock::new(|| re(r"^ {0,3}>\s?"));
static LIST: LazyLock<Regex> = LazyLock::new(|| re(r"^(\s*)([-*+]|[0-9]{1,9}[.)])\s+(.*)$"));
static HTML_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| re(r"^ {0,3}<(?:[a-zA-Z][A-Za-z0-9_-]*|/[a-zA-Z][A-Za-z0-9_-]*|!--)"));
static TABLE_SEP: LazyLock<Regex> =
    LazyLock::new(|| re(r"^\s*\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)*\|?\s*$"));
static TACHE: LazyLock<Regex> = LazyLock::new(|| re(r"^\[([ xX])\]\s+"));

/// `^ {0,3}([-*_])(?:\s*\1){2,}\s*$` — la rétro-référence `\1` n'existe pas dans `regex`.
fn est_regle(ligne: &str) -> bool {
    let sans = ligne
        .strip_prefix("   ")
        .or_else(|| ligne.strip_prefix("  "))
        .or_else(|| ligne.strip_prefix(' '))
        .unwrap_or(ligne);
    let mut it = sans.chars();
    let Some(c) = it.next().filter(|c| matches!(c, '-' | '*' | '_')) else {
        return false;
    };
    let mut n = 1;
    for x in it {
        if x == c {
            n += 1;
        } else if !(x.is_whitespace() || x == '\u{feff}') {
            return false;
        }
    }
    n >= 3
}

fn demarre_un_bloc(ligne: &str) -> bool {
    FENCE.is_match(ligne)
        || HEADING.is_match(ligne)
        || est_regle(ligne)
        || QUOTE.is_match(ligne)
        || LIST.is_match(ligne)
        || HTML_BLOCK.is_match(ligne)
}

/// Rend `md` en arbre.
pub fn render_readme(md: &str, ctx: &ReadmeContext) -> Vec<Node> {
    let normal = md.replace("\r\n", "\n").replace('\r', "\n");
    let lignes: Vec<String> = normal.split('\n').map(str::to_owned).collect();
    let mut out = Vec::new();
    blocks(&mut out, &lignes, ctx);
    out
}

fn blocks(parent: &mut Vec<Node>, lines: &[String], ctx: &ReadmeContext) {
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if trim_js(line).is_empty() {
            i += 1;
            continue;
        }

        if let Some(fence) = FENCE.captures(line) {
            let cloture = fence.get(1).map_or("", |m| m.as_str());
            let langue = fence.get(2).map_or("", |m| m.as_str());
            let mut code: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && !trim_start_js(&lines[i]).starts_with(cloture) {
                code.push(&lines[i]);
                i += 1;
            }
            i += 1; // la clôture
            let texte = code.join("\n");
            let mut c = Element::new("code");
            if !texte.is_empty() {
                c.children.push(Node::Text(texte));
            }
            if !langue.is_empty() {
                c.set("data-lang", langue);
            }
            parent.push(Node::Element(Element::with_children(
                "pre",
                vec![Node::Element(c)],
            )));
            continue;
        }

        if let Some(h) = HEADING.captures(line) {
            let niveau = h.get(1).map_or(1, |m| m.as_str().len());
            let texte = h.get(2).map_or("", |m| m.as_str());
            parent.push(Node::Element(Element::with_children(
                &format!("h{niveau}"),
                inline(texte, ctx),
            )));
            i += 1;
            continue;
        }

        if est_regle(line) {
            parent.push(Node::Element(Element::new("hr")));
            i += 1;
            continue;
        }

        if QUOTE.is_match(line) {
            let mut interieur: Vec<String> = Vec::new();
            while i < lines.len() && !trim_js(&lines[i]).is_empty() && QUOTE.is_match(&lines[i]) {
                interieur.push(QUOTE.replace(&lines[i], "").into_owned());
                i += 1;
            }
            let mut enfants = Vec::new();
            blocks(&mut enfants, &interieur, ctx);
            parent.push(Node::Element(Element::with_children("blockquote", enfants)));
            continue;
        }

        if LIST.is_match(line) {
            i = list(parent, lines, i, ctx);
            continue;
        }

        if HTML_BLOCK.is_match(line) {
            let mut html: Vec<&str> = Vec::new();
            while i < lines.len() && !trim_js(&lines[i]).is_empty() {
                html.push(&lines[i]);
                i += 1;
            }
            parent.push(Node::Html(html.join("\n")));
            continue;
        }

        if line.contains('|')
            && i + 1 < lines.len()
            && TABLE_SEP.is_match(&lines[i + 1])
            && lines[i + 1].contains('-')
        {
            i = table(parent, lines, i, ctx);
            continue;
        }

        // Paragraphe : jusqu'à la ligne vide ou un autre bloc.
        let mut para: Vec<&str> = vec![trim_js(line)];
        i += 1;
        while i < lines.len() && !trim_js(&lines[i]).is_empty() && !demarre_un_bloc(&lines[i]) {
            para.push(trim_js(&lines[i]));
            i += 1;
        }
        parent.push(Node::Element(Element::with_children(
            "p",
            inline(&para.join("\n"), ctx),
        )));
    }
}

fn leading(ligne: &str) -> usize {
    ligne.chars().count() - trim_start_js(ligne).chars().count()
}

fn est_numerote(marqueur: &str) -> bool {
    marqueur.chars().any(|c| c.is_ascii_digit())
}

/// `slice(n)` sur des caractères.
fn depuis(s: &str, n: usize) -> String {
    s.chars().skip(n).collect()
}

/// Une liste (à puces ou numérotée) à partir de `i` ; rend l'index qui suit.
fn list(parent: &mut Vec<Node>, lines: &[String], start: usize, ctx: &ReadmeContext) -> usize {
    let mut i = start;
    let Some(first) = LIST.captures(&lines[i]) else {
        return i + 1;
    };
    let indent = first.get(1).map_or(0, |m| m.as_str().chars().count());
    let marqueur0 = first.get(2).map_or("", |m| m.as_str());
    let ordered = est_numerote(marqueur0);
    let mut liste = Element::new(if ordered { "ol" } else { "ul" });
    if ordered {
        let n: u64 = marqueur0.trim_end_matches(['.', ')']).parse().unwrap_or(1);
        if n != 1 {
            liste.set("start", &n.to_string());
        }
    }

    while i < lines.len() {
        let Some(m) = LIST.captures(&lines[i]) else {
            break;
        };
        let m1 = m.get(1).map_or("", |x| x.as_str());
        let m2 = m.get(2).map_or("", |x| x.as_str());
        if m1.chars().count() != indent || est_numerote(m2) != ordered {
            break;
        }
        let content = m1.chars().count() + m2.chars().count() + 1;
        let mut item: Vec<String> = vec![m.get(3).map_or("", |x| x.as_str()).to_owned()];
        i += 1;
        // Suite de l'item : lignes indentées (sous-listes, paragraphes) ou continuation paresseuse.
        while i < lines.len() {
            let l = &lines[i];
            if trim_js(l).is_empty() {
                if let Some(next) = lines.get(i + 1)
                    && !trim_js(next).is_empty()
                    && leading(next) > indent
                {
                    item.push(String::new());
                    i += 1;
                    continue;
                }
                break;
            }
            if leading(l) > indent {
                item.push(depuis(l, content.min(leading(l))));
                i += 1;
                continue;
            }
            if LIST.is_match(l) || demarre_un_bloc(l) {
                break;
            }
            item.push(trim_js(l).to_owned());
            i += 1;
        }
        liste.children.push(Node::Element(item_de_liste(item, ctx)));
    }

    // Ligne vide entre deux items : on continue la même liste.
    if i < lines.len() && trim_js(&lines[i]).is_empty() {
        let mut j = i;
        while j < lines.len() && trim_js(&lines[j]).is_empty() {
            j += 1;
        }
        if let Some(m) = lines.get(j).and_then(|l| LIST.captures(l))
            && m.get(1).map_or(0, |x| x.as_str().chars().count()) == indent
            && est_numerote(m.get(2).map_or("", |x| x.as_str())) == ordered
        {
            let mut reste = Vec::new();
            let suivant = list(&mut reste, lines, j, ctx);
            if let Some(Node::Element(suite)) = reste.into_iter().next() {
                liste.children.extend(suite.children);
            }
            parent.push(Node::Element(liste));
            return suivant;
        }
    }
    parent.push(Node::Element(liste));
    i
}

/// Un `<li>` à partir des lignes de l'item (case à cocher, blocs, liste « serrée »).
fn item_de_liste(mut item: Vec<String>, ctx: &ReadmeContext) -> Element {
    let mut li = Element::new("li");
    if let Some(t) = TACHE.captures(&item[0]) {
        let coche = t.get(1).map_or(" ", |m| m.as_str()) != " ";
        let longueur = t.get(0).map_or(0, |m| m.as_str().chars().count());
        item[0] = depuis(&item[0], longueur);
        li.set("class", "task-list-item");
        let mut box_ = Element::new("input");
        box_.set("type", "checkbox");
        box_.set("disabled", "");
        if coche {
            box_.set("checked", "");
        }
        li.children.push(Node::Element(box_));
    }
    blocks(&mut li.children, &item, ctx);
    // Liste « serrée » : un paragraphe seul se déplie dans l'item.
    let nb_p = li
        .children
        .iter()
        .filter(|n| matches!(n, Node::Element(e) if e.tag == "p"))
        .count();
    if nb_p == 1 {
        let enfants = std::mem::take(&mut li.children);
        for n in enfants {
            match n {
                Node::Element(p) if p.tag == "p" => li.children.extend(p.children),
                autre => li.children.push(autre),
            }
        }
    }
    li
}

/// `line.trim().replace(/^\|/,'').replace(/\|$/,'').split(/(?<!\\)\|/).map(trim)`.
fn cells(ligne: &str) -> Vec<String> {
    let t = trim_js(ligne);
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut precedent: Option<char> = None;
    for c in t.chars() {
        if c == '|' && precedent != Some('\\') {
            out.push(trim_js(&cur).to_owned());
            cur.clear();
        } else {
            cur.push(c);
        }
        precedent = Some(c);
    }
    out.push(trim_js(&cur).to_owned());
    out
}

fn table(parent: &mut Vec<Node>, lines: &[String], start: usize, ctx: &ReadmeContext) -> usize {
    let mut i = start;
    let tete: Vec<Node> = cells(&lines[i])
        .iter()
        .map(|c| Node::Element(Element::with_children("th", inline(c, ctx))))
        .collect();
    let mut table = Element::new("table");
    table.children.push(Node::Element(Element::with_children(
        "thead",
        vec![Node::Element(Element::with_children("tr", tete))],
    )));
    i += 2;
    let mut corps = Element::new("tbody");
    while i < lines.len() && !trim_js(&lines[i]).is_empty() && lines[i].contains('|') {
        let tds: Vec<Node> = cells(&lines[i])
            .iter()
            .map(|c| Node::Element(Element::with_children("td", inline(c, ctx))))
            .collect();
        corps
            .children
            .push(Node::Element(Element::with_children("tr", tds)));
        i += 1;
    }
    table.children.push(Node::Element(corps));
    parent.push(Node::Element(table));
    i
}

// ─── En ligne ─────────────────────────────────────────────────────────────

static LIEN_CIBLE: LazyLock<Regex> =
    LazyLock::new(|| re(r#"^\(\s*<?([^\s)>]*)>?(?:\s+["'(][^)]*["')])?\s*\)"#));
static AUTOLIEN: LazyLock<Regex> = LazyLock::new(|| re(r"^<((?:https?://|mailto:)[^>\s]+)>"));
static BR: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^<br\s*/?>"));
static IMG: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^<img\b[^>]*>"));
static AUTRE_BALISE: LazyLock<Regex> = LazyLock::new(|| re(r"^</?[a-zA-Z][^>]*>"));

struct LienCrochets {
    label: String,
    url: String,
    /// Octet qui suit le lien dans le texte.
    fin: usize,
}

/// `[label](url)` à partir de l'octet `start` (crochets imbriqués : `[![img](a)](b)`).
fn bracket_link(text: &str, start: usize) -> Option<LienCrochets> {
    if !text[start..].starts_with('[') {
        return None;
    }
    let mut depth = 0;
    let mut j: Option<usize> = None;
    let mut it = text[start..].char_indices();
    while let Some((k, c)) = it.next() {
        if c == '\\' {
            it.next();
            continue;
        }
        if c == '[' {
            depth += 1;
        } else if c == ']' {
            depth -= 1;
            if depth == 0 {
                j = Some(start + k);
                break;
            }
        }
    }
    let j = j?;
    if !text[j + 1..].starts_with('(') {
        return None;
    }
    let m = LIEN_CIBLE.captures(&text[j + 1..])?;
    let tout = m.get(0)?;
    Some(LienCrochets {
        label: text[start + 1..j].to_owned(),
        url: m.get(1).map_or("", |x| x.as_str()).to_owned(),
        fin: j + 1 + tout.end(),
    })
}

fn est_mot(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `` ^(`+)([\s\S]*?[^`])\1(?!`) `` : (texte du code, octets consommés).
fn code_en_ligne(rest: &str) -> Option<(String, usize)> {
    let course = rest.chars().take_while(|c| *c == '`').count();
    // ★ Le moteur essaie la suite entière d'abord, puis des suites plus courtes.
    for n in (1..=course).rev() {
        let apres = &rest[n..];
        for (k, c) in apres.char_indices() {
            // `k` est le début d'un candidat de fermeture ; le contenu `apres[..k]` finit par `[^`]`.
            let _ = c;
            if k == 0 {
                continue;
            }
            let Some(dernier) = apres[..k].chars().next_back() else {
                continue;
            };
            if dernier != '`'
                && apres[k..].starts_with(&"`".repeat(n))
                && !apres[k + n..].starts_with('`')
            {
                return Some((apres[..k].to_owned(), n + k + n));
            }
        }
    }
    None
}

/// La fermeture lazy de `(?=\S)([\s\S]*?\S)<d>` : octet de début de la fermeture.
/// `apres_ferme` valide le caractère qui suit la fermeture (`(?!\w)` pour l'italique).
fn fermeture(
    rest: &str,
    ouverture: usize,
    d: &str,
    apres_ferme: impl Fn(Option<char>) -> bool,
) -> Option<usize> {
    let contenu = &rest[ouverture..];
    // (?=\S) : le premier caractère du contenu n'est pas un blanc.
    let premier = contenu.chars().next()?;
    if premier.is_whitespace() || premier == '\u{feff}' {
        return None;
    }
    for (k, c) in contenu.char_indices() {
        let q = k + c.len_utf8(); // le contenu `[..q]` finit par le caractère `c`
        if c.is_whitespace() || c == '\u{feff}' {
            continue;
        }
        if contenu[q..].starts_with(d) && apres_ferme(contenu[q + d.len()..].chars().next()) {
            return Some(ouverture + q);
        }
    }
    None
}

fn inline(text: &str, ctx: &ReadmeContext) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    let mut buf = String::new();
    fn vider(out: &mut Vec<Node>, buf: &mut String) {
        if !buf.is_empty() {
            out.push(Node::Text(std::mem::take(buf)));
        }
    }
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let Some(ch) = rest.chars().next() else {
            break;
        };
        let apres_ch = &rest[ch.len_utf8()..];
        let suivant = apres_ch.chars().next();

        if ch == '\\'
            && let Some(n) = suivant
            && "\\`*_{}[]()#+-.!|<>~".contains(n)
        {
            buf.push(n);
            i += 1 + n.len_utf8();
            continue;
        }
        if ch == '\n' {
            buf.push(' ');
            i += 1;
            continue;
        }

        if ch == '`'
            && let Some((code, conso)) = code_en_ligne(rest)
        {
            vider(&mut out, &mut buf);
            out.push(Node::Element(Element::with_children(
                "code",
                vec![Node::Text(trim_js(&code).to_owned())],
            )));
            i += conso;
            continue;
        }

        if ch == '!'
            && suivant == Some('[')
            && let Some(l) = bracket_link(text, i + 1)
        {
            vider(&mut out, &mut buf);
            out.push(Node::Element(image(&l.url, &l.label, ctx)));
            i = l.fin;
            continue;
        }

        if ch == '['
            && let Some(l) = bracket_link(text, i)
        {
            vider(&mut out, &mut buf);
            let mut lien = lien_de(&l.url, ctx);
            lien.children = inline(&l.label, ctx);
            out.push(Node::Element(lien));
            i = l.fin;
            continue;
        }

        if ch == '<' {
            if let Some(m) = AUTOLIEN.captures(rest) {
                vider(&mut out, &mut buf);
                let url = m.get(1).map_or("", |x| x.as_str());
                let mut lien = lien_de(url, ctx);
                lien.children = vec![Node::Text(url.to_owned())];
                out.push(Node::Element(lien));
                i += m.get(0).map_or(1, |x| x.end());
                continue;
            }
            if let Some(m) = BR.find(rest) {
                vider(&mut out, &mut buf);
                out.push(Node::Element(Element::new("br")));
                i += m.end();
                continue;
            }
            if let Some(m) = IMG.find(rest) {
                vider(&mut out, &mut buf);
                out.push(Node::Html(m.as_str().to_owned()));
                i += m.end();
                continue;
            }
            // Autre balise en ligne : retirée (son texte reste).
            if let Some(m) = AUTRE_BALISE.find(rest) {
                i += m.end();
                continue;
            }
        }

        if ch == '*' || ch == '_' {
            // Gras : `**x**` / `__x__` (`^(\*\*|__)(?=\S)([\s\S]*?\S)\1`).
            let d2 = format!("{ch}{ch}");
            if rest.starts_with(&d2)
                && let Some(q) = fermeture(rest, 2, &d2, |_| true)
            {
                vider(&mut out, &mut buf);
                out.push(Node::Element(Element::with_children(
                    "strong",
                    inline(&rest[2..q], ctx),
                )));
                i += q + 2;
                continue;
            }
            // Italique : `*x*` / `_x_` (`_` jamais collé à un mot : `snake_case`).
            let precede_un_mot = text[..i].chars().next_back().is_some_and(est_mot);
            if (ch == '*' || !precede_un_mot)
                && let Some(q) = fermeture(rest, 1, &ch.to_string(), |c| !c.is_some_and(est_mot))
            {
                vider(&mut out, &mut buf);
                out.push(Node::Element(Element::with_children(
                    "em",
                    inline(&rest[1..q], ctx),
                )));
                i += q + 1;
                continue;
            }
        }
        if ch == '~'
            && rest.starts_with("~~")
            && let Some(q) = fermeture(rest, 2, "~~", |_| true)
        {
            vider(&mut out, &mut buf);
            out.push(Node::Element(Element::with_children(
                "del",
                inline(&rest[2..q], ctx),
            )));
            i += q + 2;
            continue;
        }

        buf.push(ch);
        i += ch.len_utf8();
    }
    vider(&mut out, &mut buf);
    out
}

/// L'élément d'un lien : `<a href title>` ; `<span>` si l'URL est refusée.
fn lien_de(url: &str, ctx: &ReadmeContext) -> Element {
    match resolve_readme_url(url, &ctx.base) {
        None => Element::new("span"),
        Some(href) => {
            let mut a = Element::new("a");
            a.set("href", &href);
            a.set("title", &href);
            a
        }
    }
}

/// `<img alt data-src>` : jamais de `src` (la CSP refuse `img-src https:`).
fn image(url: &str, alt: &str, ctx: &ReadmeContext) -> Element {
    let mut img = Element::new("img");
    img.set("alt", alt);
    if let Some(src) = resolve_readme_url(url, &ctx.base)
        && ctx.load_images
        && (src.starts_with("http://") || src.starts_with("https://"))
    {
        img.set("data-src", &src);
    }
    img
}

// ─── Lecture de l'arbre ───────────────────────────────────────────────────

fn parcourir<'a>(nodes: &'a [Node], f: &mut impl FnMut(&'a Element)) {
    for n in nodes {
        if let Node::Element(e) = n {
            f(e);
            parcourir(&e.children, f);
        }
    }
}

/// Les `href` des liens de l'arbre, dans l'ordre du document.
pub fn links(nodes: &[Node]) -> Vec<String> {
    let mut out = Vec::new();
    parcourir(nodes, &mut |e| {
        if e.tag == "a"
            && let Some(h) = e.attr("href")
        {
            out.push(h.to_owned());
        }
    });
    out
}

/// Les URL (`data-src`) des images de l'arbre à faire charger, dans l'ordre du document.
pub fn images(nodes: &[Node]) -> Vec<String> {
    let mut out = Vec::new();
    parcourir(nodes, &mut |e| {
        if e.tag == "img"
            && let Some(s) = e.attr("data-src")
        {
            out.push(s.to_owned());
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> String {
        readme_base("alice/demo", "HEAD")
    }
    fn ctx() -> ReadmeContext {
        ReadmeContext {
            base: base(),
            load_images: true,
        }
    }

    /// Tous les éléments `tag` de l'arbre, dans l'ordre.
    fn trouver<'a>(nodes: &'a [Node], tag: &str) -> Vec<&'a Element> {
        let mut out = Vec::new();
        parcourir(nodes, &mut |e| {
            if e.tag == tag {
                out.push(e);
            }
        });
        out
    }
    fn enfants_de<'a>(e: &'a Element, tag: &str) -> Vec<&'a Element> {
        e.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(x) if x.tag == tag => Some(x),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn readme_markdown_resout_les_url_relative_raw_du_depot_absolue_gardee_ancre_javascript_refusees()
     {
        let b = base();
        assert_eq!(b, "https://raw.githubusercontent.com/alice/demo/HEAD/");
        assert_eq!(
            resolve_readme_url("./img/a.png", &b),
            Some(format!("{b}img/a.png"))
        );
        assert_eq!(
            resolve_readme_url("/docs/x.md", &b),
            Some(format!("{b}docs/x.md"))
        );
        assert_eq!(
            resolve_readme_url("https://x.org/a", &b),
            Some("https://x.org/a".into())
        );
        assert_eq!(resolve_readme_url("#usage", &b), None);
        assert_eq!(resolve_readme_url("javascript:alert(1)", &b), None);
    }

    #[test]
    fn readme_markdown_titres_paragraphes_listes_taches_code_citation_tableau() {
        let md = [
            "# Demo",
            "",
            "Un **plugin** *utile* avec `code`.",
            "",
            "- un",
            "- deux",
            "  - sous",
            "",
            "1. a",
            "2. b",
            "",
            "- [x] fait",
            "",
            "```js",
            "const a = 1 < 2;",
            "```",
            "",
            "> cité",
            "",
            "| A | B |",
            "|---|---|",
            "| 1 | 2 |",
        ]
        .join("\n");
        let el = render_readme(&md, &ctx());
        let un = |tag: &str| trouver(&el, tag).into_iter().next();
        assert_eq!(un("h1").map(Element::text_content), Some("Demo".into()));
        let p = trouver(&el, "p");
        assert_eq!(
            enfants_de(p[0], "strong").first().map(|e| e.text_content()),
            Some("plugin".into())
        );
        assert_eq!(
            enfants_de(p[0], "em").first().map(|e| e.text_content()),
            Some("utile".into())
        );
        assert_eq!(
            enfants_de(p[0], "code").first().map(|e| e.text_content()),
            Some("code".into())
        );
        // `ul > li` : au moins 2 (« un », « deux »).
        let uls = trouver(&el, "ul");
        let li_directs: usize = uls.iter().map(|u| enfants_de(u, "li").len()).sum();
        assert!(li_directs >= 2);
        // `ul ul li` : « sous ».
        let sous = trouver(&uls[0].children, "ul");
        assert_eq!(
            enfants_de(sous[0], "li").first().map(|l| l.text_content()),
            Some("sous".into())
        );
        // `ol > li` : 2.
        let ol = trouver(&el, "ol");
        assert_eq!(enfants_de(ol[0], "li").len(), 2);
        // `li.task-list-item input` coché.
        let tache = trouver(&el, "li")
            .into_iter()
            .find(|l| l.attr("class") == Some("task-list-item"));
        let input = tache.and_then(|l| enfants_de(l, "input").first().copied());
        assert!(input.is_some_and(|i| i.attr("checked").is_some()));
        assert_eq!(
            trouver(&trouver(&el, "pre")[0].children, "code")[0].text_content(),
            "const a = 1 < 2;"
        );
        assert_eq!(
            un("blockquote").map(Element::text_content),
            Some("cité".into())
        );
        assert_eq!(trouver(&el, "td").len(), 2);
    }

    #[test]
    fn readme_markdown_liens_resolus_dans_l_arbre_jamais_de_navigation_image_chargee_via_data_src()
    {
        let b = base();
        let el = render_readme(
            "[Doc](docs/a.md) [![badge](https://img.shields.io/x.svg)](https://x.org) ![shot](./s.png)",
            &ctx(),
        );
        // Le premier lien : résolu dans le dépôt (la vue branche `openLink` dessus).
        assert_eq!(links(&el).first(), Some(&format!("{b}docs/a.md")));
        assert_eq!(
            images(&el),
            vec![
                "https://img.shields.io/x.svg".to_owned(),
                format!("{b}s.png")
            ]
        );
        // L'URL distante n'est jamais posée en src (CSP) : seulement data-src.
        for img in trouver(&el, "img") {
            assert_eq!(img.attr("src"), None);
        }
    }

    #[test]
    fn readme_markdown_le_html_brut_sort_en_noeud_html_a_assainir_par_la_vue() {
        // ★ TS : le HTML est assaini par DOMPurify dans `renderReadme`. Ici l'arbre garde le
        //   HTML BRUT dans un `Node::Html` ; l'assainissement (politique de `sanitize_html`)
        //   est fait par la vue, qui a un DOM. Le même texte, donc, sans rien perdre.
        let html = "<p align=\"center\"><img src=\"logo.png\" onerror=\"alert(1)\"><script>alert(1)</script><a href=\"javascript:alert(1)\">x</a></p>";
        let el = render_readme(html, &ctx());
        assert_eq!(el, vec![Node::Html(html.to_owned())]);
        // Et la politique tranche bien comme le test TS : script retiré, onerror retiré, href refusé.
        use crate::sanitize_html::{filter_element, sanitize_opening_tag};
        assert!(filter_element("script", &[]).is_none());
        assert_eq!(
            sanitize_opening_tag("<img src=\"logo.png\" onerror=\"alert(1)\">")
                .map(|t| t.attributes),
            Some(vec![("src".to_owned(), "logo.png".to_owned())])
        );
        assert_eq!(
            sanitize_opening_tag("<a href=\"javascript:alert(1)\">").map(|t| t.attributes),
            Some(vec![])
        );
    }

    // ── Ajouts du port ──
    #[test]
    fn port_url_relatives_points_et_requetes() {
        let b = base();
        assert_eq!(
            resolve_readme_url("../x.md", &b),
            Some("https://raw.githubusercontent.com/alice/demo/x.md".into())
        );
        assert_eq!(
            resolve_readme_url("//cdn.x.org/a.png", &b),
            Some("https://cdn.x.org/a.png".into())
        );
        assert_eq!(
            resolve_readme_url("a b.png?v=1", &b),
            Some(format!("{b}a%20b.png?v=1"))
        );
        assert_eq!(
            resolve_readme_url("mailto:a@b.c", &b),
            Some("mailto:a@b.c".into())
        );
        assert_eq!(resolve_readme_url("  ", &b), None);
    }

    #[test]
    fn port_inline_emphases_code_et_echappements() {
        let el = render_readme(
            "snake_case_name et __fort__ `` a`b `` \\*pas\\* ~~barre~~",
            &ctx(),
        );
        let p = trouver(&el, "p")[0];
        assert_eq!(enfants_de(p, "strong")[0].text_content(), "fort");
        assert_eq!(enfants_de(p, "code")[0].text_content(), "a`b");
        assert_eq!(enfants_de(p, "del")[0].text_content(), "barre");
        assert!(enfants_de(p, "em").is_empty());
        assert!(p.text_content().contains("snake_case_name"));
        assert!(p.text_content().contains("*pas*"));
    }

    #[test]
    fn port_listes_serrees_numerotees_et_continuation() {
        let el = render_readme("3. a\n4. b\n\n5. c", &ctx());
        let ol = trouver(&el, "ol")[0];
        assert_eq!(ol.attr("start"), Some("3"));
        assert_eq!(enfants_de(ol, "li").len(), 3);
        // Liste serrée : pas de <p> dans l'item.
        assert!(trouver(&el, "p").is_empty());
    }

    #[test]
    fn port_regle_et_texte_non_ascii() {
        let el = render_readme("é — ok\n\n* * *\n\n## Titre é ##", &ctx());
        assert_eq!(trouver(&el, "hr").len(), 1);
        assert_eq!(trouver(&el, "h2")[0].text_content(), "Titre é");
        assert_eq!(trouver(&el, "p")[0].text_content(), "é — ok");
    }
}
