// Port de app/src/ui/views/markdown/sanitizeHtml.ts
//! La POLITIQUE de sanitize du HTML écrit dans le Markdown : quelles balises et
//! quels attributs survivent. Le HTML de Fragment est assaini « comme Obsidian »
//! (DOMPurify) avant d'être rendu — dans une note comme dans une réponse de
//! l'agent (même pipeline).
//!
//! Gardé : la mise en forme (div/span/style, tableaux, details…), le SVG et ses
//! filtres, les animations CSS ET SVG (SMIL : `<animate>`, `<animateTransform>`,
//! `<animateMotion>`, `<set>` — DOMPurify les refuse par défaut à cause de
//! `attributeName="href"` + `values="javascript:…"` : on les garde, sauf ce cas),
//! les `<iframe>` (YouTube… comme Obsidian).
//! Retiré : `<script>`, les gestionnaires `on…`, les URL `javascript:` — rien du
//! Markdown ne s'EXÉCUTE (un objet vivant passe par la vue HTML / les artefacts).
//!
//! ★ Ce module n'est PAS un assainisseur de chaînes : DOMPurify repose sur un
//!   parseur HTML (le DOM), et le port n'en embarque pas. Il fournit la
//!   POLITIQUE — [`filter_element`] dit, pour une balise et ses attributs, ce qui
//!   survit — que le parseur de la vue (webview S2, ou un parseur HTML5 dans
//!   `fragment-ui`) applique nœud par nœud. [`sanitize_opening_tag`] en est la
//!   forme « balise isolée », entièrement testable ici.
//!
//! ★ Les listes reprennent celles de DOMPurify (profils `html`, `svg`,
//!   `svgFilters`) + `ADD_TAGS`/`ADD_ATTR` de Fragment, relevées dans la version
//!   de `app/package.json`. Hors périmètre du port (notés dans la PR) : la
//!   protection contre le « DOM clobbering » (`id`/`name` qui masquent
//!   `document.*`) et le test XML (`</style>` dans un attribut).

/// Les animations SVG que DOMPurify refuse par défaut et qu'Obsidian rend.
pub const SMIL: [&str; 4] = ["animate", "set", "animatetransform", "animatemotion"];

/// Balises refusées quoi qu'il arrive (`FORBID_TAGS`).
pub const FORBID_TAGS: [&str; 5] = ["script", "object", "embed", "base", "meta"];

/// Profil `html` de DOMPurify (balises).
const TAGS_HTML: &[&str] = &[
    "a",
    "abbr",
    "acronym",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "bdi",
    "bdo",
    "big",
    "blink",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "center",
    "cite",
    "code",
    "col",
    "colgroup",
    "content",
    "data",
    "datalist",
    "dd",
    "decorator",
    "del",
    "details",
    "dfn",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "element",
    "em",
    "fieldset",
    "figcaption",
    "figure",
    "font",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "i",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "main",
    "map",
    "mark",
    "marquee",
    "menu",
    "menuitem",
    "meter",
    "nav",
    "nobr",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "picture",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "section",
    "select",
    "shadow",
    "small",
    "source",
    "spacer",
    "span",
    "strike",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "track",
    "tt",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
];

/// Profil `svg` de DOMPurify (balises). ★ `foreignobject` et `use` n'y sont pas : refusés.
const TAGS_SVG: &[&str] = &[
    "svg",
    "a",
    "altglyph",
    "altglyphdef",
    "altglyphitem",
    "animatecolor",
    "animatemotion",
    "animatetransform",
    "circle",
    "clippath",
    "defs",
    "desc",
    "ellipse",
    "filter",
    "font",
    "g",
    "glyph",
    "glyphref",
    "hkern",
    "image",
    "line",
    "lineargradient",
    "marker",
    "mask",
    "metadata",
    "mpath",
    "path",
    "pattern",
    "polygon",
    "polyline",
    "radialgradient",
    "rect",
    "stop",
    "style",
    "switch",
    "symbol",
    "text",
    "textpath",
    "title",
    "tref",
    "tspan",
    "view",
    "vkern",
];

/// Profil `svgFilters` de DOMPurify (balises).
const TAGS_SVG_FILTERS: &[&str] = &[
    "feblend",
    "fecolormatrix",
    "fecomponenttransfer",
    "fecomposite",
    "feconvolvematrix",
    "fediffuselighting",
    "fedisplacementmap",
    "fedistantlight",
    "fedropshadow",
    "feflood",
    "fefunca",
    "fefuncb",
    "fefuncg",
    "fefuncr",
    "fegaussianblur",
    "feimage",
    "femerge",
    "femergenode",
    "femorphology",
    "feoffset",
    "fepointlight",
    "fespecularlighting",
    "fespotlight",
    "fetile",
    "feturbulence",
];

/// `ADD_TAGS` de Fragment : `iframe` + les animations SMIL.
const TAGS_AJOUTEES: &[&str] = &[
    "iframe",
    "animate",
    "set",
    "animatetransform",
    "animatemotion",
];

/// Profil `html` de DOMPurify (attributs).
const ATTRS_HTML: &[&str] = &[
    "accept",
    "accept-charset",
    "accesskey",
    "action",
    "align",
    "alt",
    "autocapitalize",
    "autocomplete",
    "autopictureinpicture",
    "autoplay",
    "background",
    "bgcolor",
    "border",
    "capture",
    "cellpadding",
    "cellspacing",
    "checked",
    "cite",
    "class",
    "clear",
    "color",
    "cols",
    "colspan",
    "controls",
    "controlslist",
    "coords",
    "crossorigin",
    "datetime",
    "decoding",
    "default",
    "dir",
    "disabled",
    "disablepictureinpicture",
    "disableremoteplayback",
    "download",
    "draggable",
    "enctype",
    "enterkeyhint",
    "face",
    "for",
    "headers",
    "height",
    "hidden",
    "high",
    "href",
    "hreflang",
    "id",
    "inputmode",
    "integrity",
    "ismap",
    "kind",
    "label",
    "lang",
    "list",
    "loading",
    "loop",
    "low",
    "max",
    "maxlength",
    "media",
    "method",
    "min",
    "minlength",
    "multiple",
    "muted",
    "name",
    "nonce",
    "noshade",
    "novalidate",
    "nowrap",
    "open",
    "optimum",
    "pattern",
    "placeholder",
    "playsinline",
    "popover",
    "popovertarget",
    "popovertargetaction",
    "poster",
    "preload",
    "pubdate",
    "radiogroup",
    "readonly",
    "rel",
    "required",
    "rev",
    "reversed",
    "role",
    "rows",
    "rowspan",
    "spellcheck",
    "scope",
    "selected",
    "shape",
    "size",
    "sizes",
    "span",
    "srclang",
    "start",
    "src",
    "srcset",
    "step",
    "style",
    "summary",
    "tabindex",
    "title",
    "translate",
    "type",
    "usemap",
    "valign",
    "value",
    "width",
    "wrap",
    "xmlns",
    "slot",
];

/// Profil `svg` + `svgFilters` de DOMPurify (attributs).
const ATTRS_SVG: &[&str] = &[
    "accent-height",
    "accumulate",
    "additive",
    "alignment-baseline",
    "amplitude",
    "ascent",
    "attributename",
    "attributetype",
    "azimuth",
    "basefrequency",
    "baseline-shift",
    "begin",
    "bias",
    "by",
    "class",
    "clip",
    "clippathunits",
    "clip-path",
    "clip-rule",
    "color",
    "color-interpolation",
    "color-interpolation-filters",
    "color-profile",
    "color-rendering",
    "cx",
    "cy",
    "d",
    "dx",
    "dy",
    "diffuseconstant",
    "direction",
    "display",
    "divisor",
    "dur",
    "edgemode",
    "elevation",
    "end",
    "exponent",
    "fill",
    "fill-opacity",
    "fill-rule",
    "filter",
    "filterunits",
    "flood-color",
    "flood-opacity",
    "font-family",
    "font-size",
    "font-size-adjust",
    "font-stretch",
    "font-style",
    "font-variant",
    "font-weight",
    "fx",
    "fy",
    "g1",
    "g2",
    "glyph-name",
    "glyphref",
    "gradientunits",
    "gradienttransform",
    "height",
    "href",
    "id",
    "image-rendering",
    "in",
    "in2",
    "intercept",
    "k",
    "k1",
    "k2",
    "k3",
    "k4",
    "kerning",
    "keypoints",
    "keysplines",
    "keytimes",
    "lang",
    "lengthadjust",
    "letter-spacing",
    "kernelmatrix",
    "kernelunitlength",
    "lighting-color",
    "local",
    "marker-end",
    "marker-mid",
    "marker-start",
    "markerheight",
    "markerunits",
    "markerwidth",
    "maskcontentunits",
    "maskunits",
    "max",
    "mask",
    "media",
    "method",
    "mode",
    "min",
    "name",
    "numoctaves",
    "offset",
    "operator",
    "opacity",
    "order",
    "orient",
    "orientation",
    "origin",
    "overflow",
    "paint-order",
    "path",
    "pathlength",
    "patterncontentunits",
    "patterntransform",
    "patternunits",
    "points",
    "preservealpha",
    "preserveaspectratio",
    "primitiveunits",
    "r",
    "rx",
    "ry",
    "radius",
    "refx",
    "refy",
    "repeatcount",
    "repeatdur",
    "restart",
    "result",
    "rotate",
    "scale",
    "seed",
    "shape-rendering",
    "slope",
    "specularconstant",
    "specularexponent",
    "spreadmethod",
    "startoffset",
    "stddeviation",
    "stitchtiles",
    "stop-color",
    "stop-opacity",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke",
    "stroke-width",
    "style",
    "surfacescale",
    "systemlanguage",
    "tabindex",
    "tablevalues",
    "targetx",
    "targety",
    "transform",
    "transform-origin",
    "text-anchor",
    "text-decoration",
    "text-rendering",
    "textlength",
    "type",
    "u1",
    "u2",
    "unicode",
    "values",
    "viewbox",
    "visibility",
    "version",
    "vert-adv-y",
    "vert-origin-x",
    "vert-origin-y",
    "width",
    "word-spacing",
    "wrap",
    "writing-mode",
    "xchannelselector",
    "ychannelselector",
    "x",
    "x1",
    "x2",
    "xmlns",
    "y",
    "y1",
    "y2",
    "z",
    "zoomandpan",
    "xlink:href",
];

/// `ADD_ATTR` de Fragment (en minuscules : la comparaison l'est).
const ATTRS_AJOUTES: &[&str] = &[
    "allow",
    "allowfullscreen",
    "frameborder",
    "scrolling",
    "target",
    "attributename",
    "values",
    "dur",
    "begin",
    "end",
    "repeatcount",
    "keytimes",
    "keysplines",
    "calcmode",
    "from",
    "to",
    "by",
    "additive",
    "accumulate",
    "type",
    "path",
    "rotate",
    "fill",
];

/// Attributs « sûrs côté URL » de DOMPurify (`URI_SAFE_ATTRIBUTES`) : leur valeur
/// n'est jamais prise pour une URL.
const ATTRS_SANS_URL: &[&str] = &[
    "alt",
    "class",
    "for",
    "id",
    "label",
    "name",
    "pattern",
    "placeholder",
    "role",
    "summary",
    "title",
    "value",
    "style",
    "xmlns",
];

/// Balises où une URL `data:` est tolérée dans `src`/`href` (`DATA_URI_TAGS`).
const BALISES_DATA_URI: &[&str] = &["audio", "video", "img", "source", "image", "track"];

/// Une balise d'ouverture assainie : son nom (minuscules) et les attributs qui restent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedTag {
    pub tag_name: String,
    /// Dans l'ordre d'écriture.
    pub attributes: Vec<(String, String)>,
}

/// La balise `tag` (nom quelconque) survit-elle ?
pub fn is_tag_allowed(tag: &str) -> bool {
    let t = tag.to_ascii_lowercase();
    if FORBID_TAGS.contains(&t.as_str()) {
        return false;
    }
    [TAGS_HTML, TAGS_SVG, TAGS_SVG_FILTERS, TAGS_AJOUTEES]
        .iter()
        .any(|l| l.contains(&t.as_str()))
}

/// L'URL `value` passe-t-elle le test `IS_ALLOWED_URI` de DOMPurify ?
///
/// Les blancs et caractères de contrôle sont retirés d'abord (`ATTR_WHITESPACE`) :
/// `java\nscript:` est ainsi reconnu comme `javascript:`. Passent : un schéma
/// `http(s)`, `ftp(s)`, `mailto`, `tel`, `callto`, `sms`, `cid`, `xmpp`, `matrix` ;
/// une valeur qui ne commence pas par une lettre (chemin, `#ancre`, `/x`) ;
/// une suite de lettres/`+`/`.`/`-` NON suivie de `:` (chemin relatif `a/b`).
pub fn is_allowed_uri(value: &str) -> bool {
    let v: String = value
        .chars()
        .filter(|c| {
            let n = *c as u32;
            !(n <= 0x20
                || n == 0xa0
                || n == 0x1680
                || n == 0x180e
                || (0x2000..=0x2029).contains(&n)
                || n == 0x205f
                || n == 0x3000)
        })
        .collect();
    let bas = v.to_ascii_lowercase();
    const SCHEMAS: [&str; 11] = [
        "http:", "https:", "ftp:", "ftps:", "mailto:", "tel:", "callto:", "sms:", "cid:", "xmpp:",
        "matrix:",
    ];
    if SCHEMAS.iter().any(|s| bas.starts_with(s)) {
        return true;
    }
    let Some(premier) = bas.chars().next() else {
        // Valeur vide après nettoyage : le motif exige un caractère, mais DOMPurify
        // laisse passer une valeur vide (`else if (!value) return true`).
        return true;
    };
    if !premier.is_ascii_lowercase() {
        return true; // `[^a-z]`
    }
    // `[a-z+.\-]+(?:[^a-z+.\-:]|$)` : la suite de lettres s'arrête sur autre chose que `:`.
    let suite: String = bas
        .chars()
        .take_while(|c| c.is_ascii_lowercase() || matches!(c, '+' | '.' | '-'))
        .collect();
    match bas[suite.len()..].chars().next() {
        None => true,
        Some(c) => c != ':',
    }
}

fn est_attribut_data_ou_aria(nom: &str) -> bool {
    let corps = |p: &str, large: bool| -> bool {
        nom.strip_prefix(p).is_some_and(|r| {
            !r.is_empty()
                && r.chars().all(|c| {
                    c == '-'
                        || c == '_'
                        || c == '.' && large
                        || c.is_ascii_alphanumeric()
                        || (large && (c as u32) >= 0xb7)
                })
        })
    };
    corps("data-", true) || corps("aria-", false)
}

/// L'attribut `name="value"` de la balise `tag` survit-il ?
pub fn is_attr_allowed(tag: &str, name: &str, value: &str) -> bool {
    let tag = tag.to_ascii_lowercase();
    let n = name.to_ascii_lowercase();
    // ★ `on…` (gestionnaires), `srcdoc`, `formaction`… ne figurent dans AUCUNE liste.
    let permis = [ATTRS_HTML, ATTRS_SVG, ATTRS_AJOUTES]
        .iter()
        .any(|l| l.contains(&n.as_str()))
        || est_attribut_data_ou_aria(&n);
    if !permis {
        return false;
    }
    if ATTRS_SANS_URL.contains(&n.as_str()) || est_attribut_data_ou_aria(&n) {
        return true;
    }
    if is_allowed_uri(value) {
        return true;
    }
    // `data:` toléré dans src/href des médias.
    let sans_blancs: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    matches!(n.as_str(), "src" | "xlink:href" | "href")
        && tag != "script"
        && sans_blancs.to_ascii_lowercase().starts_with("data:")
        && BALISES_DATA_URI.contains(&tag.as_str())
}

/// ★ Une animation qui viserait un lien (`href`) peut y injecter `javascript:` : retirée.
fn animation_interdite(attrs: &[(String, String)]) -> bool {
    let get = |nom: &str| -> &str {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nom))
            .map_or("", |(_, v)| v.as_str())
    };
    let cible = get("attributename").trim().to_ascii_lowercase();
    let valeurs = ["values", "to", "from", "by"]
        .iter()
        .map(|a| get(a))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    cible == "href" || cible == "xlink:href" || valeurs.contains("javascript:")
}

/// La politique appliquée à UN élément : `None` = l'élément est retiré (avec son
/// contenu pour `script`/`style`-like, comme DOMPurify) ; `Some(attrs)` = il reste,
/// avec les seuls attributs permis (ordre conservé, premier gagnant en cas de doublon).
pub fn filter_element(tag: &str, attrs: &[(String, String)]) -> Option<Vec<(String, String)>> {
    if !is_tag_allowed(tag) {
        return None;
    }
    let lc = tag.to_ascii_lowercase();
    if SMIL.contains(&lc.as_str()) && animation_interdite(attrs) {
        return None;
    }
    let mut vus: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (nom, valeur) in attrs {
        let n = nom.to_ascii_lowercase();
        if vus.contains(&n) {
            continue;
        }
        vus.push(n);
        if is_attr_allowed(&lc, nom, valeur) {
            out.push((nom.clone(), valeur.clone()));
        }
    }
    Some(out)
}

// ─── Balise d'ouverture isolée ────────────────────────────────────────────

/// Décode les références de caractères qu'un parseur HTML décode dans une valeur
/// d'attribut — indispensable AVANT le test d'URL : `&#106;avascript:` est
/// `javascript:`.
fn decoder_entites(s: &str) -> String {
    let mut out = String::new();
    let mut reste = s;
    while let Some(i) = reste.find('&') {
        out.push_str(&reste[..i]);
        let apres = &reste[i + 1..];
        let fin = apres.find(';').filter(|&f| f <= 10);
        let decode = fin.and_then(|f| {
            let nom = &apres[..f];
            let c = match nom {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
                    .ok()
                    .and_then(char::from_u32),
                n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            c.map(|c| (c, f))
        });
        match decode {
            Some((c, f)) => {
                out.push(c);
                reste = &apres[f + 1..];
            }
            None => {
                out.push('&');
                reste = apres;
            }
        }
    }
    out.push_str(reste);
    out
}

/// Découpe `<nom a="1" b='2' c=3 d>` en (nom, attributs) à la manière du tokenizer HTML.
fn lire_balise(tag: &str) -> Option<(String, Vec<(String, String)>)> {
    let t = tag.trim_start().strip_prefix('<')?.trim_start();
    let mut chars = t.char_indices().peekable();
    let (_, premier) = chars.next()?;
    if !premier.is_ascii_alphabetic() {
        return None;
    }
    let mut fin_nom = premier.len_utf8();
    while let Some(&(i, c)) = chars.peek() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            fin_nom = i + c.len_utf8();
            chars.next();
        } else {
            break;
        }
    }
    let nom = t[..fin_nom].to_ascii_lowercase();
    let reste: Vec<char> = t[fin_nom..].chars().collect();
    let mut i = 0;
    let mut attrs: Vec<(String, String)> = Vec::new();
    while i < reste.len() {
        match reste[i] {
            c if c.is_whitespace() || c == '/' => i += 1,
            '>' => break,
            _ => {
                let debut = i;
                while i < reste.len()
                    && !reste[i].is_whitespace()
                    && !matches!(reste[i], '=' | '>' | '/')
                {
                    i += 1;
                }
                let n: String = reste[debut..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase();
                while i < reste.len() && reste[i].is_whitespace() {
                    i += 1;
                }
                let mut valeur = String::new();
                if i < reste.len() && reste[i] == '=' {
                    i += 1;
                    while i < reste.len() && reste[i].is_whitespace() {
                        i += 1;
                    }
                    if i < reste.len() && (reste[i] == '"' || reste[i] == '\'') {
                        let q = reste[i];
                        i += 1;
                        let d = i;
                        while i < reste.len() && reste[i] != q {
                            i += 1;
                        }
                        valeur = reste[d..i].iter().collect();
                        i += 1;
                    } else {
                        let d = i;
                        while i < reste.len() && !reste[i].is_whitespace() && reste[i] != '>' {
                            i += 1;
                        }
                        valeur = reste[d..i].iter().collect();
                    }
                }
                if !n.is_empty() && !attrs.iter().any(|(k, _)| *k == n) {
                    attrs.push((n, decoder_entites(&valeur)));
                }
            }
        }
    }
    Some((nom, attrs))
}

/// Les attributs d'une balise ouvrante, assainis (pour baliser du texte en ligne).
/// Rend `None` si la balise est refusée.
pub fn sanitize_opening_tag(tag: &str) -> Option<SanitizedTag> {
    let (nom, attrs) = lire_balise(tag)?;
    let gardes = filter_element(&nom, &attrs)?;
    Some(SanitizedTag {
        tag_name: nom,
        attributes: gardes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(paires: &[(&str, &str)]) -> Vec<(String, String)> {
        paires
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }
    /// Les noms d'attributs gardés pour `<tag attrs>` (None = élément retiré).
    fn gardes(tag: &str, paires: &[(&str, &str)]) -> Option<Vec<String>> {
        filter_element(tag, &a(paires)).map(|v| v.into_iter().map(|(k, _)| k).collect())
    }

    // ★ Les 4 premiers cas du TS vérifient la SORTIE SÉRIALISÉE de DOMPurify (une
    //   chaîne HTML) ; ici, les MÊMES données sont posées élément par élément sur la
    //   politique (même verdict, sans parseur HTML).

    #[test]
    fn sanitize_html_garde_la_mise_en_forme_le_svg_les_animations_svg_et_css_details_iframe() {
        assert_eq!(
            filter_element("div", &a(&[("style", "color: red")])),
            Some(a(&[("style", "color: red")]))
        );
        for t in ["b", "details", "summary", "svg", "circle", "g", "style"] {
            assert!(is_tag_allowed(t), "{t}");
        }
        assert_eq!(
            gardes(
                "animate",
                &[
                    ("attributeName", "r"),
                    ("values", "1;3;1"),
                    ("dur", "1s"),
                    ("repeatCount", "indefinite")
                ]
            ),
            Some(vec![
                "attributeName".into(),
                "values".into(),
                "dur".into(),
                "repeatCount".into()
            ])
        );
        assert_eq!(
            gardes(
                "animateTransform",
                &[
                    ("attributeName", "transform"),
                    ("type", "translate"),
                    ("values", "0 0;5 0"),
                    ("dur", "2s")
                ]
            ),
            Some(vec![
                "attributeName".into(),
                "type".into(),
                "values".into(),
                "dur".into()
            ])
        );
        assert_eq!(
            gardes("set", &[("attributeName", "opacity"), ("to", "0.5")]),
            Some(vec!["attributeName".into(), "to".into()])
        );
        assert_eq!(
            gardes(
                "iframe",
                &[
                    ("src", "https://www.youtube.com/embed/x"),
                    ("allowfullscreen", "")
                ]
            ),
            Some(vec!["src".into(), "allowfullscreen".into()])
        );
    }

    #[test]
    fn sanitize_html_retire_ce_qui_s_execute_script_on_javascript_object_embed() {
        assert_eq!(gardes("p", &[("onclick", "alert(1)")]), Some(vec![]));
        for t in ["script", "object", "embed"] {
            assert_eq!(filter_element(t, &a(&[("src", "x")])), None, "{t}");
        }
        assert_eq!(
            gardes("a", &[("href", "javascript:alert(1)")]),
            Some(vec![])
        );
        assert_eq!(
            gardes("img", &[("src", "x"), ("onerror", "alert(1)")]),
            Some(vec!["src".into()])
        );
        assert_eq!(
            gardes("iframe", &[("src", "javascript:alert(1)")]),
            Some(vec![])
        );
    }

    #[test]
    fn sanitize_html_une_animation_svg_qui_viserait_un_lien_href_javascript_est_retiree() {
        assert_eq!(
            filter_element(
                "animate",
                &a(&[("attributeName", "href"), ("values", "javascript:alert(1)")])
            ),
            None
        );
        assert_eq!(
            filter_element(
                "set",
                &a(&[
                    ("attributeName", "xlink:href"),
                    ("to", "javascript:alert(1)")
                ])
            ),
            None
        );
        // Le reste du SVG de l'exemple survit.
        assert!(is_tag_allowed("text"));
        assert!(is_tag_allowed("a"));
    }

    #[test]
    fn sanitize_html_foreign_object_piege_rien_ne_s_execute() {
        assert_eq!(filter_element("foreignObject", &[]), None);
        assert_eq!(
            gardes("iframe", &[("srcdoc", "<script>alert(1)</script>")]),
            Some(vec![])
        );
        assert!(!is_tag_allowed("script"));
    }

    #[test]
    fn sanitize_html_sanitize_opening_tag_les_attributs_gardes_d_une_balise_en_ligne_ou_null() {
        assert_eq!(
            sanitize_opening_tag("<span style=\"color: red\" onclick=\"x()\">"),
            Some(SanitizedTag {
                tag_name: "span".into(),
                attributes: a(&[("style", "color: red")])
            })
        );
        assert_eq!(
            sanitize_opening_tag("<mark>"),
            Some(SanitizedTag {
                tag_name: "mark".into(),
                attributes: vec![]
            })
        );
        assert_eq!(sanitize_opening_tag("<script>"), None);
    }

    // ── Ajouts du port : les pièges que le parseur du DOM réglait pour DOMPurify ──
    #[test]
    fn port_uri_javascript_masque_par_blancs_ou_entites() {
        assert!(!is_allowed_uri("java\nscript:alert(1)"));
        assert!(!is_allowed_uri("  JavaScript:alert(1)"));
        assert_eq!(
            sanitize_opening_tag("<a href=\"&#106;avascript:alert(1)\" title=\"ok\">")
                .map(|t| t.attributes),
            Some(a(&[("title", "ok")]))
        );
        assert_eq!(
            sanitize_opening_tag("<a href='&#x6A;avascript:x'>").map(|t| t.attributes),
            Some(vec![])
        );
    }

    #[test]
    fn port_uri_permises() {
        for u in [
            "https://x.org/a",
            "mailto:a@b.c",
            "#ancre",
            "/chemin",
            "a/b.md",
            "./x",
            "",
        ] {
            assert!(is_allowed_uri(u), "{u}");
        }
        assert!(!is_allowed_uri("vbscript:x"));
        assert!(!is_allowed_uri("data:text/html,x"));
    }

    #[test]
    fn port_data_uri_toleree_seulement_pour_les_medias() {
        assert!(is_attr_allowed("img", "src", "data:image/png;base64,AAAA"));
        assert!(!is_attr_allowed("a", "href", "data:text/html,<script>"));
        assert!(!is_attr_allowed("iframe", "src", "data:text/html,x"));
    }

    #[test]
    fn port_attributs_data_et_aria_et_doublons() {
        assert!(is_attr_allowed("div", "data-x", "1"));
        assert!(is_attr_allowed("div", "aria-label", "1"));
        assert!(!is_attr_allowed("div", "onmouseover", "1"));
        assert_eq!(
            sanitize_opening_tag("<div class=a class=b>").map(|t| t.attributes),
            Some(a(&[("class", "a")]))
        );
    }
}
