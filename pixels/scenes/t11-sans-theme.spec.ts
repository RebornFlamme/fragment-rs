// Scène T11 « t11-sans-theme » : « Rose » est choisi puis RETIRÉ (thème vide) : l'écran revient à la
// palette de Fragment, même menu ouvert que `t11-rose` pour que les deux se comparent.
import { sceneT11 } from './_t11';

sceneT11('t11-sans-theme', async ({ act, capturer, sonde, choisirTheme, p }) => {
    await choisirTheme('Rose');
    await choisirTheme('');
    await act.click(150, 117, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await sonde('menu', p, '.menu');
    // ★ Le survol de la ligne visée est peint ou non selon la course : le pointeur est garé hors du menu.
    await act.hover(700, 500);
    await act.wait(300);
    await capturer();
});
