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

        /// <summary>EA has started: the Compass window is there from the start, with a note until a project is open
        /// (and it starts Compass at once when EA was opened with a project).</summary>
        public void EA_OnPostInitialized(object repository)
        {
            Log.Write("PostInitialized, project open: " + !string.IsNullOrEmpty(Connection(repository)));
            try
            {
                Window(repository);
                if (!string.IsNullOrEmpty(Connection(repository)) && !current.Started)
                    current.Start(Connection(repository), repository);
            }
            catch (Exception e)
            {
                Log.Write("PostInitialized: " + e.Message);
            }
        }

        /// <summary>The Compass window in EA (a dockable window like Properties; EA remembers where the user put it),
        /// created once, with the "no project" note until Compass starts in it.</summary>
        static CompassTab Window(object repository)
        {
            dynamic repo = repository;
            if (current == null || current.IsDisposed)
            {
                CompassTab tab = repo.AddWindow("Loonar Compass", "LoonarCompass.Tab") as CompassTab;
                if (tab == null)
                    throw new InvalidOperationException("EA did not create the Compass window.");
                current = tab;
                tab.Stop(L10n.Get("No project is open: open a .qea file or a repository to work with Compass."));
            }
            repo.ShowAddinWindow("Loonar Compass");
            return current;
        }

        /// <summary>A new element in EA: for event agents (Compass in the tab). Nothing is changed here.</summary>
        public bool EA_OnPostNewElement(object repository, object info)
        {
            Pass("new_element", info, "ElementID");
            return false;
        }

        /// <summary>A new diagram in EA: for event agents.</summary>
        public bool EA_OnPostNewDiagram(object repository, object info)
        {
            Pass("new_diagram", info, "DiagramID");
            return false;
        }

        /// <summary>A relationship drawn in EA: Compass checks it by the model rules (System Output). Nothing is changed here.</summary>
        public bool EA_OnPostNewConnector(object repository, object info)
        {
            Pass("connector", info, "ConnectorID");
            return false;
        }

        /// <summary>An element, relationship, diagram or package changed in EA (name, stereotype, tagged values…): Compass
        /// checks it by the model rules and passes it to event agents. New elements are checked after their first edit
        /// (EA creates them as "Class1" and the name is typed next).</summary>
        public void EA_OnNotifyContextItemModified(object repository, string guid, int type)
        {
            if (current == null || current.IsDisposed)
                return;
            try
            {
                dynamic r = repository;
                switch (type)
                {
                    case 4: // otElement
                        current.Event("element:" + r.GetElementByGuid(guid).ElementID);
                        break;
                    case 7: // otConnector
                        current.Event("connector_changed:" + r.GetConnectorByGuid(guid).ConnectorID);
                        break;
                    case 8: // otDiagram
                        current.Event("diagram_changed:" + r.GetDiagramByGuid(guid).DiagramID);
                        break;
                    case 5: // otPackage
                        current.Event("package_changed:" + r.GetPackageByGuid(guid).PackageID);
                        break;
                }
            }
            catch (Exception e)
            {
                Log.Write("ContextItemModified: " + e.Message);
            }
        }

        public bool EA_OnPostNewPackage(object repository, object info) { Pass("package_created", info, "PackageID"); return false; }
        public bool EA_OnPostNewAttribute(object repository, object info) { Pass("attribute_created", info, "AttributeID"); return false; }
        public bool EA_OnPostNewMethod(object repository, object info) { Pass("method_created", info, "MethodID"); return false; }
        public bool EA_OnPostNewGlossaryTerm(object repository, object info) { Pass("glossary_term_created", info, "GlossaryID"); return false; }

        // Before a deletion EA asks the add-in: event agents get the event, and a guard (an agent in "guard" mode in
        // Compass) can stop it. True = EA goes ahead.
        public bool EA_OnPreDeleteElement(object repository, object info) { return Ask("element_deleting", info, "ElementID"); }
        public bool EA_OnPreDeleteConnector(object repository, object info) { return Ask("connector_deleting", info, "ConnectorID"); }
        public bool EA_OnPreDeleteDiagram(object repository, object info) { return Ask("diagram_deleting", info, "DiagramID"); }
        public bool EA_OnPreDeletePackage(object repository, object info) { return Ask("package_deleting", info, "PackageID"); }
        public bool EA_OnPreDeleteAttribute(object repository, object info) { return Ask("attribute_deleting", info, "AttributeID"); }
        public bool EA_OnPreDeleteMethod(object repository, object info) { return Ask("method_deleting", info, "MethodID"); }
        public bool EA_OnPreDeleteGlossaryTerm(object repository, object info) { return Ask("glossary_term_deleting", info, "GlossaryID"); }

        static bool Ask(string kind, object info, string property)
        {
            if (current == null || current.IsDisposed)
                return true;
            try
            {
                dynamic properties = info;
                string line = kind + ":" + properties.Get(property).Value;
                current.Event(line);
                return current.Guard(kind, line);
            }
            catch (Exception e)
            {
                Log.Write(kind + " guard: " + e.Message);
                return true;
            }
        }

        static void Pass(string kind, object info, string property)
        {
            if (current == null || current.IsDisposed)
                return;
            try
            {
                dynamic properties = info;
                current.Event(kind + ":" + properties.Get(property).Value);
            }
            catch (Exception e)
            {
                Log.Write(kind + " event: " + e.Message);
            }
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
                CompassTab tab = Window(repository);
                if (!tab.Started && !string.IsNullOrEmpty(connection))
                    tab.Start(connection, repository);
            }
            catch (Exception e)
            {
                MessageBox.Show(e.Message, "Loonar Compass", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>The open Compass tab: it always works with the project open in this EA window.</summary>
        static CompassTab current;

        /// <summary>A project opened in EA: Compass starts in its window by itself (or switches to the new project).</summary>
        public void EA_FileOpen(object repository)
        {
            Log.Write("FileOpen, tab open: " + (current != null && !current.IsDisposed));
            try
            {
                Window(repository).Start(Connection(repository), repository);
            }
            catch (Exception e)
            {
                Log.Write("FileOpen: " + e.Message);
            }
        }

        /// <summary>The project closed in EA: Compass does not work without one, so it closes and the window shows the
        /// "no project" note again.</summary>
        public void EA_FileClose(object repository)
        {
            Log.Write("FileClose");
            if (current != null && !current.IsDisposed)
                current.Stop(L10n.Get("No project is open: open a .qea file or a repository to work with Compass."));
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

        /// <summary>Path of LoonarCompass.exe: next to the add-in (installation); LOONAR_COMPASS_EXE only in a debug build
        /// of the add-in (development): a released add-in never starts a program named by the environment.</summary>
        public static string Exe()
        {
#if DEBUG
            string fromEnv = Environment.GetEnvironmentVariable("LOONAR_COMPASS_EXE");
            if (!string.IsNullOrEmpty(fromEnv) && File.Exists(fromEnv))
                return fromEnv;
#endif
            string dir = Path.GetDirectoryName(new Uri(Assembly.GetExecutingAssembly().CodeBase).LocalPath);
            string exe = Path.Combine(dir, "LoonarCompass.exe");
            if (!File.Exists(exe))
                throw new FileNotFoundException("Loonar Compass was not found next to the add-in: " + exe);
            return exe;
        }

        /// <summary>Arguments of Compass: the project (in a file: a database connection string has quotes and semicolons)
        /// and, for the tab, the name under which this EA's repository is in the Running Object Table.</summary>
        public static string Arguments(string connection, string rotName, out string file)
        {
            // The connection goes in a file: a database connection string has quotes and semicolons. Compass deletes
            // the file as soon as it has read it; the add-in deletes it when Compass does not start.
            file = Path.Combine(Path.GetTempPath(), "loonar-ea-model-" + Guid.NewGuid().ToString("N") + ".txt");
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
                    {
                        Log.Write("output: " + line);
                        r.WriteOutput(OutputTab, line, 0);
                    }
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
            // The file name, or the database part of a connection (never the password).
            project = connection.Contains(" --- ") ? connection.Substring(0, connection.IndexOf(" --- ")) : Path.GetFileName(connection);
            Log.Write("Start: " + project);
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
            string exe = Exe();
            string modelFile;
            string arguments = Arguments(connection, rotName, out modelFile);
            try
            {
                // Started in its own folder (not in EA's current folder, e.g. the folder of an opened model).
                process = Process.Start(new ProcessStartInfo(exe, arguments) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(exe) });
            }
            catch
            {
                try { File.Delete(modelFile); } catch { }
                throw;
            }
            started = DateTime.Now;
            finder.Start();
        }

        /// <summary>Closes Compass (it saves its conversation on a normal close) and shows a note in the tab.</summary>
        public bool Running
        {
            get { return process != null && child != IntPtr.Zero; }
        }

        /// <summary>Compass was started in this window (it may still be opening its window).</summary>
        public bool Started
        {
            get { return process != null; }
        }

        /// <summary>A model event for the live check in Compass (handed over in a file, like the references).</summary>
        public void Event(string line)
        {
            if (rotName == null || process == null)
                return;
            try
            {
                File.AppendAllText(Path.Combine(Path.GetTempPath(), "loonar-ea-event-" + rotName + ".txt"), line + Environment.NewLine);
            }
            catch
            {
            }
        }

        /// <summary>Asks Compass whether a deletion may go ahead: only when a guard watches this event (Compass lists them in
        /// a file). Waits at most 40 seconds; no answer = the deletion is stopped, so a protected object is never lost
        /// because Compass did not answer.</summary>
        public bool Guard(string kind, string line)
        {
            if (rotName == null || process == null)
                return true;
            string temp = Path.GetTempPath();
            try
            {
                string guarded = Path.Combine(temp, "loonar-ea-guards-" + rotName + ".txt");
                if (!File.Exists(guarded) || Array.IndexOf(File.ReadAllLines(guarded), kind) < 0)
                    return true;
            }
            catch
            {
                return true;
            }
            string id = Guid.NewGuid().ToString("N");
            string request = Path.Combine(temp, "loonar-ea-guard-" + rotName + "-" + id + ".req");
            string response = Path.Combine(temp, "loonar-ea-guard-" + rotName + "-" + id + ".res");
            Cursor previous = Cursor.Current;
            try
            {
                Cursor.Current = Cursors.WaitCursor;
                File.WriteAllText(request, line);
                DateTime until = DateTime.Now.AddSeconds(40);
                while (DateTime.Now < until)
                {
                    if (File.Exists(response))
                    {
                        string[] answer = File.ReadAllLines(response);
                        File.Delete(response);
                        if (answer.Length > 0 && answer[0] == "block")
                        {
                            string reason = answer.Length > 1 ? answer[1] : "";
                            Cursor.Current = previous;
                            MessageBox.Show(L10n.Get("Loonar Compass stopped this deletion:") + "\n\n" + reason, "Loonar Compass", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                            return false;
                        }
                        return true;
                    }
                    // A pumping wait, not Sleep: Compass reads the object being deleted from this EA (Repository.SQLQuery)
                    // while EA waits here; the calls of COM are served during the wait, the user's input is not.
                    System.Threading.Thread.CurrentThread.Join(100);
                }
                Out(DateTime.Now.ToString("HH:mm:ss") + "  " + L10n.Get("No decision of the guard in time: the deletion is stopped."));
                Cursor.Current = previous;
                MessageBox.Show(L10n.Get("No decision of the guard in time: the deletion is stopped.") + "\n\n" + L10n.Get("Try again in a moment. If it happens again, check Loonar Compass (it must be open in EA)."), "Loonar Compass", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return false;
            }
            catch (Exception e)
            {
                Log.Write("Guard: " + e.Message);
                return true;
            }
            finally
            {
                Cursor.Current = previous;
                try { File.Delete(request); } catch { }
            }
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
                try
                {
                    FollowPanel();
                }
                catch (Exception e)
                {
                    Log.Write("FollowPanel failed: " + e.Message);
                }
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
                // Up the real parents only (GetAncestor stops at the top-level window; GetParent would go on to an
                // owner window, often hidden and empty, and the size would come out as zero).
                IntPtr desktop = GetDesktopWindow();
                for (IntPtr p = host; p != IntPtr.Zero && p != desktop; p = GetAncestor(p, 1))
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
            int cw, ch;
            if (w > 0 && h > 0 && OwnSize(out cw, out ch) && (w != cw || h != ch))
                MoveWindow(Handle, 0, 0, w, h, true);
            // EA resizes this control itself (without OnResize reaching Compass): Compass always gets its size.
            RECT inner;
            int ow, oh;
            bool sized = OwnSize(out ow, out oh);
            bool got = GetWindowRect(child, out inner);
            if (sized)
                Fit();
        }

        [StructLayout(LayoutKind.Sequential)]
        struct RECT { public int Left, Top, Right, Bottom; }
        [StructLayout(LayoutKind.Sequential)]
        struct POINT { public int X, Y; }
        [DllImport("user32.dll")]
        static extern bool GetWindowRect(IntPtr window, out RECT rect);
        [DllImport("user32.dll")]
        static extern IntPtr GetAncestor(IntPtr window, uint flags);
        [DllImport("user32.dll")]
        static extern IntPtr GetDesktopWindow();
        [DllImport("user32.dll")]
        static extern bool ClientToScreen(IntPtr window, ref POINT point);
        [DllImport("user32.dll")]
        static extern bool GetClientRect(IntPtr window, out RECT rect);

        /// <summary>The real size of this control's window (WinForms' ClientSize lags when EA resizes it).</summary>
        bool OwnSize(out int w, out int h)
        {
            RECT c;
            w = h = 0;
            if (!IsHandleCreated || !GetClientRect(Handle, out c))
                return false;
            w = c.Right - c.Left;
            h = c.Bottom - c.Top;
            return w > 0 && h > 0;
        }

        /// <summary>Every Compass window in this control gets its size: Compass may put more than one window here at
        /// start, and the one drawing the chat is not always the one docked first.</summary>
        void Fit()
        {
            int w, h;
            if (!OwnSize(out w, out h))
                return;
            for (IntPtr c = GetWindow(Handle, GW_CHILD); c != IntPtr.Zero; c = GetWindow(c, GW_HWNDNEXT))
            {
                if (c == note.Handle)
                    continue;
                RECT r;
                if (GetWindowRect(c, out r) && (r.Right - r.Left != w || r.Bottom - r.Top != h))
                    MoveWindow(c, 0, 0, w, h, true);
            }
        }

        const uint GW_CHILD = 5, GW_HWNDNEXT = 2;
        [DllImport("user32.dll")]
        static extern IntPtr GetWindow(IntPtr window, uint command);

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
                case "No project is open: open a .qea file or a repository to work with Compass.": return lang == "de" ? "Kein Projekt geöffnet: Öffnen Sie eine .qea-Datei oder ein Repository, um mit Compass zu arbeiten." : lang == "fr" ? "Aucun projet ouvert : ouvrez un fichier .qea ou un référentiel pour travailler avec Compass." : "Brak otwartego projektu: otwórz plik .qea albo repozytorium, aby pracować z Compass.";
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
                case "Loonar Compass stopped this deletion:": return lang == "de" ? "Loonar Compass hat dieses Löschen gestoppt:" : lang == "fr" ? "Loonar Compass a arrêté cette suppression :" : "Loonar Compass zatrzymał to usunięcie:";
                case "No decision of the guard in time: the deletion is stopped.": return lang == "de" ? "Keine Entscheidung des Wächters rechtzeitig: das Löschen wird gestoppt." : lang == "fr" ? "Pas de décision du gardien à temps : la suppression est arrêtée." : "Strażnik nie zdecydował na czas: usunięcie zostaje wstrzymane.";
                case "Try again in a moment. If it happens again, check Loonar Compass (it must be open in EA).": return lang == "de" ? "Versuchen Sie es gleich noch einmal. Passiert es wieder, prüfen Sie Loonar Compass (es muss in EA geöffnet sein)." : lang == "fr" ? "Réessayez dans un instant. Si cela se reproduit, vérifiez Loonar Compass (il doit être ouvert dans EA)." : "Spróbuj za chwilę. Jeśli to się powtórzy, sprawdź Loonar Compass (musi być otwarty w EA).";
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
