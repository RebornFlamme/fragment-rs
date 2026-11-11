// Port des règles du live preview d'app/src/ui/views/markdown/editor.css et embeds.css.
//! Les styles des marques et des lignes, en NOMS de jetons du thème (`fragment-theme`, champs de
//! `Palette` et `Forme`) et en `em` du texte : jamais une couleur, jamais un pixel réestimé.
//!
//! Les valeurs viennent d'`editor.css` (voir `docs/constantes-rendu.md`, section
//! `ui/views/markdown/editor.css`) ; le numéro de ligne CSS suit chaque constante.

use super::{GenreLigne, GenreMarque};

/// `--font-text-size` par défaut (`editor.css:36`, le repli de `var(--font-text-size, 16px)`).
pub const TAILLE_TEXTE_DEFAUT: f32 = 16.0;

/// `--indent-width` (`editor.css:208`) : la cellule commune à une puce, une case et un niveau
/// d'indentation ; tous leurs centres tombent sur le même axe.
pub const LARGEUR_INDENTATION_EM: f32 = 1.6;

/// Interligne des titres (`.cm-ob-h1` … `h6`, `line-height: 1.25`, `editor.css:149`).
pub const INTERLIGNE_TITRE: f32 = 1.25;

/// Taille des titres 1 à 6, en `em` du texte (`editor.css:153` à `158`).
pub const TAILLES_TITRES_EM: [f32; 6] = [1.9, 1.6, 1.35, 1.18, 1.05, 1.0];

/// Taille du code, en ligne et en bloc (`.cm-ob-code` et `.cm-ob-codeblock`, `editor.css:93` et
/// `105`).
pub const TAILLE_CODE_EM: f32 = 0.9;

/// Style d'une marque en ligne. Un champ `None` / `false` = hérité du texte.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StyleMarque {
    /// Jeton de couleur du texte (`Palette`).
    pub couleur: Option<&'static str>,
    /// Jeton de couleur du fond (`Palette`).
    pub fond: Option<&'static str>,
    /// Jeton de graisse (`Forme`).
    pub graisse: Option<&'static str>,
    /// Jeton de pile de polices (`Forme`).
    pub police: Option<&'static str>,
    /// Taille relative au texte, en `em`.
    pub taille_em: Option<f32>,
    /// Jeton de rayon des coins du fond (`Forme`).
    pub rayon: Option<&'static str>,
    /// Marge intérieure du fond (vertical, horizontal), en `em`.
    pub marge_em: Option<(f32, f32)>,
    pub opacite: Option<f32>,
    pub italique: bool,
    pub barre: bool,
    pub souligne: bool,
}

/// Style d'une ligne entière. Un champ `None` / `false` = hérité.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StyleLigne {
    pub couleur: Option<&'static str>,
    pub fond: Option<&'static str>,
    pub graisse: Option<&'static str>,
    pub police: Option<&'static str>,
    pub taille_em: Option<f32>,
    /// Interligne, en multiple de la taille de la ligne.
    pub interligne: Option<f32>,
    pub opacite: Option<f32>,
    pub italique: bool,
    /// Bordure gauche : (multiple du jeton `border_width`, jeton de couleur).
    pub bordure_gauche: Option<(f32, &'static str)>,
    /// Retrait gauche du texte, en `em`.
    pub retrait_gauche_em: Option<f32>,
    /// Coins arrondis en haut / en bas (jeton de rayon).
    pub rayon_haut: Option<&'static str>,
    pub rayon_bas: Option<&'static str>,
    /// Marge intérieure haute / basse, en `em`.
    pub marge_haut_em: Option<f32>,
    pub marge_bas_em: Option<f32>,
    /// Hauteur minimale, en `em`.
    pub hauteur_min_em: Option<f32>,
    /// Traits d'indentation : (nombre de niveaux, jeton de couleur, opacité du `color-mix`).
    pub traits: Option<(usize, &'static str, f32)>,
}

pub(crate) fn style_marque(g: GenreMarque) -> StyleMarque {
    match g {
        // `.cm-ob-strong` (editor.css:87).
        GenreMarque::Gras => StyleMarque {
            graisse: Some("font_bold"),
            ..Default::default()
        },
        // `.cm-ob-em` (editor.css:88).
        GenreMarque::Italique => StyleMarque {
            italique: true,
            ..Default::default()
        },
        // `.cm-ob-strike` (editor.css:89) : barré, à 70 %.
        GenreMarque::Barre => StyleMarque {
            barre: true,
            opacite: Some(0.7),
            ..Default::default()
        },
        // `.cm-hl` (editor.css:58 à 63).
        GenreMarque::Surligne => StyleMarque {
            fond: Some("text_highlight_bg"),
            rayon: Some("radius_xs"),
            marge_em: Some((0.05, 0.15)),
            ..Default::default()
        },
        // `.cm-ob-code` (editor.css:91 à 97).
        GenreMarque::Code => StyleMarque {
            police: Some("font_monospace"),
            taille_em: Some(TAILLE_CODE_EM),
            fond: Some("background_modifier_hover"),
            rayon: Some("radius_s"),
            marge_em: Some((0.1, 0.35)),
            ..Default::default()
        },
        // `.cm-ob-link` (editor.css:161 à 165).
        GenreMarque::Lien => StyleMarque {
            couleur: Some("link_color"),
            souligne: true,
            ..Default::default()
        },
    }
}

pub(crate) fn style_ligne(g: GenreLigne) -> StyleLigne {
    match g {
        // `.cm-ob-h1` … `.cm-ob-h6` (editor.css:146 à 158) ; le H6 à 80 %.
        GenreLigne::Titre(n) => {
            let i = usize::from(n.clamp(1, 6)) - 1;
            StyleLigne {
                graisse: Some("font_bold"),
                taille_em: Some(TAILLES_TITRES_EM[i]),
                interligne: Some(INTERLIGNE_TITRE),
                opacite: (n == 6).then_some(0.8),
                ..Default::default()
            }
        }
        // `.cm-ob-quote` (editor.css:138 à 143) : trois fois `border_width`.
        GenreLigne::Citation => StyleLigne {
            bordure_gauche: Some((3.0, "background_modifier_border_hover")),
            retrait_gauche_em: Some(0.8),
            couleur: Some("text_muted"),
            italique: true,
            ..Default::default()
        },
        // `.cm-ob-codeblock` (editor.css:103 à 107).
        GenreLigne::Code => StyleLigne {
            police: Some("font_monospace"),
            taille_em: Some(TAILLE_CODE_EM),
            fond: Some("background_modifier_hover"),
            ..Default::default()
        },
        // `.cm-ob-codeblock-first` (editor.css:112 à 117).
        GenreLigne::CodePremiere => StyleLigne {
            rayon_haut: Some("radius_sm"),
            marge_haut_em: Some(0.7),
            hauteur_min_em: Some(1.15),
            ..Default::default()
        },
        // `.cm-ob-codeblock-last` (editor.css:118 à 123).
        GenreLigne::CodeDerniere => StyleLigne {
            rayon_bas: Some("radius_sm"),
            marge_bas_em: Some(0.7),
            hauteur_min_em: Some(1.15),
            ..Default::default()
        },
        // `.cm-indent-line` (editor.css:282) : un trait de 1 px au centre de chaque cellule,
        // `--indent-guide` = `text_normal` à 22 % (editor.css:209).
        GenreLigne::Indentation { niveaux } => StyleLigne {
            traits: Some((niveaux, "text_normal", 0.22)),
            ..Default::default()
        },
    }
}

/// Taille (px) d'un titre de niveau `n` pour un texte de `taille_texte` px.
#[must_use]
pub fn taille_titre(n: u8, taille_texte: f32) -> f32 {
    TAILLES_TITRES_EM[usize::from(n.clamp(1, 6)) - 1] * taille_texte
}
