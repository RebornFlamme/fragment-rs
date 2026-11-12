// Scène T3 « t3-note-ouverte » : une note ouverte seule, docks repliés. Aucun focus encore donné à
// l'éditeur : rien n'est révélé en source (la règle « révélé au 1er focus » de la vue Markdown).
import { sceneT3a } from './_t3a';

sceneT3a('t3-note-ouverte', 'note-ouverte.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('ligne titre', '.doc-editor .cm-line', 0);
    await sonde('ligne paragraphe', '.doc-editor .cm-line', 2);
    await capturer();
});
