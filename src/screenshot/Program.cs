using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Screenshot
{
    class Program
    {
        static int Main(string[] args)
        {
            if (args.Length == 0 || args[0] == "--help")
            {
                PrintUsage();
                return 0;
            }

            string? title = null;
            int? pid = null;
            bool screen = false;
            int? monitor = null;
            string? region = null;
            string output = "screenshot.png";

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
                    case "--screen":
                        screen = true;
                        break;
                    case "--monitor" or "-m":
                        monitor = int.Parse(NextArg(args, ref i, "--monitor"));
                        break;
                    case "--region" or "-r":
                        region = NextArg(args, ref i, "--region");
                        break;
                    case "--output" or "-o":
                        output = NextArg(args, ref i, "--output");
                        break;
                    default:
                        Console.Error.WriteLine($"Unknown argument: {args[i]}");
                        return 1;
                }
            }

            int targetCount = (title != null ? 1 : 0) + (pid != null ? 1 : 0)
                + (screen ? 1 : 0) + (monitor != null ? 1 : 0) + (region != null ? 1 : 0);

            if (targetCount == 0)
            {
                Console.Error.WriteLine("Error: no capture target specified. Use --help for usage.");
                return 1;
            }
            if (targetCount > 1)
            {
                Console.Error.WriteLine("Error: specify exactly one capture target.");
                return 1;
            }

            InitGdiPlus();
            IntPtr hBitmap = IntPtr.Zero;

            try
            {
                if (screen)
                {
                    int x = GetSystemMetrics(SM_XVIRTUALSCREEN);
                    int y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                    int w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                    int h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
                    hBitmap = CaptureRect(x, y, w, h);
                }
                else if (monitor != null)
                {
                    var monitors = GetMonitors();
                    if (monitor < 1 || monitor > monitors.Count)
                    {
                        Console.Error.WriteLine($"Monitor {monitor} not found. Available: 1-{monitors.Count}");
                        return 1;
                    }
                    var mi = monitors[monitor.Value - 1].Info;
                    int x = mi.rcMonitor.Left;
                    int y = mi.rcMonitor.Top;
                    int w = mi.rcMonitor.Right - mi.rcMonitor.Left;
                    int h = mi.rcMonitor.Bottom - mi.rcMonitor.Top;
                    hBitmap = CaptureRect(x, y, w, h);
                }
                else if (region != null)
                {
                    var parts = region.Split(',');
                    if (parts.Length != 4)
                    {
                        Console.Error.WriteLine("Error: --region requires x,y,width,height (e.g., 0,0,800,600)");
                        return 1;
                    }
                    int rx = int.Parse(parts[0]);
                    int ry = int.Parse(parts[1]);
                    int rw = int.Parse(parts[2]);
                    int rh = int.Parse(parts[3]);
                    hBitmap = CaptureRect(rx, ry, rw, rh);
                }
                else if (title != null || pid != null)
                {
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
                            Console.Error.WriteLine("Use 'winctl list' to see available windows.");
                            return 1;
                        }
                    }
                    hBitmap = CaptureWindow(hwnd);
                }

                string absPath = Path.GetFullPath(output);
                SaveBitmapAsPng(hBitmap, absPath);
                Console.WriteLine(absPath);
                return 0;
            }
            finally
            {
                if (hBitmap != IntPtr.Zero) DeleteObject(hBitmap);
                ShutdownGdiPlus();
            }
        }

        private static string NextArg(string[] args, ref int index, string flag)
        {
            index++;
            if (index >= args.Length)
                throw new ArgumentException($"{flag} requires a value");
            return args[index];
        }

        private static void PrintUsage()
        {
            Console.WriteLine(@"screenshot - Capture screenshots on Windows

Usage:
  screenshot --title <partial-title> [--output path.png]
  screenshot --pid <process-id> [--output path.png]
  screenshot --screen [--output path.png]
  screenshot --monitor <N> [--output path.png]
  screenshot --region <x,y,w,h> [--output path.png]

Target (exactly one required):
  --title, -t     Capture window by partial title (case-insensitive)
  --pid, -p       Capture window by process ID
  --screen        Capture the entire screen (all monitors)
  --monitor, -m   Capture a specific monitor (1-based)
  --region, -r    Capture a region: x,y,width,height

Output:
  --output, -o    Output file path (default: screenshot.png)");
        }

        #region Win32 Imports

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

        [DllImport("user32.dll")]
        private static extern bool IsIconic(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);

        [DllImport("user32.dll")]
        private static extern IntPtr GetDC(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);

        [DllImport("user32.dll")]
        private static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint dwFlags);

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFOEX lpmi);

        [DllImport("user32.dll")]
        private static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip,
            EnumMonitorsProc lpfnEnum, IntPtr dwData);

        [DllImport("user32.dll")]
        private static extern int GetSystemMetrics(int nIndex);

        [DllImport("gdi32.dll")]
        private static extern IntPtr CreateCompatibleDC(IntPtr hdc);

        [DllImport("gdi32.dll")]
        private static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int nWidth, int nHeight);

        [DllImport("gdi32.dll")]
        private static extern IntPtr SelectObject(IntPtr hdc, IntPtr hgdiobj);

        [DllImport("gdi32.dll")]
        private static extern bool BitBlt(IntPtr hdcDest, int xDest, int yDest, int wDest, int hDest,
            IntPtr hdcSrc, int xSrc, int ySrc, uint dwRop);

        [DllImport("gdi32.dll")]
        private static extern bool DeleteDC(IntPtr hdc);

        [DllImport("gdi32.dll")]
        private static extern bool DeleteObject(IntPtr hObject);

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

        private const int SW_RESTORE = 9;
        private const uint SRCCOPY = 0x00CC0020;
        private const uint PW_RENDERFULLCONTENT = 0x00000002;
        private const int SM_XVIRTUALSCREEN = 76;
        private const int SM_YVIRTUALSCREEN = 77;
        private const int SM_CXVIRTUALSCREEN = 78;
        private const int SM_CYVIRTUALSCREEN = 79;

        #endregion

        #region GDI+ Imports for PNG saving

        [DllImport("gdiplus.dll")]
        private static extern int GdiplusStartup(out IntPtr token, ref GdiplusStartupInput input, IntPtr output);

        [DllImport("gdiplus.dll")]
        private static extern int GdiplusShutdown(IntPtr token);

        [DllImport("gdiplus.dll")]
        private static extern int GdipCreateBitmapFromHBITMAP(IntPtr hbm, IntPtr hpal, out IntPtr bitmap);

        [DllImport("gdiplus.dll")]
        private static extern int GdipSaveImageToFile(IntPtr image, [MarshalAs(UnmanagedType.LPWStr)] string filename,
            ref Guid clsidEncoder, IntPtr encoderParams);

        [DllImport("gdiplus.dll")]
        private static extern int GdipDisposeImage(IntPtr image);

        [DllImport("gdiplus.dll")]
        private static extern int GdipGetImageEncodersSize(out int numEncoders, out int size);

        [DllImport("gdiplus.dll")]
        private static extern int GdipGetImageEncoders(int numEncoders, int size, IntPtr encoders);

        [StructLayout(LayoutKind.Sequential)]
        private struct GdiplusStartupInput
        {
            public int GdiplusVersion;
            public IntPtr DebugEventCallback;
            public bool SuppressBackgroundThread;
            public bool SuppressExternalCodecs;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct ImageCodecInfo
        {
            public Guid Clsid;
            public Guid FormatID;
            public IntPtr CodecName;
            public IntPtr DllName;
            public IntPtr FormatDescription;
            public IntPtr FilenameExtension;
            public IntPtr MimeType;
            public int Flags;
            public int Version;
            public int SigCount;
            public int SigSize;
            public IntPtr SigPattern;
            public IntPtr SigMask;
        }

        #endregion

        private static IntPtr _gdipToken;

        private static void InitGdiPlus()
        {
            var input = new GdiplusStartupInput { GdiplusVersion = 1 };
            GdiplusStartup(out _gdipToken, ref input, IntPtr.Zero);
        }

        private static void ShutdownGdiPlus()
        {
            GdiplusShutdown(_gdipToken);
        }

        private static Guid GetPngEncoderClsid()
        {
            GdipGetImageEncodersSize(out int numEncoders, out int size);
            IntPtr buf = Marshal.AllocHGlobal(size);
            GdipGetImageEncoders(numEncoders, size, buf);

            int structSize = Marshal.SizeOf<ImageCodecInfo>();
            for (int i = 0; i < numEncoders; i++)
            {
                var codec = Marshal.PtrToStructure<ImageCodecInfo>(buf + i * structSize);
                string? mime = Marshal.PtrToStringUni(codec.MimeType);
                if (mime == "image/png")
                {
                    Marshal.FreeHGlobal(buf);
                    return codec.Clsid;
                }
            }

            Marshal.FreeHGlobal(buf);
            throw new Exception("PNG encoder not found");
        }

        private static void SaveBitmapAsPng(IntPtr hBitmap, string path)
        {
            int status = GdipCreateBitmapFromHBITMAP(hBitmap, IntPtr.Zero, out IntPtr gpBitmap);
            if (status != 0)
                throw new Exception($"GdipCreateBitmapFromHBITMAP failed: {status}");

            Guid pngClsid = GetPngEncoderClsid();
            status = GdipSaveImageToFile(gpBitmap, path, ref pngClsid, IntPtr.Zero);
            GdipDisposeImage(gpBitmap);

            if (status != 0)
                throw new Exception($"GdipSaveImageToFile failed: {status}");
        }

        private static IntPtr CaptureRect(int x, int y, int width, int height)
        {
            IntPtr hdcScreen = GetDC(IntPtr.Zero);
            IntPtr hdcMem = CreateCompatibleDC(hdcScreen);
            IntPtr hBitmap = CreateCompatibleBitmap(hdcScreen, width, height);
            IntPtr hOld = SelectObject(hdcMem, hBitmap);

            BitBlt(hdcMem, 0, 0, width, height, hdcScreen, x, y, SRCCOPY);

            SelectObject(hdcMem, hOld);
            DeleteDC(hdcMem);
            ReleaseDC(IntPtr.Zero, hdcScreen);

            return hBitmap;
        }

        private static IntPtr CaptureWindow(IntPtr hwnd)
        {
            if (IsIconic(hwnd))
                ShowWindow(hwnd, SW_RESTORE);

            GetWindowRect(hwnd, out RECT rect);
            int w = rect.Right - rect.Left;
            int h = rect.Bottom - rect.Top;

            IntPtr hdcScreen = GetDC(IntPtr.Zero);
            IntPtr hdcMem = CreateCompatibleDC(hdcScreen);
            IntPtr hBitmap = CreateCompatibleBitmap(hdcScreen, w, h);
            IntPtr hOld = SelectObject(hdcMem, hBitmap);

            PrintWindow(hwnd, hdcMem, PW_RENDERFULLCONTENT);

            SelectObject(hdcMem, hOld);
            DeleteDC(hdcMem);
            ReleaseDC(IntPtr.Zero, hdcScreen);

            return hBitmap;
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

        private static List<(IntPtr Handle, MONITORINFOEX Info)> GetMonitors()
        {
            var monitors = new List<(IntPtr, MONITORINFOEX)>();
            EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (IntPtr hMonitor, IntPtr _, ref RECT __, IntPtr ___) =>
            {
                var mi = new MONITORINFOEX { cbSize = Marshal.SizeOf<MONITORINFOEX>() };
                GetMonitorInfo(hMonitor, ref mi);
                monitors.Add((hMonitor, mi));
                return true;
            }, IntPtr.Zero);
            return monitors;
        }
    }
}
