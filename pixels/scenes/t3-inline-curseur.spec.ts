// Scène T3 « t3-inline-curseur » : un clic dans la première ligne de texte mis en forme, qui révèle la
// source de CETTE ligne seulement (les autres restent rendues). Point cliqué : bord gauche + 40.
import { sceneT3a } from './_t3a';

sceneT3a('t3-inline-curseur', 'inline.md', async ({ act, p, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('ligne cliquée', '.doc-editor .cm-line', 2);
    const b = (await p.locator('.doc-editor .cm-line').nth(2).boundingBox())!;
    await act.click(Math.round(b.x + 40), Math.round(b.y + b.height / 2));
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
