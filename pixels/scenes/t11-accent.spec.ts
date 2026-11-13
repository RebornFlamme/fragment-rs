// Scène T11 « t11-accent » : un accent choisi (turquoise) PAR-DESSUS « Rose ». Le champ de renommage de
// « Journal.md » montre ses dérivés (sélection de texte en accent à 35 %). Côté candidat : la commande
// pixels:explorer-rename {path:"Journal.md"}, comme t2-explorateur-renommer.
import { sceneT11 } from './_t11';

sceneT11('t11-accent', async ({ act, capturer, sonde, choisirTheme, choisirAccent, p }) => {
    await choisirTheme('Rose');
    await choisirAccent('#1fb5a3');
    await act.click(150, 273, { button: 'right' });
    await p.locator('.menu .menu-item-title', { hasText: 'Renommer' }).click();
    await p.waitForSelector('.nav-rename-input', { timeout: 5_000 });
    await sonde('champ', p, '.nav-rename-input');
    await act.wait(200);
    await capturer();
});
