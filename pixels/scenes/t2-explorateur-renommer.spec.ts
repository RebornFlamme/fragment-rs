// Scène T2 « t2-explorateur-renommer » : le champ de renommage en place sur « Journal.md ».
// Côté Fragment : menu contextuel de la ligne, puis « Renommer… » (comme explorer-menus.spec.ts) ;
// côté candidat : la commande pixels:explorer-rename {path:"Journal.md"} (contrat de T2.md §5).
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur-renommer', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 273, { button: 'right' });
    await p.locator('.menu .menu-item-title', { hasText: 'Renommer' }).click();
    await p.waitForSelector('.nav-rename-input', { timeout: 5_000 });
    await sonde('champ', p, '.nav-rename-input');
    await act.wait(200);
    await capturer();
});
