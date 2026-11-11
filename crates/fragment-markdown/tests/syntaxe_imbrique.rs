// Port de app/src/ui/views/markdown/language.ts (`MathHighlight`, `codeHighlightStyle`) et de
// livePreview.ts (`codeLanguages`) : tests de la couche des langages imbriqués.
//! Tests de `fragment_markdown::syntaxe::imbrique` (fr-t3.2).
//!
//! Trois familles :
//! 1. `stex_*` : la coloration des formules, comparée à des relevés du VRAI parseur de Fragment
//!    (`StreamLanguage` + `stexMath` + `codeHighlightStyle` du dépôt de Fragment, avec des classes
//!    à la place des couleurs), produits HORS du dépôt sur des formules réécrites. Les plages sont
//!    relatives à l'intérieur de la formule, en unités UTF-16.
//! 2. `code_*` : un test par langage et par règle de l'analyseur générique (aucun relevé : la
//!    référence, des grammaires Lezer, n'est pas portée ; les attentes sont écrites à la main).
//! 3. `oracle_*` : la couche rendue par `reanalyser` est ÉGALE à celle d'une analyse complète,
//!    après des éditions au hasard sur des documents pleins de blocs et de formules.

use fragment_markdown::syntaxe::imbrique::{Imbrique, JetonCode};
use fragment_markdown::syntaxe::{self, Changement};

// ─── Aides ────────────────────────────────────────────────────────────────────

fn couche(texte: &str) -> Imbrique {
    Imbrique::analyser(texte, &syntaxe::analyser(texte))
}

/// Les jetons de `texte`, sous la forme (texte du jeton, nom du jeton du thème).
fn jetons(texte: &str) -> Vec<(String, &'static str)> {
    couche(texte)
        .jetons()
        .map(|(e, j)| (texte[e.from..e.to].to_owned(), j.nom()))
        .collect()
}

fn a(texte: &str, nom: &'static str) -> (String, &'static str) {
    (texte.to_owned(), nom)
}

/// Les jetons d'une formule `inner` (comme `$$inner$$`, en bloc), en (début UTF-16, fin, nom),
/// relatifs au début de l'intérieur.
fn maths(inner: &str) -> Vec<(usize, usize, &'static str)> {
    let fin = if inner.ends_with('\n') { "" } else { "\n" };
    let doc = format!("$${inner}{fin}$$");
    let c = couche(&doc);
    c.jetons_dans(&doc, 0, doc.len())
        .iter()
        .map(|p| (p.plage.utf16.from - 2, p.plage.utf16.to - 2, p.jeton.nom()))
        .collect()
}

const T: &str = "code_tag";
const P: &str = "code_punctuation";
const V: &str = "code_value";

// ─── Les noms ─────────────────────────────────────────────────────────────────

#[test]
fn les_jetons_sont_des_noms_de_jetons_du_theme() {
    use fragment_theme::Palette;
    for j in JetonCode::TOUS {
        assert!(
            Palette::CHAMPS.contains(&j.nom()),
            "{} absent du thème",
            j.nom()
        );
    }
    let mut noms: Vec<_> = JetonCode::TOUS.iter().map(|j| j.nom()).collect();
    noms.sort_unstable();
    noms.dedup();
    assert_eq!(noms.len(), 11, "un jeton par couleur `--code-*`");
    assert!(JetonCode::Commentaire.italique());
    assert!(!JetonCode::MotCle.italique());
}

// ─── Maths (relevés de Lezer) ─────────────────────────────────────────────────

#[test]
fn stex_commande_accolades_et_variables() {
    // Un texte qui commence par une ligne vide passe en mode TEXTE : on teste d'abord une formule
    // qui n'en a pas, avec un en-tête (`x`), pour rester en mode maths sur la première ligne.
    // Relevé : \frac{a}{b} + x^2 % note (une seule ligne, mode maths).
    let doc = "$\\frac{a}{b} + x^2 % note$";
    let c = couche(doc);
    let v: Vec<_> = c
        .jetons_dans(doc, 0, doc.len())
        .iter()
        .map(|p| (p.plage.utf16.from - 1, p.plage.utf16.to - 1, p.jeton.nom()))
        .collect();
    assert_eq!(
        v,
        [
            (0, 5, T),
            (5, 6, P),
            (6, 7, V),
            (7, 9, P),
            (9, 10, V),
            (10, 11, P),
            (14, 15, V),
            (15, 16, T),
            (16, 17, T),
            (18, 24, "code_comment"),
        ]
    );
}

#[test]
fn stex_operateurs_sans_jeton_et_nombres() {
    // Relevé : x+1.5y
    let doc = "$x+1.5y$";
    let c = couche(doc);
    let v: Vec<_> = c
        .jetons_dans(doc, 0, doc.len())
        .iter()
        .map(|p| (p.plage.utf16.from - 1, p.plage.utf16.to - 1, p.jeton.nom()))
        .collect();
    assert_eq!(v, [(0, 1, V), (2, 5, T), (5, 6, V)]);
}

#[test]
fn stex_environnement_et_erreurs() {
    // Relevé : \begin{aligned} a &= 1 \\ b \end{aligned}   (mode maths : `\\` = deux erreurs)
    let doc = "$\\begin{aligned} a &= 1 \\\\ b \\end{aligned}$";
    let c = couche(doc);
    let v: Vec<_> = c
        .jetons_dans(doc, 0, doc.len())
        .iter()
        .map(|p| (p.plage.utf16.from - 1, p.plage.utf16.to - 1, p.jeton.nom()))
        .collect();
    assert_eq!(
        v,
        [
            (0, 6, T),
            (6, 7, P),
            (7, 14, V),
            (14, 15, P),
            (16, 17, V),
            (18, 19, T),
            (21, 22, T),
            (23, 25, "code_important"),
            (26, 27, V),
            (28, 32, T),
            (32, 33, P),
            (33, 40, V),
            (40, 41, P),
        ]
    );
}

#[test]
fn stex_unites_utf16_pour_les_caracteres_hors_ascii() {
    // Relevé : « é \é + {{ » : `é` n'est pas une lettre de formule (erreur), les plages sont en
    // UTF-16 (é = 1 unité, 2 octets).
    let doc = "$é \\é + {{$";
    let c = couche(doc);
    let v: Vec<_> = c
        .jetons_dans(doc, 0, doc.len())
        .iter()
        .map(|p| (p.plage.utf16.from - 1, p.plage.utf16.to - 1, p.jeton.nom()))
        .collect();
    assert_eq!(
        v,
        [
            (0, 1, "code_important"),
            (2, 4, "code_important"),
            (7, 9, P)
        ]
    );
}

#[test]
fn stex_une_ligne_vide_repasse_en_mode_texte() {
    // ★ Relevé : `blankLine` remet l'automate en mode texte. Une formule en bloc commence par une
    // ligne vide (l'intérieur suit `$$`) : `\frac` y est coloré, mais `a` et `b` n'ont aucun jeton.
    assert_eq!(
        maths("\n\\frac{a}{b}\n"),
        [(1, 6, T), (6, 7, P), (8, 10, P), (11, 12, P)]
    );
}

#[test]
fn stex_commandes_a_arguments_en_mode_texte() {
    // Relevé : x, ligne vide (mode texte), puis \usepackage{ab} \label{c} 12 } \\ %c
    assert_eq!(
        maths("x\n\n\\usepackage{ab} \\label{c} 12 } \\\\ %c\n"),
        [
            (0, 1, V),
            (3, 14, T),
            (14, 15, P),
            (15, 17, V),
            (17, 18, P),
            (19, 25, T),
            (25, 26, P),
            (26, 27, V),
            (27, 28, P),
            (29, 31, V),
            (32, 33, "code_important"),
            (34, 36, T),
            (37, 39, "code_comment"),
        ]
    );
}

#[test]
fn stex_arguments_de_begin_documentclass_importmodule() {
    // Relevé : ligne vide, puis begin/documentclass/importmodule et leurs arguments.
    assert_eq!(
        maths("\n\\begin{a}{b}[c] x_1 \\documentclass[a]{b} \\importmodule{m}{n}\n\\end{a}\n"),
        [
            (1, 7, T),
            (7, 8, P),
            (8, 9, V),
            (9, 11, P),
            (12, 14, P),
            (15, 16, P),
            (21, 35, T),
            (35, 36, P),
            (37, 39, P),
            (39, 40, V),
            (40, 41, P),
            (42, 55, T),
            (55, 56, P),
            (56, 57, "code_string"),
            (57, 59, P),
            (59, 60, V),
            (60, 61, P),
            (62, 66, T),
            (66, 67, P),
            (67, 68, V),
            (68, 69, P),
        ]
    );
}

#[test]
fn stex_delimiteurs_de_formule_dans_le_texte() {
    // Relevé : après la ligne vide, `\[ … \]`, `\( … \)` et `$ … $` ouvrent des formules
    // (mots-clés) ; `3.5e2` est un « atome » en mode texte.
    assert_eq!(
        maths("a \\\\ b\n\\foo{x}\n\n\\foo{x} 3.5e2 \\[ x \\] \\( y \\) \\$ done $z$"),
        [
            (0, 1, V),
            (2, 4, "code_important"),
            (5, 6, V),
            (7, 11, T),
            (11, 12, P),
            (12, 13, V),
            (13, 14, P),
            (16, 20, T),
            (20, 21, P),
            (22, 23, P),
            (24, 29, V),
            (30, 32, "code_keyword"),
            (33, 34, V),
            (35, 37, "code_keyword"),
            (38, 40, "code_keyword"),
            (41, 42, V),
            (43, 45, "code_keyword"),
            (46, 48, T),
            (54, 55, "code_keyword"),
            (55, 56, V),
            (56, 57, "code_keyword"),
        ]
    );
}

#[test]
fn maths_en_ligne_et_en_ligne_affichee_sans_leurs_delimiteurs() {
    assert_eq!(
        jetons("a $x^2$ et $$y$$ b"),
        [a("x", V), a("^", T), a("2", T), a("y", V),]
    );
}

#[test]
fn maths_vides_ou_non_fermees() {
    assert!(jetons("$$ $$").is_empty() || !jetons("$$ $$").iter().any(|(t, _)| t == "$"));
    // ★ Un `$$` jamais fermé : le nœud va jusqu'à la fin ; Fragment retire quand même 2 caractères.
    let doc = "p\n$$\n\\sum x";
    let c = couche(doc);
    let dernier_octet = c.jetons().map(|(e, _)| e.to).max().unwrap_or(0);
    assert!(
        dernier_octet <= doc.len() - 2,
        "les 2 derniers octets sont exclus"
    );
}

// ─── Blocs de code ────────────────────────────────────────────────────────────

#[test]
fn code_rust() {
    let doc = "```rust\nfn main() { let x = 1; } // fin\n```\n";
    assert_eq!(
        jetons(doc),
        [
            a("fn", "code_keyword"),
            a("main", "code_function"),
            a("{", P),
            a("let", "code_keyword"),
            a("=", "code_operator"),
            a("1", T),
            a("}", P),
            a("// fin", "code_comment"),
        ]
    );
}

#[test]
fn code_rust_macros_durees_de_vie_et_caracteres() {
    let doc = "```rs\nprintln!(\"a\"); let c = 'x'; fn f<'a>() {}\n```";
    let j = jetons(doc);
    assert!(j.contains(&a("println", "code_property")));
    assert!(j.contains(&a("\"a\"", "code_string")));
    assert!(j.contains(&a("'x'", "code_string")));
    assert!(
        j.contains(&a("'a", T)),
        "une durée de vie n'est pas une chaîne : {j:?}"
    );
}

#[test]
fn code_javascript_et_alias() {
    for info in ["js", "javascript", "jsx", "JS", "js title=\"a.js\""] {
        let doc = format!("```{info}\nconst x = `a${{1}}`;\n```");
        let j = jetons(&doc);
        assert!(j.contains(&a("const", "code_keyword")), "{info}");
        assert!(j.contains(&a("`a${1}`", "code_string")), "{info} : {j:?}");
    }
    let j = jetons("```ts\nlet n: number = 1;\n```");
    assert!(j.contains(&a("number", T)));
}

#[test]
fn code_json_cles_et_valeurs() {
    let doc = "```json\n{\"a\": [1, true, null, \"b\"]}\n```";
    assert_eq!(
        jetons(doc),
        [
            a("{", P),
            a("\"a\"", "code_property"),
            a("1", T),
            a("true", V),
            a("null", V),
            a("\"b\"", "code_string"),
            a("}", P),
        ]
    );
}

#[test]
fn code_python_triple_guillemets_sur_plusieurs_lignes() {
    let doc = "```python\ndef f():\n    \"\"\"doc\n    suite\"\"\"\n    return None  # x\n```";
    let j = jetons(doc);
    assert!(j.contains(&a("def", "code_keyword")));
    assert!(j.contains(&a("f", "code_function")));
    assert!(j.contains(&a("\"\"\"doc", "code_string")));
    assert!(
        j.contains(&a("    suite\"\"\"", "code_string"))
            || j.contains(&a("suite\"\"\"", "code_string"))
    );
    assert!(j.contains(&a("None", V)));
    assert!(j.contains(&a("# x", "code_comment")));
}

#[test]
fn code_commentaire_de_bloc_sur_plusieurs_lignes() {
    let doc = "```c\nint a; /* un\ndeux */ int b;\n```";
    let j = jetons(doc);
    assert!(j.contains(&a("/* un", "code_comment")));
    assert!(j.contains(&a("deux */", "code_comment")));
    assert_eq!(j.iter().filter(|(t, n)| t == "int" && *n == T).count(), 2);
}

#[test]
fn code_shell_variables() {
    let j = jetons("```bash\nif [ -n \"$HOME\" ]; then echo $USER; fi # ok\n```");
    assert!(j.contains(&a("if", "code_keyword")));
    assert!(j.contains(&a("$USER", V)));
    assert!(j.contains(&a("# ok", "code_comment")));
}

#[test]
fn code_css() {
    let j = jetons("```css\n.a > b { color: #fff; margin: 4px !important; width: calc(1px) }\n```");
    assert!(j.contains(&a(".a", T)));
    assert!(j.contains(&a("color", "code_property")));
    assert!(j.contains(&a("#fff", V)));
    assert!(j.contains(&a("4px", T)));
    assert!(j.contains(&a("!important", "code_important")));
    assert!(j.contains(&a("calc", "code_function")));
}

#[test]
fn code_html() {
    let j = jetons("```html\n<!-- c -->\n<a href=\"x\" id=y>t</a>\n```");
    assert!(j.contains(&a("<!-- c -->", "code_comment")));
    assert!(j.contains(&a("a", T)));
    assert!(j.contains(&a("href", "code_property")));
    assert!(j.contains(&a("\"x\"", "code_string")));
}

#[test]
fn code_yaml_et_toml() {
    let j = jetons("```yaml\ntitre: Un\nliste:\n  - nom: x # c\n```");
    assert!(j.contains(&a("titre", "code_property")));
    assert!(j.contains(&a("liste", "code_property")));
    assert!(j.contains(&a("nom", "code_property")));
    assert!(j.contains(&a("# c", "code_comment")));
    let j = jetons("```toml\n[paquet]\nnom = \"x\"\nn = 3\n```");
    assert!(j.contains(&a("[paquet]", T)));
    assert!(j.contains(&a("nom", "code_property")));
    assert!(j.contains(&a("\"x\"", "code_string")));
}

#[test]
fn code_sql_insensible_a_la_casse() {
    let j = jetons("```sql\nSELECT a FROM t WHERE x IS NULL -- fin\n```");
    assert!(j.contains(&a("SELECT", "code_keyword")));
    assert!(j.contains(&a("FROM", "code_keyword")));
    assert!(j.contains(&a("NULL", V)));
    assert!(j.contains(&a("-- fin", "code_comment")));
}

#[test]
fn code_langage_inconnu_ou_absent_n_est_pas_colore() {
    assert!(jetons("```\nlet x = 1;\n```").is_empty());
    assert!(jetons("```klingon\nlet x = 1;\n```").is_empty());
    assert!(jetons("```markdown\n# t\n```").is_empty());
    assert!(
        jetons("    let x = 1;\n").is_empty(),
        "un code indenté n'a pas de langage"
    );
    assert!(
        jetons("`let x`").is_empty(),
        "le code en ligne n'est pas coloré"
    );
}

#[test]
fn code_dans_une_citation_les_prefixes_de_ligne_sont_exclus() {
    let doc = "> ```js\n> let a = 1;\n> let b = 2;\n> ```\n";
    let j = jetons(doc);
    assert_eq!(
        j.iter()
            .filter(|(t, n)| t == "let" && *n == "code_keyword")
            .count(),
        2,
        "{j:?}"
    );
    assert!(j.iter().all(|(t, _)| !t.contains('>')), "{j:?}");
}

#[test]
fn code_dans_une_liste() {
    let doc = "- a\n\n  ```rust\n  let x = 1;\n  ```\n- b\n";
    let j = jetons(doc);
    assert!(j.contains(&a("let", "code_keyword")), "{j:?}");
}

#[test]
fn code_bloc_non_ferme_colore_jusqu_a_la_fin() {
    let j = jetons("```rust\nlet x = 1;\n\nlet y = 2;");
    assert_eq!(j.iter().filter(|(t, _)| t == "let").count(), 2);
}

// ─── Lecture par plage (viewport) ─────────────────────────────────────────────

#[test]
fn jetons_dans_rend_les_plages_utf16_et_se_limite_a_la_plage() {
    let doc = "😀 é\n\n```js\nlet é = \"😀\";\nvar b;\n```\n\n$x$\n";
    let c = couche(doc);
    let tous = c.jetons_dans(doc, 0, doc.len());
    let ligne_var = doc.find("var").unwrap_or(0);
    let seuls = c.jetons_dans(doc, ligne_var, ligne_var + 5);
    assert_eq!(seuls.len(), 1, "{seuls:?}");
    assert_eq!(&doc[seuls[0].plage.octets.range()], "var");
    for p in &tous {
        let texte = &doc[p.plage.octets.range()];
        let utf16: usize = texte.chars().map(char::len_utf16).sum();
        assert_eq!(p.plage.utf16.to - p.plage.utf16.from, utf16);
        let avant: usize = doc[..p.plage.octets.from]
            .chars()
            .map(char::len_utf16)
            .sum();
        assert_eq!(p.plage.utf16.from, avant, "{texte:?}");
    }
    assert!(
        tous.iter()
            .any(|p| &doc[p.plage.octets.range()] == "\"😀\"")
    );
}

// ─── Oracle : incrémental = analyse complète ──────────────────────────────────

struct Hasard(u64);
impl Hasard {
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn sous(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }
}

/// De quoi ouvrir, fermer, casser ou allonger blocs de code et formules.
const MORCEAUX: &[&str] = &[
    "x",
    "a",
    " ",
    "\n",
    "\n\n",
    "```",
    "```js\n",
    "```rust\n",
    "```json\n",
    "~~~",
    "$",
    "$$",
    "$$\n",
    "\\frac{",
    "}",
    "\\",
    "{",
    "%",
    "\"",
    "'",
    "`",
    "/*",
    "*/",
    "//",
    "#",
    "> ",
    "- ",
    "  ",
    "é",
    "😀",
    "---\n",
    "| a |\n| - |\n",
    "<div>",
    "let ",
    "1.5",
];

fn texte_de_depart() -> Vec<String> {
    let mut docs = vec![
        String::new(),
        "p\n\n```js\nlet a = `x\ny`;\n```\n\n$a^2$ et $$b$$\n\n$$\n\\frac{a}{b}\n$$\n\n```rust\nfn f() {}\n```\n".into(),
        "> ```python\n> \"\"\"a\n> b\"\"\"\n> x = 1\n> ```\n\n- liste\n\n  ```json\n  {\"a\": 1}\n  ```\n\n$$\nx\n$$\n".into(),
        "---\ntitre: a\n---\n```css\n.a { color: red }\n```\n$x$ ```sh\n$HOME\n```\n```\nsans langage\n".into(),
        "texte\n$$\nouvert\n\n```toml\n[a]\nb = 1\n".into(),
    ];
    // Les documents du dépôt de tests de l'arbre.
    let dossier = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/syntaxe/");
    for nom in ["blocs", "inline", "listes", "html", "maths_ouvertes"] {
        if let Ok(t) = std::fs::read_to_string(format!("{dossier}{nom}.md")) {
            docs.push(t);
        }
    }
    docs
}

fn oracle(nom: &str, depart: &str, graine: u64, n: usize) {
    let mut h = Hasard(graine | 1);
    let mut t = depart.to_owned();
    let mut arbre = syntaxe::analyser(&t);
    let mut courante = Imbrique::analyser(&t, &arbre);
    for k in 0..n {
        let mut de = h.sous(t.len() + 1);
        while !t.is_char_boundary(de) {
            de -= 1;
        }
        let mut fin = de;
        if h.sous(3) > 0 && de < t.len() {
            let portee = if h.sous(4) == 0 { 300 } else { 10 };
            fin = (de + 1 + h.sous(portee)).min(t.len());
            while !t.is_char_boundary(fin) {
                fin += 1;
            }
        }
        let insere: String = if h.sous(3) == 0 && fin > de {
            String::new()
        } else {
            (0..1 + h.sous(2))
                .map(|_| MORCEAUX[h.sous(MORCEAUX.len())])
                .collect()
        };
        let mut nouveau = t.clone();
        nouveau.replace_range(de..fin, &insere);
        let c = if k % 2 == 0 {
            Changement::remplacement(de, fin, &insere)
        } else {
            Changement::entre(&t, &nouveau)
        };
        arbre = arbre.reanalyser(&nouveau, c);
        courante = courante.reanalyser(&nouveau, &arbre, c);
        let complete = Imbrique::analyser(&nouveau, &syntaxe::analyser(&nouveau));
        assert!(
            courante == complete,
            "{nom}, édition {k} ({de}..{fin} ← {insere:?})\n  incrémental : {:?}\n  complet     : {:?}",
            courante.jetons().collect::<Vec<_>>(),
            complete.jetons().collect::<Vec<_>>()
        );
        t = nouveau;
    }
}

#[test]
fn oracle_incremental_identique_a_l_analyse_complete() {
    for (i, d) in texte_de_depart().iter().enumerate() {
        oracle(
            &format!("document {i}"),
            d,
            0xA5A5_1234_9E37_79B9 ^ (i as u64 * 7919),
            500,
        );
    }
}

#[test]
fn oracle_changement_incoherent_retombe_sur_l_analyse_complete() {
    let t = "```js\nlet a;\n```\n";
    let arbre = syntaxe::analyser(t);
    let c = couche(t);
    let faux = Changement {
        de: 3,
        fin_ancien: 99,
        fin_nouveau: 1,
    };
    let nouveau = "```js\n";
    let arbre2 = syntaxe::analyser(nouveau);
    assert_eq!(
        c.reanalyser(nouveau, &arbre2, faux),
        Imbrique::analyser(nouveau, &arbre2)
    );
    let _ = arbre;
}

#[test]
fn oracle_une_frappe_dans_un_bloc_ne_recolore_que_ce_bloc() {
    // Deux blocs ; on tape dans le second : le premier est repris tel quel (même nombre de régions
    // et mêmes jetons), le second reflète la frappe.
    let t = "```js\nlet a = 1;\n```\n\ntexte\n\n```js\nlet b = 2;\n```\n";
    let arbre = syntaxe::analyser(t);
    let avant = Imbrique::analyser(t, &arbre);
    let p = t.rfind("2;").unwrap_or(0);
    let mut nouveau = t.to_owned();
    nouveau.insert(p, 'x');
    let c = Changement::remplacement(p, p, "x");
    let arbre2 = arbre.reanalyser(&nouveau, c);
    let apres = avant.clone().reanalyser(&nouveau, &arbre2, c);
    assert_eq!(apres, Imbrique::analyser(&nouveau, &arbre2));
    assert_eq!(apres.nombre_de_regions(), 2);
    let premiers: Vec<_> = apres.jetons().take(3).collect();
    let anciens: Vec<_> = avant.jetons().take(3).collect();
    assert_eq!(premiers, anciens);
}

// ─── Mesure ───────────────────────────────────────────────────────────────────

/// 10 000 lignes : des paragraphes, des listes, des blocs de code de plusieurs langages et des
/// formules, comme une grosse note de cours.
fn dix_mille_lignes() -> String {
    let mut t = String::new();
    for i in 0..1000 {
        t.push_str(&format!(
            "# Section {i}\n\nUn paragraphe avec $x_{i}^2$ et du *texte* en [lien](http://a.b).\n\n"
        ));
        t.push_str("- une puce\n- une autre\n\n");
        match i % 3 {
            0 => t.push_str("```rust\nfn f(a: u32) -> u32 {\n    a + 1 // incrément\n}\n```\n\n"),
            1 => t.push_str("```json\n{\"a\": [1, 2, true], \"b\": \"x\"}\n```\n\n"),
            _ => t.push_str("$$\n\\sum_{k=1}^{n} \\frac{a_k}{b_k}\n$$\n\n"),
        }
    }
    t
}

/// Coût d'une frappe : arbre incrémental SEUL, puis arbre + couche des langages imbriqués.
/// `cargo test -p fragment-markdown --release --test syntaxe_imbrique -- --ignored --nocapture mesure`
#[test]
#[ignore = "mesure, à lancer en --release"]
fn mesure_imbrique_dix_mille_lignes() {
    let base = dix_mille_lignes();
    let mut h = Hasard(0xABCD_EF01);
    let (mut seul, mut avec) = (Vec::new(), Vec::new());
    for passe in 0..2 {
        let mut t = base.clone();
        let mut arbre = syntaxe::analyser(&t);
        let mut courante = Imbrique::analyser(&t, &arbre);
        for k in 0..300 {
            let mut p = if k < 100 {
                t.len() / 2 + k
            } else {
                h.sous(t.len())
            };
            while !t.is_char_boundary(p) {
                p -= 1;
            }
            let car = ["x", " ", "\n", "|", "`", "$"][h.sous(6)];
            t.insert_str(p, car);
            let c = Changement::remplacement(p, p, car);
            let t0 = std::time::Instant::now();
            arbre = arbre.reanalyser(&t, c);
            let d_arbre = t0.elapsed();
            if passe == 1 {
                let t1 = std::time::Instant::now();
                courante = courante.reanalyser(&t, &arbre, c);
                avec.push(d_arbre + t1.elapsed());
            } else {
                seul.push(d_arbre);
            }
        }
        if passe == 1 {
            assert_eq!(courante, Imbrique::analyser(&t, &arbre));
        }
    }
    for (nom, v) in [
        ("arbre seul", &mut seul),
        ("arbre + langages imbriqués", &mut avec),
    ] {
        v.sort();
        println!(
            "{nom} : min {:?} / médiane {:?} / p95 {:?} / max {:?}",
            v[0],
            v[v.len() / 2],
            v[v.len() * 95 / 100],
            v[v.len() - 1]
        );
    }
    let arbre = syntaxe::analyser(&base);
    let t0 = std::time::Instant::now();
    let tout = Imbrique::analyser(&base, &arbre);
    println!(
        "analyse complète de la couche : {:?} ({} régions)",
        t0.elapsed(),
        tout.nombre_de_regions()
    );
}
