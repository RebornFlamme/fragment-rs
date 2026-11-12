// Scène T3 « t3-repli » : le premier titre de niveau deux (ligne 5) est replié par le chevron de la
// gouttière ; son contenu et son sous-titre disparaissent, le titre suivant reste.
import { sceneT3a } from './_t3a';

sceneT3a('t3-repli', 'repli.md', async ({ act, capturer, sonde }) => {
    await act.waitStable();
    await sonde('titre à replier', '.doc-editor .cm-line', 4);
    await act.replier(5);
    await act.waitStable();
    await capturer();
});
