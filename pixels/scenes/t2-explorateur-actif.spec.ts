// Scène T2 « t2-explorateur-actif » : « Accueil » ouvert dans la feuille active, la ligne correspondante
// est marquée. ★ Exige une vue de fichier (la note : T3) : le JSON porte « depuis »: « T3 ».
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur-actif', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 195);          // Accueil.md
    await p.waitForSelector('.workspace-leaf.mod-active .view-header', { timeout: 15_000 });
    await act.wait(500);
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('ligne active', p, '.tree-item-self.is-active');
    await capturer();
});
