using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;

namespace WinCtl
{
    internal record WindowInfo(
        string Title, string Process, int Pid,
        int Width, int Height, int X, int Y,
        string State, int Monitor, bool Topmost);

    internal record MonitorData(int Number, string Device,
        int Width, int Height, int X, int Y,
        int WorkLeft, int WorkTop, int WorkWidth, int WorkHeight, bool Primary);

    [JsonSerializable(typeof(WindowInfo))]
    [JsonSerializable(typeof(List<WindowInfo>))]
    [JsonSerializable(typeof(MonitorData))]
    [JsonSerializable(typeof(List<MonitorData>))]
    [JsonSourceGenerationOptions(WriteIndented = true)]
    internal partial class AppJsonContext : JsonSerializerContext { }

    class Program
    {
        #region Win32 Imports

        [DllImport("user32.dll")]
        private static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter,
            int X, int Y, int cx, int cy, uint uFlags);

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll")]
        private static extern int GetWindowLong(IntPtr hWnd, int nIndex);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

        [DllImport("user32.dll")]
        private static extern bool IsZoomed(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool IsIconic(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint dwFlags);

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFOEX lpmi);

        [DllImport("user32.dll")]
        private static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip,
            EnumMonitorsProc lpfnEnum, IntPtr dwData);

        private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
        private delegate bool EnumMonitorsProc(IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData);

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT { public int Left, Top, Right, Bottom; }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        private struct MONITORINFOEX
        {
            public int cbSize;
            public RECT rcMonitor;
            public RECT rcWork;
            public uint dwFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
            public string szDevice;
        }

        private static readonly IntPtr HWND_TOPMOST = new(-1);
        private static readonly IntPtr HWND_NOTOPMOST = new(-2);
        private const uint SWP_NOMOVE = 0x0002;
        private const uint SWP_NOSIZE = 0x0001;
        private const uint SWP_NOZORDER = 0x0004;
        private const uint MONITOR_DEFAULTTONEAREST = 0x00000002;
        private const int SW_MAXIMIZE = 3;
        private const int SW_MINIMIZE = 6;
        private const int SW_RESTORE = 9;
        private const int GWL_EXSTYLE = -20;
        private const int WS_EX_TOPMOST = 0x00000008;

        #endregion

        static int Main(string[] args)
        {
            if (args.Length == 0 || args[0] == "--help")
            {
                PrintUsage();
                return 0;
            }

            switch (args[0])
            {
                case "list":
                    return RunList(args);
                case "monitor-info":
                    return RunMonitorInfo();
                case "wait-for":
                    return RunWaitFor(args);
            }

            string? title = null;
            int? pid = null;
            int? width = null, height = null, x = null, y = null;
            int? monitor = null;
            string? snap = null;
            bool maximize = false, minimize = false, restore = false, center = false;
            bool? topmost = null;
            bool info = false;

            for (int i = 0; i < args.Length; i++)
            {
                switch (args[i])
                {
                    case "--title" or "-t":
                        title = NextArg(args, ref i, "--title");
                        break;
                    case "--pid" or "-p":
                        pid = int.Parse(NextArg(args, ref i, "--pid"));
                        break;
                    case "--width" or "-w":
                        width = int.Parse(NextArg(args, ref i, "--width"));
                        break;
                    case "--height" or "-h":
                        height = int.Parse(NextArg(args, ref i, "--height"));
                        break;
                    case "--x":
                        x = int.Parse(NextArg(args, ref i, "--x"));
                        break;
                    case "--y":
                        y = int.Parse(NextArg(args, ref i, "--y"));
                        break;
                    case "--monitor" or "-m":
                        monitor = int.Parse(NextArg(args, ref i, "--monitor"));
                        break;
                    case "--snap":
                        snap = NextArg(args, ref i, "--snap");
                        break;
                    case "--maximize":
                        maximize = true;
                        break;
                    case "--minimize":
                        minimize = true;
                        break;
                    case "--restore":
                        restore = true;
                        break;
                    case "--center":
                        center = true;
                        break;
                    case "--topmost":
                        topmost = true;
                        break;
                    case "--no-topmost":
                        topmost = false;
                        break;
                    case "--info":
                        info = true;
                        break;
                    default:
                        Console.Error.WriteLine($"Unknown argument: {args[i]}");
                        return 1;
                }
            }

            if (title == null && pid == null)
            {
                Console.Error.WriteLine("Error: --title or --pid is required");
                return 1;
            }

            IntPtr hwnd;
            if (pid != null)
            {
                hwnd = FindWindowByPid(pid.Value);
                if (hwnd == IntPtr.Zero)
                {
                    Console.Error.WriteLine($"No visible window found for PID {pid}");
                    return 1;
                }
            }
            else
            {
                hwnd = FindWindowByPartialTitle(title!);
                if (hwnd == IntPtr.Zero)
                {
                    Console.Error.WriteLine($"Window not found: \"{title}\"");
                    Console.Error.WriteLine("Use 'list' to see available windows.");
                    return 1;
                }
            }

            if (info)
            {
                var wi = GetWindowInfo(hwnd);
                Console.WriteLine(JsonSerializer.Serialize(wi, AppJsonContext.Default.WindowInfo));
                return 0;
            }

            bool hasAction = width != null || height != null || x != null || y != null
                || maximize || minimize || restore || center || topmost != null
                || snap != null || monitor != null;

            if (!hasAction)
            {
                Console.Error.WriteLine("Error: no action specified. Use --help for usage.");
                return 1;
            }

            if (restore || (!minimize && !maximize))
            {
                if (IsIconic(hwnd) || IsZoomed(hwnd))
                    ShowWindow(hwnd, SW_RESTORE);
            }

            if (maximize)
            {
                SetForegroundWindow(hwnd);
                ShowWindow(hwnd, SW_MAXIMIZE);
                PrintDone(hwnd, "Maximized");
                return 0;
            }
            if (minimize)
            {
                ShowWindow(hwnd, SW_MINIMIZE);
                PrintDone(hwnd, "Minimized");
                return 0;
            }

            if (monitor != null)
            {
                var monitors = GetMonitors();
                if (monitor < 1 || monitor > monitors.Count)
                {
                    Console.Error.WriteLine($"Monitor {monitor} not found. Available: 1-{monitors.Count}");
                    return 1;
                }
                var target = monitors[monitor.Value - 1];
                GetWindowRect(hwnd, out RECT wr);
                int ww = wr.Right - wr.Left;
                int wh = wr.Bottom - wr.Top;
                var curWork = GetCurrentMonitorWork(hwnd);
                int nx = target.WorkLeft + Math.Min(wr.Left - curWork.Left, Math.Max(0, target.WorkWidth - ww));
                int ny = target.WorkTop + Math.Min(wr.Top - curWork.Top, Math.Max(0, target.WorkHeight - wh));
                SetWindowPos(hwnd, IntPtr.Zero, nx, ny, ww, wh, SWP_NOZORDER);
            }

            // Topmost before snap/resize so it applies regardless of other flags
            if (topmost != null)
            {
                IntPtr zOrder = topmost.Value ? HWND_TOPMOST : HWND_NOTOPMOST;
                SetWindowPos(hwnd, zOrder, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                PrintDone(hwnd, topmost.Value ? "Set topmost" : "Removed topmost");
                if (snap == null && width == null && height == null && x == null && y == null && !center)
                    return 0;
            }

            if (snap != null)
            {
                var work = GetCurrentMonitorWork(hwnd);
                int halfW = work.Width / 2;
                int halfH = work.Height / 2;
                switch (snap.ToLowerInvariant())
                {
                    case "left":
                        SetForegroundWindow(hwnd);
                        SetWindowPos(hwnd, IntPtr.Zero, work.Left, work.Top, halfW, work.Height, SWP_NOZORDER);
                        PrintDone(hwnd, $"Snapped left ({halfW}x{work.Height})");
                        return 0;
                    case "right":
                        SetForegroundWindow(hwnd);
                        SetWindowPos(hwnd, IntPtr.Zero, work.Left + halfW, work.Top, halfW, work.Height, SWP_NOZORDER);
                        PrintDone(hwnd, $"Snapped right ({halfW}x{work.Height})");
                        return 0;
                    case "top":
                        SetForegroundWindow(hwnd);
                        SetWindowPos(hwnd, IntPtr.Zero, work.Left, work.Top, work.Width, halfH, SWP_NOZORDER);
                        PrintDone(hwnd, $"Snapped top ({work.Width}x{halfH})");
                        return 0;
                    case "bottom":
                        SetForegroundWindow(hwnd);
                        SetWindowPos(hwnd, IntPtr.Zero, work.Left, work.Top + halfH, work.Width, halfH, SWP_NOZORDER);
                        PrintDone(hwnd, $"Snapped bottom ({work.Width}x{halfH})");
                        return 0;
                    default:
                        Console.Error.WriteLine($"Unknown snap direction: {snap}. Use left, right, top, bottom.");
                        return 1;
                }
            }

            GetWindowRect(hwnd, out RECT rect);
            int curX = rect.Left, curY = rect.Top;
            int curW = rect.Right - rect.Left, curH = rect.Bottom - rect.Top;

            int newW = width ?? curW;
            int newH = height ?? curH;
            int newX = x ?? curX;
            int newY = y ?? curY;

            if (center)
            {
                var work = GetCurrentMonitorWork(hwnd);
                newX = work.Left + (work.Width - newW) / 2;
                newY = work.Top + (work.Height - newH) / 2;
            }

            SetForegroundWindow(hwnd);
            bool ok = SetWindowPos(hwnd, IntPtr.Zero, newX, newY, newW, newH, SWP_NOZORDER);
            if (!ok)
            {
                Console.Error.WriteLine("SetWindowPos failed");
                return 1;
            }

            PrintDone(hwnd, $"Resized to {newW}x{newH} at ({newX},{newY})");
            return 0;
        }

        #region Subcommands

        private static int RunList(string[] args)
        {
            bool json = false;
            string? filter = null;

            for (int i = 1; i < args.Length; i++)
            {
                switch (args[i])
                {
                    case "--json":
                        json = true;
                        break;
                    case "--filter" or "-f":
                        filter = NextArg(args, ref i, "--filter");
                        break;
                    default:
                        Console.Error.WriteLine($"Unknown list argument: {args[i]}");
                        return 1;
                }
            }

            var windows = EnumerateWindows(filter);

            if (json)
            {
                Console.WriteLine(JsonSerializer.Serialize(windows, AppJsonContext.Default.ListWindowInfo));
            }
            else
            {
                Console.WriteLine($"{"Title",-50} {"Process",-20} {"PID",7} {"Size",-14} {"Pos",-14} {"State",-10} {"Mon",3}");
                Console.WriteLine(new string('-', 122));
                foreach (var w in windows)
                {
                    string t = w.Title.Length > 47 ? w.Title[..47] + "..." : w.Title;
                    string proc = w.Process.Length > 17 ? w.Process[..17] + "..." : w.Process;
                    Console.WriteLine($"{t,-50} {proc,-20} {w.Pid,7} {w.Width}x{w.Height,-10} ({w.X},{w.Y}){"",-4} {w.State,-10} {w.Monitor,3}");
                }
            }

            return 0;
        }

        private static int RunMonitorInfo()
        {
            var monitors = GetMonitors();
            Console.WriteLine(JsonSerializer.Serialize(monitors, AppJsonContext.Default.ListMonitorData));
            return 0;
        }

        private static int RunWaitFor(string[] args)
        {
            if (args.Length < 2)
            {
                Console.Error.WriteLine("Usage: wait-for --title <partial-title> [--timeout <seconds>]");
                return 1;
            }

            string? title = null;
            int timeoutSec = 30;

            for (int i = 1; i < args.Length; i++)
            {
                switch (args[i])
                {
                    case "--title" or "-t":
                        title = NextArg(args, ref i, "--title");
                        break;
                    case "--timeout":
                        timeoutSec = int.Parse(NextArg(args, ref i, "--timeout"));
                        break;
                    default:
                        Console.Error.WriteLine($"Unknown argument: {args[i]}");
                        return 1;
                }
            }

            if (title == null)
            {
                Console.Error.WriteLine("Error: --title is required");
                return 1;
            }

            var deadline = DateTime.UtcNow.AddSeconds(timeoutSec);
            while (DateTime.UtcNow < deadline)
            {
                var hwnd = FindWindowByPartialTitle(title);
                if (hwnd != IntPtr.Zero)
                {
                    var wi = GetWindowInfo(hwnd);
                    Console.WriteLine(JsonSerializer.Serialize(wi, AppJsonContext.Default.WindowInfo));
                    return 0;
                }
                Thread.Sleep(500);
            }

            Console.Error.WriteLine($"Timed out after {timeoutSec}s waiting for window: \"{title}\"");
            return 1;
        }

        #endregion

        #region Helpers

        private static string NextArg(string[] args, ref int index, string flag)
        {
            index++;
            if (index >= args.Length)
                throw new ArgumentException($"{flag} requires a value");
            return args[index];
        }

        private static void PrintDone(IntPtr hwnd, string action)
        {
            var sb = new StringBuilder(256);
            GetWindowText(hwnd, sb, 256);
            Console.WriteLine($"{action}: \"{sb}\"");
        }

        private static void PrintUsage()
        {
            Console.WriteLine(@"winctl - Resize, move, and manage windows

Usage:
  winctl --title <partial-title> [options]
  winctl --pid <process-id> [options]
  winctl list [--json] [--filter <term>]
  winctl monitor-info
  winctl wait-for --title <partial-title> [--timeout <seconds>]

Target:
  --title, -t     Partial window title (case-insensitive)
  --pid, -p       Target window by process ID

Resize/Move:
  --width, -w     New width in pixels
  --height, -h    New height in pixels
  --x             New X position
  --y             New Y position

Window State:
  --maximize      Maximize the window
  --minimize      Minimize the window
  --restore       Restore from minimized/maximized

Layout:
  --center        Center window on its current monitor
  --snap <dir>    Snap to left, right, top, or bottom half
  --monitor, -m   Move window to monitor N (1-based)
  --topmost       Pin window above all others
  --no-topmost    Remove always-on-top

Query:
  --info          Print window details as JSON

Subcommands:
  list            List visible windows (add --json for structured output)
  monitor-info    List monitors with resolution and work area as JSON
  wait-for        Wait until a window appears (--timeout default 30s)");
        }

        private static string GetWindowState(IntPtr hwnd)
        {
            if (IsIconic(hwnd)) return "minimized";
            if (IsZoomed(hwnd)) return "maximized";
            return "normal";
        }

        private static int GetMonitorNumber(IntPtr hwnd)
        {
            var hMon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            var monitors = GetMonitorHandles();
            for (int i = 0; i < monitors.Count; i++)
            {
                if (monitors[i] == hMon) return i + 1;
            }
            return 1;
        }

        private static bool IsTopmost(IntPtr hwnd)
        {
            int exStyle = GetWindowLong(hwnd, GWL_EXSTYLE);
            return (exStyle & WS_EX_TOPMOST) != 0;
        }

        private static WindowInfo GetWindowInfo(IntPtr hwnd)
        {
            var sb = new StringBuilder(256);
            GetWindowText(hwnd, sb, 256);
            GetWindowRect(hwnd, out RECT r);
            GetWindowThreadProcessId(hwnd, out uint procId);

            string procName;
            try { procName = Process.GetProcessById((int)procId).ProcessName + ".exe"; }
            catch { procName = "unknown"; }

            return new WindowInfo(
                sb.ToString(), procName, (int)procId,
                r.Right - r.Left, r.Bottom - r.Top, r.Left, r.Top,
                GetWindowState(hwnd), GetMonitorNumber(hwnd), IsTopmost(hwnd));
        }

        private static List<WindowInfo> EnumerateWindows(string? filter)
        {
            var results = new List<WindowInfo>();

            EnumWindows((hWnd, _) =>
            {
                if (!IsWindowVisible(hWnd)) return true;

                var sb = new StringBuilder(256);
                GetWindowText(hWnd, sb, 256);
                string t = sb.ToString();
                if (string.IsNullOrWhiteSpace(t)) return true;

                if (filter != null && !t.Contains(filter, StringComparison.OrdinalIgnoreCase))
                    return true;

                results.Add(GetWindowInfo(hWnd));
                return true;
            }, IntPtr.Zero);

            return results;
        }

        private static List<MonitorData> GetMonitors()
        {
            var handles = GetMonitorHandles();
            var result = new List<MonitorData>();

            for (int i = 0; i < handles.Count; i++)
            {
                var mi = new MONITORINFOEX { cbSize = Marshal.SizeOf<MONITORINFOEX>() };
                GetMonitorInfo(handles[i], ref mi);
                result.Add(new MonitorData(
                    i + 1, mi.szDevice.TrimEnd('\0'),
                    mi.rcMonitor.Right - mi.rcMonitor.Left,
                    mi.rcMonitor.Bottom - mi.rcMonitor.Top,
                    mi.rcMonitor.Left, mi.rcMonitor.Top,
                    mi.rcWork.Left, mi.rcWork.Top,
                    mi.rcWork.Right - mi.rcWork.Left,
                    mi.rcWork.Bottom - mi.rcWork.Top,
                    (mi.dwFlags & 1) != 0));
            }

            return result;
        }

        private static List<IntPtr> GetMonitorHandles()
        {
            var handles = new List<IntPtr>();
            EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (IntPtr hMonitor, IntPtr _, ref RECT __, IntPtr ___) =>
            {
                handles.Add(hMonitor);
                return true;
            }, IntPtr.Zero);
            return handles;
        }

        private record WorkArea(int Left, int Top, int Width, int Height);

        private static WorkArea GetCurrentMonitorWork(IntPtr hwnd)
        {
            var hMon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            var mi = new MONITORINFOEX { cbSize = Marshal.SizeOf<MONITORINFOEX>() };
            GetMonitorInfo(hMon, ref mi);
            return new WorkArea(
                mi.rcWork.Left, mi.rcWork.Top,
                mi.rcWork.Right - mi.rcWork.Left,
                mi.rcWork.Bottom - mi.rcWork.Top);
        }

        private static IntPtr FindWindowByPartialTitle(string partialTitle)
        {
            IntPtr found = IntPtr.Zero;

            EnumWindows((hWnd, _) =>
            {
                if (!IsWindowVisible(hWnd)) return true;

                var sb = new StringBuilder(256);
                GetWindowText(hWnd, sb, 256);

                if (sb.ToString().Contains(partialTitle, StringComparison.OrdinalIgnoreCase))
                {
                    found = hWnd;
                    return false;
                }
                return true;
            }, IntPtr.Zero);

            return found;
        }

        private static IntPtr FindWindowByPid(int targetPid)
        {
            IntPtr found = IntPtr.Zero;

            EnumWindows((hWnd, _) =>
            {
                if (!IsWindowVisible(hWnd)) return true;
                GetWindowThreadProcessId(hWnd, out uint procId);
                if ((int)procId != targetPid) return true;

                var sb = new StringBuilder(256);
                GetWindowText(hWnd, sb, 256);
                if (string.IsNullOrWhiteSpace(sb.ToString())) return true;

                found = hWnd;
                return false;
            }, IntPtr.Zero);

            return found;
        }

        #endregion
    }
}
