// Scène T3 « t3-titre-curseur » : un clic dans le titre de niveau deux, qui révèle ses marques « ## »
// (les autres titres restent rendus). Le point cliqué est celui du JSON : x = bord gauche de la ligne + 40.
import { sceneT3a } from './_t3a';

sceneT3a('t3-titre-curseur', 'titres.md', async ({ act, p, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('titre niveau deux', '.doc-editor .cm-ob-h2', 0);
    const b = (await p.locator('.doc-editor .cm-ob-h2').first().boundingBox())!;
    await act.click(Math.round(b.x + 40), Math.round(b.y + b.height / 2));
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
