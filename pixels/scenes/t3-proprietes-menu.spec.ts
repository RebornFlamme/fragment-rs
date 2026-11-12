// Scène T3 « t3-proprietes-menu » : le menu de l'icône de la propriété « statut » ouvert, l'entrée
// « Property type » survolée (sous-menu des types affiché). Le pointeur reste sur l'entrée : pas de
// « hover 2,2 » ici.
import { sceneT3b } from './_t3b';

sceneT3b('t3-proprietes-menu', 'proprietes.md', async ({ act, p, capturer, sonde }) => {
    // ★ Un menu resté ouvert par le thème précédent masquerait celui-ci : on referme d'abord.
    await act.key('Escape');
    await p.waitForFunction(() => document.querySelectorAll('.menu').length === 0);
    await act.click(344, 270);
    await p.waitForSelector('.menu-item', { timeout: 3_000 });
    const item = p.locator('.menu-item', { hasText: 'Property type' });
    const b = (await item.boundingBox())!;
    // ★ Coordonnées entières : le JSON du candidat porte exactement les mêmes.
    await act.hover(Math.round(b.x + b.width / 2), Math.round(b.y + b.height / 2));
    await p.waitForSelector('.menu-item:has-text("Automatic")', { timeout: 3_000 });
    await sonde('entrée Property type', '.menu-item:has-text("Property type")');
    await sonde('menu', '.menu', 0);
    await sonde('sous-menu', '.menu', 1);
    await act.wait(200);
    await capturer();
    await act.key('Escape');
});
