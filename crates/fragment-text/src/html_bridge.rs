// Port de app/src/shared/htmlBridge.ts (la partie PURE : injection du pont,
// ressources servables, aplatissement du texte)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le PONT d'une page HTML du coffre — en TS, le script que le protocole injecte
//  en tête d'un `.html` servi à la vue HTML, et qui APLATIT la page pour en faire
//  une surface de texte : ses nœuds texte dans l'ordre, un `\n` entre deux blocs ;
//  position = index + 1, 0 = le coin (voir `html_text_model`).
//
//  Le script lui-même (`HTML_BRIDGE_SCRIPT`, du JS exécuté PAR la page : thème,
//  hauteur, liens, clavier, sélection, rects) n'est pas porté ici : il n'a de sens
//  qu'avec le moteur de page que fragment-html choisira. Ce fichier garde les
//  trois règles pures qu'il contient :
//    - `inject_html_bridge` : OÙ le pont s'insère dans le HTML d'une page ;
//    - `page_servable` : CE QUE la racine `page` sert à une page vivante ;
//    - `flatten_page` : COMMENT la page devient du texte (la fonction `aplatir`
//      du script), sur un arbre de nœuds minimal plutôt que sur le DOM.
// ═══════════════════════════════════════════════════════════════════════════

/// La marque des messages du pont (l'app ignore tout autre message).
pub const HTML_BRIDGE_MARK: &str = "fragmentHtml";

/// Le HTML d'une page, le pont injecté le PLUS TÔT possible : juste après la
/// balise `<head>` (avant tout script de la page), sinon après `<html>`, sinon en tête.
/// `script` : le contenu de la balise `<script>` à poser.
pub fn inject_html_bridge(html: &str, script: &str) -> String {
    let balise = format!("<script>{script}</script>");
    let coupe = fin_de_balise_ouvrante(html, "head")
        .or_else(|| fin_de_balise_ouvrante(html, "html"))
        .unwrap_or(0);
    let mut out = String::with_capacity(html.len() + balise.len());
    out.push_str(&html[..coupe]);
    out.push_str(&balise);
    out.push_str(&html[coupe..]);
    out
}

/// TS : `/<nom(\s[^>]*)?>/i` — l'index juste après la première balise ouvrante
/// `<nom …>` (casse ignorée). `<header>` n'est pas `<head>` : après le nom vient
/// `>` ou un blanc, rien d'autre.
fn fin_de_balise_ouvrante(html: &str, nom: &str) -> Option<usize> {
    let bas = html.to_ascii_lowercase();
    let motif = format!("<{nom}");
    let mut depuis = 0;
    while let Some(rel) = bas[depuis..].find(&motif) {
        let debut = depuis + rel;
        let apres = debut + motif.len();
        match bas.as_bytes().get(apres) {
            Some(b'>') => return Some(apres + 1),
            Some(c) if c.is_ascii_whitespace() => {
                // `[^>]*>` : jusqu'au premier `>` ; sans `>`, pas de balise.
                return bas[apres..].find('>').map(|k| apres + k + 1);
            }
            _ => depuis = apres,
        }
    }
    None
}

/// Ce que la racine `page` sert à une page HTML vivante : des RESSOURCES WEB, hors
/// des dossiers cachés. Une note (.md), un PDF, un réglage ne sont jamais lisibles
/// par un script de page (il pourrait les envoyer sur le réseau). Pur, testé.
const EXTENSIONS_WEB: &[&str] = &[
    "html", "htm", "js", "mjs", "cjs", "css", "json", "map", "wasm", "png", "jpg", "jpeg", "gif",
    "webp", "avif", "svg", "ico", "bmp", "woff", "woff2", "ttf", "otf", "mp3", "wav", "ogg", "oga",
    "flac", "m4a", "mp4", "webm", "ogv", "mov", "glb", "gltf", "bin", "obj", "mtl", "hdr", "ktx2",
    "csv", "tsv", "xml", "txt", "vtt", "geojson",
];

pub fn page_servable(chemin: &str) -> bool {
    let segments: Vec<&str> = chemin.split('/').filter(|s| !s.is_empty()).collect();
    let Some(nom) = segments.last() else {
        return false;
    };
    if segments.iter().any(|s| s.starts_with('.')) {
        return false;
    }
    match nom.rfind('.') {
        Some(point) if point > 0 => {
            let ext = nom[point + 1..].to_ascii_lowercase();
            EXTENSIONS_WEB.contains(&ext.as_str())
        }
        _ => false,
    }
}

/// Un nœud du DOM d'une page, réduit à ce que l'aplatissement lit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HtmlNode {
    Text(String),
    Element {
        tag: String,
        children: Vec<HtmlNode>,
    },
}

impl HtmlNode {
    pub fn text(s: &str) -> Self {
        HtmlNode::Text(s.to_owned())
    }

    pub fn element(tag: &str, children: Vec<HtmlNode>) -> Self {
        HtmlNode::Element {
            tag: tag.to_owned(),
            children,
        }
    }
}

/// Un nœud texte retenu dans le texte aplati.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSegment {
    /// L'ordinal du nœud texte dans le parcours en profondeur du `<body>` : tous
    /// les nœuds texte RENCONTRÉS comptent (ceux ignorés comme blancs de mise en
    /// forme aussi ; pas le contenu des éléments sautés, qui n'est pas parcouru).
    /// Le propriétaire du DOM retrouve le nœud en comptant pareil.
    pub node: usize,
    /// Son début dans le texte aplati (octets)…
    pub start: usize,
    /// … et sa longueur.
    pub len: usize,
}

/// Le texte aplati d'une page et ses segments.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FlatText {
    pub text: String,
    pub segs: Vec<TextSegment>,
}

/// Les éléments dont le contenu n'est pas du texte de la page.
fn saute(tag: &str) -> bool {
    matches!(
        tag,
        "SCRIPT"
            | "STYLE"
            | "NOSCRIPT"
            | "TEMPLATE"
            | "TITLE"
            | "TEXTAREA"
            | "SELECT"
            | "CANVAS"
            | "VIDEO"
            | "AUDIO"
            | "OBJECT"
            | "IFRAME"
    )
}

/// Les éléments de BLOC : un `\n` virtuel les sépare (liste de balises, pas de
/// `getComputedStyle`).
fn bloc(tag: &str) -> bool {
    matches!(
        tag,
        "ADDRESS"
            | "ARTICLE"
            | "ASIDE"
            | "BLOCKQUOTE"
            | "BR"
            | "CAPTION"
            | "DD"
            | "DETAILS"
            | "DIALOG"
            | "DIV"
            | "DL"
            | "DT"
            | "FIELDSET"
            | "FIGCAPTION"
            | "FIGURE"
            | "FOOTER"
            | "FORM"
            | "H1"
            | "H2"
            | "H3"
            | "H4"
            | "H5"
            | "H6"
            | "HEADER"
            | "HR"
            | "LI"
            | "MAIN"
            | "NAV"
            | "OL"
            | "P"
            | "PRE"
            | "SECTION"
            | "SUMMARY"
            | "TABLE"
            | "TBODY"
            | "TD"
            | "TFOOT"
            | "TH"
            | "THEAD"
            | "TR"
            | "UL"
    )
}

struct Aplatisseur {
    morceaux: Vec<String>,
    segs: Vec<TextSegment>,
    n: usize,
    dernier: char,
    virtuel: bool,
    ordinal: usize,
}

impl Aplatisseur {
    fn coupe(&mut self) {
        if self.dernier != '\n' {
            self.morceaux.push("\n".to_owned());
            self.n += 1;
            self.dernier = '\n';
            self.virtuel = true;
        }
    }

    fn visite(&mut self, enfants: &[HtmlNode], pre: bool) {
        for c in enfants {
            match c {
                HtmlNode::Text(d) => {
                    let ordinal = self.ordinal;
                    self.ordinal += 1;
                    // Les blancs de mise en forme de la source (retour + indentation) ne sont pas du texte.
                    if d.is_empty()
                        || (!pre
                            && d.chars().all(crate::find_passage::est_blanc)
                            && d.contains('\n'))
                    {
                        continue;
                    }
                    self.segs.push(TextSegment {
                        node: ordinal,
                        start: self.n,
                        len: d.len(),
                    });
                    self.morceaux.push(d.clone());
                    self.n += d.len();
                    self.dernier = d.chars().next_back().unwrap_or('\n');
                    self.virtuel = false;
                }
                HtmlNode::Element { tag, children } => {
                    let tag = tag.to_ascii_uppercase();
                    if saute(&tag) {
                        continue;
                    }
                    let est_bloc = bloc(&tag);
                    if est_bloc {
                        self.coupe();
                    }
                    self.visite(children, pre || tag == "PRE");
                    if est_bloc {
                        self.coupe();
                    }
                }
            }
        }
    }
}

/// Le texte aplati des enfants du `<body>` (TS : `aplatir`) : les nœuds texte dans
/// l'ordre, un `\n` entre deux blocs, sans le saut virtuel qui fermerait le dernier.
pub fn flatten_page(body: &[HtmlNode]) -> FlatText {
    let mut a = Aplatisseur {
        morceaux: Vec::new(),
        segs: Vec::new(),
        n: 0,
        dernier: '\n',
        virtuel: false,
        ordinal: 0,
    };
    a.visite(body, false);
    // Le saut virtuel qui fermait le dernier bloc n'est pas du texte.
    if a.virtuel {
        a.morceaux.pop();
    }
    FlatText {
        text: a.morceaux.concat(),
        segs: a.segs,
    }
}
