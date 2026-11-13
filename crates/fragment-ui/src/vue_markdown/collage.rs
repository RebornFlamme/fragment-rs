// Port de app/src/ui/views/markdown/pasteImage.ts et du collage de MarkdownView.ts (onPaste,
// insertPastedImages) : les images du presse-papiers écrites dans le coffre, puis leurs embeds
// insérés à la sélection.
//! Le collage dans une note. Le texte, c'est l'éditeur qui s'en charge ; ce module ne prend que
//! les IMAGES, et refuse tout en lecture seule.

use fragment_core::resolve_link::LinkVault;
use fragment_core::{
    AttachmentLocation, DataWriteOptions, LocalDateTime, TFile, Vault, VaultError,
};
use fragment_markdown::coller_image::{
    CoffreImages, ElementPresse, FichierPresse, enregistrer_images_collees,
    images_du_presse_papiers, transaction_de_collage,
};
use fragment_text::{Editor, TextSurface as _};

use super::vue::{Liens, VueMarkdown};

/// Ce que le collage a fait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueCollage {
    /// Pas d'image : le collage de texte suit son cours normal (l'éditeur).
    PasDImage,
    /// Lecture seule : rien ne se modifie.
    LectureSeule,
    /// Les embeds insérés, dans l'ordre.
    Insere(Vec<String>),
    /// Le coffre a refusé (« Impossible d'enregistrer l'image collée. »).
    Echec(String),
}

/// Le message de la notice d'échec.
pub const NOTICE_ECHEC_COLLAGE: &str = "Impossible d’enregistrer l’image collée.";

/// Le coffre vu par le collage d'image.
struct CoffreDeLaVue<'a>(&'a Vault);

impl LinkVault for CoffreDeLaVue<'_> {
    type File = TFile;
    fn get_file_by_path(&self, path: &str) -> Option<TFile> {
        self.0.get_file_by_path(path)
    }
    fn get_files(&self) -> Vec<TFile> {
        self.0.get_files()
    }
}

impl CoffreImages for CoffreDeLaVue<'_> {
    type Erreur = VaultError;
    fn assurer_dossier(&mut self, dossier: &str) -> Result<(), VaultError> {
        self.0.ensure_folder(dossier).map(|_| ())
    }
    fn chemin_existe(&self, chemin: &str) -> bool {
        self.0.get_abstract_file_by_path(chemin).is_some()
    }
    fn creer_binaire(&mut self, chemin: &str, octets: &[u8]) -> Result<TFile, VaultError> {
        self.0
            .create_binary(chemin, octets, DataWriteOptions::default())
    }
}

/// Le réglage « Default location for new attachments ».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReglageAttachements {
    pub emplacement: AttachmentLocation,
    pub dossier: String,
}

/// Colle le contenu d'un presse-papiers dans la note. La sélection est relue APRÈS l'écriture :
/// les embeds vont là où est le curseur maintenant.
pub fn coller(
    vue: &VueMarkdown,
    fichiers: &[FichierPresse],
    elements: &[ElementPresse],
    reglage: &ReglageAttachements,
    horloge: impl Fn() -> LocalDateTime,
) -> IssueCollage {
    coller_dans(vue.liens(), fichiers, elements, reglage, horloge)
}

/// [`coller`], depuis les poignées que le rendu possède.
pub fn coller_dans(
    liens: &Liens,
    fichiers: &[FichierPresse],
    elements: &[ElementPresse],
    reglage: &ReglageAttachements,
    horloge: impl Fn() -> LocalDateTime,
) -> IssueCollage {
    if liens.lecture_seule() {
        return IssueCollage::LectureSeule;
    }
    let images = images_du_presse_papiers(fichiers, elements);
    if images.is_empty() {
        return IssueCollage::PasDImage;
    }
    let note = liens.chemin();
    let mut coffre = CoffreDeLaVue(liens.vault());
    let embeds = match enregistrer_images_collees(
        &mut coffre,
        &images,
        note.as_deref(),
        reglage.emplacement,
        &reglage.dossier,
        horloge,
    ) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("fragment : markdown : collage d’image impossible : {e}");
            return IssueCollage::Echec(NOTICE_ECHEC_COLLAGE.to_owned());
        }
    };
    let editeur = liens.editeur_interne();
    let spec = transaction_de_collage(&editeur.get_value(), editeur.get_selection(), &embeds);
    editeur.transaction(spec);
    IssueCollage::Insere(embeds)
}
