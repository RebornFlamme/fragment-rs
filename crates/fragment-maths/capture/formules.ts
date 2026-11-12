// Les formules du spike S5 (écrites pour le spike, d'après les cas des specs editor-hell et
// markdown-html : puissance, fraction, racine, somme, intégrale, matrice, lettres grecques,
// ensembles, chimie). Même liste que src/formules.rs côté Rust.
export const FORMULES: { nom: string; tex: string; bloc: boolean }[] = [
    { nom: 'puissance', tex: 'x^2', bloc: false },
    { nom: 'grec', tex: '\\alpha + \\beta = \\gamma', bloc: false },
    { nom: 'racine', tex: '\\sqrt{2} \\approx 1.414', bloc: false },
    { nom: 'euler', tex: 'e^{i\\pi} + 1 = 0', bloc: false },
    { nom: 'fraction-en-ligne', tex: '\\frac{a}{b}', bloc: false },
    { nom: 'ensemble', tex: 'n \\in \\mathbb{N}', bloc: false },
    { nom: 'fraction', tex: '\\frac{a+b}{c-d}', bloc: true },
    { nom: 'somme', tex: '\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}', bloc: true },
    { nom: 'integrale', tex: '\\int_0^1 f(x)\\,dx', bloc: true },
    { nom: 'matrice', tex: '\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}', bloc: true },
    { nom: 'chimie', tex: '\\ce{2H2 + O2 -> 2H2O}', bloc: true },
    { nom: 'cas', tex: 'f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{sinon} \\end{cases}', bloc: true },
];
