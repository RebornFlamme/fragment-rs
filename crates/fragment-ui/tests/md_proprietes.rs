//! Tests headless des Properties (`fr-t3-proprietes`). Les noms suivent les cas de la spec e2e
//! `properties.spec.ts` de Fragment ; les notes sont réécrites (ADR-007).
//!
//! Le contrôleur est pur : chaque geste rend une [`Edition`] que le banc applique au texte, comme
//! la vue l'applique à l'éditeur (une transaction).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::rc::Rc;

use fragment_core::{MetadataTypeManager, StockageMemoire};
use fragment_markdown::blocs::{AnalyseRepli, ContenuBloc, EtatRepli, calculer};
use fragment_markdown::frontmatter_model::{self as fm, PropertyType, PropertyValue};
use fragment_ui::md_proprietes::modele::{
    Action, Effet, Focus, Proprietes, Sortie, cible_de_depot, est_coche, est_un_glisser, lire_bloc,
    type_affiche,
};
use fragment_ui::md_proprietes::saisie;

const PROPS: &str = "---\ntags:\n  - 2023-2024\ntest:\n---\nCorps\n";

struct Banc {
    texte: String,
    p: Proprietes,
    types: MetadataTypeManager,
    disque: StockageMemoire,
    effets: Vec<Effet>,
}

impl Banc {
    fn new(texte: &str) -> Banc {
        let disque = StockageMemoire::new();
        let types = MetadataTypeManager::new(Rc::new(disque.clone()), Rc::new(|| 0));
        types.charger();
        Banc {
            texte: texte.to_owned(),
            p: Proprietes::new(false),
            types,
            disque,
            effets: Vec::new(),
        }
    }

    fn appliquer(&mut self, s: Sortie) {
        if let Some(e) = &s.edition {
            self.texte = e.appliquer(&self.texte);
        }
        self.effets = s.effets;
    }

    fn focus(&self) -> Option<Focus> {
        self.effets.iter().find_map(|e| match e {
            Effet::Focus(f) => Some(*f),
            _ => None,
        })
    }

    fn type_de(&self, cle: &str) -> PropertyType {
        let b = lire_bloc(&self.texte).unwrap();
        let p = b.props.iter().find(|p| p.key == cle).unwrap();
        type_affiche(&self.types, p)
    }
}

fn blocs_de(texte: &str) -> Vec<(ContenuBloc, usize, usize)> {
    use fragment_markdown::decorations::{Contexte, Curseur, Revelation};
    use fragment_markdown::syntaxe::{Etendue, analyser};
    let arbre = analyser(texte);
    let sel = [Etendue::new(texte.len(), texte.len())];
    let c = Curseur {
        selections: &sel,
        focus: true,
        a_eu_le_focus: true,
    };
    let ctx = Contexte {
        revelation: Revelation::ApresPremierFocus,
        ..Contexte::default()
    };
    calculer(texte, &arbre, &c, &ctx, false)
        .into_iter()
        .map(|b| (b.contenu, b.premiere_ligne, b.derniere_ligne))
        .collect()
}

#[test]
fn taper_tirets_en_tete_de_note_cree_les_properties_et_ouvre_une_propriete() {
    let mut b = Banc::new("--");
    let e = saisie::saisie("--", 2, "-").expect("le 3e tiret crée le bloc");
    assert_eq!(e.curseur, Some(7));
    b.texte = e.appliquer("--");
    assert_eq!(b.texte, "---\n---");
    assert!(lire_bloc(&b.texte).is_some_and(|x| x.props.is_empty()));
    let s = b.p.apres_creation_du_bloc();
    b.appliquer(s);
    assert_eq!(b.focus(), Some(Focus::CleAttente));
    b.p.saisir_cle_attente("test");
    let s = b.p.valider_cle_attente(&b.texte, &b.types, "test", true);
    b.appliquer(s);
    assert_eq!(b.texte, "---\ntest:\n---");
    // Entrée sur la clé : le focus passe à la valeur.
    assert_eq!(b.focus(), Some(Focus::Valeur(0)));
    let s = b.p.definir_valeur(&b.texte, 0, "bonjour".into());
    b.appliquer(s);
    assert_eq!(b.texte, "---\ntest: bonjour\n---");
    // Avec un corps sous la ligne : le curseur saute la ligne neuve.
    let e = saisie::saisie("--\nsuite", 2, "-").unwrap();
    assert_eq!(e.appliquer("--\nsuite"), "---\n---\nsuite");
    assert_eq!(e.curseur, Some(8));
}

#[test]
fn affiche_les_proprietes_pastilles_de_tags_empty_source_cachee() {
    let b = Banc::new(PROPS);
    let bloc = lire_bloc(&b.texte).unwrap();
    assert_eq!(bloc.props.len(), 2);
    assert_eq!(fm::as_list(&bloc.props[0].value), ["2023-2024"]);
    assert_eq!(type_affiche(&b.types, &bloc.props[0]), PropertyType::Tags);
    // « Empty » : une valeur nulle, que le champ montre en marque substituée.
    assert_eq!(bloc.props[1].value, PropertyValue::Null);
    // La source est remplacée par le widget : le bloc couvre les lignes 0 à 4.
    let blocs = blocs_de(&b.texte);
    assert!(
        blocs
            .iter()
            .any(|x| matches!(x.0, ContenuBloc::Proprietes { .. }) && x.1 == 0 && x.2 == 4),
        "{blocs:?}"
    );
}

#[test]
fn menu_property_type_number_ecrit_types_json() {
    let mut b = Banc::new(PROPS);
    let menu = b.p.menu_ligne(&b.texte, &b.types, 1);
    let sous = &menu[0].sous_menu;
    assert_eq!(menu[0].titre, "Property type");
    assert_eq!(sous[0].titre, "Automatic (Text)");
    assert!(sous[0].coche);
    let nombre = sous.iter().find(|e| e.titre == "Number").unwrap();
    let a = nombre.action.clone().unwrap();
    assert_eq!(a, Action::Type("test".into(), Some(PropertyType::Number)));
    let s = b.p.executer(&a, &b.texte, &b.types, "");
    b.appliquer(s);
    b.types.vider();
    let json: serde_json::Value =
        serde_json::from_str(&b.disque.contenu("types.json").unwrap()).unwrap();
    assert_eq!(json, serde_json::json!({ "types": { "test": "number" } }));
    assert_eq!(b.type_de("test"), PropertyType::Number);
    assert_eq!(b.texte, PROPS, "assigner un type ne touche pas à la note");
}

#[test]
fn clic_droit_remove_supprime_la_propriete() {
    let mut b = Banc::new(PROPS);
    let menu = b.p.menu_ligne(&b.texte, &b.types, 1);
    let titres: Vec<_> = menu.iter().map(|e| e.titre.as_str()).collect();
    assert_eq!(titres, ["Property type", "Cut", "Copy", "Paste", "Remove"]);
    let rm = menu.iter().find(|e| e.titre == "Remove").unwrap();
    assert!(rm.avertissement);
    let s =
        b.p.executer(rm.action.as_ref().unwrap(), &b.texte, &b.types, "");
    b.appliquer(s);
    assert_eq!(b.texte, "---\ntags:\n  - 2023-2024\n---\nCorps\n");
}

#[test]
fn glisser_l_icone_reordonne_les_proprietes() {
    let mut b = Banc::new(PROPS);
    assert!(!est_un_glisser(2.0, 1.0));
    assert!(est_un_glisser(0.0, -5.0));
    // Deux lignes de 30 px : on lâche près du haut de la première.
    let cible = cible_de_depot(&[0.0, 30.0], 60.0, 2.0);
    assert_eq!(cible, 0);
    let s = b.p.deplacer(&b.texte, 1, cible);
    b.appliquer(s);
    assert_eq!(b.texte, "---\ntest:\ntags:\n  - 2023-2024\n---\nCorps\n");
    // Déposer au même endroit ne change rien.
    assert!(b.p.deplacer(&b.texte, 0, 1).edition.is_none());
    assert_eq!(cible_de_depot(&[0.0, 30.0], 60.0, 58.0), 2);
}

#[test]
fn add_property_tags_et_case_a_cocher() {
    let mut b = Banc::new(PROPS);
    let s = b.p.ajouter(false);
    b.appliquer(s);
    let s = b.p.valider_cle_attente(&b.texte, &b.types, "auteur", true);
    b.appliquer(s);
    let s = b.p.definir_valeur(&b.texte, 2, "moi".into());
    b.appliquer(s);
    assert_eq!(
        b.texte,
        "---\ntags:\n  - 2023-2024\ntest:\nauteur: moi\n---\nCorps\n"
    );

    // Un tag ajouté (sans #), un autre retiré par sa croix.
    let s =
        b.p.ajouter_element(&b.texte, 0, PropertyType::Tags, "#neuf");
    b.appliquer(s);
    assert!(
        b.texte.contains("tags:\n  - 2023-2024\n  - neuf\n"),
        "{}",
        b.texte
    );
    let s = b.p.retirer_element(&b.texte, 0, 0);
    b.appliquer(s);
    assert!(b.texte.contains("tags:\n  - neuf\ntest:"), "{}", b.texte);
    // Un doublon n'est pas ajouté.
    let d = b.p.ajouter_element(&b.texte, 0, PropertyType::Tags, "neuf");
    assert!(d.edition.is_none());

    let mut c = Banc::new("---\nfait: false\n---\nTexte\n");
    let bloc = lire_bloc(&c.texte).unwrap();
    assert_eq!(
        type_affiche(&c.types, &bloc.props[0]),
        PropertyType::Checkbox
    );
    assert!(!est_coche(&bloc.props[0].value));
    let s = c.p.definir_valeur(&c.texte, 0, PropertyValue::Bool(true));
    c.appliquer(s);
    assert_eq!(c.texte, "---\nfait: true\n---\nTexte\n");
}

#[test]
fn le_curseur_de_l_hote_ne_tombe_jamais_dans_le_frontmatter_cache() {
    let texte = PROPS;
    let fin = texte.find("Corps").unwrap();
    // Un curseur dans la source cachée, ou sur son bord, est chassé sur la ligne d'après.
    assert_eq!(saisie::curseur_hors_du_bloc(texte, 0, 0), Some(fin));
    assert_eq!(saisie::curseur_hors_du_bloc(texte, 6, 6), Some(fin));
    assert_eq!(saisie::curseur_hors_du_bloc(texte, fin, fin), None);
    assert_eq!(saisie::curseur_hors_du_bloc(texte, fin + 2, fin + 2), None);
    // Une sélection large garde ses bornes.
    assert_eq!(saisie::curseur_hors_du_bloc(texte, 0, 5), None);
    // ↑ depuis la première ligne qui suit : on entre dans la dernière valeur.
    let mut p = Proprietes::new(false);
    assert!(saisie::entrer_par_le_haut(texte, fin, true));
    assert!(!saisie::entrer_par_le_haut(texte, fin, false));
    let s = p.focus_derniere(texte).unwrap();
    assert_eq!(s.effets, [Effet::Focus(Focus::Valeur(1))]);
    // Retour arrière en début de cette ligne : sélectionne le bloc.
    let avec_suite = "---\na: 1\n---\nsuite";
    assert_eq!(saisie::retour_arriere(avec_suite, 13), Some((0, 12)));
    // Un bloc qui termine le document : la frappe part sur une ligne neuve.
    let fini = "---\na: 1\n---";
    let e = saisie::frappe_apres_bloc_final(fini, fini.len(), Some("z")).unwrap();
    assert_eq!(e.appliquer(fini), "---\na: 1\n---\nz");
    let e = saisie::saisie(fini, fini.len(), "z").unwrap();
    assert_eq!(e.appliquer(fini), "---\na: 1\n---\nz");
}

#[test]
fn le_chevron_de_gouttiere_replie_et_deplie_les_properties() {
    let mut b = Banc::new(PROPS);
    // Replié : la source de la note n'est jamais touchée.
    let arbre = fragment_markdown::syntaxe::analyser(&b.texte);
    let analyse = AnalyseRepli::new(&b.texte, &arbre);
    let mut repli = EtatRepli::default();
    assert!(repli.basculer(&analyse, 0), "la ligne 0 est repliable");
    assert!(repli.proprietes_repliees);
    assert_eq!(b.texte, PROPS);
    assert!(repli.basculer(&analyse, 0));
    assert!(!repli.proprietes_repliees);
    // Ajouter dans un bloc replié le déplie d'abord.
    let s = b.p.ajouter(true);
    assert_eq!(s.effets, [Effet::Deplier, Effet::Focus(Focus::CleAttente)]);
}

#[test]
fn nouvelle_propriete_cliquer_son_icone_ne_la_fait_pas_disparaitre() {
    let mut b = Banc::new("---\n---");
    let s = b.p.ajouter(false);
    b.appliquer(s);
    // Clé vide : le menu de type s'ouvre, la ligne reste.
    let menu = b.p.menu_attente();
    let nombre = menu[0]
        .sous_menu
        .iter()
        .find(|e| e.titre == "Number")
        .unwrap();
    let s =
        b.p.executer(nombre.action.as_ref().unwrap(), &b.texte, &b.types, "");
    b.appliquer(s);
    assert!(b.p.attente().is_some());
    assert_eq!(b.p.type_de_l_attente(), PropertyType::Number);
    assert_eq!(b.focus(), Some(Focus::CleAttente));
    assert_eq!(b.texte, "---\n---");
    // La clé validée reçoit le type choisi d'avance.
    let s = b.p.valider_cle_attente(&b.texte, &b.types, "note", true);
    b.appliquer(s);
    assert_eq!(b.texte, "---\nnote:\n---");
    assert_eq!(b.type_de("note"), PropertyType::Number);
    // Une clé tapée : le clic sur l'icône la valide, puis le menu s'ouvre sur la nouvelle ligne.
    let s = b.p.ajouter(false);
    b.appliquer(s);
    b.p.saisir_cle_attente("statut");
    let (s, ouvrir) = b.p.icone_de_l_attente(&b.texte, &b.types, "statut");
    b.appliquer(s);
    assert_eq!(b.texte, "---\nnote:\nstatut:\n---");
    assert_eq!(ouvrir, Some(1));
    let menu = b.p.menu_ligne(&b.texte, &b.types, 1);
    let case = menu[0]
        .sous_menu
        .iter()
        .find(|e| e.titre == "Checkbox")
        .unwrap();
    let s =
        b.p.executer(case.action.as_ref().unwrap(), &b.texte, &b.types, "");
    b.appliquer(s);
    assert_eq!(b.type_de("statut"), PropertyType::Checkbox);
}

#[test]
fn clic_droit_sur_l_en_tete_clear_properties_supprime_tout_le_bloc() {
    let mut b = Banc::new(PROPS);
    let menu = b.p.menu_bloc();
    assert_eq!(menu[0].titre, "Clear properties");
    assert!(menu[0].avertissement);
    let s =
        b.p.executer(menu[0].action.as_ref().unwrap(), &b.texte, &b.types, "");
    // UNE édition : annulable d'un coup (la vue en fait une transaction isolée).
    assert!(s.edition.is_some());
    b.appliquer(s);
    assert_eq!(b.texte, "Corps\n");
    assert!(lire_bloc(&b.texte).is_none());
}

#[test]
fn plus_aucune_propriete_le_bloc_frontmatter_est_supprime() {
    let mut b = Banc::new("---\nfait: false\n---\nTexte\n");
    let menu = b.p.menu_ligne(&b.texte, &b.types, 0);
    let rm = menu.iter().find(|e| e.titre == "Remove").unwrap();
    let s =
        b.p.executer(rm.action.as_ref().unwrap(), &b.texte, &b.types, "");
    assert_eq!(s.edition.as_ref().unwrap().curseur, Some(0));
    b.appliquer(s);
    assert_eq!(b.texte, "Texte\n");

    // `---` puis Échap sur la clé : le bloc créé disparaît.
    let mut v = Banc::new("---\n---");
    let s = v.p.ajouter(false);
    v.appliquer(s);
    let s = v.p.abandonner_attente(&v.texte);
    v.appliquer(s);
    assert_eq!(v.texte, "");
}

#[test]
fn cles_en_double_et_collage() {
    let mut b = Banc::new(PROPS);
    let s = b.p.ajouter(false);
    b.appliquer(s);
    let s = b.p.valider_cle_attente(&b.texte, &b.types, "test", false);
    assert!(s.edition.is_none());
    assert!(matches!(s.effets[0], Effet::Notice(_)));
    let y = Proprietes::copier(&b.texte, 0).unwrap();
    assert_eq!(y, "tags:\n  - 2023-2024\n");
    let s = b.p.coller(&b.texte, 1, "auteur: moi\n");
    b.appliquer(s);
    assert_eq!(
        b.texte,
        "---\ntags:\n  - 2023-2024\ntest:\nauteur: moi\n---\nCorps\n"
    );
    let vide = b.p.coller(&b.texte, 0, "pas du yaml: [");
    assert!(matches!(vide.effets[0], Effet::Notice(_)));
}
