//! Les formules du spike S5, écrites pour le spike d'après les cas des specs `editor-hell` et
//! `markdown-html` (puissance, fraction, racine, somme, intégrale, matrice, grec, ensembles,
//! chimie, cas). Même liste, même ordre que `capture/formules.ts` (côté Fragment).

/// Une formule : son nom de fichier, sa source LaTeX (sans délimiteurs), son mode, et sa
/// traduction en syntaxe typst (écrite à la main : typst ne lit pas le LaTeX, voir S5).
#[derive(Clone, Copy, Debug)]
pub struct Formule {
    pub nom: &'static str,
    pub tex: &'static str,
    /// `$$…$$` sur ses propres lignes (bloc centré) ; sinon `$…$` dans une phrase.
    pub bloc: bool,
    pub typst: &'static str,
}

pub const FORMULES: &[Formule] = &[
    Formule {
        nom: "puissance",
        tex: "x^2",
        bloc: false,
        typst: "x^2",
    },
    Formule {
        nom: "grec",
        tex: r"\alpha + \beta = \gamma",
        bloc: false,
        typst: "alpha + beta = gamma",
    },
    Formule {
        nom: "racine",
        tex: r"\sqrt{2} \approx 1.414",
        bloc: false,
        typst: "sqrt(2) approx 1.414",
    },
    Formule {
        nom: "euler",
        tex: r"e^{i\pi} + 1 = 0",
        bloc: false,
        typst: "e^(i pi) + 1 = 0",
    },
    Formule {
        nom: "fraction-en-ligne",
        tex: r"\frac{a}{b}",
        bloc: false,
        typst: "a/b",
    },
    Formule {
        nom: "ensemble",
        tex: r"n \in \mathbb{N}",
        bloc: false,
        typst: "n in NN",
    },
    Formule {
        nom: "fraction",
        tex: r"\frac{a+b}{c-d}",
        bloc: true,
        typst: "(a+b)/(c-d)",
    },
    Formule {
        nom: "somme",
        tex: r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
        bloc: true,
        typst: "sum_(n=1)^infinity 1/n^2 = pi^2/6",
    },
    Formule {
        nom: "integrale",
        tex: r"\int_0^1 f(x)\,dx",
        bloc: true,
        typst: "integral_0^1 f(x) thin d x",
    },
    Formule {
        nom: "matrice",
        tex: r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
        bloc: true,
        typst: "mat(a, b; c, d)",
    },
    Formule {
        nom: "chimie",
        tex: r"\ce{2H2 + O2 -> 2H2O}",
        bloc: true,
        typst: r#"2 upright(H)_2 + upright(O)_2 -> 2 upright(H)_2 upright(O)"#,
    },
    Formule {
        nom: "cas",
        tex: r"f(x) = \begin{cases} 1 & x > 0 \\ 0 & \text{sinon} \end{cases}",
        bloc: true,
        typst: r#"f(x) = cases(1 & x > 0, 0 & "sinon")"#,
    },
];
