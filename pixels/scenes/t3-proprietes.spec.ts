// Scène T3 « t3-proprietes » : Properties (frontmatter) : pastilles de tags, texte, case à cocher, « Empty ».
import { sceneT3b } from './_t3b';

sceneT3b('t3-proprietes', 'proprietes.md', async ({ act, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('première ligne', '.doc-editor .cm-line', 0);
    await sonde('icône propriété 1', '.metadata-property-icon', 1);
    await sonde('tags', '.metadata-property', 0);
    await sonde('bouton lecture seule', '.view-action');
    await capturer();
});
