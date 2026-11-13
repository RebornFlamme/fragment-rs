//! Compile la ressource Windows (icône de l'exécutable et des fenêtres, `assets/identite.rc`).
//! Sans effet hors Windows.

fn main() {
    println!("cargo:rerun-if-changed=assets/identite.rc");
    println!("cargo:rerun-if-changed=assets/brand/app-icon.ico");
    #[cfg(windows)]
    {
        embed_resource::compile("assets/identite.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap_or_else(|e| panic!("ressource Windows (icône) : {e}"));
    }
}
