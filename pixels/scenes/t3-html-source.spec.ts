// Scène T3 « t3-html-source » : le curseur dans la div : la source HTML à la place du rendu.
import { sceneT3c } from './_t3c';

sceneT3c('t3-html-source', 'html-blocs.md', async ({ act, p, capturer, sonde }) => {
    // Un clic dans le bloc pose le curseur contre lui : sa source se révèle.
    await act.click(300, 140);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
