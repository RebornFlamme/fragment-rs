// Port de app/src/ui/views/pdf/pdfOutline.ts
//
// La table des matières d'un PDF (son « outline »), résolue en numéros de page.
//
// Côté TS, pdf.js rend l'outline brut et `resoudreOutline` / `resoudreDestination`
// (async, sur `PDFDocumentProxy`) le résolvent. Ici, SEUL le modèle pur est porté :
// l'arbre résolu, le `y` d'une destination explicite et la révélation d'une page.
// La résolution des destinations nommées viendra avec le moteur (pdfium, phase 3).

/// Une entrée de la table des matières, déjà résolue.
#[derive(Debug, Clone, PartialEq)]
pub struct EntreeToc {
    pub titre: String,
    /// 1-based ; `None` si la destination est introuvable ou externe (URL).
    pub page: Option<u32>,
    /// Le haut visé, en coordonnées PDF (origine en BAS de page) ; `None` = haut de page.
    pub y: Option<f64>,
    pub enfants: Vec<EntreeToc>,
}

/// Un élément d'une destination explicite `[refPage, /XYZ, x, y, zoom]`.
///
/// ★ Le TS lit un tableau hétérogène (`unknown[]`) ; on le type à la main, avec
///   seulement les cas que `hautDeDestination` distingue.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementDest {
    /// `null` du tableau PDF.
    Nul,
    Nombre(f64),
    /// Un nom PDF (`/XYZ` → `Nom("XYZ")`).
    Nom(String),
    /// Tout le reste (référence de page, objet…).
    Autre,
}

/// Le `y` d'une destination explicite, quand son type en porte un.
pub fn haut_de_destination(dest: &[ElementDest]) -> Option<f64> {
    let type_ = match dest.get(1) {
        Some(ElementDest::Nom(n)) => Some(n.as_str()),
        _ => None,
    };
    let y = match type_ {
        Some("XYZ") => dest.get(3),
        Some("FitH" | "FitBH") => dest.get(2),
        _ => None,
    };
    match y {
        Some(ElementDest::Nombre(v)) => Some(*v),
        _ => None,
    }
}

/// L'entrée à révéler pour une page — la DERNIÈRE, dans l'ordre de lecture,
/// qui commence au plus tard à cette page — avec le chemin de ses ancêtres (à
/// déplier). `None` si aucune entrée ne précède la page.
pub fn entree_pour_page(entrees: &[EntreeToc], page: u32) -> Option<Vec<&EntreeToc>> {
    fn parcourir<'a>(
        liste: &'a [EntreeToc],
        chemin: &[&'a EntreeToc],
        page: u32,
        meilleur: &mut Option<Vec<&'a EntreeToc>>,
    ) {
        for entree in liste {
            let mut ici = chemin.to_vec();
            ici.push(entree);
            if entree.page.is_some_and(|p| p <= page) {
                *meilleur = Some(ici.clone());
            }
            parcourir(&entree.enfants, &ici, page, meilleur);
        }
    }
    let mut meilleur = None;
    parcourir(entrees, &[], page, &mut meilleur);
    meilleur
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(titre: &str, page: Option<u32>, enfants: Vec<EntreeToc>) -> EntreeToc {
        EntreeToc {
            titre: titre.into(),
            page,
            y: None,
            enfants,
        }
    }

    fn toc() -> Vec<EntreeToc> {
        vec![
            e("Intro", Some(1), vec![]),
            e(
                "Chapitre 1",
                Some(3),
                vec![e("1.1", Some(4), vec![]), e("1.2", Some(7), vec![])],
            ),
            e(
                "Chapitre 2",
                Some(10),
                vec![e("Lien externe", None, vec![])],
            ),
        ]
    }

    fn titres(r: Option<Vec<&EntreeToc>>) -> Vec<String> {
        r.map(|v| v.iter().map(|x| x.titre.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn entree_pour_page_rend_la_derniere_entree_commencee_avec_ses_ancetres() {
        let toc = toc();
        assert_eq!(titres(entree_pour_page(&toc, 5)), ["Chapitre 1", "1.1"]);
        assert_eq!(titres(entree_pour_page(&toc, 9)), ["Chapitre 1", "1.2"]);
        assert_eq!(titres(entree_pour_page(&toc, 3)), ["Chapitre 1"]);
        assert_eq!(titres(entree_pour_page(&toc, 50)), ["Chapitre 2"]);
    }

    #[test]
    fn entree_pour_page_rien_avant_la_premiere_entree() {
        assert!(entree_pour_page(&[e("Tard", Some(4), vec![])], 2).is_none());
        assert!(entree_pour_page(&[], 1).is_none());
    }

    #[test]
    fn haut_de_destination_lit_le_y_des_destinations_qui_en_portent_un() {
        use ElementDest::{Autre, Nom, Nombre, Nul};
        let n = |s: &str| Nom(s.into());
        assert_eq!(
            haut_de_destination(&[Autre, n("XYZ"), Nombre(0.0), Nombre(700.0), Nul]),
            Some(700.0)
        );
        assert_eq!(
            haut_de_destination(&[Autre, n("FitH"), Nombre(500.0)]),
            Some(500.0)
        );
        assert_eq!(haut_de_destination(&[Autre, n("Fit")]), None);
        assert_eq!(
            haut_de_destination(&[Autre, n("XYZ"), Nombre(0.0), Nul, Nul]),
            None
        );
    }
}
