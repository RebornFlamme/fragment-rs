// Port de app/e2e/pdf-find.spec.ts (tests 2 à 6), en tests du MODÈLE de la recherche PDF
// (fragment_pdf::recherche) : sans navigateur, sans pdfium. Même nom que le test e2e, en
// snake_case ; les cas AJOUTÉS (normalisation du texte, offsets, navigation, surlignages)
// suivent. Données réécrites : 30 pages de texte, quatre occurrences de « spin ».
//
// Non portés (hors modèle, à la barre montée : fr-t4-recherche-ui) : focus du champ,
// ouverture de la barre au clavier, clics sur Aa et le menu, DOM des surlignages,
// mesure en pixels du surlignage à 200 %. Le test « 200 % » est ici sa part pure :
// la plage rendue couvre le bon mot dans le texte de la couche.

#![allow(clippy::expect_used, clippy::unwrap_used)] // fichier de tests

use fragment_pdf::pdf_find_offsets::{Plage, creer_convertisseur};
use fragment_pdf::recherche::{
    EtatRecherche, EtatTrouve, ItemTexte, OptionsRecherche, PageRecherche, Recherche,
    ResultatRecherche, TypeRecherche, texte_compteur,
};

/// Le texte des 30 pages : « Spin spins » page 3, « Le SPIN total » page 10, « un spin
/// seul » page 20, « Page n » ailleurs.
fn textes() -> Vec<String> {
    (1..=30)
        .map(|n| match n {
            3 => "Spin spins".to_string(),
            10 => "Le SPIN total".to_string(),
            20 => "un spin seul".to_string(),
            n => format!("Page {n}"),
        })
        .collect()
}

fn document(textes: &[String]) -> Recherche {
    Recherche::new(
        textes
            .iter()
            .map(|t| PageRecherche::depuis_items(&[ItemTexte::new(t)]))
            .collect(),
    )
}

fn etat(type_: TypeRecherche, requete: &str, options: OptionsRecherche) -> EtatRecherche {
    EtatRecherche {
        type_,
        requete: requete.to_string(),
        vers_arriere: false,
        options,
    }
}

fn toujours(_: usize) -> bool {
    true
}

/// Une recherche de requête neuve depuis la page `courante`.
fn chercher(
    r: &mut Recherche,
    requete: &str,
    options: OptionsRecherche,
    courante: usize,
) -> ResultatRecherche {
    r.chercher(
        etat(TypeRecherche::Nouvelle, requete, options),
        courante,
        &toujours,
    )
}

/// Entrée (`sens` = 1) ou Maj+Entrée (`sens` = -1) depuis la page `courante`.
fn naviguer(
    r: &mut Recherche,
    requete: &str,
    options: OptionsRecherche,
    sens: i32,
    courante: usize,
) -> ResultatRecherche {
    let mut e = etat(TypeRecherche::Encore, requete, options);
    e.vers_arriere = sens < 0;
    r.chercher(e, courante, &toujours)
}

fn defaut() -> OptionsRecherche {
    OptionsRecherche::default()
}

/// Le compteur « n of m » après une recherche.
fn compteur(res: &ResultatRecherche, requete: &str) -> String {
    res.texte(requete)
}

// ── pdf-find.spec.ts ─────────────────────────────────────────────────

#[test]
fn la_frappe_cherche_dans_tout_le_document_entree_maj_entree_naviguent() {
    let mut r = document(&textes());
    // On part de la page 15 (le lien) : le premier résultat après elle est page 20.
    let res = chercher(&mut r, "spin", defaut(), 15);
    assert_eq!(compteur(&res, "spin"), "4 of 4");
    assert_eq!(res.page_cible, Some(20));
    assert_eq!(r.selection(), Some((19, 0)));
    // Un surlignage sélectionné sur la page 20.
    assert!(r.surlignages_page(19).iter().any(|s| s.selectionne));

    // Entrée : boucle → le premier.
    let res = naviguer(&mut r, "spin", defaut(), 1, 20);
    assert_eq!(compteur(&res, "spin"), "1 of 4");
    assert_eq!(res.page_cible, Some(3));
    assert_eq!(res.etat, EtatTrouve::Boucle);

    // Maj+Entrée : boucle arrière → le dernier.
    let res = naviguer(&mut r, "spin", defaut(), -1, 3);
    assert_eq!(compteur(&res, "spin"), "4 of 4");
    assert_eq!(res.etat, EtatTrouve::Boucle);
}

#[test]
fn aa_respecte_la_casse_whole_words_ne_garde_que_les_mots() {
    let mut r = document(&textes());
    let casse = OptionsRecherche {
        casse: true,
        ..defaut()
    };
    let res = r.chercher(
        etat(TypeRecherche::ChangementCasse, "spin", casse),
        20,
        &toujours,
    );
    // « spins » (page 3) et « spin » (page 20).
    assert_eq!(res.total, 2);

    let mots = OptionsRecherche {
        mot_entier: true,
        ..casse
    };
    let res = r.chercher(
        etat(TypeRecherche::ChangementMotEntier, "spin", mots),
        20,
        &toujours,
    );
    assert_eq!(compteur(&res, "spin"), "1 of 1");

    let res = chercher(&mut r, "introuvable", mots, 20);
    assert_eq!(compteur(&res, "introuvable"), "No results");
    assert_eq!(res.etat, EtatTrouve::NonTrouve);
    assert!(res.aucun_resultat("introuvable"));
}

#[test]
fn a_200_la_couche_de_texte_est_a_l_echelle_le_surlignage_couvre_le_bon_mot() {
    // La part pure du test : la plage rendue, ramenée dans le texte de la couche,
    // couvre « seul » et finit avec l'item.
    let mut r = document(&textes());
    let res = chercher(&mut r, "seul", defaut(), 15);
    assert_eq!(compteur(&res, "seul"), "1 of 1");
    let surlignage = r.surlignages_page(19);
    assert_eq!(surlignage.len(), 1);
    assert!(surlignage[0].selectionne);

    let bruts = ["un spin seul"];
    let conv = creer_convertisseur(&bruts, &bruts, str::to_string).expect("listes alignées");
    let plage = conv
        .convertir_plage(surlignage[0].index as i64, surlignage[0].longueur as i64)
        .expect("dans le texte");
    assert_eq!(plage, Plage { from: 8, to: 12 });
    assert_eq!(&bruts[0][plage.from..plage.to], "seul");
}

#[test]
fn echap_ferme_la_barre_et_efface_les_surlignages() {
    let mut r = document(&textes());
    let res = chercher(&mut r, "spin", defaut(), 15);
    assert!(res.texte("spin").ends_with("of 4"));
    assert!(!r.surlignages_page(19).is_empty());

    let res = r.fermer();
    assert!(!r.surligne());
    for page in 0..30 {
        assert!(r.surlignages_page(page).is_empty());
    }
    assert_eq!(res.etat, EtatTrouve::Trouve);
}

// ── Ajoutés : navigation ─────────────────────────────────────────────

#[test]
fn une_nouvelle_requete_part_de_la_page_affichee() {
    let mut r = document(&textes());
    // Page 3 : deux occurrences, on prend la première de la page.
    let res = chercher(&mut r, "spin", defaut(), 3);
    assert_eq!(compteur(&res, "spin"), "1 of 4");
    // Page 10 : sa propre occurrence.
    let res = chercher(&mut r, "spin", defaut(), 10);
    assert_eq!(compteur(&res, "spin"), "3 of 4");
}

#[test]
fn entree_passe_a_l_occurrence_suivante_de_la_page_puis_a_la_page_suivante() {
    let mut r = document(&textes());
    chercher(&mut r, "spin", defaut(), 3);
    let res = naviguer(&mut r, "spin", defaut(), 1, 3);
    assert_eq!(compteur(&res, "spin"), "2 of 4");
    assert_eq!(r.selection(), Some((2, 1)));
    let res = naviguer(&mut r, "spin", defaut(), 1, 3);
    assert_eq!(compteur(&res, "spin"), "3 of 4");
    assert_eq!(res.page_cible, Some(10));
    assert_eq!(res.etat, EtatTrouve::Trouve);
}

#[test]
fn maj_entree_revient_a_la_derniere_occurrence_de_la_page_precedente() {
    let mut r = document(&textes());
    chercher(&mut r, "spin", defaut(), 10);
    let res = naviguer(&mut r, "spin", defaut(), -1, 10);
    assert_eq!(r.selection(), Some((2, 1)));
    assert_eq!(compteur(&res, "spin"), "2 of 4");
    assert!(res.precedent);
}

#[test]
fn suivant_depuis_une_page_quittee_repart_de_la_page_affichee() {
    let mut r = document(&textes());
    chercher(&mut r, "spin", defaut(), 3);
    // Le lecteur a défilé à la page 15, la page 3 n'est plus à l'écran.
    let mut e = etat(TypeRecherche::Encore, "spin", defaut());
    e.vers_arriere = false;
    let res = r.chercher(e, 15, &|p| p != 3);
    assert_eq!(compteur(&res, "spin"), "4 of 4");
    assert_eq!(res.page_cible, Some(20));
}

#[test]
fn une_requete_vide_ne_trouve_rien_et_rien_n_est_ecrit() {
    let mut r = document(&textes());
    let res = chercher(&mut r, "", defaut(), 1);
    assert_eq!(res.etat, EtatTrouve::Trouve);
    assert_eq!((res.courant, res.total), (0, 0));
    assert_eq!(compteur(&res, ""), "");
    assert_eq!(compteur(&res, "   "), "");
}

#[test]
fn le_compteur_ecrit_trois_points_quand_aucun_total_n_est_connu() {
    assert_eq!(texte_compteur("a", EtatTrouve::Trouve, 0, 0), "…");
    assert_eq!(texte_compteur("a", EtatTrouve::Boucle, 2, 5), "2 of 5");
    assert_eq!(
        texte_compteur("a", EtatTrouve::NonTrouve, 0, 0),
        "No results"
    );
}

#[test]
fn tout_surligner_marque_toutes_les_occurrences_de_la_page() {
    let mut r = document(&textes());
    let tout = OptionsRecherche {
        tout_surligner: true,
        ..defaut()
    };
    chercher(&mut r, "spin", tout, 3);
    let s = r.surlignages_page(2);
    assert_eq!(s.len(), 2);
    assert_eq!(s.iter().filter(|x| x.selectionne).count(), 1);
    // Sans l'option : la seule courante.
    chercher(&mut r, "spin", defaut(), 3);
    assert_eq!(r.surlignages_page(2).len(), 1);
    assert_eq!(r.surlignages_page(9).len(), 0);
}

#[test]
fn changer_tout_surligner_ne_deplace_pas_la_selection() {
    let mut r = document(&textes());
    chercher(&mut r, "spin", defaut(), 3);
    let tout = OptionsRecherche {
        tout_surligner: true,
        ..defaut()
    };
    r.chercher(
        etat(TypeRecherche::ChangementToutSurligner, "spin", tout),
        3,
        &toujours,
    );
    assert_eq!(r.selection(), Some((2, 0)));
    assert_eq!(r.surlignages_page(2).len(), 2);
}

#[test]
fn un_document_sans_page_ne_trouve_rien() {
    let mut r = Recherche::new(Vec::new());
    let res = chercher(&mut r, "spin", defaut(), 1);
    assert_eq!(res.etat, EtatTrouve::NonTrouve);
}

// ── Ajoutés : normalisation du texte et correspondance ───────────────

/// Les occurrences de `requete` dans une page à un seul item : (index, longueur) bruts.
fn occurrences(brut: &str, requete: &str, options: OptionsRecherche) -> Vec<(usize, usize)> {
    let mut r = Recherche::new(vec![PageRecherche::depuis_texte_brut(brut)]);
    chercher(&mut r, requete, options, 1);
    r.occurrences_page(0).to_vec()
}

#[test]
fn une_ligature_se_cherche_en_lettres_et_ses_offsets_restent_bruts() {
    // « ﬁn » : un caractère + « n » = 2 unités brutes pour 3 lettres normalisées.
    assert_eq!(
        occurrences("la \u{FB01}n du volume", "fin", defaut()),
        vec![(3, 2)]
    );
    // Et la requête peut elle-même porter la ligature.
    assert_eq!(
        occurrences("la fin du volume", "\u{FB01}n", defaut()),
        vec![(3, 3)]
    );
}

#[test]
fn les_accents_sont_ignores_par_defaut() {
    let brut = "Résumé de café";
    assert_eq!(occurrences(brut, "resume", defaut()), vec![(0, 6)]);
    assert_eq!(occurrences(brut, "cafe", defaut()), vec![(10, 4)]);
}

#[test]
fn match_diacritics_distingue_les_accents() {
    let accents = OptionsRecherche {
        diacritiques: true,
        ..defaut()
    };
    let brut = "Résumé de cafe café";
    assert_eq!(occurrences(brut, "cafe", accents), vec![(10, 4)]);
    assert_eq!(occurrences(brut, "café", accents), vec![(15, 4)]);
    assert_eq!(occurrences(brut, "resume", accents), vec![]);
    assert_eq!(occurrences(brut, "résumé", accents), vec![(0, 6)]);
}

#[test]
fn les_blancs_de_la_requete_valent_un_ou_plusieurs_espaces() {
    assert_eq!(
        occurrences("moment    cinétique", "moment cin", defaut()),
        vec![(0, 13)]
    );
}

#[test]
fn un_saut_de_ligne_vaut_un_espace_pour_la_recherche() {
    let page = PageRecherche::depuis_items(&[
        ItemTexte {
            texte: "moment".to_string(),
            fin_de_ligne: true,
        },
        ItemTexte::new("cinétique"),
    ]);
    let mut r = Recherche::new(vec![page]);
    chercher(&mut r, "moment cin", defaut(), 1);
    // Offsets dans les chaînes des items mises bout à bout (sans le saut de ligne) :
    // « moment » (6) + « cin » (3).
    assert_eq!(r.occurrences_page(0), &[(0, 9)]);
}

#[test]
fn un_mot_coupe_en_fin_de_ligne_se_recolle() {
    let page = PageRecherche::depuis_items(&[
        ItemTexte {
            texte: "pro-".to_string(),
            fin_de_ligne: true,
        },
        ItemTexte::new("gramme"),
    ]);
    assert_eq!(page.contenu(), "programme");
    let mut r = Recherche::new(vec![page]);
    chercher(&mut r, "programme", defaut(), 1);
    assert_eq!(r.occurrences_page(0), &[(0, 10)]);
}

#[test]
fn les_guillemets_et_apostrophes_typographiques_se_replient() {
    assert_eq!(
        occurrences("l\u{2019}atome", "l'atome", defaut()),
        vec![(0, 7)]
    );
    assert_eq!(
        occurrences("le \u{201C}spin\u{201D}", "\"spin\"", defaut()),
        vec![(3, 6)]
    );
}

#[test]
fn la_ponctuation_de_la_requete_tolere_des_espaces() {
    assert_eq!(occurrences("a , b", "a,b", defaut()), vec![(0, 5)]);
    assert_eq!(occurrences("f(x)", "f(", defaut()), vec![(0, 2)]);
}

#[test]
fn les_metacaracteres_de_la_requete_sont_litteraux() {
    assert_eq!(occurrences("1+1=2", "1+1", defaut()), vec![(0, 3)]);
    assert_eq!(occurrences("a.b", "a.b", defaut()), vec![(0, 3)]);
    assert_eq!(occurrences("axb", "a.b", defaut()), vec![]);
    assert_eq!(occurrences("coût $5", "$5", defaut()), vec![(5, 2)]);
}

#[test]
fn les_mots_entiers_tiennent_compte_des_ruptures_de_mot() {
    let mots = OptionsRecherche {
        mot_entier: true,
        ..defaut()
    };
    assert_eq!(
        occurrences("spin spins spin-up", "spin", mots),
        vec![(0, 4), (11, 4)]
    );
    assert_eq!(occurrences("spin spins spin-up", "spin", defaut()).len(), 3);
    assert_eq!(occurrences("Spin", "spin", mots), vec![(0, 4)]);
}

#[test]
fn la_casse_est_ignoree_par_defaut() {
    let casse = OptionsRecherche {
        casse: true,
        ..defaut()
    };
    assert_eq!(occurrences("Spin spin SPIN", "spin", defaut()).len(), 3);
    assert_eq!(occurrences("Spin spin SPIN", "spin", casse), vec![(5, 4)]);
}

// ── Ajoutés : offsets hors plan de base ──────────────────────────────

#[test]
fn les_offsets_comptent_en_unites_utf16() {
    // Un émoji (2 unités) avant le mot : le décalage est de 3, pas de 2 caractères.
    assert_eq!(
        occurrences("\u{1F600} spin", "spin", defaut()),
        vec![(3, 4)]
    );
}
