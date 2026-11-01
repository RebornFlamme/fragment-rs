// Port de app/src/ui/core/artefacts/ArtefactManager.ts
//! (en cours)

use crate::types::{Filter, Rec};

/// Le record correspond-il au filtre ?
pub fn matches(r: &Rec, f: Option<&Filter>) -> bool {
    let Some(f) = f else { return true };
    if f.id.as_ref().is_some_and(|id| &r.id != id) {
        return false;
    }
    if f.type_.is_some() && r.type_ != f.type_ {
        return false;
    }
    if let Some(s) = &f.scope {
        let Some(rs) = &r.scope else { return false };
        if s.kind.is_some_and(|k| rs.kind() != k) {
            return false;
        }
        if let Some(p) = &s.path
            && !matches!(rs, crate::types::ArtefactScope::File { path } if path == p)
        {
            return false;
        }
        if let Some(c) = &s.collection
            && !matches!(rs, crate::types::ArtefactScope::Plugin { collection } if collection == c)
        {
            return false;
        }
    }
    true
}

pub struct ArtefactManager;
pub struct ArtefactManagerOptions;
pub trait VaultFiles {}
