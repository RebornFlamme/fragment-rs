// Port des cas d'app/e2e/editor-hell.spec.ts (« repli imbriqué », « un clic sur chaque ligne SOUS un
// bloc », Properties) et des règles de mathBlockField / embedField (decorationPlugin.ts), htmlBlockField
// (htmlDecorations.ts), tableField (table/tableField.ts) et propertiesField (properties/frontmatterField.ts),
// au niveau du MODÈLE.
//! Tests de `fragment_markdown::blocs` (fr-t3-blocs-modele). Les documents sont réécrits.

use fragment_markdown::blocs::{
    self, AnalyseRepli, BlocCalcule, ContenuBloc, Edition, EtatRepli, ModeBloc, NatureRepli,
};
use fragment_markdown::decorations::{Contexte, Curseur, Revelation};
use fragment_markdown::syntaxe::modele_rangees::{GenreBloc, ModeleRangees, Placement, lignes_de};
use fragment_markdown::syntaxe::{self, Etendue};

// ─── Outils ──────────────────────────────────────────────────────────────────

/// Les blocs de `texte` avec le curseur vide en `pos` (UTF-16), éditeur hôte.
fn blocs_en(texte: &str, pos: usize) -> Vec<BlocCalcule> {
    blocs_avec(
        texte,
        Etendue::new(pos, pos),
        &Contexte::default(),
        true,
        true,
    )
}

fn blocs_avec(
    texte: &str,
    sel: Etendue,
    ctx: &Contexte,
    focus: bool,
    a_eu_le_focus: bool,
) -> Vec<BlocCalcule> {
    let arbre = syntaxe::analyser(texte);
    let selections = [sel];
    let curseur = Curseur {
        selections: &selections,
        focus,
        a_eu_le_focus,
    };
    blocs::calculer(texte, &arbre, &curseur, ctx, false)
}

fn lecture_seule() -> Contexte<'static> {
    Contexte {
        lecture_seule: true,
        ..Contexte::default()
    }
}

/// Le seul bloc du document (le test échoue sinon).
fn un_bloc(texte: &str, pos: usize) -> BlocCalcule {
    let mut v = blocs_en(texte, pos);
    assert_eq!(v.len(), 1, "un seul bloc attendu : {v:?}");
    v.remove(0)
}

fn analyse(texte: &str) -> AnalyseRepli {
    AnalyseRepli::new(texte, &syntaxe::analyser(texte))
}

/// Les lignes (à partir de 0) dont la plage de repli existe.
fn repliables(texte: &str) -> Vec<usize> {
    analyse(texte).toutes().iter().map(|p| p.ligne).collect()
}

const MATHS: &str = "avant\n$$\n\\frac{a}{b}\n$$\naprès";

// ─── Maths $$ ────────────────────────────────────────────────────────────────

#[test]
fn maths_curseur_dehors_le_rendu_remplace_les_lignes_entieres() {
    let b = un_bloc(MATHS, 0);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (1, 3));
    assert_eq!(b.source.texte(MATHS), "$$\n\\frac{a}{b}\n$$");
    assert_eq!(
        b.contenu,
        ContenuBloc::Maths {
            tex: "\n\\frac{a}{b}\n".into()
        }
    );
}

#[test]
fn maths_curseur_dedans_la_source_reste_et_le_rendu_s_ajoute_dessous() {
    // Au milieu de la formule (ligne « \frac »).
    let b = un_bloc(MATHS, 10);
    assert_eq!(b.mode, ModeBloc::SourceEtRendu);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (1, 3));
}

#[test]
fn maths_les_bornes_comptent_comme_dedans() {
    // Juste avant le `$$` d'ouverture (début de la ligne 1) et juste après le `$$` final.
    assert_eq!(un_bloc(MATHS, 6).mode, ModeBloc::SourceEtRendu);
    let fin = MATHS.find("après").unwrap() - 1;
    assert_eq!(un_bloc(MATHS, fin).mode, ModeBloc::SourceEtRendu);
    // Une position de plus loin : dehors.
    assert_eq!(un_bloc(MATHS, fin + 2).mode, ModeBloc::Remplace);
}

#[test]
fn maths_vide_n_est_pas_un_bloc() {
    assert!(blocs_en("a\n$$\n  \n$$\nb", 0).is_empty());
}

#[test]
fn maths_non_ferme_garde_son_tex() {
    let b = un_bloc("$$\nx^2", 0);
    assert_eq!(
        b.contenu,
        ContenuBloc::Maths {
            tex: "\nx^2".into()
        }
    );
}

#[test]
fn maths_en_lecture_seule_le_rendu_remplace_toujours() {
    let v = blocs_avec(MATHS, Etendue::new(10, 10), &lecture_seule(), true, true);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].mode, ModeBloc::Remplace);
}

#[test]
fn maths_rien_n_est_revele_avant_le_premier_focus_d_une_note() {
    let ctx = Contexte {
        revelation: Revelation::ApresPremierFocus,
        ..Contexte::default()
    };
    let v = blocs_avec(MATHS, Etendue::new(10, 10), &ctx, false, false);
    assert_eq!(v[0].mode, ModeBloc::Remplace);
    let v = blocs_avec(MATHS, Etendue::new(10, 10), &ctx, false, true);
    assert_eq!(v[0].mode, ModeBloc::SourceEtRendu);
}

#[test]
fn maths_seule_la_selection_principale_revele() {
    let arbre = syntaxe::analyser(MATHS);
    let sels = [Etendue::new(0, 0), Etendue::new(10, 10)];
    let curseur = Curseur {
        selections: &sels,
        focus: true,
        a_eu_le_focus: true,
    };
    let v = blocs::calculer(MATHS, &arbre, &curseur, &Contexte::default(), false);
    assert_eq!(v[0].mode, ModeBloc::Remplace);
}

#[test]
fn maths_positions_utf16_avec_un_emoji() {
    let t = "😀\n$$\nx\n$$";
    let b = un_bloc(t, 0);
    assert_eq!(b.source.utf16, Etendue::new(3, 10));
    assert_eq!(b.source.octets, Etendue::new(5, 12));
    // Le curseur en UTF-16 (3 = début de la ligne des maths) révèle.
    assert_eq!(un_bloc(t, 3).mode, ModeBloc::SourceEtRendu);
}

// ─── Embeds ![[…]] ───────────────────────────────────────────────────────────

#[test]
fn embed_seul_sur_sa_ligne_remplace_la_ligne() {
    let t = "a\n![[image.png]]\nb";
    let b = un_bloc(t, 0);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (1, 1));
    assert_eq!(
        b.contenu,
        ContenuBloc::Embed {
            interieur: "image.png".into()
        }
    );
}

#[test]
fn embed_seul_curseur_dessus_source_et_rendu_dessous() {
    let t = "a\n![[image.png]]\nb";
    assert_eq!(un_bloc(t, 6).mode, ModeBloc::SourceEtRendu);
}

#[test]
fn embed_avec_blancs_autour_reste_un_bloc() {
    let t = "a\n  ![[note|300]]  \nb";
    assert!(matches!(
        un_bloc(t, 0).contenu,
        ContenuBloc::Embed { ref interieur } if interieur == "note|300"
    ));
}

#[test]
fn embed_au_milieu_d_une_phrase_n_est_pas_un_bloc() {
    assert!(blocs_en("texte ![[large.png|300]] inline", 0).is_empty());
}

#[test]
fn embed_vide_n_est_pas_un_bloc() {
    assert!(blocs_en("a\n![[ ]]\nb", 0).is_empty());
}

// ─── Tableaux ────────────────────────────────────────────────────────────────

const TABLEAU: &str = "avant\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\naprès";

#[test]
fn tableau_remplace_ses_lignes_entieres() {
    let b = un_bloc(TABLEAU, 0);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (1, 3));
    assert_eq!(
        b.contenu,
        ContenuBloc::Tableau {
            source: "| A | B |\n| --- | --- |\n| 1 | 2 |".into()
        }
    );
}

#[test]
fn tableau_n_est_jamais_revele_meme_sous_le_curseur() {
    // Le curseur dans la source : on édite les cellules, la source ne reparaît pas.
    assert_eq!(un_bloc(TABLEAU, 10).mode, ModeBloc::Remplace);
    assert_eq!(
        un_bloc(TABLEAU, 10).source.texte(TABLEAU).lines().count(),
        3
    );
}

#[test]
fn tableau_deux_tableaux_deux_blocs() {
    let t = "| a |\n| - |\n| 1 |\n\ntexte\n\n| b |\n| - |\n| 2 |";
    let v = blocs_en(t, 0);
    assert_eq!(v.len(), 2);
    assert_eq!((v[1].premiere_ligne, v[1].derniere_ligne), (6, 8));
}

// ─── Front matter (Properties) ───────────────────────────────────────────────

const NOTE: &str = "---\ntags:\n  - a\n  - b\nstatut: brouillon\n---\nsous properties";

#[test]
fn proprietes_valides_toujours_remplacees() {
    let b = un_bloc(NOTE, 0);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!(b.contenu, ContenuBloc::Proprietes { replie: false });
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (0, 5));
    assert_eq!(b.source.utf16, Etendue::new(0, 43));
}

#[test]
fn proprietes_remplacees_meme_curseur_dedans() {
    assert_eq!(un_bloc(NOTE, 8).mode, ModeBloc::Remplace);
}

#[test]
fn proprietes_yaml_invalide_la_source_reste_visible() {
    assert!(blocs_en("---\nclé: [non fermé\n---\ntexte", 0).is_empty());
}

#[test]
fn proprietes_non_fermees_ne_sont_pas_un_bloc() {
    // Le `---` de tête est alors une règle (ou un titre Setext), jamais des Properties.
    assert!(
        blocs_en("---\nstatut: brouillon\ntexte", 0)
            .iter()
            .all(|b| !matches!(b.contenu, ContenuBloc::Proprietes { .. }))
    );
}

#[test]
fn proprietes_le_trait_du_front_matter_n_est_pas_un_separateur() {
    let v = blocs_en(NOTE, 0);
    assert!(v.iter().all(|b| b.contenu != ContenuBloc::Separateur));
}

#[test]
fn proprietes_l_etat_replie_est_porte_par_le_bloc() {
    let arbre = syntaxe::analyser(NOTE);
    let curseur = Curseur::default();
    let v = blocs::calculer(NOTE, &arbre, &curseur, &Contexte::default(), true);
    assert_eq!(v[0].contenu, ContenuBloc::Proprietes { replie: true });
}

#[test]
fn proprietes_en_ligne_deux_sur_un_document_sans_trait_final() {
    // Un `---` en ligne 3 qui n'est pas en tête ne fait pas un front matter.
    assert!(
        blocs_en("a\n---\nb: 1\n---\n", 0)
            .iter()
            .all(|b| !matches!(b.contenu, ContenuBloc::Proprietes { .. }))
    );
}

// ─── HTML et commentaires ────────────────────────────────────────────────────

const HTML: &str = "avant\n\n<div class=\"x\">\n  <b>gras</b>\n</div>\n\naprès";

#[test]
fn html_bloc_remplace_hors_curseur() {
    let b = un_bloc(HTML, 0);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (2, 4));
    assert!(matches!(&b.contenu, ContenuBloc::Html { source } if source.starts_with("<div")));
}

#[test]
fn html_bloc_sous_le_curseur_source_seule() {
    // Dans la source : pas de rendu en plus (le bouton « </> » ramène la source, ici déjà là).
    let b = un_bloc(HTML, 20);
    assert_eq!(b.mode, ModeBloc::SourceSeule);
}

#[test]
fn html_bloc_en_lecture_seule_toujours_rendu() {
    let v = blocs_avec(HTML, Etendue::new(20, 20), &lecture_seule(), true, true);
    assert_eq!(v[0].mode, ModeBloc::Remplace);
}

#[test]
fn commentaire_de_bloc_cache_hors_curseur() {
    let t = "avant\n\n<!-- note\nprivée -->\n\naprès";
    let b = un_bloc(t, 0);
    assert_eq!(b.contenu, ContenuBloc::Commentaire);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (2, 3));
}

#[test]
fn commentaire_de_bloc_sous_le_curseur_source_seule() {
    let t = "avant\n\n<!-- note\nprivée -->\n\naprès";
    assert_eq!(un_bloc(t, 12).mode, ModeBloc::SourceSeule);
}

// ─── Code et séparateur ──────────────────────────────────────────────────────

const CODE: &str = "avant\n```js\nconst a = 1;\n\nconst b = 2;\n```\naprès";

#[test]
fn code_porte_sa_langue_et_garde_sa_source() {
    let b = un_bloc(CODE, 0);
    assert_eq!(
        b.contenu,
        ContenuBloc::Code {
            langue: "js".into()
        }
    );
    assert_eq!(b.mode, ModeBloc::SourceSeule);
    assert_eq!((b.premiere_ligne, b.derniere_ligne), (1, 5));
}

#[test]
fn code_sans_langue() {
    let b = un_bloc("```\nx\n```", 0);
    assert_eq!(
        b.contenu,
        ContenuBloc::Code {
            langue: String::new()
        }
    );
}

#[test]
fn code_les_dieses_ne_sont_pas_des_titres_dans_le_code() {
    // Pas de bloc de plus : le bloc de code reste un seul bloc.
    assert_eq!(blocs_en("```\n# pas un titre\n```", 0).len(), 1);
}

#[test]
fn separateur_remplace_hors_curseur_et_source_dessus() {
    let t = "a\n\n***\n\nb";
    let b = un_bloc(t, 0);
    assert_eq!(b.contenu, ContenuBloc::Separateur);
    assert_eq!(b.mode, ModeBloc::Remplace);
    assert_eq!(un_bloc(t, 4).mode, ModeBloc::SourceSeule);
}

// ─── Plusieurs blocs dans leur ordre (le document de l'e2e « clic sous un bloc ») ─────────────

#[test]
fn les_blocs_d_un_document_riche_sont_tries_et_typés() {
    let t = format!("{NOTE}\n\n{TABLEAU}\n{CODE}\n$$\nx\n$$\n![[image.png]]\n***\nfin");
    let v = blocs_en(&t, t.len());
    let genres: Vec<_> = v
        .iter()
        .map(|b| match b.contenu {
            ContenuBloc::Proprietes { .. } => "props",
            ContenuBloc::Tableau { .. } => "tableau",
            ContenuBloc::Code { .. } => "code",
            ContenuBloc::Maths { .. } => "maths",
            ContenuBloc::Embed { .. } => "embed",
            ContenuBloc::Separateur => "sep",
            _ => "autre",
        })
        .collect();
    assert_eq!(
        genres,
        ["props", "tableau", "code", "maths", "embed", "sep"]
    );
    assert!(
        v.windows(2)
            .all(|w| w[0].derniere_ligne < w[1].premiere_ligne)
    );
}

// ─── Traduction vers le modèle de rangées ────────────────────────────────────

fn modele_de(texte: &str, pos: usize, repli: &EtatRepli, hauteur: f32) -> ModeleRangees {
    let arbre = syntaxe::analyser(texte);
    let selections = [Etendue::new(pos, pos)];
    let curseur = Curseur {
        selections: &selections,
        focus: true,
        a_eu_le_focus: true,
    };
    let blocs = blocs::calculer(
        texte,
        &arbre,
        &curseur,
        &Contexte::default(),
        repli.proprietes_repliees,
    );
    let mut m = ModeleRangees::default();
    let a = AnalyseRepli::new(texte, &arbre);
    blocs::integrer(&mut m, &lignes_de(texte), &blocs, repli, &a, &|_| hauteur);
    m.normaliser();
    m
}

#[test]
fn rangees_bloc_remplace_masque_ses_lignes_et_dicte_la_hauteur() {
    let m = modele_de(MATHS, 0, &EtatRepli::default(), 48.0);
    assert_eq!(m.blocs.len(), 1);
    assert_eq!(m.blocs[0].genre, GenreBloc::Maths);
    assert_eq!(m.blocs[0].placement, Placement::ALaPlace);
    let f = m.vers_fork(&lignes_de(MATHS));
    assert!(f.blocs.is_empty());
    // Les trois lignes de la source, masquées ; la hauteur est dictée à la première.
    assert_eq!(f.masques, [6..8, 9..20, 21..23]);
    assert_eq!(f.hauteurs, [(1, 48.0), (2, 0.0), (3, 0.0)]);
}

#[test]
fn rangees_bloc_sous_le_curseur_ajoute_de_la_hauteur_sous_la_derniere_ligne() {
    let m = modele_de(MATHS, 10, &EtatRepli::default(), 48.0);
    assert_eq!(m.blocs[0].placement, Placement::Dessous);
    let f = m.vers_fork(&lignes_de(MATHS));
    assert!(f.masques.is_empty());
    assert!(f.hauteurs.is_empty());
    assert_eq!(f.blocs, [(3, 48.0)]);
}

#[test]
fn rangees_commentaire_cache_masque_ses_lignes_sans_hauteur() {
    let t = "avant\n\n<!-- note\nprivée -->\n\naprès";
    let m = modele_de(t, 0, &EtatRepli::default(), 99.0);
    assert!(m.blocs.is_empty());
    let f = m.vers_fork(&lignes_de(t));
    assert_eq!(f.hauteurs, [(2, 0.0), (3, 0.0)]);
    assert_eq!(f.masques.len(), 2);
}

#[test]
fn rangees_code_et_separateur_ne_changent_pas_les_rangees() {
    let t = format!("{CODE}\n***\nfin");
    let m = modele_de(&t, t.len(), &EtatRepli::default(), 30.0);
    assert_eq!(m, ModeleRangees::default());
}

#[test]
fn rangees_source_seule_ne_change_rien() {
    // Bloc HTML sous le curseur : la source seule.
    let m = modele_de(HTML, 20, &EtatRepli::default(), 77.0);
    assert!(m.blocs.is_empty() && m.masques.is_empty());
}

#[test]
fn rangees_proprietes_remplacees_a_la_hauteur_du_widget() {
    let m = modele_de(NOTE, 0, &EtatRepli::default(), 60.0);
    let f = m.vers_fork(&lignes_de(NOTE));
    assert_eq!(f.hauteurs[0], (0, 60.0));
    assert_eq!(f.hauteurs.len(), 6);
}

// ─── Repli : plages ──────────────────────────────────────────────────────────

const REPLI: &str = "## A\ntexte A\n### A1\ntexte A1\n## B\ntexte B";

#[test]
fn repli_un_titre_replie_jusqu_au_prochain_titre_de_niveau_inferieur_ou_egal() {
    let a = analyse(REPLI);
    let p = a.plage(0).unwrap();
    assert_eq!(p.nature, NatureRepli::Titre(2));
    // Du bout de « ## A » jusqu'à la fin de « texte A1 » (sans « ## B »).
    assert_eq!(p.plage.utf16, Etendue::new(4, 28));
    assert_eq!((p.ligne, p.derniere_ligne), (0, 3));
    assert_eq!(a.plage(2).unwrap().derniere_ligne, 3);
    assert_eq!(a.plage(4).unwrap().derniere_ligne, 5);
}

#[test]
fn repli_imbrique_replier_un_h2_cache_son_h3() {
    let t = REPLI;
    let a = analyse(t);
    let mut etat = EtatRepli::default();
    assert!(etat.basculer(&a, 0));
    // « ### A1 » (ligne 2) et « texte A1 » (ligne 3) sont cachés ; « ## B » reste.
    assert_eq!(etat.lignes_cachees(&a), [(1, 3)]);
    // Le déplier les rend.
    assert!(etat.basculer(&a, 0));
    assert!(etat.lignes_cachees(&a).is_empty());
    assert!(etat.plis().is_empty());
}

#[test]
fn repli_la_source_n_est_jamais_modifiee_par_le_repli() {
    // Le modèle ne contient que des plages : le texte reste celui de l'e2e.
    let a = analyse(REPLI);
    let mut etat = EtatRepli::default();
    etat.basculer(&a, 0);
    etat.basculer(&a, 0);
    assert_eq!(REPLI, "## A\ntexte A\n### A1\ntexte A1\n## B\ntexte B");
}

#[test]
fn repli_un_h3_replie_seulement_son_corps() {
    let a = analyse(REPLI);
    let p = a.plage(2).unwrap();
    assert_eq!(p.nature, NatureRepli::Titre(3));
    assert_eq!((p.ligne, p.derniere_ligne), (2, 3));
}

#[test]
fn repli_n_englobe_pas_les_lignes_vides_de_fin() {
    let t = "# T\ntexte\n\n\n# U\nautre";
    let p = analyse(t).plage(0).unwrap();
    assert_eq!(p.derniere_ligne, 1);
    assert_eq!(p.plage.utf16, Etendue::new(3, 9));
}

#[test]
fn repli_un_titre_sans_corps_n_est_pas_repliable() {
    assert_eq!(repliables("# A\n# B\ntexte"), [1]);
    assert_eq!(repliables("# Seul"), Vec::<usize>::new());
}

#[test]
fn repli_un_titre_sans_corps_ne_retombe_pas_sur_l_indentation() {
    // Le titre n'a pas de corps ; la ligne indentée suivante ne fait pas de lui un parent.
    assert!(analyse("# T\n  indenté").plage(0).is_some());
    assert!(analyse("# T\n# U\n  indenté").plage(0).is_none());
}

#[test]
fn repli_un_diese_dans_un_bloc_de_code_n_est_pas_un_titre() {
    let t = "```\n# pas un titre\n  suite\n```";
    // Aucune plage de titre ; seule l'indentation peut s'appliquer (la ligne « # … » est parent).
    let p = analyse(t).plage(1).unwrap();
    assert_eq!(p.nature, NatureRepli::Indentation);
}

#[test]
fn repli_un_titre_dans_une_citation_n_est_pas_repliable() {
    assert!(analyse("> # T\n> corps\n\nfin").plage(0).is_none());
}

#[test]
fn repli_les_titres_de_tous_les_niveaux() {
    let t = "# 1\na\n## 2\nb\n### 3\nc\n#### 4\nd\n##### 5\ne\n###### 6\nf";
    let a = analyse(t);
    for (ligne, niveau) in (0..12).step_by(2).zip(1u8..) {
        assert_eq!(a.plage(ligne).unwrap().nature, NatureRepli::Titre(niveau));
    }
    // Le H1 avale tout.
    assert_eq!(a.plage(0).unwrap().derniere_ligne, 11);
}

#[test]
fn repli_une_liste_imbriquee() {
    let t = "- a\n  - a1\n    - a11\n  - a2\n- b";
    let a = analyse(t);
    let p = a.plage(0).unwrap();
    assert_eq!(p.nature, NatureRepli::Liste);
    assert_eq!(p.derniere_ligne, 3);
    assert_eq!(a.plage(1).unwrap().derniere_ligne, 2);
    assert!(a.plage(2).is_none());
    assert!(a.plage(4).is_none());
}

#[test]
fn repli_une_liste_a_puces_et_une_liste_numerotee_se_replient() {
    assert_eq!(
        analyse("1. a\n   - x\n2. b").plage(0).unwrap().nature,
        NatureRepli::Liste
    );
}

#[test]
fn repli_l_indentation_replie_les_lignes_plus_indentees() {
    let t = "parent\n    enfant\n    autre\nfrère";
    let p = analyse(t).plage(0).unwrap();
    assert_eq!(p.nature, NatureRepli::Indentation);
    assert_eq!(p.derniere_ligne, 2);
}

#[test]
fn repli_une_ligne_vide_au_milieu_est_toleree() {
    let t = "parent\n  a\n\n  b\nfrère";
    assert_eq!(analyse(t).plage(0).unwrap().derniere_ligne, 3);
    // …mais pas celle de la fin.
    let t = "parent\n  a\n\nfrère";
    assert_eq!(analyse(t).plage(0).unwrap().derniere_ligne, 1);
}

#[test]
fn repli_une_ligne_vide_n_est_pas_repliable() {
    assert!(analyse("a\n\n  b").plage(1).is_none());
}

#[test]
fn repli_les_properties_se_replient_sur_leur_premiere_ligne() {
    let a = analyse(NOTE);
    let p = a.plage(0).unwrap();
    assert_eq!(p.nature, NatureRepli::Proprietes);
    assert_eq!((p.ligne, p.derniere_ligne), (0, 5));
    // La bascule ne cache aucune ligne : elle ne change que le rendu du widget.
    let mut etat = EtatRepli::default();
    assert!(etat.basculer(&a, 0));
    assert!(etat.proprietes_repliees);
    assert!(etat.lignes_cachees(&a).is_empty());
    assert!(etat.ligne_repliee(&a, 0));
    etat.basculer(&a, 0);
    assert!(!etat.proprietes_repliees);
}

#[test]
fn repli_un_frontmatter_invalide_n_est_pas_repliable_comme_bloc() {
    let a = analyse("---\nclé: [non fermé\n---\ntexte");
    assert!(
        a.plage(0)
            .is_none_or(|p| p.nature != NatureRepli::Proprietes)
    );
}

#[test]
fn repli_plages_en_utf16() {
    let t = "# é😀\ncorps";
    let p = analyse(t).plage(0).unwrap();
    // « # é😀 » = 5 unités UTF-16 (le point de code hors BMP en compte deux).
    assert_eq!(p.plage.utf16, Etendue::new(5, 11));
    assert_eq!(p.plage.octets, Etendue::new(8, 14));
}

#[test]
fn repli_dix_mille_lignes_restent_rapides() {
    let mut t = String::new();
    for i in 0..2500 {
        t.push_str(&format!("# T{i}\ntexte\n  - a\n  - b\n"));
    }
    let a = analyse(&t);
    assert_eq!(a.nombre_lignes(), 10_001);
    assert_eq!(a.toutes().len(), 2500 * 2);
}

// ─── Repli : état, remappage ─────────────────────────────────────────────────

fn replie(texte: &str, ligne: usize) -> (EtatRepli, AnalyseRepli) {
    let a = analyse(texte);
    let mut etat = EtatRepli::default();
    etat.basculer(&a, ligne);
    (etat, a)
}

#[test]
fn remappage_une_insertion_avant_decale_le_pli() {
    let (mut etat, _) = replie(REPLI, 0);
    assert_eq!(etat.plis(), [Etendue::new(4, 28)]);
    etat.remapper(&[Edition::remplacement(0, 0, 3)]);
    assert_eq!(etat.plis(), [Etendue::new(7, 31)]);
}

#[test]
fn remappage_une_frappe_au_bout_de_la_ligne_parent_reste_dehors() {
    // On tape à la fin de « ## A » : la ligne grandit, le pli commence après.
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[Edition::remplacement(4, 4, 2)]);
    assert_eq!(etat.plis(), [Etendue::new(6, 30)]);
}

#[test]
fn remappage_une_frappe_juste_apres_la_fin_du_pli_reste_dehors() {
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[Edition::remplacement(28, 28, 5)]);
    assert_eq!(etat.plis(), [Etendue::new(4, 28)]);
}

#[test]
fn remappage_une_edition_dans_le_pli_le_rallonge_ou_le_raccourcit() {
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[Edition::remplacement(12, 12, 4)]);
    assert_eq!(etat.plis(), [Etendue::new(4, 32)]);
    etat.remapper(&[Edition::remplacement(12, 16, 0)]);
    assert_eq!(etat.plis(), [Etendue::new(4, 28)]);
}

#[test]
fn remappage_supprimer_le_debut_du_pli_le_perd() {
    // La suppression de « ## A\ntexte A » : le début du pli tombe dans du texte supprimé.
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[Edition::remplacement(2, 12, 0)]);
    assert!(etat.plis().is_empty());
}

#[test]
fn remappage_supprimer_tout_le_pli_le_perd() {
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[Edition::remplacement(4, 28, 0)]);
    assert!(etat.plis().is_empty());
}

#[test]
fn remappage_une_suite_d_editions_se_lit_dans_l_ordre() {
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[
        Edition::remplacement(0, 0, 2),
        Edition::remplacement(0, 2, 0),
    ]);
    assert_eq!(etat.plis(), [Etendue::new(4, 28)]);
}

#[test]
fn remappage_plusieurs_plis_independants() {
    let t = "# A\na\n# B\nb\n# C\nc";
    let a = analyse(t);
    let mut etat = EtatRepli::default();
    etat.basculer(&a, 0);
    etat.basculer(&a, 4);
    assert_eq!(etat.plis().len(), 2);
    // On supprime la section B (lignes 2 et 3) : le pli de A et celui de C survivent, décalés.
    etat.remapper(&[Edition::remplacement(6, 12, 0)]);
    assert_eq!(etat.plis(), [Etendue::new(3, 5), Etendue::new(9, 11)]);
}

#[test]
fn remappage_ne_touche_pas_au_repli_des_properties() {
    let a = analyse(NOTE);
    let mut etat = EtatRepli::default();
    etat.basculer(&a, 0);
    etat.remapper(&[Edition::remplacement(0, 0, 4)]);
    assert!(etat.proprietes_repliees);
}

#[test]
fn deplier_au_second_clic_meme_apres_une_edition() {
    let (mut etat, _) = replie(REPLI, 0);
    etat.remapper(&[Edition::remplacement(0, 0, 3)]);
    let t2 = format!("xyz{REPLI}");
    let a2 = analyse(&t2);
    assert!(etat.ligne_repliee(&a2, 0));
    etat.basculer(&a2, 0);
    assert!(etat.plis().is_empty());
}

#[test]
fn repli_nest_et_deplie_par_titre_remonte() {
    // Replier A puis A1 : deux plis ; déplier A rend A1 encore replié.
    let a = analyse(REPLI);
    let mut etat = EtatRepli::default();
    etat.basculer(&a, 2);
    etat.basculer(&a, 0);
    assert_eq!(etat.plis().len(), 2);
    assert_eq!(etat.lignes_cachees(&a), [(1, 3)]);
    etat.basculer(&a, 0);
    assert_eq!(etat.lignes_cachees(&a), [(3, 3)]);
}

// ─── Repli → modèle de rangées ───────────────────────────────────────────────

#[test]
fn rangees_le_repli_masque_les_lignes_cachees_a_hauteur_nulle() {
    let (etat, _) = replie(REPLI, 0);
    let m = modele_de(REPLI, 0, &etat, 0.0);
    let f = m.vers_fork(&lignes_de(REPLI));
    assert_eq!(f.hauteurs, [(1, 0.0), (2, 0.0), (3, 0.0)]);
    // texte A, ### A1, texte A1.
    assert_eq!(f.masques, [5..12, 13..19, 20..28]);
}

#[test]
fn rangees_le_repli_retire_la_hauteur_et_la_police_d_un_titre_cache() {
    let (etat, a) = replie(REPLI, 0);
    let mut m = ModeleRangees::default();
    // Les décorations avaient dicté une grande hauteur et une police au « ### A1 » (ligne 2).
    m.hauteurs
        .push(fragment_markdown::syntaxe::modele_rangees::HauteurLigne {
            ligne: 2,
            hauteur: 30.0,
        });
    m.polices
        .push(fragment_markdown::syntaxe::modele_rangees::PoliceLigne {
            ligne: 2,
            taille: 20.0,
        });
    blocs::integrer(&mut m, &lignes_de(REPLI), &[], &etat, &a, &|_| 0.0);
    m.normaliser();
    assert_eq!(
        m.hauteurs.iter().find(|h| h.ligne == 2).unwrap().hauteur,
        0.0
    );
    assert!(m.polices.iter().all(|p| p.ligne != 2));
}

#[test]
fn rangees_un_bloc_dans_une_section_repliee_disparait_du_modele() {
    let t = "# A\n$$\nx\n$$\n# B";
    let (etat, _) = replie(t, 0);
    let m = modele_de(t, t.len(), &etat, 40.0);
    assert!(m.blocs.is_empty());
    let f = m.vers_fork(&lignes_de(t));
    assert!(f.blocs.is_empty());
    assert_eq!(f.hauteurs, [(1, 0.0), (2, 0.0), (3, 0.0)]);
}

// ─── Le curseur chassé d'une source cachée ───────────────────────────────────

#[test]
fn curseur_dans_les_properties_part_sur_la_ligne_d_apres() {
    let r = fragment_markdown::blocs::plage_proprietes(NOTE)
        .unwrap()
        .utf16;
    let repli = EtatRepli::default();
    let apres = r.to + 1;
    assert_eq!(
        blocs::chasser_curseur(0, NOTE.len(), Some(r), &repli),
        apres
    );
    assert_eq!(
        blocs::chasser_curseur(10, NOTE.len(), Some(r), &repli),
        apres
    );
    // Au bout du `---` final : aussi.
    assert_eq!(
        blocs::chasser_curseur(r.to, NOTE.len(), Some(r), &repli),
        apres
    );
    // Déjà après : on ne bouge pas.
    assert_eq!(
        blocs::chasser_curseur(apres, NOTE.len(), Some(r), &repli),
        apres
    );
    assert_eq!(
        blocs::chasser_curseur(apres + 3, NOTE.len(), Some(r), &repli),
        apres + 3
    );
}

#[test]
fn curseur_properties_qui_terminent_le_document_vont_a_la_fin() {
    let t = "---\na: 1\n---";
    let r = fragment_markdown::blocs::plage_proprietes(t).unwrap().utf16;
    assert_eq!(r.to, t.len());
    assert_eq!(
        blocs::chasser_curseur(5, t.len(), Some(r), &EtatRepli::default()),
        t.len()
    );
}

#[test]
fn curseur_sans_properties_ne_bouge_pas() {
    assert_eq!(
        blocs::chasser_curseur(0, 10, None, &EtatRepli::default()),
        0
    );
}

#[test]
fn curseur_dans_une_plage_repliee_remonte_au_bout_de_la_ligne_parent() {
    let (etat, _) = replie(REPLI, 0);
    // « ## A » finit en 4 ; le pli couvre (4, 28].
    assert_eq!(blocs::chasser_curseur(15, REPLI.len(), None, &etat), 4);
    assert_eq!(blocs::chasser_curseur(28, REPLI.len(), None, &etat), 4);
    // Sur la ligne parent, après le pli : inchangé.
    assert_eq!(blocs::chasser_curseur(4, REPLI.len(), None, &etat), 4);
    assert_eq!(blocs::chasser_curseur(29, REPLI.len(), None, &etat), 29);
}

#[test]
fn curseur_dans_des_plis_imbriques_remonte_jusqu_au_plus_exterieur() {
    let a = analyse(REPLI);
    let mut etat = EtatRepli::default();
    etat.basculer(&a, 2);
    etat.basculer(&a, 0);
    assert_eq!(blocs::chasser_curseur(25, REPLI.len(), None, &etat), 4);
}
