// Port de app/src/ui/views/pdf/PdfEditor.ts (MODÈLE seulement : l'espace de positions
// et l'indexation du texte ; la surface `DocumentSurface` elle-même viendra avec
// la vue, phase 3 — elle exige un vrai document).
//
// ── L'espace de positions ────────────────────────────────────────────────
//
//  Une « position » sur un PDF est une PAGE, plus un index de caractère dans son
//  texte. On réserve un bloc d'offsets FIXE par page :
//
//      pos = (page - 1) * PAGE_STRIDE + 1 + indexDuCaractere
//      pos = (page - 1) * PAGE_STRIDE                        ← la page elle-même
//
//  ★ POURQUOI un pas fixe plutôt qu'un cumul des longueurs de texte : un cumul
//    exigerait d'extraire le texte de TOUTES les pages avant de pouvoir nommer
//    la moindre position — donc de parser un document de 800 pages à l'ouverture,
//    pour un lecteur qui n'en verra que trois. Avec un pas fixe, l'offset d'une
//    page se calcule sans rien connaître de son contenu, et le texte peut être
//    extrait paresseusement, page par page. L'espace est creux : seules les
//    comparaisons comptent.
//
//  L'offset « nu » de la page est l'ancre de repli : c'est lui qu'on rend quand
//  il n'y a aucun texte sous le curseur — page scannée, marge, blanc entre deux
//  paragraphes. Un trait posé là s'ancre à la page.
//
//  ★ Les offsets sont des `i64` (le TS : `number` entier) et les longueurs de
//    texte des unités UTF-16, comme `string.length` : ce sont les offsets des
//    ancres d'annotations déjà persistées.

/// Le nombre d'offsets réservés par page. Large devant tout texte de page réel
/// (un million de caractères ≈ 300 pages de roman sur UNE page), et assez petit
/// pour qu'un PDF de 9000 pages tienne loin du plus grand entier sûr.
pub const PAGE_STRIDE: i64 = 1_000_000;

/// La page (1-indexée) qui contient cet offset.
///
/// ★ Division euclidienne = `Math.floor(pos / PAGE_STRIDE)` du TS, y compris pour
///   un offset négatif.
pub fn page_de_pos(pos: i64) -> i64 {
    pos.div_euclid(PAGE_STRIDE) + 1
}

/// L'offset qui désigne la page elle-même (son coin haut-gauche).
pub fn pos_de_page(page: i64) -> i64 {
    (page - 1) * PAGE_STRIDE
}

/// L'index du caractère visé dans le texte de sa page, ou -1 pour l'ancre de page.
///
/// ★ Le `%` de JavaScript garde le signe du dividende ; pour `pos >= 0` (le seul cas
///   réel) il égale `rem_euclid`. On prend `rem_euclid` pour ne jamais rendre < -1.
pub fn index_de_pos(pos: i64) -> i64 {
    pos.rem_euclid(PAGE_STRIDE) - 1
}

/// Un fragment de texte et sa place dans l'espace d'offsets de la page.
///
/// `T` est ce qui porte le fragment à l'écran (le `div` du TS : il donne la
/// géométrie). Le modèle n'en lit rien, il le range.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfTextItem<T> {
    /// Index du premier caractère dans le texte de la page (unités UTF-16).
    pub from: usize,
    /// Longueur en unités UTF-16.
    pub len: usize,
    pub div: T,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageText<T> {
    /// Le texte de la page, tous fragments concaténés.
    pub text: String,
    pub items: Vec<PdfTextItem<T>>,
    /// L'échelle à laquelle la couche de texte a été BÂTIE (périmée pendant un
    /// pinch : c'est ce qui permet de ramener ses mesures en unités non zoomées).
    pub echelle: Option<f64>,
}

/// Indexe les deux tableaux parallèles que rend la couche de texte.
///
/// `divs[i] == None` : un fragment SANS élément (un `<br>` de mise en page).
///
/// ★ Pourquoi une fonction pure : c'est la seule pièce de la couche de texte qui
///   se vérifie sans navigateur ni PDF.
pub fn indexer_texte<T: Clone>(divs: &[Option<T>], strs: &[&str]) -> PdfPageText<T> {
    let mut items = Vec::new();
    let mut text = String::new();
    let mut longueur = 0usize;
    for (i, s) in strs.iter().enumerate() {
        let len = s.encode_utf16().count();
        // Un item sans span n'a pas de géométrie : on le saute, mais son texte
        // compte quand même dans les offsets, sinon la sélection et le
        // surlignage se décaleraient d'autant.
        if let Some(Some(div)) = divs.get(i) {
            items.push(PdfTextItem {
                from: longueur,
                len,
                div: div.clone(),
            });
        }
        text.push_str(s);
        longueur += len;
    }
    PdfPageText {
        text,
        items,
        echelle: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── espace d'offsets page-majeur ────────────────────────────────────────

    #[test]
    fn la_premiere_page_commence_a_zero() {
        assert_eq!(pos_de_page(1), 0);
        assert_eq!(page_de_pos(0), 1);
    }

    #[test]
    fn fait_correspondre_page_et_offset_dans_les_deux_sens() {
        for page in [1, 2, 7, 400] {
            assert_eq!(page_de_pos(pos_de_page(page)), page);
        }
    }

    #[test]
    fn distingue_l_ancre_de_page_du_premier_caractere() {
        // L'offset nu désigne la PAGE ; -1 est le code de « pas un caractère ».
        assert_eq!(index_de_pos(pos_de_page(3)), -1);
        // Le premier caractère de la page 3 est juste après.
        assert_eq!(index_de_pos(pos_de_page(3) + 1), 0);
        assert_eq!(page_de_pos(pos_de_page(3) + 1), 3);
    }

    #[test]
    fn garde_les_caracteres_dans_la_page_de_leur_bloc() {
        let dernier = pos_de_page(2) + PAGE_STRIDE - 1;
        assert_eq!(page_de_pos(dernier), 2);
        assert_eq!(page_de_pos(dernier + 1), 3);
    }

    #[test]
    fn ordonne_les_pages_comparer_des_offsets_c_est_comparer_des_pages() {
        // C'est ce qui permet à posVisibility de raisonner sur des plages sans
        // rien savoir des pages.
        assert!(pos_de_page(2) > pos_de_page(1) + PAGE_STRIDE - 1);
    }

    // ── indexer_texte ───────────────────────────────────────────────────────

    fn froms_lens<T>(t: &PdfPageText<T>) -> Vec<(usize, usize)> {
        t.items.iter().map(|i| (i.from, i.len)).collect()
    }

    #[test]
    fn indexer_texte_concatene_les_fragments_et_les_indexe_dans_l_ordre() {
        let t = indexer_texte(
            &[Some(()), Some(()), Some(())],
            &["Bonjour ", "le ", "monde"],
        );
        assert_eq!(t.text, "Bonjour le monde");
        assert_eq!(froms_lens(&t), [(0, 8), (8, 3), (11, 5)]);
    }

    #[test]
    fn indexer_texte_gere_un_document_sans_aucun_texte_un_scan() {
        let t = indexer_texte::<()>(&[], &[]);
        assert_eq!(t.text, "");
        assert!(t.items.is_empty());
    }

    #[test]
    fn indexer_texte_compte_le_texte_des_fragments_sans_span_sans_les_indexer() {
        // Les entrées de mise en page sans élément : les sauter dans les items est
        // juste, mais oublier leur texte décalerait tous les offsets suivants.
        let t = indexer_texte(&[Some(()), None, Some(())], &["abc", "\n", "def"]);
        assert_eq!(t.text, "abc\ndef");
        assert_eq!(t.items.iter().map(|i| i.from).collect::<Vec<_>>(), [0, 4]);
    }

    #[test]
    fn indexer_texte_tolere_un_fragment_vide() {
        let t = indexer_texte(&[Some(()), Some(())], &["", "x"]);
        assert_eq!(t.text, "x");
        assert_eq!(froms_lens(&t), [(0, 0), (0, 1)]);
    }
}
