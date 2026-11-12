// Scène T3 « t3-html-source » : le curseur dans la div : la source HTML à la place du rendu.
import { sceneT3c } from './_t3c';

sceneT3c('t3-html-source', 'html-blocs.md', async ({ act, capturer }) => {
    // Un clic en bout de titre, puis deux flèches vers le bas : le curseur entre dans le bloc (ligne vide, puis bloc).
    await act.click(800, 200);
    await act.key('ArrowDown');
    await act.key('ArrowDown');
    await act.figerSvg();
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
