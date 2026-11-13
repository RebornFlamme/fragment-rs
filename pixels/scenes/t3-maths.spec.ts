// Scène T3 « t3-maths » : Maths en ligne et en bloc, rendues (aucun focus donné).
import { sceneT3b } from './_t3b';

sceneT3b('t3-maths', 'maths.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('math en ligne', '.cm-math-inline', 0);
    await sonde('math bloc', '.cm-math-block', 0);
    await capturer();
});
