<#
.SYNOPSIS
  Sentinelle du mode invisible (fr-9x4) : prouve, pendant tout un lancement d'Electron, que Fragment
  ne vient jamais devant l'utilisateur.
.DESCRIPTION
  Lancée (fenêtre cachée) par pixels/invisible/garde.ts, une par processus de test Playwright. Elle
  surveille les processus DESCENDANTS de -Racine (le processus de test : Electron en est l'enfant).
  Règle (décision de l'orchestrateur, 13/11) — ÉCHEC si et seulement si :
    - « premier-plan-electron » : le premier plan appartient à un processus du run ;
    - « fenetre-sur-moniteur » : un rectangle de fenêtre visible d'un processus du run intersecte un moniteur ;
    - « barre-des-taches » : pendant un lancement, un bouton de la barre des tâches dont l'Appid ou le nom
      évoque Electron/Fragment apparaît ou change (relevé UI Automation comparé à celui d'AVANT).
  Un changement de premier plan entre fenêtres qui ne sont PAS du run (l'utilisateur passe de Chrome à
  la barre des tâches) n'est pas un échec : ligne « premier-plan-utilisateur », informative seulement.
  ★ Run 4 (13/11) : l'ancienne règle « premier plan identique d'AVANT à APRÈS » a arrêté le run sur
    chrome → Shell_TrayWnd, geste de l'utilisateur, Electron resté à x=3912 : faux positif.
  Contrôle par événements (SetWinEventHook : premier plan, apparition, déplacement) ET par balayage
  toutes les 50 ms (barre des tâches : fil à part, toutes les 250 ms et à chaque apparition). Au premier
  écart : ligne ECHEC, Electron (tous les descendants) tué, fichier -Drapeau écrit (le garde refuse
  ensuite tout lancement), code de sortie 3.
  Protocole : commandes sur stdin (AVANT <id>, APRES <id>, QUITTER), une ligne JSON par réponse sur
  stdout, tout recopié dans -Journal. Lecture seule sur Win32 : rien n'est déplacé ni activé.
  -AutoTest : contrôle les fonctions elles-mêmes SANS Electron, en lecture seule sur les fenêtres
  déjà présentes (dont les cas synthétiques de la règle), puis sort (0 = tout bon).
.NOTES
  Windows PowerShell 5.1 : le C# d'Add-Type est compilé en C# 5 (pas d'interpolation ni d'out var).
#>
param(
    [int]$Racine = 0,
    [string]$Journal = '',
    [string]$Drapeau = '',
    [switch]$AutoTest
)

$ErrorActionPreference = 'Stop'

$source = @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Windows.Automation;

public static class SentinelleInvisible
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    struct MSG { public IntPtr hwnd; public uint message; public IntPtr wParam; public IntPtr lParam; public uint time; public int x; public int y; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct PROCESSENTRY32W
    {
        public uint dwSize; public uint cntUsage; public uint th32ProcessID; public IntPtr th32DefaultHeapID;
        public uint th32ModuleID; public uint cntThreads; public uint th32ParentProcessID; public int pcPriClassBase;
        public uint dwFlags; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string szExeFile;
    }

    delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);
    delegate bool MonitorEnumProc(IntPtr hMonitor, IntPtr hdc, ref RECT rect, IntPtr data);
    delegate void WinEventProc(IntPtr hook, uint evt, IntPtr hwnd, int idObject, int idChild, uint thread, uint time);

    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr clip, MonitorEnumProc cb, IntPtr data);
    [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr hmod, WinEventProc cb, uint pid, uint tid, uint flags);
    [DllImport("user32.dll")] static extern bool UnhookWinEvent(IntPtr hook);
    [DllImport("user32.dll")] static extern bool PeekMessage(out MSG msg, IntPtr hwnd, uint min, uint max, uint remove);
    [DllImport("user32.dll")] static extern bool TranslateMessage(ref MSG msg);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG msg);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out int val, int size);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint pid);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern bool Process32FirstW(IntPtr snap, ref PROCESSENTRY32W e);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern bool Process32NextW(IntPtr snap, ref PROCESSENTRY32W e);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    const uint EVENT_SYSTEM_FOREGROUND = 0x0003;
    const uint EVENT_OBJECT_SHOW = 0x8002;
    const uint EVENT_OBJECT_LOCATIONCHANGE = 0x800B;
    const uint WINEVENT_OUTOFCONTEXT = 0x0000;
    const int OBJID_WINDOW = 0;

    // ---- État ----------------------------------------------------------------------------------
    static int racine;
    static int moi;
    static string journal;
    static string drapeau;
    static readonly object verrou = new object();
    static List<RECT> moniteurs = new List<RECT>();
    static HashSet<uint> nos = new HashSet<uint>();
    static Dictionary<uint, string> noms = new Dictionary<uint, string>();
    static IntPtr fgAvant = IntPtr.Zero;
    static IntPtr fgDernier = IntPtr.Zero;
    static int changementsUtilisateur = 0;
    static string idActif = null;
    // Barre des tâches : relevé de référence (AVANT) et dernier relevé du fil UI Automation.
    static Dictionary<string, string> barreAvant = null;
    static long barreAvantSeq = 0;
    static Dictionary<string, string> barreDerniere = null;
    static long barreSeq = 0;
    static long barreDernierSucces = 0;
    static string barreErreur = null;
    static AutoResetEvent barreReveil = new AutoResetEvent(false);
    static Stopwatch horloge = Stopwatch.StartNew();
    static bool echec = false;
    static bool scanDemande = false;
    static long balayages = 0;
    static int fenetresVuesMax = 0;
    static HashSet<long> fenetresConnues = new HashSet<long>();
    static WinEventProc rappel; // gardé vivant (sinon le GC le libère sous le crochet)

    // ---- Sortie --------------------------------------------------------------------------------
    static string Esc(string s)
    {
        if (s == null) return "";
        StringBuilder b = new StringBuilder();
        foreach (char c in s)
        {
            if (c == '"' || c == '\\') { b.Append('\\'); b.Append(c); }
            else if (c < 32) b.Append(' ');
            else b.Append(c);
        }
        return b.ToString();
    }

    public static void Ecrire(string json)
    {
        lock (verrou)
        {
            string ligne = "{\"t\":" + DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() + "," + json.Substring(1);
            // ★ Journal D'ABORD : quand le processus de test sort, stdout devient un tube rompu et
            //   l'écriture console lève — sans ce try, la sentinelle mourait avant de journaliser APRÈS.
            if (!string.IsNullOrEmpty(journal)) { try { File.AppendAllText(journal, ligne + "\n"); } catch { } }
            try { Console.Out.WriteLine(ligne); Console.Out.Flush(); } catch { }
        }
    }

    static string R(RECT r) { return "[" + r.Left + "," + r.Top + "," + r.Right + "," + r.Bottom + "]"; }

    // ---- Moniteurs -----------------------------------------------------------------------------
    public static List<RECT> Moniteurs()
    {
        List<RECT> l = new List<RECT>();
        EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, delegate (IntPtr h, IntPtr dc, ref RECT r, IntPtr d) { l.Add(r); return true; }, IntPtr.Zero);
        return l;
    }

    /// Intersection d'aire non nulle (bornes droite/bas exclues, comme RECT).
    public static bool Intersecte(RECT a, RECT b)
    {
        return a.Left < b.Right && b.Left < a.Right && a.Top < b.Bottom && b.Top < a.Bottom;
    }

    public static bool SurUnMoniteur(RECT r, List<RECT> mons)
    {
        foreach (RECT m in mons) if (Intersecte(r, m)) return true;
        return false;
    }

    /// Position hors de TOUS les moniteurs : à droite du bord droit le plus à droite (+ 2000 px), en haut
    /// du plus haut. Un rectangle qui commence là ne peut intersecter aucun moniteur, quelle que soit sa taille.
    public static int[] HorsEcran(List<RECT> mons)
    {
        int droite = 0, haut = 0;
        bool premier = true;
        foreach (RECT m in mons)
        {
            if (premier) { droite = m.Right; haut = m.Top; premier = false; }
            droite = Math.Max(droite, m.Right);
            haut = Math.Min(haut, m.Top);
        }
        return new int[] { droite + 2000, haut };
    }

    // ---- Processus -----------------------------------------------------------------------------
    /// Descendants de `racine` (hors `exclu` et ses propres descendants), avec le nom de l'exécutable.
    public static Dictionary<uint, string> Descendants(int racine, int exclu)
    {
        Dictionary<uint, uint> parent = new Dictionary<uint, uint>();
        Dictionary<uint, string> exe = new Dictionary<uint, string>();
        IntPtr snap = CreateToolhelp32Snapshot(0x2, 0);
        if (snap == new IntPtr(-1)) return new Dictionary<uint, string>();
        try
        {
            PROCESSENTRY32W e = new PROCESSENTRY32W();
            e.dwSize = (uint)Marshal.SizeOf(typeof(PROCESSENTRY32W));
            if (Process32FirstW(snap, ref e))
            {
                do { parent[e.th32ProcessID] = e.th32ParentProcessID; exe[e.th32ProcessID] = e.szExeFile; }
                while (Process32NextW(snap, ref e));
            }
        }
        finally { CloseHandle(snap); }
        Dictionary<uint, string> res = new Dictionary<uint, string>();
        foreach (uint pid in parent.Keys)
        {
            // On remonte les parents : la racine atteinte sans passer par `exclu` = un descendant.
            uint p = pid;
            bool garde = false;
            for (int i = 0; i < 64; i++)
            {
                if (p == (uint)exclu) break;
                uint pp;
                if (!parent.TryGetValue(p, out pp) || pp == p || pp == 0) break;
                if (pp == (uint)racine) { garde = true; break; }
                p = pp;
            }
            if (garde && pid != (uint)racine && pid != (uint)exclu) res[pid] = exe[pid];
        }
        return res;
    }

    public static void Tuer(IEnumerable<uint> pids)
    {
        foreach (uint pid in pids)
        {
            try { Process.GetProcessById((int)pid).Kill(); } catch { }
        }
    }

    // ---- Fenêtres ------------------------------------------------------------------------------
    public class Fenetre { public IntPtr H; public uint Pid; public RECT Rect; public string Classe; public bool Masquee; }

    static bool Masquee(IntPtr h)
    {
        int cloak;
        if (DwmGetWindowAttribute(h, 14, out cloak, 4) == 0 && cloak != 0) return true; // DWMWA_CLOAKED
        return false;
    }

    static string Classe(IntPtr h)
    {
        StringBuilder s = new StringBuilder(256);
        GetClassNameW(h, s, 256);
        return s.ToString();
    }

    /// Fenêtres de premier niveau VISIBLES (WS_VISIBLE, non réduites) des processus `pids`.
    public static List<Fenetre> FenetresVisibles(HashSet<uint> pids)
    {
        List<Fenetre> l = new List<Fenetre>();
        EnumWindows(delegate (IntPtr h, IntPtr p)
        {
            uint pid;
            GetWindowThreadProcessId(h, out pid);
            if (!pids.Contains(pid) || !IsWindowVisible(h) || IsIconic(h)) return true;
            RECT r;
            if (!GetWindowRect(h, out r)) return true;
            Fenetre f = new Fenetre();
            f.H = h; f.Pid = pid; f.Rect = r; f.Classe = Classe(h); f.Masquee = Masquee(h);
            l.Add(f);
            return true;
        }, IntPtr.Zero);
        return l;
    }

    public static string DecrireFenetre(IntPtr h)
    {
        if (h == IntPtr.Zero) return "{\"hwnd\":0}";
        uint pid;
        GetWindowThreadProcessId(h, out pid);
        RECT r;
        GetWindowRect(h, out r);
        string exe = "";
        try { exe = Process.GetProcessById((int)pid).ProcessName; } catch { }
        return "{\"hwnd\":" + h.ToInt64() + ",\"pid\":" + pid + ",\"exe\":\"" + Esc(exe) + "\",\"classe\":\"" + Esc(Classe(h)) + "\",\"rect\":" + R(r) + "}";
    }

    // ---- Règle (fonctions pures, testées par -AutoTest) -----------------------------------------
    /// Verdict sur le premier plan : « electron » (ÉCHEC) s'il appartient à un processus du run ;
    /// « utilisateur » (journalisé seulement) s'il a changé entre deux fenêtres hors run ; sinon null.
    public static string VerdictPremierPlan(IntPtr fg, uint fgPid, HashSet<uint> run, IntPtr precedent)
    {
        if (fg != IntPtr.Zero && run.Contains(fgPid)) return "electron";
        if (fg != precedent) return "utilisateur";
        return null;
    }

    static readonly Regex evoqueElectron = new Regex("electron|fragment", RegexOptions.IgnoreCase);

    /// Boutons de la barre des tâches (clé = Appid, valeur = nom) qui évoquent Electron/Fragment et sont
    /// NOUVEAUX ou ont CHANGÉ de nom (ex. « 1 fenêtre » → « 2 fenêtres ») depuis le relevé d'AVANT.
    /// Un bouton disparu, ou qui n'évoque ni l'un ni l'autre (Chrome, l'horloge…), n'est jamais retenu.
    public static List<string> BoutonsSuspects(Dictionary<string, string> avant, Dictionary<string, string> maintenant)
    {
        List<string> l = new List<string>();
        foreach (KeyValuePair<string, string> kv in maintenant)
        {
            if (!evoqueElectron.IsMatch(kv.Key) && !evoqueElectron.IsMatch(kv.Value)) continue;
            string v;
            if (!avant.TryGetValue(kv.Key, out v) || v != kv.Value) l.Add(kv.Key);
        }
        return l;
    }

    // ---- Barre des tâches (UI Automation, lecture seule) -----------------------------------------
    /// Boutons de la barre des tâches principale : Appid (AutomationId) → nom accessible. Lève si la barre
    /// est introuvable. ★ Électron met skipTaskbar par ITaskbarList::DeleteTab, pas par un style de
    /// fenêtre : les styles ne disent rien, seule la barre elle-même fait foi.
    public static Dictionary<string, string> BoutonsBarre()
    {
        AutomationElement barre = AutomationElement.RootElement.FindFirst(TreeScope.Children,
            new PropertyCondition(AutomationElement.ClassNameProperty, "Shell_TrayWnd"));
        if (barre == null) throw new InvalidOperationException("Shell_TrayWnd introuvable");
        AutomationElementCollection bs = barre.FindAll(TreeScope.Descendants,
            new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Button));
        Dictionary<string, string> d = new Dictionary<string, string>();
        foreach (AutomationElement b in bs)
        {
            string cls = b.Current.ClassName ?? "";
            if (cls.StartsWith("SystemTray.") || cls == "ToggleButton") continue; // zone de notification, Démarrer…
            string cle = b.Current.AutomationId ?? "";
            if (cle == "") cle = cls;
            string k = cle;
            for (int i = 2; d.ContainsKey(k); i++) k = cle + "#" + i;
            d[k] = b.Current.Name ?? "";
        }
        return d;
    }

    static string Json(Dictionary<string, string> d, IEnumerable<string> cles)
    {
        StringBuilder b = new StringBuilder("{");
        bool premier = true;
        foreach (string k in cles)
        {
            if (!premier) b.Append(",");
            premier = false;
            string v;
            b.Append("\"" + Esc(k) + "\":\"" + Esc(d.TryGetValue(k, out v) ? v : "(absent)") + "\"");
        }
        return b.Append("}").ToString();
    }

    static void FilBarre()
    {
        while (true)
        {
            try
            {
                Dictionary<string, string> d = BoutonsBarre();
                lock (verrou) { barreDerniere = d; barreSeq++; barreDernierSucces = horloge.ElapsedMilliseconds; barreErreur = null; }
            }
            catch (Exception e)
            {
                lock (verrou) { barreErreur = e.GetType().Name + ": " + e.Message; }
            }
            barreReveil.WaitOne(250);
        }
    }

    /// Attend un relevé de la barre postérieur à l'appel (au plus `delai` ms) ; null si aucun.
    static Dictionary<string, string> ReleveFrais(int delai, out long seq)
    {
        long depart;
        lock (verrou) depart = barreSeq;
        barreReveil.Set();
        Stopwatch m = Stopwatch.StartNew();
        while (m.ElapsedMilliseconds < delai)
        {
            lock (verrou) { if (barreSeq > depart) { seq = barreSeq; return barreDerniere; } }
            Thread.Sleep(10);
        }
        seq = 0;
        return null;
    }

    // ---- Contrôle ------------------------------------------------------------------------------
    static void Echec(string preuve, string detail)
    {
        if (echec) return;
        echec = true;
        List<uint> cibles = new List<uint>(Descendants(racine, moi).Keys);
        Ecrire("{\"type\":\"ECHEC\",\"preuve\":\"" + preuve + "\",\"id\":\"" + Esc(idActif) + "\",\"detail\":" + detail + ",\"tues\":" + cibles.Count + "}");
        Tuer(cibles);
        if (!string.IsNullOrEmpty(drapeau))
        {
            try { File.WriteAllText(drapeau, "Preuve « " + preuve + " » en échec (lancement " + idActif + ") : " + detail + "\n"); } catch { }
        }
    }

    static void Balayer()
    {
        balayages++;
        Dictionary<uint, string> d = Descendants(racine, moi);
        nos = new HashSet<uint>(d.Keys);
        noms = d;
        // 1. Premier plan : jamais à un processus du run. Un changement entre fenêtres de l'utilisateur
        //    est journalisé (preuve informative), pas un échec.
        IntPtr fg = GetForegroundWindow();
        uint fgPid;
        GetWindowThreadProcessId(fg, out fgPid);
        string verdict = VerdictPremierPlan(fg, fgPid, nos, fgDernier);
        if (verdict == "electron") { Echec("premier-plan-electron", DecrireFenetre(fg)); return; }
        if (verdict == "utilisateur")
        {
            if (idActif != null)
            {
                changementsUtilisateur++;
                Ecrire("{\"type\":\"premier-plan-utilisateur\",\"id\":\"" + Esc(idActif) + "\",\"avant\":" + DecrireFenetre(fgDernier) + ",\"maintenant\":" + DecrireFenetre(fg) + "}");
            }
            fgDernier = fg;
        }
        // 2. Barre des tâches (pendant un lancement) : aucun bouton Electron/Fragment nouveau ou changé.
        if (idActif != null && barreAvant != null)
        {
            Dictionary<string, string> d2 = null;
            long seq, succes;
            string err;
            lock (verrou) { d2 = barreDerniere; seq = barreSeq; succes = barreDernierSucces; err = barreErreur; }
            if (seq > barreAvantSeq && d2 != null)
            {
                List<string> sus = BoutonsSuspects(barreAvant, d2);
                if (sus.Count > 0) { Echec("barre-des-taches", "{\"avant\":" + Json(barreAvant, sus) + ",\"maintenant\":" + Json(d2, sus) + "}"); return; }
            }
            // Sans relevé réussi depuis 3 s, la preuve n'est plus tenue : échec (barre illisible).
            if (horloge.ElapsedMilliseconds - succes > 3000)
            {
                Echec("barre-des-taches-illisible", "{\"erreur\":\"" + Esc(err) + "\",\"depuisMs\":" + (horloge.ElapsedMilliseconds - succes) + "}");
                return;
            }
        }
        // 3. Aucun rectangle de fenêtre visible du run sur un moniteur.
        List<Fenetre> fs = FenetresVisibles(nos);
        if (fs.Count > fenetresVuesMax) fenetresVuesMax = fs.Count;
        foreach (Fenetre f in fs)
        {
            if (fenetresConnues.Add(f.H.ToInt64()))
            {
                Ecrire("{\"type\":\"fenetre\",\"id\":\"" + Esc(idActif) + "\",\"hwnd\":" + f.H.ToInt64() + ",\"pid\":" + f.Pid + ",\"exe\":\"" + Esc(noms.ContainsKey(f.Pid) ? noms[f.Pid] : "") + "\",\"classe\":\"" + Esc(f.Classe) + "\",\"masquee\":" + (f.Masquee ? "true" : "false") + ",\"rect\":" + R(f.Rect) + "}");
            }
            if (SurUnMoniteur(f.Rect, moniteurs)) { Echec("fenetre-sur-moniteur", DecrireFenetre(f.H)); return; }
        }
    }

    static void SurEvenement(IntPtr hook, uint evt, IntPtr hwnd, int idObject, int idChild, uint thread, uint time)
    {
        if (evt == EVENT_SYSTEM_FOREGROUND || idObject == OBJID_WINDOW) scanDemande = true;
        // Une fenêtre qui apparaît peut gagner un bouton : relevé de la barre sans attendre les 250 ms.
        if (evt == EVENT_OBJECT_SHOW && idObject == OBJID_WINDOW) barreReveil.Set();
    }

    static void Commande(string ligne)
    {
        string[] m = ligne.Trim().Split(new char[] { ' ' }, 2);
        string verbe = m[0];
        string id = m.Length > 1 ? m[1] : "";
        if (verbe == "AVANT")
        {
            if (idActif != null) Ecrire("{\"type\":\"apres-implicite\",\"id\":\"" + Esc(idActif) + "\"}");
            moniteurs = Moniteurs();
            fgAvant = GetForegroundWindow();
            fgDernier = fgAvant;
            changementsUtilisateur = 0;
            // Relevé de référence de la barre AVANT le lancement : sans lui, pas de preuve, pas de lancement.
            long seq;
            Dictionary<string, string> b = ReleveFrais(5000, out seq);
            if (b == null)
            {
                string err;
                lock (verrou) err = barreErreur;
                idActif = id;
                Echec("barre-des-taches-illisible", "{\"erreur\":\"" + Esc(err) + "\",\"quand\":\"avant\"}");
                return;
            }
            barreAvant = b;
            barreAvantSeq = seq;
            idActif = id;
            Balayer();
            if (!echec) Ecrire("{\"type\":\"avant\",\"id\":\"" + Esc(id) + "\",\"premierPlan\":" + DecrireFenetre(fgAvant) + ",\"barre\":" + Json(b, b.Keys) + "}");
        }
        else if (verbe == "APRES")
        {
            // Dernier relevé de la barre, frais, avant de conclure.
            long seq;
            ReleveFrais(2000, out seq);
            Balayer();
            if (!echec)
            {
                IntPtr fg = GetForegroundWindow();
                long nb;
                lock (verrou) nb = barreSeq - barreAvantSeq;
                Ecrire("{\"type\":\"apres\",\"id\":\"" + Esc(id) + "\",\"ok\":true,\"identique\":" + (fg == fgAvant ? "true" : "false") + ",\"changementsUtilisateur\":" + changementsUtilisateur + ",\"premierPlan\":" + DecrireFenetre(fg) + ",\"balayages\":" + balayages + ",\"relevesBarre\":" + nb + ",\"fenetresMax\":" + fenetresVuesMax + "}");
            }
            idActif = null;
            barreAvant = null;
            balayages = 0;
            fenetresVuesMax = 0;
        }
        else if (verbe != "")
        {
            Ecrire("{\"type\":\"commande-inconnue\",\"ligne\":\"" + Esc(ligne) + "\"}");
        }
    }

    public static int Run(int racine_, string journal_, string drapeau_)
    {
        racine = racine_;
        journal = journal_;
        drapeau = drapeau_;
        moi = Process.GetCurrentProcess().Id;
        // ★ Coordonnées PHYSIQUES : un fil non conscient du DPI lirait des rectangles mis à l'échelle.
        SetThreadDpiAwarenessContext(new IntPtr(-4)); // PER_MONITOR_AWARE_V2
        moniteurs = Moniteurs();
        int[] hors = HorsEcran(moniteurs);
        StringBuilder ms = new StringBuilder("[");
        for (int i = 0; i < moniteurs.Count; i++) { if (i > 0) ms.Append(","); ms.Append(R(moniteurs[i])); }
        ms.Append("]");

        System.Collections.Concurrent.ConcurrentQueue<string> file = new System.Collections.Concurrent.ConcurrentQueue<string>();
        bool finEntree = false;
        Thread lecteur = new Thread(delegate ()
        {
            string l;
            while ((l = Console.In.ReadLine()) != null) file.Enqueue(l);
            finEntree = true;
        });
        lecteur.IsBackground = true;
        lecteur.Start();
        Thread filBarre = new Thread(FilBarre);
        filBarre.IsBackground = true;
        filBarre.SetApartmentState(ApartmentState.MTA);
        filBarre.Start();

        rappel = new WinEventProc(SurEvenement);
        IntPtr h1 = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, IntPtr.Zero, rappel, 0, 0, WINEVENT_OUTOFCONTEXT);
        IntPtr h2 = SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW, IntPtr.Zero, rappel, 0, 0, WINEVENT_OUTOFCONTEXT);
        IntPtr h3 = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, IntPtr.Zero, rappel, 0, 0, WINEVENT_OUTOFCONTEXT);
        Ecrire("{\"type\":\"pret\",\"racine\":" + racine + ",\"sentinelle\":" + moi + ",\"moniteurs\":" + ms + ",\"horsEcran\":[" + hors[0] + "," + hors[1] + "],\"crochets\":" + ((h1 != IntPtr.Zero && h2 != IntPtr.Zero && h3 != IntPtr.Zero) ? "true" : "false") + "}");

        Stopwatch montre = Stopwatch.StartNew();
        long dernier = 0;
        try
        {
            while (!echec)
            {
                MSG msg;
                while (PeekMessage(out msg, IntPtr.Zero, 0, 0, 1)) { TranslateMessage(ref msg); DispatchMessage(ref msg); }
                string c;
                bool quitter = false;
                while (file.TryDequeue(out c))
                {
                    if (c.Trim() == "QUITTER") { quitter = true; break; }
                    Commande(c);
                    if (echec) break;
                }
                if (quitter || echec) break;
                if (finEntree)
                {
                    // Le processus de test a disparu (fin du run) : dernier contrôle, et relevé APRÈS du
                    // lancement encore ouvert — le garde n'a pas toujours le temps de le demander.
                    if (idActif != null) Commande("APRES " + idActif); else Balayer();
                    break;
                }
                if (scanDemande || montre.ElapsedMilliseconds - dernier >= 50)
                {
                    scanDemande = false;
                    dernier = montre.ElapsedMilliseconds;
                    Balayer();
                }
                Thread.Sleep(10);
            }
        }
        finally
        {
            UnhookWinEvent(h1); UnhookWinEvent(h2); UnhookWinEvent(h3);
        }
        Ecrire("{\"type\":\"fin\",\"echec\":" + (echec ? "true" : "false") + "}");
        return echec ? 3 : 0;
    }

    // ---- Auto-test (sans Electron, lecture seule) ----------------------------------------------
    public static void Journal(string j) { journal = j; }

    public static int AutoTest()
    {
        int ko = 0;
        SetThreadDpiAwarenessContext(new IntPtr(-4));
        List<RECT> mons = Moniteurs();
        Ecrire("{\"type\":\"autotest\",\"cas\":\"moniteurs\",\"ok\":" + (mons.Count > 0 ? "true" : "false") + ",\"nombre\":" + mons.Count + "}");
        if (mons.Count == 0) ko++;

        // Intersection : un rectangle posé à HorsEcran n'intersecte rien ; un rectangle au centre du
        // premier moniteur, si ; un rectangle collé au bord droit (bord exclu), non.
        int[] hors = HorsEcran(mons);
        RECT loin = new RECT(); loin.Left = hors[0]; loin.Top = hors[1]; loin.Right = hors[0] + 4000; loin.Bottom = hors[1] + 3000;
        RECT m0 = mons.Count > 0 ? mons[0] : new RECT();
        RECT centre = new RECT(); centre.Left = (m0.Left + m0.Right) / 2; centre.Top = (m0.Top + m0.Bottom) / 2; centre.Right = centre.Left + 10; centre.Bottom = centre.Top + 10;
        int maxDroite = hors[0] - 2000;
        RECT colle = new RECT(); colle.Left = maxDroite; colle.Top = m0.Top; colle.Right = maxDroite + 100; colle.Bottom = m0.Top + 100;
        bool okLoin = !SurUnMoniteur(loin, mons), okCentre = SurUnMoniteur(centre, mons), okColle = !SurUnMoniteur(colle, mons);
        Ecrire("{\"type\":\"autotest\",\"cas\":\"intersection\",\"ok\":" + ((okLoin && okCentre && okColle) ? "true" : "false") + ",\"horsEcran\":[" + hors[0] + "," + hors[1] + "]}");
        if (!(okLoin && okCentre && okColle)) ko++;

        // Premier plan : la fenêtre au premier plan existe et est sur un moniteur (fenêtre de l'utilisateur).
        IntPtr fg = GetForegroundWindow();
        RECT rfg;
        bool okFg = fg != IntPtr.Zero && GetWindowRect(fg, out rfg) && SurUnMoniteur(rfg, mons);
        Ecrire("{\"type\":\"autotest\",\"cas\":\"premier-plan\",\"ok\":" + (okFg ? "true" : "false") + ",\"fenetre\":" + DecrireFenetre(fg) + "}");
        if (!okFg) ko++;

        // Énumération : les fenêtres visibles du processus du premier plan contiennent ce premier plan.
        uint fgPid;
        GetWindowThreadProcessId(fg, out fgPid);
        HashSet<uint> s = new HashSet<uint>(); s.Add(fgPid);
        List<Fenetre> fs = FenetresVisibles(s);
        bool okEnum = false;
        foreach (Fenetre f in fs) if (f.H == fg) okEnum = true;
        Ecrire("{\"type\":\"autotest\",\"cas\":\"enumeration\",\"ok\":" + (okEnum ? "true" : "false") + ",\"fenetres\":" + fs.Count + "}");
        if (!okEnum) ko++;

        // Arbre et arrêt : un enfant caché (ping) est vu comme descendant, puis tué.
        ProcessStartInfo psi = new ProcessStartInfo("ping.exe", "-n 60 127.0.0.1");
        psi.CreateNoWindow = true; psi.UseShellExecute = false; psi.RedirectStandardOutput = true;
        Process enfant = Process.Start(psi);
        int moi_ = Process.GetCurrentProcess().Id;
        Thread.Sleep(200);
        Dictionary<uint, string> d = Descendants(moi_, -1);
        bool vu = d.ContainsKey((uint)enfant.Id);
        Dictionary<uint, string> dExclu = Descendants(moi_, enfant.Id);
        bool exclu = !dExclu.ContainsKey((uint)enfant.Id);
        Tuer(new uint[] { (uint)enfant.Id });
        bool tue = enfant.WaitForExit(5000);
        Ecrire("{\"type\":\"autotest\",\"cas\":\"arbre-et-arret\",\"ok\":" + ((vu && exclu && tue) ? "true" : "false") + ",\"vu\":" + (vu ? "true" : "false") + ",\"exclu\":" + (exclu ? "true" : "false") + ",\"tue\":" + (tue ? "true" : "false") + "}");
        if (!(vu && exclu && tue)) ko++;

        // Règle du premier plan (cas synthétiques, run 4 compris).
        HashSet<uint> run = new HashSet<uint>(); run.Add(6040);
        IntPtr chrome = new IntPtr(22873786), barreFg = new IntPtr(196748), elec = new IntPtr(2624128);
        bool r1 = VerdictPremierPlan(barreFg, 15008, run, chrome) == "utilisateur"; // run 4 : chrome → barre des tâches
        bool r2 = VerdictPremierPlan(chrome, 10700, run, chrome) == null;            // rien ne bouge
        bool r3 = VerdictPremierPlan(elec, 6040, run, chrome) == "electron";         // Electron au premier plan
        bool r4 = VerdictPremierPlan(elec, 6040, run, elec) == "electron";           // … même s'il l'était déjà
        bool r5 = VerdictPremierPlan(IntPtr.Zero, 0, run, chrome) == "utilisateur";  // bureau sans premier plan
        bool okPp = r1 && r2 && r3 && r4 && r5;
        Ecrire("{\"type\":\"autotest\",\"cas\":\"regle-premier-plan\",\"ok\":" + (okPp ? "true" : "false") + ",\"cas_run4_utilisateur\":" + (r1 ? "true" : "false") + "}");
        if (!okPp) ko++;

        // Règle de la barre des tâches (cas synthétiques).
        Dictionary<string, string> av = new Dictionary<string, string>();
        av["Appid: Chrome"] = "Google Chrome - 1 fenêtre en cours d’exécution épinglé";
        av["Appid: electron.app.Fragment"] = "Fragment - 1 fenêtre en cours d’exécution épinglé";
        Dictionary<string, string> chromeChange = new Dictionary<string, string>(av);
        chromeChange["Appid: Chrome"] = "Google Chrome - 2 fenêtres en cours d’exécution épinglé";
        Dictionary<string, string> nouveau = new Dictionary<string, string>(av);
        nouveau["Appid: C:\\x\\node_modules\\electron\\dist\\electron.exe"] = "Electron - 1 fenêtre en cours d’exécution";
        Dictionary<string, string> groupe = new Dictionary<string, string>(av);
        groupe["Appid: electron.app.Fragment"] = "Fragment - 2 fenêtres en cours d’exécution épinglé";
        Dictionary<string, string> titre = new Dictionary<string, string>(av);
        titre["Appid: inconnu"] = "Fragment";
        Dictionary<string, string> disparu = new Dictionary<string, string>();
        bool b1 = BoutonsSuspects(av, av).Count == 0;
        bool b2 = BoutonsSuspects(av, chromeChange).Count == 0;
        bool b3 = BoutonsSuspects(av, nouveau).Count == 1;
        bool b4 = BoutonsSuspects(av, groupe).Count == 1;
        bool b5 = BoutonsSuspects(av, titre).Count == 1;
        bool b6 = BoutonsSuspects(av, disparu).Count == 0;
        bool okB = b1 && b2 && b3 && b4 && b5 && b6;
        Ecrire("{\"type\":\"autotest\",\"cas\":\"regle-barre\",\"ok\":" + (okB ? "true" : "false") + "}");
        if (!okB) ko++;

        // Lecture réelle de la barre des tâches (UI Automation) : au moins un bouton d'application.
        bool okLect = false;
        string detailLect = "";
        try
        {
            Stopwatch sw = Stopwatch.StartNew();
            Dictionary<string, string> d1 = BoutonsBarre();
            int applis = 0;
            foreach (string k in d1.Keys) if (k.StartsWith("Appid:")) applis++;
            okLect = applis > 0;
            detailLect = ",\"boutons\":" + d1.Count + ",\"applis\":" + applis + ",\"ms\":" + sw.ElapsedMilliseconds + ",\"suspectsContreSoi\":" + BoutonsSuspects(d1, BoutonsBarre()).Count;
        }
        catch (Exception e) { detailLect = ",\"erreur\":\"" + Esc(e.Message) + "\""; }
        Ecrire("{\"type\":\"autotest\",\"cas\":\"lecture-barre\",\"ok\":" + (okLect ? "true" : "false") + detailLect + "}");
        if (!okLect) ko++;

        // Le premier plan n'a pas bougé pendant l'auto-test (informatif : un geste de l'utilisateur
        // n'est plus un échec, règle du 13/11).
        bool stable = GetForegroundWindow() == fg;
        Ecrire("{\"type\":\"autotest\",\"cas\":\"premier-plan-stable\",\"ok\":true,\"informatif\":true,\"stable\":" + (stable ? "true" : "false") + "}");
        Ecrire("{\"type\":\"autotest-fin\",\"echecs\":" + ko + "}");
        return ko == 0 ? 0 : 1;
    }
}
'@

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, WindowsBase
# UI Automation (barre des tâches) : assemblages .NET Framework du GAC, rien à installer.
$noms = @('UIAutomationClient', 'UIAutomationTypes', 'WindowsBase')
$refs = [AppDomain]::CurrentDomain.GetAssemblies() | Where-Object { $noms -contains $_.GetName().Name } | ForEach-Object { $_.Location }
Add-Type -TypeDefinition $source -Language CSharp -ReferencedAssemblies $refs
if ($AutoTest) {
    if ($Journal) { [void](New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Journal)) }
    [SentinelleInvisible]::Journal($Journal)
    [SentinelleInvisible]::Ecrire('{"type":"autotest-debut"}')
    exit [SentinelleInvisible]::AutoTest()
}
if ($Racine -le 0) { throw 'Sentinelle : -Racine (PID du processus de test) est obligatoire.' }
exit [SentinelleInvisible]::Run($Racine, $Journal, $Drapeau)
