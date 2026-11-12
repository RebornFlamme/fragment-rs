// Scène T3 « t3-maths-curseur » : un clic sur la formule en ligne place le curseur dedans, la source
// « $…$ » est révélée à la place du rendu (la formule en bloc reste rendue).
import { sceneT3b } from './_t3b';

sceneT3b('t3-maths-curseur', 'maths.md', async ({ act, capturer, sonde }) => {
    await act.click(519, 255);
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('ligne de la formule', '.doc-editor .cm-line', 3);
    await capturer();
});
