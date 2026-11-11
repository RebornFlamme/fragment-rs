// Port de app/src/ui/views/markdown/pasteImage.ts (et du collage de MarkdownView.insertPastedImages)
//! Le collage d'image, sans DOM : quelles images viennent d'un presse-papiers, où et sous quel nom
//! les écrire dans le coffre, quel lien insérer, et la transaction qui l'insère.
//!
//! Les noms et le dossier suivent Obsidian (« Pasted image 20261004213512.png », réglage
//! « Default location for new attachments ») ; la partie pure vit dans `fragment_core::attachments`.

use fragment_core::resolve_link::{LinkFile, LinkVault, resolve_link};
use fragment_core::{
    AttachmentLocation, LocalDateTime, attachment_folder_for, extension_for_mime, pasted_image_name,
};
use fragment_text::{EditorRange, EditorTransactionSpec};

use crate::commandes::insertion::inserer_a_la_selection;

/// Un fichier du presse-papiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FichierPresse {
    pub mime: String,
    pub octets: Vec<u8>,
}

/// Un élément de `DataTransfer.items` : une capture d'écran arrive ici, pas dans `files`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementPresse {
    /// `kind === 'file'` (sinon `string`).
    pub est_fichier: bool,
    pub mime: String,
    /// `getAsFile()` : peut être absent.
    pub fichier: Option<FichierPresse>,
}

fn est_image(mime: &str) -> bool {
    mime.starts_with("image/")
}

/// Les images d'un presse-papiers. Deux sources, parce que Chromium ne les remplit pas pareil
/// selon l'origine : un fichier copié depuis l'explorateur de l'OS arrive dans `fichiers`, une
/// capture d'écran dans `elements`. `fichiers` l'emporte quand il contient une image.
#[must_use]
pub fn images_du_presse_papiers(
    fichiers: &[FichierPresse],
    elements: &[ElementPresse],
) -> Vec<FichierPresse> {
    let des_fichiers: Vec<FichierPresse> = fichiers
        .iter()
        .filter(|f| est_image(&f.mime))
        .cloned()
        .collect();
    if !des_fichiers.is_empty() {
        return des_fichiers;
    }
    elements
        .iter()
        .filter(|e| e.est_fichier && est_image(&e.mime))
        .filter_map(|e| e.fichier.clone())
        .collect()
}

/// Ce que le collage demande au coffre, en plus de sa lecture ([`LinkVault`]).
pub trait CoffreImages: LinkVault {
    type Erreur;
    /// Crée le dossier (et ses parents) s'il manque ; `""` = la racine.
    fn assurer_dossier(&mut self, dossier: &str) -> Result<(), Self::Erreur>;
    /// Un fichier existe-t-il déjà à ce chemin ?
    fn chemin_existe(&self, chemin: &str) -> bool;
    /// Écrit les octets et rend le fichier créé.
    fn creer_binaire(&mut self, chemin: &str, octets: &[u8]) -> Result<Self::File, Self::Erreur>;
}

/// Le premier chemin LIBRE de `dossier` (`""` = racine) pour `base.ext` : `base 1.ext`,
/// `base 2.ext`… si le nom est pris (l'anti-collision d'Obsidian). `ext` sans point, vide = aucune.
#[must_use]
pub fn chemin_disponible(
    dossier: &str,
    base: &str,
    ext: &str,
    existe: impl Fn(&str) -> bool,
) -> String {
    let prefixe = if dossier.is_empty() {
        String::new()
    } else {
        format!("{dossier}/")
    };
    let suffixe = if ext.is_empty() {
        String::new()
    } else {
        format!(".{ext}")
    };
    let mut chemin = format!("{prefixe}{base}{suffixe}");
    let mut n = 1;
    while existe(&chemin) {
        chemin = format!("{prefixe}{base} {n}{suffixe}");
        n += 1;
    }
    chemin
}

/// Le texte du lien vers `fichier` : son NOM s'il suffit à le retrouver (le cas normal : horodaté,
/// il est unique), sinon son chemin complet, pour qu'un homonyme plus court ailleurs dans le
/// coffre ne capte pas l'embed.
#[must_use]
pub fn texte_du_lien<V: LinkVault>(coffre: &V, fichier: &V::File, source: Option<&str>) -> String {
    match resolve_link(coffre, fichier.name(), source) {
        Some(trouve) if trouve.path() == fichier.path() => fichier.name().to_owned(),
        _ => fichier.path().to_owned(),
    }
}

/// Écrit les images collées dans le coffre — à l'emplacement du réglage, sous le nom horodaté —
/// et rend les embeds `![[…]]` à insérer, dans l'ordre. `horloge` donne l'heure LOCALE, relue à
/// chaque image (deux images dans la même seconde : `… 1.png`).
///
/// # Errors
/// La première erreur du coffre (dossier ou écriture) ; les images déjà écrites le restent.
pub fn enregistrer_images_collees<C: CoffreImages>(
    coffre: &mut C,
    images: &[FichierPresse],
    note: Option<&str>,
    emplacement: AttachmentLocation,
    dossier_reglage: &str,
    horloge: impl Fn() -> LocalDateTime,
) -> Result<Vec<String>, C::Erreur> {
    let dossier = attachment_folder_for(emplacement, dossier_reglage, note);
    coffre.assurer_dossier(&dossier)?;

    let mut embeds = Vec::with_capacity(images.len());
    for image in images {
        let ext = extension_for_mime(&image.mime);
        let nom = pasted_image_name(horloge(), ext);
        // Le nom sans son `.ext` : c'est la base de l'anti-collision.
        let base = &nom[..nom.len() - (ext.len() + 1)];
        let chemin = chemin_disponible(&dossier, base, ext, |p| coffre.chemin_existe(p));
        let fichier = coffre.creer_binaire(&chemin, &image.octets)?;
        embeds.push(format!("![[{}]]", texte_du_lien(coffre, &fichier, note)));
    }
    Ok(embeds)
}

/// La transaction qui insère les embeds à la sélection (relue APRÈS l'écriture asynchrone : là où
/// est le curseur maintenant). Un curseur sur la ligne vide sous un tableau garde sa ligne vide.
#[must_use]
pub fn transaction_de_collage(
    doc: &str,
    sel: EditorRange,
    embeds: &[String],
) -> EditorTransactionSpec {
    inserer_a_la_selection(doc, sel, &embeds.join("\n"))
}
