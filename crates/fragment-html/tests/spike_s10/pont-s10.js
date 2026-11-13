// Pont de test du spike S10, écrit pour ce dépôt (ADR-007) : hello, hauteur, touche, focus.
// Il parle à l'hôte par le canal qui existe : `parent.postMessage` dans une iframe (comme le pont
// de Fragment), sinon `window.ipc` (webview de premier niveau, wry).
(function () {
    var dansCadre = window.parent !== window;
    function envoyer(type, champs) {
        var m = Object.assign({ fragmentHtml: 1, type: type }, champs);
        if (dansCadre) window.parent.postMessage(m, '*');
        else if (window.ipc) window.ipc.postMessage(JSON.stringify(m));
    }
    window.__envoyer = envoyer;
    envoyer('hello', { cadre: dansCadre });
    function hauteur() {
        envoyer('height', { height: document.documentElement.scrollHeight });
    }
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', hauteur);
    else hauteur();
    addEventListener('keydown', function (e) {
        envoyer('key', { key: e.key, ctrlKey: e.ctrlKey });
    }, true);
    addEventListener('focus', function () { envoyer('focus', { on: true }); });
    addEventListener('blur', function () { envoyer('focus', { on: false }); });
})();
