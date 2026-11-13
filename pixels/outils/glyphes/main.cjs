// Fenêtre Electron vide du banc du relevé des glyphes (fr-p3.4) : le moteur même des références.
const { app, BrowserWindow } = require('electron');

app.whenReady().then(() => {
    const w = new BrowserWindow({ width: 400, height: 300, useContentSize: true, show: true, frame: false });
    w.loadURL('about:blank');
});
