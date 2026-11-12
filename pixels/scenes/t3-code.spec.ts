// Scène T3 « t3-code » : Blocs de code, avec le badge de langage (aucun focus donné).
import { sceneT3b } from './_t3b';

sceneT3b('t3-code', 'code.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('code', '.cm-ob-codeblock', 0);
    await sonde('badge', '.cm-ob-code-lang', 0);
    await capturer();
});
