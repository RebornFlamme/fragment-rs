// Scène T2 « t2-gestionnaire-liste » : le gestionnaire avec deux coffres connus, dont un DISPARU (ligne
// barrée, « introuvable », non cliquable). Aucun coffre courant : le gestionnaire s'ouvre seul. Le JSON
// de cette scène est écrit par fr-t2-gestionnaire. ★ Les chemins sont FIXES (voir _t2.ts) : ils
// s'affichent dans la liste.
import { sceneT2, COFFRE, COFFRE_DISPARU } from './_t2';

sceneT2('t2-gestionnaire-liste', async ({ act, capturer, sonde, p }) => {
    await p.waitForSelector('.vault-manager-item', { timeout: 10_000 });
    await act.hover(2, 2);
    await act.waitStable();
    for (let i = 0; i < 2; i++) await sonde(`coffre ${i + 1}`, p, '.vault-manager-item', i);
    await capturer();
}, {
    fenetre: 'gestion',
    config: {
        vaultRoot: null,
        vaults: [
            { path: COFFRE, lastOpened: 1_700_000_100_000 },
            { path: COFFRE_DISPARU, lastOpened: 1_700_000_000_000 },
        ],
    },
});
