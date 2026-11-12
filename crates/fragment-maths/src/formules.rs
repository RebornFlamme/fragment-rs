//! Les formules du spike S5, écrites pour le spike d'après les cas des specs `editor-hell` et
//! `markdown-html` (puissance, fraction, racine, somme, intégrale, matrice, grec, ensembles,
//! chimie, cas). Même liste, même ordre que `capture/formules.ts` (côté Fragment).

/// Une formule : son nom de fichier, sa source LaTeX (sans délimiteurs) et son mode.
#[derive(Clone, Copy, Debug)]
pub struct Formule {
    pub nom: &'static str,
    pub tex: &'static str,
    /// `$$…$$` sur ses propres lignes (bloc centré) ; sinon `$…$` dans une phrase.
    pub bloc: bool,
}

pub const FORMULES: &[Formule] = &[
    Formule {
        nom: "puissance",
        tex: "x^2",
        bloc: false,
    },
    Formule {
        nom: "grec",
        tex: r"\alpha + \beta = \gamma",
        bloc: false,
    },
    Formule {
        nom: "racine",
        tex: r"\sqrt{2} \approx 1.414",
        bloc: false,
    },
    Formule {
        nom: "euler",
        tex: r"e^{i\pi} + 1 = 0",
        bloc: false,
    },
    Formule {
        nom: "fraction-en-ligne",
        tex: r"\frac{a}{b}",
        bloc: false,
    },
    Formule {
        nom: "ensemble",
        tex: r"n \in \mathbb{N}",
        bloc: false,
    },
    Formule {
        nom: "fraction",
        tex: r"\frac{a+b}{c-d}",
        bloc: true,
    },
    Formule {
        nom: "somme",
        tex: r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}",
        bloc: true,
    },
    Formule {
        nom: "integrale",
        tex: r"\int_0^1 f(x)\,dx",
        bloc: true,
    },
    Formule {
        nom: "matrice",
        tex: r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
        bloc: true,
    },
    Formule {
        nom: "chimie",
        tex: r"\ce{2H2 + O2 -> 2H2O}",
        bloc: true,
    },
    Formule {
        nom: "cas",
        tex: r"f(x) = \begin{cases} 1 & x > 0 \\ 0 & \text{sinon} \end{cases}",
        bloc: true,
    },
];
