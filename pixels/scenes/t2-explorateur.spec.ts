// Scène T2 « t2-explorateur » : la racine du coffre vault-t2, dossiers fermés, tri (dossiers d'abord,
// puis noms à l'ordre naturel : « Chapitre 2 » avant « Chapitre 10 », casse ignorée).
// Port de app/e2e/explorer-menus.spec.ts (état de départ de l'arbre) ; décor dans _t2.ts.
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur', async ({ act, capturer, sonde, p }) => {
    for (let i = 0; i < 9; i++) await sonde(`ligne ${i + 1}`, p, '.tree-item-self[data-path]', i);
    await sonde('pied de dock', p, '.workspace-drawer-vault-switcher');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
