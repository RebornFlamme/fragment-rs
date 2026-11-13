// Port de app/src/ui/views/agent/AgentChatProvider.ts (`reduireImage`, `ranger`)
//
// Les PIÈCES JOINTES d'un tour avant qu'elles ne partent : une image trop grande est réduite,
// un fichier « autre » est posé dans le dossier des pièces de l'agent.
//
// ★ Écart assumé : le TS décode et réencode avec le navigateur (`createImageBitmap`,
//   `OffscreenCanvas`). Ici la DÉCISION est pure et testée (quand réduire, à quelle taille, en
//   quel format, avec quel repli) ; le pixel est confié à un [`CodecImage`] que l'hôte fournit
//   (crate `image` côté app). Sans codec, ou si le codec échoue, la pièce part telle quelle.

use std::path::Path;

use fragment_chat::api::ChatAttachment;

use crate::attachment_blocks::attachment_kind;
use crate::contrat::AttachmentKind;

/// Le plus grand côté d'une image envoyée (au-delà, l'API la réduit de toute façon).
pub const COTE_MAX: u32 = 1568;
/// La taille max d'une image en base64 (5 Mo), soit environ 3,7 Mo d'octets.
pub const OCTETS_MAX: usize = 3_700_000;

/// Le format d'une image réencodée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatImage {
    Png,
    Jpeg,
}

impl FormatImage {
    pub fn mime(self) -> &'static str {
        match self {
            FormatImage::Png => "image/png",
            FormatImage::Jpeg => "image/jpeg",
        }
    }
}

/// Le pixel : décoder une image pour en connaître la taille, la redessiner à une autre taille.
pub trait CodecImage {
    /// (largeur, hauteur), ou `None` si l'image est indécodable.
    fn dimensions(&self, data: &[u8], mime: &str) -> Option<(u32, u32)>;
    /// L'image redessinée en `largeur` × `hauteur` puis encodée ; `None` si l'encodage échoue.
    fn reencoder(
        &self,
        data: &[u8],
        mime: &str,
        largeur: u32,
        hauteur: u32,
        format: FormatImage,
        qualite: f32,
    ) -> Option<Vec<u8>>;
}

/// Une image trop grande (côté > 1568 px ou > 3,7 Mo) est RÉDUITE avant l'envoi ; une image d'un
/// format que le modèle ne lit pas (bmp…) est convertie si le codec sait la décoder. Le reste
/// (gif, svg, non-image, indécodable) passe tel quel.
pub fn reduire_image(a: &ChatAttachment, codec: Option<&dyn CodecImage>) -> ChatAttachment {
    if !a.mime.starts_with("image/") || a.mime == "image/gif" || a.mime == "image/svg+xml" {
        return a.clone();
    }
    let Some(codec) = codec else {
        return a.clone();
    };
    let native = attachment_kind(&a.name, &a.mime) == AttachmentKind::Image;
    let Some((l, h)) = codec.dimensions(&a.data, &a.mime) else {
        return a.clone();
    };
    let grand = f64::from(l.max(h));
    if grand == 0.0 {
        return a.clone();
    }
    let k = (f64::from(COTE_MAX) / grand).min(1.0);
    if native && k >= 1.0 && a.data.len() <= OCTETS_MAX {
        return a.clone();
    }
    let cote = |v: u32| (f64::from(v) * k).round() as u32;
    let (lw, lh) = (cote(l), cote(h));
    if lw == 0 || lh == 0 {
        return a.clone(); // un canevas vide : le TS retombe sur la pièce d'origine
    }
    let format = if a.mime == "image/jpeg" {
        FormatImage::Jpeg
    } else {
        FormatImage::Png
    };
    let Some(mut data) = codec.reencoder(&a.data, &a.mime, lw, lh, format, 0.9) else {
        return a.clone();
    };
    let mut sortie = format;
    if data.len() > OCTETS_MAX {
        // Encore trop lourde : un JPEG plus serré.
        match codec.reencoder(&a.data, &a.mime, lw, lh, FormatImage::Jpeg, 0.85) {
            Some(d) => {
                data = d;
                sortie = FormatImage::Jpeg;
            }
            None => return a.clone(),
        }
    }
    ChatAttachment {
        name: a.name.clone(),
        mime: sortie.mime().to_string(),
        data,
    }
}

/// Le nom d'un fichier rangé : l'horodatage ISO (`:` et `.` en `-`), puis le nom de la pièce
/// sans les caractères que Windows refuse (une suite devient un seul `_`).
pub fn nom_de_rangement(nom: &str, maintenant_ms: i64) -> String {
    let mut propre = String::new();
    let mut dans_suite = false;
    for c in nom.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            if !dans_suite {
                propre.push('_');
            }
            dans_suite = true;
        } else {
            propre.push(c);
            dans_suite = false;
        }
    }
    format!("{}-{propre}", horodatage(maintenant_ms))
}

/// `2026-11-13T10-20-30-123Z` : l'ISO 8601 de `toISOString()`, `:` et `.` remplacés par `-`.
pub fn horodatage(ms: i64) -> String {
    let jours = ms.div_euclid(86_400_000);
    let reste = ms.rem_euclid(86_400_000);
    // Jours depuis 1970 → date civile (algorithme de H. Hinnant).
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let jde = z.rem_euclid(146_097);
    let ae = (jde - jde / 1460 + jde / 36_524 - jde / 146_096) / 365;
    let a = ae + ere * 400;
    let jda = jde - (365 * ae + ae / 4 - ae / 100);
    let mp = (5 * jda + 2) / 153;
    let j = jda - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = if m <= 2 { a + 1 } else { a };
    format!(
        "{an:04}-{m:02}-{j:02}T{:02}-{:02}-{:02}-{:03}Z",
        reste / 3_600_000,
        reste / 60_000 % 60,
        reste / 1000 % 60,
        reste % 1000
    )
}

/// Un fichier joint « autre » : rangé dans le dossier des pièces de l'agent ; rend son chemin.
pub fn ranger(
    dossier: &Path,
    a: &ChatAttachment,
    maintenant_ms: i64,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dossier)?;
    let chemin = dossier.join(nom_de_rangement(&a.name, maintenant_ms));
    std::fs::write(&chemin, &a.data)?;
    Ok(chemin)
}

/// Décode du base64 standard (avec ou sans bourrage) ; `None` si un caractère n'en est pas.
pub fn decoder_base64(s: &str) -> Option<Vec<u8>> {
    let mut sortie = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            sortie.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachment_blocks::base64;
    use std::cell::RefCell;

    /// Un codec scripté : toutes les images font `dim`, le n-ième réencodage rend `octets[n]` octets.
    struct Codec {
        dim: Option<(u32, u32)>,
        octets: Vec<usize>,
        appels: RefCell<Vec<(u32, u32, FormatImage)>>,
    }

    impl CodecImage for Codec {
        fn dimensions(&self, _d: &[u8], _m: &str) -> Option<(u32, u32)> {
            self.dim
        }
        fn reencoder(
            &self,
            _d: &[u8],
            _m: &str,
            l: u32,
            h: u32,
            f: FormatImage,
            _q: f32,
        ) -> Option<Vec<u8>> {
            let n = self.appels.borrow().len();
            self.appels.borrow_mut().push((l, h, f));
            self.octets.get(n).map(|o| vec![7; *o])
        }
    }

    fn codec(dim: Option<(u32, u32)>, octets: &[usize]) -> Codec {
        Codec {
            dim,
            octets: octets.to_vec(),
            appels: RefCell::new(Vec::new()),
        }
    }

    fn image(nom: &str, mime: &str, n: usize) -> ChatAttachment {
        ChatAttachment {
            name: nom.into(),
            mime: mime.into(),
            data: vec![1; n],
        }
    }

    #[test]
    fn une_petite_image_native_passe_telle_quelle() {
        let c = codec(Some((800, 600)), &[]);
        let a = image("a.png", "image/png", 1000);
        assert_eq!(reduire_image(&a, Some(&c)), a);
        assert!(c.appels.borrow().is_empty());
    }

    #[test]
    fn une_image_de_plus_de_1568_px_est_reduite_au_prorata() {
        let c = codec(Some((3136, 1568)), &[500]);
        let r = reduire_image(&image("a.png", "image/png", 1000), Some(&c));
        assert_eq!(
            c.appels.borrow().as_slice(),
            &[(1568, 784, FormatImage::Png)]
        );
        assert_eq!(
            (r.mime.as_str(), r.data.len(), r.name.as_str()),
            ("image/png", 500, "a.png")
        );
    }

    #[test]
    fn une_image_de_plus_de_3_7_mo_est_reencodee_a_la_meme_taille() {
        let c = codec(Some((1000, 1000)), &[900]);
        let r = reduire_image(&image("a.jpg", "image/jpeg", OCTETS_MAX + 1), Some(&c));
        assert_eq!(
            c.appels.borrow().as_slice(),
            &[(1000, 1000, FormatImage::Jpeg)]
        );
        assert_eq!(r.mime, "image/jpeg");
    }

    #[test]
    fn un_png_encore_trop_lourd_retombe_sur_un_jpeg() {
        let c = codec(Some((2000, 2000)), &[OCTETS_MAX + 5, 800]);
        let r = reduire_image(&image("a.png", "image/png", 10), Some(&c));
        assert_eq!(
            c.appels.borrow().as_slice(),
            &[
                (1568, 1568, FormatImage::Png),
                (1568, 1568, FormatImage::Jpeg)
            ]
        );
        assert_eq!((r.mime.as_str(), r.data.len()), ("image/jpeg", 800));
    }

    #[test]
    fn un_format_non_natif_decodable_est_converti_en_png() {
        let c = codec(Some((10, 10)), &[40]);
        let r = reduire_image(&image("a.bmp", "image/bmp", 5000), Some(&c));
        assert_eq!((r.mime.as_str(), r.data.len()), ("image/png", 40));
    }

    #[test]
    fn gif_svg_non_image_indecodable_et_sans_codec_partent_tels_quels() {
        let c = codec(Some((9000, 9000)), &[1]);
        for a in [
            image("a.gif", "image/gif", 9),
            image("a.svg", "image/svg+xml", 9),
            image("a.pdf", "application/pdf", 9),
        ] {
            assert_eq!(reduire_image(&a, Some(&c)), a);
        }
        assert!(c.appels.borrow().is_empty());
        let inconnu = codec(None, &[1]);
        let a = image("a.png", "image/png", 9);
        assert_eq!(reduire_image(&a, Some(&inconnu)), a);
        assert_eq!(reduire_image(&a, None), a);
        // Un encodage qui échoue : la pièce d'origine.
        let muet = codec(Some((4000, 10)), &[]);
        assert_eq!(reduire_image(&a, Some(&muet)), a);
    }

    #[test]
    fn horodatage_iso_sans_deux_points_ni_point() {
        assert_eq!(horodatage(0), "1970-01-01T00-00-00-000Z");
        assert_eq!(horodatage(1_792_497_600_123), "2026-10-20T12-00-00-123Z");
        assert_eq!(horodatage(951_782_400_000), "2000-02-29T00-00-00-000Z");
    }

    #[test]
    fn le_nom_range_assainit_les_caracteres_interdits() {
        assert_eq!(
            nom_de_rangement("a:b\\\\c?.zip", 0),
            "1970-01-01T00-00-00-000Z-a_b_c_.zip"
        );
    }

    #[test]
    fn ranger_ecrit_le_fichier_dans_le_dossier() {
        let dossier =
            std::env::temp_dir().join(format!("fragment-agent-pieces-{}", std::process::id()));
        let a = image("archive.zip", "application/zip", 4);
        let chemin = ranger(&dossier, &a, 0).unwrap_or_default();
        assert!(
            chemin.starts_with(&dossier)
                && chemin.ends_with("1970-01-01T00-00-00-000Z-archive.zip")
        );
        assert_eq!(std::fs::read(&chemin).unwrap_or_default(), vec![1; 4]);
        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn base64_aller_retour() {
        for n in 0..9u8 {
            let d: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(5)).collect();
            assert_eq!(decoder_base64(&base64(&d)).unwrap_or_default(), d);
        }
        assert_eq!(decoder_base64("é"), None);
    }
}
