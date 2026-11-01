// Port de app/src/ui/core/workspace/breadcrumb.ts
//
// Le fil d'Ariane du view-header — la partie PURE, sans DOM ni Vault : un chemin entre,
// des segments sortent. FileView ne fait que les dessiner.
//
// ★ Profondeur bornée : au-delà de `max_depth` dossiers, on garde le PREMIER (l'ancrage
//   dans le coffre) et les `max_depth - 1` DERNIERS (le contexte immédiat) ; tout ce qui
//   est au milieu est replié dans UN seul segment `…`, qui garde la liste des dossiers
//   masqués (pour un menu au clic).

pub const BREADCRUMB_MAX_DEPTH: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbFolder {
    /// Chemin du dossier depuis la racine du coffre (sans slash final).
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreadcrumbSegment {
    Folder(BreadcrumbFolder),
    Ellipsis { hidden: Vec<BreadcrumbFolder> },
}

/// Les dossiers ANCÊTRES d'un fichier, de la racine vers le fichier (racine exclue).
pub fn ancestor_folders(file_path: &str) -> Vec<BreadcrumbFolder> {
    let mut parts: Vec<&str> = file_path.split('/').collect();
    parts.pop(); // le fichier lui-même
    (0..parts.len())
        .map(|i| BreadcrumbFolder {
            name: parts[i].to_owned(),
            path: parts[..=i].join("/"),
        })
        .collect()
}

pub fn breadcrumb_segments(file_path: &str, max_depth: usize) -> Vec<BreadcrumbSegment> {
    let folders = ancestor_folders(file_path);
    let as_segments = |fs: &[BreadcrumbFolder]| -> Vec<BreadcrumbSegment> {
        fs.iter().cloned().map(BreadcrumbSegment::Folder).collect()
    };

    // max_depth < 2 n'a pas de forme « premier … derniers » : on borne à 2.
    let max = max_depth.max(2);
    if folders.len() <= max {
        return as_segments(&folders);
    }

    let tail = max - 1;
    let mut out = as_segments(&folders[..1]);
    out.push(BreadcrumbSegment::Ellipsis {
        hidden: folders[1..folders.len() - tail].to_vec(),
    });
    out.extend(as_segments(&folders[folders.len() - tail..]));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier(name: &str, path: &str) -> BreadcrumbFolder {
        BreadcrumbFolder {
            name: name.into(),
            path: path.into(),
        }
    }

    /// Rend les segments en texte (`a / … / c`) — lisible dans les assertions.
    fn rendu(path: &str, max: usize) -> String {
        breadcrumb_segments(path, max)
            .iter()
            .map(|s| match s {
                BreadcrumbSegment::Ellipsis { .. } => "…".to_owned(),
                BreadcrumbSegment::Folder(f) => f.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(" / ")
    }

    #[test]
    fn donne_les_chemins_cumules_racine_exclue() {
        assert_eq!(
            ancestor_folders("a/b/note.md"),
            vec![dossier("a", "a"), dossier("b", "a/b")]
        );
    }

    #[test]
    fn fichier_a_la_racine_aucun_dossier() {
        assert_eq!(ancestor_folders("note.md"), vec![]);
    }

    #[test]
    fn sous_la_profondeur_max_tous_les_dossiers() {
        assert_eq!(rendu("a/b/c/note.md", 3), "a / b / c");
    }

    #[test]
    fn au_dela_premier_puis_les_derniers() {
        assert_eq!(rendu("a/b/c/d/e/note.md", 3), "a / … / d / e");
    }

    #[test]
    fn le_points_garde_la_liste_des_dossiers_masques_dans_lordre() {
        let seg = &breadcrumb_segments("a/b/c/d/e/note.md", 3)[1];
        assert_eq!(
            seg,
            &BreadcrumbSegment::Ellipsis {
                hidden: vec![dossier("b", "a/b"), dossier("c", "a/b/c")],
            }
        );
    }

    #[test]
    fn une_profondeur_max_inferieure_a_2_est_bornee_a_2() {
        assert_eq!(rendu("a/b/c/note.md", 1), "a / … / c");
    }

    #[test]
    fn fichier_a_la_racine_aucun_segment() {
        assert_eq!(breadcrumb_segments("note.md", BREADCRUMB_MAX_DEPTH), vec![]);
    }
}
