using System.Runtime.InteropServices;
using System.Text;

namespace AirPodsDesktop.Services;

/// <summary>
/// Classic Win32 notification-area (tray) icon via Shell_NotifyIcon.
/// Avoids third-party XAML tray controls that break the XAML compiler.
/// </summary>
public sealed class TrayIconService : IDisposable
{
    private const uint NIM_ADD = 0x00000000;
    private const uint NIM_MODIFY = 0x00000001;
    private const uint NIM_DELETE = 0x00000002;
    private const uint NIF_MESSAGE = 0x00000001;
    private const uint NIF_ICON = 0x00000002;
    private const uint NIF_TIP = 0x00000004;
    private const uint NIF_SHOWTIP = 0x00000080;

    private const uint WM_LBUTTONUP = 0x0202;
    private const uint WM_RBUTTONUP = 0x0205;
    private const uint WM_USER = 0x0400;
    private const uint WM_TRAYICON = WM_USER + 1;

    private const uint TPM_RIGHTBUTTON = 0x0002;
    private const uint TPM_RETURNCMD = 0x0100;
    private const uint TPM_NONOTIFY = 0x0080;
    private const uint MF_STRING = 0x00000000;
    private const uint MF_SEPARATOR = 0x00000800;
    private const uint MF_CHECKED = 0x00000008;

    private IntPtr _hwnd;
    private IntPtr _hIcon;
    private bool _added;
    private IntPtr _msgHwnd;
    private WndProcDelegate? _wndProc;

    private delegate IntPtr WndProcDelegate(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    public event EventHandler? OpenRequested;
    public event EventHandler<bool>? EarDetectionToggled;
    public event EventHandler? PauseRequested;
    public event EventHandler? PlayRequested;
    public event EventHandler? ExitRequested;

    public bool EarDetectionChecked { get; set; } = true;

    public void Initialize(IntPtr ownerHwnd)
    {
        // Create a message-only window to receive tray callbacks (no subclassing of the XAML window).
        _wndProc = (hWnd, msg, wParam, lParam) =>
        {
            if (msg == WM_TRAYICON)
            {
                HandleCallback(msg, lParam);
                return IntPtr.Zero;
            }
            return DefWindowProcW(hWnd, msg, wParam, lParam);
        };

        var wc = new WNDCLASSW
        {
            lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_wndProc),
            hInstance = GetModuleHandleW(null),
            lpszClassName = "AirPodsDesktop.TrayMsgWindow",
        };
        RegisterClassW(ref wc);
        _msgHwnd = CreateWindowExW(0, "AirPodsDesktop.TrayMsgWindow", "", 0, 0, 0, 0, 0, new IntPtr(-3), IntPtr.Zero, wc.hInstance, IntPtr.Zero);
        _hwnd = _msgHwnd;

        _hIcon = LoadIconW(IntPtr.Zero, new IntPtr(32512)); // IDI_APPLICATION

        var nid = new NOTIFYICONDATA
        {
            cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATA>(),
            hWnd = _hwnd,
            uID = 1,
            uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage = WM_TRAYICON,
            hIcon = _hIcon,
            szTip = "AirPodsDesktop",
        };
        _added = Shell_NotifyIconW(NIM_ADD, ref nid);
    }

    public void SetTooltip(string text)
    {
        if (!_added) return;
        var nid = new NOTIFYICONDATA
        {
            cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATA>(),
            hWnd = _hwnd,
            uID = 1,
            uFlags = NIF_TIP | NIF_SHOWTIP,
            szTip = text,
        };
        Shell_NotifyIconW(NIM_MODIFY, ref nid);
    }

    /// <summary>Call from the window procedure for the tray callback message.</summary>
    public void HandleCallback(uint msg, IntPtr lParam)
    {
        if (msg != WM_TRAYICON) return;
        var mouse = (uint)(lParam.ToInt64() & 0xFFFF);
        if (mouse == WM_LBUTTONUP)
        {
            OpenRequested?.Invoke(this, EventArgs.Empty);
        }
        else if (mouse == WM_RBUTTONUP)
        {
            ShowMenu();
        }
    }

    private void ShowMenu()
    {
        var hMenu = CreatePopupMenu();
        AppendMenuW(hMenu, MF_STRING, 100, "打开主界面");
        AppendMenuW(hMenu, MF_SEPARATOR, 0, string.Empty);
        AppendMenuW(hMenu, MF_STRING | (EarDetectionChecked ? MF_CHECKED : 0), 101, "入耳检测");
        AppendMenuW(hMenu, MF_SEPARATOR, 0, string.Empty);
        AppendMenuW(hMenu, MF_STRING, 103, "暂停媒体");
        AppendMenuW(hMenu, MF_STRING, 104, "播放媒体");
        AppendMenuW(hMenu, MF_SEPARATOR, 0, string.Empty);
        AppendMenuW(hMenu, MF_STRING, 105, "退出");

        GetCursorPos(out var pt);
        SetForegroundWindow(_hwnd);
        var cmd = TrackPopupMenuEx(hMenu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.X, pt.Y, _hwnd, IntPtr.Zero);
        DestroyMenu(hMenu);

        switch (cmd)
        {
            case 100: OpenRequested?.Invoke(this, EventArgs.Empty); break;
            case 101:
                EarDetectionChecked = !EarDetectionChecked;
                EarDetectionToggled?.Invoke(this, EarDetectionChecked);
                break;
            case 103: PauseRequested?.Invoke(this, EventArgs.Empty); break;
            case 104: PlayRequested?.Invoke(this, EventArgs.Empty); break;
            case 105: ExitRequested?.Invoke(this, EventArgs.Empty); break;
        }
    }

    public void Dispose()
    {
        if (_added)
        {
            var nid = new NOTIFYICONDATA
            {
                cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATA>(),
                hWnd = _hwnd,
                uID = 1,
            };
            Shell_NotifyIconW(NIM_DELETE, ref nid);
            _added = false;
        }
        if (_msgHwnd != IntPtr.Zero)
        {
            DestroyWindow(_msgHwnd);
            _msgHwnd = IntPtr.Zero;
            _wndProc = null;
        }
        GC.SuppressFinalize(this);
    }

    // ---- P/Invoke ----

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct NOTIFYICONDATA
    {
        public uint cbSize;
        public IntPtr hWnd;
        public uint uID;
        public uint uFlags;
        public uint uCallbackMessage;
        public IntPtr hIcon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string szTip;
        public uint dwState;
        public uint dwStateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)]
        public string szInfo;
        public uint uTimeoutOrVersion;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)]
        public string szInfoTitle;
        public uint dwInfoFlags;
        public Guid guidItem;
        public IntPtr hBalloonIcon;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT { public int X; public int Y; }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern bool Shell_NotifyIconW(uint dwMessage, ref NOTIFYICONDATA lpData);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WNDCLASSW
    {
        public uint style;
        public IntPtr lpfnWndProc;
        public int cbClsExtra;
        public int cbWndExtra;
        public IntPtr hInstance;
        public IntPtr hIcon;
        public IntPtr hCursor;
        public IntPtr hbrBackground;
        public string? lpszMenuName;
        public string lpszClassName;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern ushort RegisterClassW(ref WNDCLASSW lpWndClass);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateWindowExW(
        uint dwExStyle, string lpClassName, string lpWindowName, uint dwStyle,
        int x, int y, int nWidth, int nHeight, IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);

    [DllImport("user32.dll")]
    private static extern IntPtr DefWindowProcW(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandleW(string? lpModuleName);

    [DllImport("user32.dll")]
    private static extern IntPtr LoadIconW(IntPtr hInstance, IntPtr lpIconName);

    [DllImport("user32.dll")]
    private static extern IntPtr CreatePopupMenu();

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern bool AppendMenuW(IntPtr hMenu, uint uFlags, uint uIDNewItem, string lpNewItem);

    [DllImport("user32.dll")]
    private static extern bool DestroyMenu(IntPtr hMenu);

    [DllImport("user32.dll")]
    private static extern bool DestroyWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern uint TrackPopupMenuEx(IntPtr hMenu, uint uFlags, int x, int y, IntPtr hWnd, IntPtr lptpm);

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);
}
