// Scène T4 « t4-selection » : du texte sélectionné dans la couche de texte, à la souris, de la première
// ligne du corps jusqu'au milieu de la troisième (page 1).
import { sceneT4 } from './_t4';

sceneT4('t4-selection', async ({ act, capturer, p }) => {
    const spans = p.locator('.pdf-page[data-page="1"] .pdf-text-layer span');
    const premier = await spans.nth(1).boundingBox();
    const troisieme = await spans.nth(3).boundingBox();
    if (!premier || !troisieme) throw new Error('lignes de la page 1 introuvables');
    await act.drag(
        { x: Math.round(premier.x + 1), y: Math.round(premier.y + premier.height / 2) },
        { x: Math.round(troisieme.x + troisieme.width / 2), y: Math.round(troisieme.y + troisieme.height / 2) },
    );
    // La barre flottante de la sélection (Start Side Chat) est celle du chat, de T7 : hors du périmètre de T4, masquée.
    await p.addStyleTag({ content: ".toolbar{display:none!important}" });
    await act.waitStable();
    await capturer();
});
