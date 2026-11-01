// Port de app/src/ui/core/Platform.ts
//
// `Platform` — la surface Obsidian que les plugins interrogent pour s'adapter à l'OS
// (A31). Fragment est desktop-only, donc les booléens « bureau » sont constants ; seul
// l'OS se déduit. ★ Là où le TS lit le `userAgent` (navigateur), on lit l'OS de COMPILATION
// (`cfg!`) : pas de navigateur dans fragment-rs (docs/contrats.md : « drapeaux OS »).

/// L'OS courant, façon Obsidian : des booléens plutôt qu'une énumération.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformFlags {
    /// Fragment ne tourne que sur bureau.
    pub is_desktop: bool,
    pub is_mobile: bool,
    pub is_desktop_app: bool,
    pub is_mobile_app: bool,
    pub is_mac_os: bool,
    pub is_win: bool,
    pub is_linux: bool,
    pub is_ios_app: bool,
    pub is_android_app: bool,
}

pub const PLATFORM: PlatformFlags = PlatformFlags {
    is_desktop: true,
    is_mobile: false,
    is_desktop_app: true,
    is_mobile_app: false,
    is_mac_os: cfg!(target_os = "macos"),
    is_win: cfg!(target_os = "windows"),
    // Android embarque « Linux » dans son UA ; en Rust target_os = "android" est distinct.
    is_linux: cfg!(target_os = "linux"),
    is_ios_app: false,
    is_android_app: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Ajouté : Platform.ts n'a pas de test TS ───
    #[test]
    fn bureau_seulement_et_un_seul_os() {
        const { assert!(PLATFORM.is_desktop && PLATFORM.is_desktop_app) };
        const { assert!(!PLATFORM.is_mobile && !PLATFORM.is_mobile_app) };
        let n = [PLATFORM.is_mac_os, PLATFORM.is_win, PLATFORM.is_linux]
            .iter()
            .filter(|b| **b)
            .count();
        assert!(n <= 1);
    }
}
