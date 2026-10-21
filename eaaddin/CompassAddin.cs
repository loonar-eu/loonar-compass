// Loonar Compass add-in for Sparx Enterprise Architect: a menu and a tab inside EA that hosts the Compass window.
// The add-in is a thin shell: Compass itself (LoonarCompass.exe) does all the work and talks to the open EA through COM.
// Built with the .NET Framework compiler (build.ps1); registered for the current user only (register.ps1 / the installer).
using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Windows.Forms;

[assembly: AssemblyTitle("Loonar Compass add-in for Enterprise Architect")]
[assembly: AssemblyVersion("1.0.0.0")]
[assembly: ComVisible(false)]

namespace LoonarCompass
{
    /// <summary>The add-in EA loads at start (EAAddins64\LoonarCompass = "LoonarCompass.Addin").</summary>
    [ComVisible(true)]
    [Guid("6C1E5B7A-3D2F-4B8E-9A41-2F7C0D5E8B13")]
    [ProgId("LoonarCompass.Addin")]
    [ClassInterface(ClassInterfaceType.AutoDispatch)]
    public class Addin
    {
        const string Menu = "-&Loonar Compass";
        const string OpenTab = "Open Compass in EA";
        const string OpenWindow = "Open Compass in its own window";
        const string AddReference = "Add to the Compass conversation";

        public string EA_Connect(object repository)
        {
            return "";
        }

        /// <summary>The Compass menu goes to the Start ribbon (next to the other everyday tools), not Specialize.</summary>
        public string EA_GetRibbonCategory(object repository)
        {
            return "Start";
        }

        public object EA_GetMenuItems(object repository, string location, string menuName)
        {
            // Right-click on an element, package or diagram: "Loonar Compass" → "Add to the Compass conversation"
            // (EA shows add-in entries of a context menu under the add-in's own submenu).
            if (location == "TreeView" || location == "Diagram")
                return menuName == "" ? L10n.Get(AddReference) : "";
            // Ribbon: one button that opens Compass in EA (the standalone app is started from the Start menu).
            return menuName == "" ? "Loonar Compass" : "";
        }

        public void EA_GetMenuState(object repository, string location, string menuName, string itemName, ref bool isEnabled, ref bool isChecked)
        {
            isEnabled = !string.IsNullOrEmpty(Connection(repository));

        }

        public void EA_MenuClick(object repository, string location, string menuName, string itemName)
        {
            try
            {
                string connection = Connection(repository);
                if (itemName == L10n.Get(AddReference))
                {
                    string reference = Reference(repository);
                    if (reference == null)
                        return;
                    // Compass not open yet: it opens first, then takes the reference.
                    if (current == null || current.IsDisposed)
                        EA_MenuClick(repository, "MainMenu", "", "Loonar Compass");
                    else
                        ((dynamic)repository).ShowAddinWindow("Loonar Compass");
                    if (current != null)
                        current.Send(reference);
                    return;
                }
                dynamic repo = repository;
                // A dockable window like Properties (EA remembers where the user put it), not a tab among the diagrams.
                if (current != null && !current.IsDisposed)
                {
                    repo.ShowAddinWindow("Loonar Compass");
                    return;
                }
                object control = repo.AddWindow("Loonar Compass", "LoonarCompass.Tab");
                CompassTab tab = control as CompassTab;
                if (tab == null)
                    throw new InvalidOperationException("EA did not create the Compass window.");
                current = tab;
                repo.ShowAddinWindow("Loonar Compass");
                tab.Start(connection, repository);
            }
            catch (Exception e)
            {
                MessageBox.Show(e.Message, "Loonar Compass", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>The open Compass tab: it always works with the project open in this EA window.</summary>
        static CompassTab current;

        /// <summary>Another project opened in EA: the open Compass tab switches to it. (Tests: LOONAR_ADDIN_AUTOTAB=1
        /// opens the tab as soon as a project is opened.)</summary>
        public void EA_FileOpen(object repository)
        {
            Log.Write("FileOpen, tab open: " + (current != null && !current.IsDisposed));
            if (current != null && !current.IsDisposed)
                current.Start(Connection(repository), repository);
            else if (Environment.GetEnvironmentVariable("LOONAR_ADDIN_AUTOTAB") == "1")
                EA_MenuClick(repository, "MainMenu", "", "Loonar Compass");
        }

        /// <summary>The project closed in EA: Compass does not work without one, so it closes.</summary>
        public void EA_FileClose(object repository)
        {
            Log.Write("FileClose");
            if (current != null && !current.IsDisposed)
                current.Stop(L10n.Get("Open a project in Enterprise Architect to work with Loonar Compass."));
        }

        public void EA_Disconnect()
        {
            // EA closes: Compass closes with it (it saves its conversation).
            if (current != null && !current.IsDisposed)
                current.Stop("");
            GC.Collect();
            GC.WaitForPendingFinalizers();
        }

        /// <summary>The object picked in EA as a short reference for the message, e.g. «Package: Shop (id 5)».</summary>
        static string Reference(object repository)
        {
            dynamic repo = repository;
            int type = (int)repo.GetContextItemType();
            dynamic item = repo.GetContextObject();
            if (item == null)
                return null;
            switch (type)
            {
                case 4: return "«" + L10n.Get("Element") + ": " + item.Name + " (" + item.Type + ", id " + item.ElementID + ")»";
                case 5: return "«" + L10n.Get("Package") + ": " + item.Name + " (id " + item.PackageID + ")»";
                case 8: return "«" + L10n.Get("Diagram") + ": " + item.Name + " (id " + item.DiagramID + ")»";
                case 7: return "«" + L10n.Get("Connector") + ": " + item.Name + " (" + item.Type + ", id " + item.ConnectorID + ")»";
                case 23: return "«" + L10n.Get("Attribute") + ": " + item.Name + " (id " + item.AttributeID + ")»";
                case 24: return "«" + L10n.Get("Operation") + ": " + item.Name + " (id " + item.MethodID + ")»";
            }
            return null;
        }

        static string Connection(object repository)
        {
            try
            {
                dynamic repo = repository;
                return (string)repo.ConnectionString;
            }
            catch
            {
                return "";
            }
        }
    }

    /// <summary>The EA tab: starts Compass and docks its window inside the tab.</summary>
    [ComVisible(true)]
    [Guid("9B4D2E61-7A3C-4F15-8E2B-5D0C6A1F9E47")]
    [ProgId("LoonarCompass.Tab")]
    [ClassInterface(ClassInterfaceType.AutoDispatch)]
    public class CompassTab : UserControl
    {
        Process process;
        IntPtr child = IntPtr.Zero;
        uint attached;
        IntPtr candidate = IntPtr.Zero;
        DateTime candidateSince;
        int rotCookie;
        string rotName;
        readonly Timer finder = new Timer { Interval = 200 };
        DateTime started;
        readonly Label note = new Label { Dock = DockStyle.Fill, TextAlign = System.Drawing.ContentAlignment.MiddleCenter };

        public CompassTab()
        {
            Controls.Add(note);
            finder.Tick += (s, e) => Dock_();
        }

        /// <summary>Path of LoonarCompass.exe: next to the add-in (installation), or LOONAR_COMPASS_EXE (development).</summary>
        public static string Exe()
        {
            string fromEnv = Environment.GetEnvironmentVariable("LOONAR_COMPASS_EXE");
            if (!string.IsNullOrEmpty(fromEnv) && File.Exists(fromEnv))
                return fromEnv;
            string dir = Path.GetDirectoryName(new Uri(Assembly.GetExecutingAssembly().CodeBase).LocalPath);
            string exe = Path.Combine(dir, "LoonarCompass.exe");
            if (!File.Exists(exe))
                throw new FileNotFoundException("Loonar Compass was not found next to the add-in: " + exe);
            return exe;
        }

        /// <summary>Arguments of Compass: the project (in a file: a database connection string has quotes and semicolons)
        /// and, for the tab, the name under which this EA's repository is in the Running Object Table.</summary>
        public static string Arguments(string connection, string rotName)
        {
            // The connection goes in a file: a database connection string has quotes and semicolons.
            string file = Path.Combine(Path.GetTempPath(), "loonar-ea-model-" + Guid.NewGuid().ToString("N") + ".txt");
            File.WriteAllText(file, connection);
            return (rotName != null ? "--ea-tab --ea-rot " + rotName + " " : "") + "--ea-model \"" + file + "\"";
        }

        /// <summary>EA's repository of this window (for the "Loonar Compass" tab of System Output).</summary>
        object repo;
        const string OutputTab = "Loonar Compass";

        /// <summary>A line in the "Loonar Compass" tab of System Output (and in the add-in log).</summary>
        void Out(string text)
        {
            Log.Write(text);
            try
            {
                dynamic r = repo;
                if (r != null)
                    r.WriteOutput(OutputTab, text, 0);
            }
            catch
            {
            }
        }

        /// <summary>What Compass did (steps of the assistant), handed over in a file and shown in System Output.</summary>
        void ForwardOutput()
        {
            if (rotName == null)
                return;
            string file = Path.Combine(Path.GetTempPath(), "loonar-ea-output-" + rotName + ".txt");
            if (!File.Exists(file))
                return;
            string[] lines;
            try
            {
                lines = File.ReadAllLines(file);
                File.Delete(file);
            }
            catch
            {
                return;
            }
            try
            {
                dynamic r = repo;
                foreach (string line in lines)
                    if (line.Trim().Length > 0)
                        r.WriteOutput(OutputTab, line, 0);
            }
            catch
            {
            }
        }

        public void Start(string connection, object repository)
        {
            repo = repository;
            try
            {
                dynamic r = repository;
                r.CreateOutputTab(OutputTab);
                r.EnsureOutputVisible(OutputTab);
            }
            catch
            {
            }
            Log.Write("Start: " + (connection.Length > 80 ? connection.Substring(0, 80) : connection));
            // The file name, or the database part of a connection (never the password).
            project = connection.Contains(" --- ") ? connection.Substring(0, connection.IndexOf(" --- ")) : Path.GetFileName(connection);
            Stop("");
            note.Text = L10n.Get("Starting Loonar Compass…");
            // Compass must write through THIS EA window, not the first EA found in the system: the repository is
            // registered in the Running Object Table under a name with this EA's process id.
            rotName = "LoonarCompass.EA." + Process.GetCurrentProcess().Id;
            Revoke();
            try
            {
                IRunningObjectTable rot;
                IMoniker moniker;
                GetRunningObjectTable(0, out rot);
                CreateItemMoniker("!", rotName, out moniker);
                rotCookie = rot.Register(1, repository, moniker);
            }
            catch (Exception e)
            {
                Log.Write("ROT registration failed: " + e.Message);
            }
            process = Process.Start(new ProcessStartInfo(Exe(), Arguments(connection, rotName)) { UseShellExecute = false });
            started = DateTime.Now;
            finder.Start();
        }

        /// <summary>Closes Compass (it saves its conversation on a normal close) and shows a note in the tab.</summary>
        public bool Running
        {
            get { return process != null && child != IntPtr.Zero; }
        }

        /// <summary>Hands a reference to Compass (it takes it into the message field) and brings the Compass tab forward.</summary>
        public void Send(string reference)
        {
            if (rotName == null)
                return;
            File.AppendAllText(Path.Combine(Path.GetTempPath(), "loonar-ea-insert-" + rotName + ".txt"), reference + Environment.NewLine);
            GiveKeyboard();
        }

        void Revoke()
        {
            if (rotCookie == 0)
                return;
            try
            {
                IRunningObjectTable rot;
                GetRunningObjectTable(0, out rot);
                rot.Revoke(rotCookie);
            }
            catch
            {
            }
            rotCookie = 0;
        }

        public void Stop(string message)
        {
            if (process != null)
                Out(DateTime.Now.ToString("HH:mm:ss") + "  " + L10n.Get("Loonar Compass closed"));
            Log.Write("Stop, process running: " + (process != null));
            finder.Stop();
            Process running = process;
            process = null;
            if (attached != 0)
            {
                AttachThreadInput(GetCurrentThreadId(), attached, false);
                attached = 0;
            }
            try
            {
                if (running != null && !running.HasExited)
                {
                    if (child != IntPtr.Zero)
                        PostMessage(child, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
                    if (!running.WaitForExit(5000))
                        running.Kill();
                }
            }
            catch
            {
            }
            child = IntPtr.Zero;
            Revoke();
            finder.Interval = 200;
            note.Text = message;
            note.Visible = true;
        }

        string project = "";

        string ProjectName()
        {
            return project.Length > 120 ? project.Substring(0, 120) : project;
        }

        void Dock_()
        {
            ForwardOutput();
            if (process == null)
                return;
            process.Refresh();
            if (process.HasExited)
            {
                Log.Write("Compass exited");
                finder.Stop();
                note.Text = L10n.Get("Loonar Compass was closed.");
                child = IntPtr.Zero;
                return;
            }
            // A window Compass replaced (e.g. after switching the graphics renderer) is docked again.
            IntPtr main = process.MainWindowHandle;
            // A docked window is a child, so a visible top-level window of Compass is a new one (Compass replaced its
            // window after start): that one goes into the panel.
            if (child != IntPtr.Zero && (!IsWindow(child) || (main != IntPtr.Zero && main != child && IsWindowVisible(main))))
            {
                Log.Write("Docked window is gone, looking again");
                child = IntPtr.Zero;
                finder.Interval = 200;
            }
            if (child != IntPtr.Zero)
            {
                FollowPanel();
                // EA may re-create the tab's window (docking, floating): Compass goes back into it.
                if (GetParent(child) != Handle)
                {
                    Log.Write("Compass window left the tab, docking again");
                    SetParent(child, Handle);
                    Fit();
                }
                return;
            }
            IntPtr window = process.MainWindowHandle;
            // Docked only when the same visible window has been there for a second (not a window about to be replaced).
            if (window != IntPtr.Zero && IsWindowVisible(window) && window != candidate)
            {
                candidate = window;
                candidateSince = DateTime.Now;
                return;
            }
            if (window != IntPtr.Zero && (DateTime.Now - candidateSince).TotalMilliseconds < 1000)
                return;
            if (window == IntPtr.Zero || !IsWindowVisible(window))
            {
                if ((DateTime.Now - started).TotalSeconds > 60)
                {
                    finder.Stop();
                    note.Text = L10n.Get("Loonar Compass did not open its window.");
                }
                return;
            }
            child = window;
            Out(DateTime.Now.ToString("HH:mm:ss") + "  " + L10n.Get("Loonar Compass is working with the project") + ": " + ProjectName());
            long style = GetWindowLongPtr(child, GWL_STYLE).ToInt64();
            style = (style & ~(WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU)) | WS_CHILD;
            SetWindowLongPtr(child, GWL_STYLE, new IntPtr(style));
            SetParent(child, Handle);
            // A window of another process gets the keyboard only when both UI threads share their input state.
            GiveKeyboard();
            note.Visible = false;
            Fit();
            // The timer keeps watching whether Compass is still running and follows the size of EA's panel.
            finder.Interval = 400;
        }

        /// <summary>EA clips the add-in control to its panel instead of resizing it when the panel gets narrower: the
        /// control (and Compass in it) follows the size of the window EA put it in.</summary>
        void FollowPanel()
        {
            IntPtr host = GetParent(Handle);
            RECT area;
            if (host == IntPtr.Zero || !GetClientRect(host, out area))
                return;
            int w = area.Right - area.Left, h = area.Bottom - area.Top;
            // The part actually visible: EA's containers (tabbed panels) may be larger than what they show, so the
            // control is limited to the intersection of all its parents' client areas, up to EA's main window.
            RECT own;
            if (GetWindowRect(Handle, out own))
            {
                int right = int.MaxValue, bottom = int.MaxValue;
                for (IntPtr p = host; p != IntPtr.Zero; p = GetParent(p))
                {
                    RECT c;
                    if (!GetClientRect(p, out c))
                        break;
                    POINT corner = new POINT { X = c.Right, Y = c.Bottom };
                    ClientToScreen(p, ref corner);
                    right = Math.Min(right, corner.X);
                    bottom = Math.Min(bottom, corner.Y);
                }
                if (right != int.MaxValue)
                {
                    w = Math.Min(w, right - own.Left);
                    h = Math.Min(h, bottom - own.Top);
                }
            }
            if (w > 0 && h > 0 && (w != Width || h != Height))
            {
                SetBounds(0, 0, w, h);
                Fit();
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        struct RECT { public int Left, Top, Right, Bottom; }
        [StructLayout(LayoutKind.Sequential)]
        struct POINT { public int X, Y; }
        [DllImport("user32.dll")]
        static extern bool GetWindowRect(IntPtr window, out RECT rect);
        [DllImport("user32.dll")]
        static extern bool ClientToScreen(IntPtr window, ref POINT point);
        [DllImport("user32.dll")]
        static extern bool GetClientRect(IntPtr window, out RECT rect);

        void Fit()
        {
            if (child != IntPtr.Zero)
                MoveWindow(child, 0, 0, ClientSize.Width, ClientSize.Height, true);
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            if (child != IntPtr.Zero)
            {
                SetParent(child, Handle);
                Fit();
            }
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            Fit();
        }

        /// <summary>A click inside the Compass window (reported to the tab as WM_PARENTNOTIFY) gives it the keyboard:
        /// a window of another process does not take the focus by itself when clicked.</summary>
        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WM_PARENTNOTIFY && child != IntPtr.Zero)
            {
                int ev = m.WParam.ToInt32() & 0xFFFF;
                if (ev == WM_LBUTTONDOWN || ev == WM_RBUTTONDOWN || ev == WM_MBUTTONDOWN)
                    GiveKeyboard();
            }
            base.WndProc(ref m);
        }

        /// <summary>Joins the input of EA's and Compass's UI threads again and gives Compass the keyboard. EA hides and
        /// shows the panel when the user switches docked windows (e.g. Notes), which may leave the input detached.</summary>
        void GiveKeyboard()
        {
            if (child == IntPtr.Zero)
                return;
            // Join the input of EA's and Compass's threads again (EA may drop it when panels are switched) and move
            // the focus. Only the add-in joins them: Compass itself only sets the focus.
            uint pid;
            uint thread = GetWindowThreadProcessId(child, out pid);
            if (attached != 0)
                AttachThreadInput(GetCurrentThreadId(), attached, false);
            attached = thread;
            AttachThreadInput(GetCurrentThreadId(), attached, true);
            SetFocus(child);
        }

        protected override void OnVisibleChanged(EventArgs e)
        {
            base.OnVisibleChanged(e);
            if (Visible)
                GiveKeyboard();
        }

        /// <summary>A click in the tab gives the Compass window the keyboard focus.</summary>
        protected override void OnGotFocus(EventArgs e)
        {
            base.OnGotFocus(e);
            GiveKeyboard();
        }

        protected override void Dispose(bool disposing)
        {
            Stop("");
            base.Dispose(disposing);
        }

        const int GWL_STYLE = -16;
        const long WS_CHILD = 0x40000000L, WS_POPUP = 0x80000000L, WS_CAPTION = 0x00C00000L, WS_THICKFRAME = 0x00040000L;
        const long WS_MINIMIZEBOX = 0x00020000L, WS_MAXIMIZEBOX = 0x00010000L, WS_SYSMENU = 0x00080000L;
        const uint WM_CLOSE = 0x0010;
        const int WM_PARENTNOTIFY = 0x0210, WM_LBUTTONDOWN = 0x0201, WM_RBUTTONDOWN = 0x0204, WM_MBUTTONDOWN = 0x0207;

        [DllImport("user32.dll", SetLastError = true)]
        static extern IntPtr SetParent(IntPtr child, IntPtr parent);
        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
        static extern IntPtr GetWindowLongPtr(IntPtr window, int index);
        [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
        static extern IntPtr SetWindowLongPtr(IntPtr window, int index, IntPtr value);
        [DllImport("user32.dll")]
        static extern bool MoveWindow(IntPtr window, int x, int y, int width, int height, bool repaint);
        [DllImport("ole32.dll")]
        static extern int GetRunningObjectTable(int reserved, out IRunningObjectTable rot);
        [DllImport("ole32.dll", CharSet = CharSet.Unicode)]
        static extern int CreateItemMoniker(string delimiter, string item, out IMoniker moniker);
        [DllImport("user32.dll")]
        static extern IntPtr GetParent(IntPtr window);
        [DllImport("user32.dll")]
        static extern bool IsWindow(IntPtr window);
        [DllImport("user32.dll")]
        static extern bool IsWindowVisible(IntPtr window);
        [DllImport("user32.dll")]
        static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
        [DllImport("kernel32.dll")]
        static extern uint GetCurrentThreadId();
        [DllImport("user32.dll")]
        static extern bool AttachThreadInput(uint from, uint to, bool attach);
        [DllImport("user32.dll")]
        static extern IntPtr SetFocus(IntPtr window);
        [DllImport("user32.dll")]
        static extern bool PostMessage(IntPtr window, uint message, IntPtr w, IntPtr l);
    }

    /// <summary>Diagnostic log of the add-in: %TEMP%\loonar-ea-addin.log (kept small).</summary>
    static class Log
    {
        public static void Write(string text)
        {
            try
            {
                string file = Path.Combine(Path.GetTempPath(), "loonar-ea-addin.log");
                if (File.Exists(file) && new FileInfo(file).Length > 200000)
                    File.Delete(file);
                File.AppendAllText(file, DateTime.Now.ToString("HH:mm:ss.fff") + " " + text + Environment.NewLine);
            }
            catch
            {
            }
        }
    }

    /// <summary>Menu texts in the language of Compass (ui.language in its settings), Polish by default.</summary>
    static class L10n
    {
        static string language;

        public static string Get(string english)
        {
            string lang = Language();
            if (lang == "en")
                return english;
            switch (english)
            {
                case "Open Compass in EA": return lang == "de" ? "Compass in EA öffnen" : lang == "fr" ? "Ouvrir Compass dans EA" : "Otwórz Compass w EA";
                case "Open Compass in its own window": return lang == "de" ? "Compass in eigenem Fenster öffnen" : lang == "fr" ? "Ouvrir Compass dans sa propre fenêtre" : "Otwórz Compass w osobnym oknie";
                case "Starting Loonar Compass…": return lang == "de" ? "Loonar Compass wird gestartet…" : lang == "fr" ? "Démarrage de Loonar Compass…" : "Uruchamianie Loonar Compass…";
                case "Open a project in Enterprise Architect to work with Loonar Compass.": return lang == "de" ? "Öffnen Sie ein Projekt in Enterprise Architect, um mit Loonar Compass zu arbeiten." : lang == "fr" ? "Ouvrez un projet dans Enterprise Architect pour travailler avec Loonar Compass." : "Otwórz projekt w Enterprise Architect, aby pracować z Loonar Compass.";
                case "Add to the Compass conversation": return lang == "de" ? "Zur Compass-Unterhaltung hinzufügen" : lang == "fr" ? "Ajouter à la conversation Compass" : "Dodaj do rozmowy w Compass";
                case "Element": return lang == "de" ? "Element" : lang == "fr" ? "Élément" : "Element";
                case "Package": return lang == "de" ? "Paket" : lang == "fr" ? "Paquetage" : "Pakiet";
                case "Diagram": return lang == "de" ? "Diagramm" : lang == "fr" ? "Diagramme" : "Diagram";
                case "Connector": return lang == "de" ? "Beziehung" : lang == "fr" ? "Relation" : "Relacja";
                case "Attribute": return lang == "de" ? "Attribut" : lang == "fr" ? "Attribut" : "Atrybut";
                case "Operation": return lang == "de" ? "Operation" : lang == "fr" ? "Opération" : "Operacja";
                case "Loonar Compass is working with the project": return lang == "de" ? "Loonar Compass arbeitet mit dem Projekt" : lang == "fr" ? "Loonar Compass travaille sur le projet" : "Loonar Compass pracuje na projekcie";
                case "Loonar Compass closed": return lang == "de" ? "Loonar Compass geschlossen" : lang == "fr" ? "Loonar Compass fermé" : "Loonar Compass zamknięty";
                case "Loonar Compass was closed.": return lang == "de" ? "Loonar Compass wurde geschlossen." : lang == "fr" ? "Loonar Compass a été fermé." : "Loonar Compass został zamknięty.";
                case "Loonar Compass did not open its window.": return lang == "de" ? "Loonar Compass hat sein Fenster nicht geöffnet." : lang == "fr" ? "Loonar Compass n'a pas ouvert sa fenêtre." : "Loonar Compass nie otworzył okna.";
            }
            return english;
        }

        static string Language()
        {
            if (language != null)
                return language;
            language = "pl";
            try
            {
                string props = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Loonar Compass", "app.properties");
                foreach (string line in File.ReadAllLines(props))
                    if (line.StartsWith("ui.language="))
                        language = line.Substring("ui.language=".Length).Trim();
            }
            catch
            {
            }
            return language;
        }
    }
}
