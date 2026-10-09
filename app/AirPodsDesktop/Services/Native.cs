using System.Runtime.InteropServices;

namespace AirPodsDesktop.Services;

/// <summary>
/// P/Invoke bindings to the Rust core (<c>apd_core.dll</c>).
/// Struct layouts must match <c>crates/apd-core/src/ffi/mod.rs</c>.
/// </summary>
public static unsafe partial class Native
{
    private const string Dll = "apd_core";

    // ---- Events ------------------------------------------------------------

    public enum ApdEvent : uint
    {
        StateUpdated = 0,
        LidToggled = 1,
        BothInEar = 2,
        Lost = 3,
        Disconnected = 4,
        ScannerStarted = 5,
        ScannerStopped = 6,
        LowBattery = 7,
    }

    [StructLayout(LayoutKind.Sequential)]
    public unsafe struct ApdBattery
    {
        public int Percent;          // 0-100, or -1
        public byte IsCharging;
        public fixed byte Pad[3];
    }

    [StructLayout(LayoutKind.Sequential)]
    public unsafe struct ApdPod
    {
        public ApdBattery Battery;
        public byte IsInEar;
        public fixed byte Pad[7];
    }

    [StructLayout(LayoutKind.Sequential)]
    public unsafe struct ApdCase
    {
        public ApdBattery Battery;
        public byte IsBothPodsInCase;
        public byte IsLidOpened;
        public fixed byte Pad[2];
    }

    [StructLayout(LayoutKind.Sequential)]
    public unsafe struct ApdDeviceState
    {
        public uint Model;
        public ApdPod Left;
        public ApdPod Right;
        public ApdCase Case;
        public short Rssi;
        public fixed byte Pad[6];
    }

    [StructLayout(LayoutKind.Sequential)]
    public unsafe struct ApdSettings
    {
        public short RssiMin;
        public fixed byte Pad0[6];
        public ulong BoundDeviceAddress;
        public byte AutomaticEarDetection;
        public byte LowAudioLatency;
        public byte AutoStart;
        public byte ShowPopupOnConnect;
        public fixed byte Pad1[4];
    }

    public unsafe delegate void ApdEventCallback(
        uint eventId, ApdDeviceState* state, int aux, IntPtr userData);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr apd_version();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern void apd_string_free(IntPtr s);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern void apd_set_event_callback(ApdEventCallback? cb, IntPtr userData);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern int apd_init();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern int apd_start_scanner();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern int apd_stop_scanner();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern unsafe int apd_get_state(ApdDeviceState* state);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr apd_get_display_name();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern ulong apd_get_device_address();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern void apd_tick();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern unsafe int apd_get_settings(ApdSettings* settings);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern int apd_set_settings(ApdSettings settings);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr apd_get_theme();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern unsafe int apd_set_theme(byte* themeUtf8);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern int apd_media_pause();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    public static extern int apd_media_play();

    // ---- Helpers -----------------------------------------------------------

    public static unsafe string? PtrToUtf8AndFree(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        try
        {
            return Marshal.PtrToStringUTF8(ptr);
        }
        finally
        {
            apd_string_free(ptr);
        }
    }

    public static string Version()
        => PtrToUtf8AndFree(apd_version()) ?? "0.0.0";

    public static string DisplayName()
        => PtrToUtf8AndFree(apd_get_display_name()) ?? "AirPods";

    public static string Theme()
        => PtrToUtf8AndFree(apd_get_theme()) ?? "system";

    public static unsafe void SetTheme(string theme)
    {
        var bytes = System.Text.Encoding.UTF8.GetBytes(theme + "\0");
        fixed (byte* p = bytes)
        {
            apd_set_theme(p);
        }
    }
}

/// <summary>Managed view of device state for the UI.</summary>
public sealed record DeviceInfo(
    uint Model,
    string ModelName,
    BatteryInfo Left,
    BatteryInfo Right,
    CaseInfo Case,
    short Rssi)
{
    public bool IsAnyInEar => Left.IsInEar || Right.IsInEar;
    public bool IsBothInEar => Left.IsInEar && Right.IsInEar;
    public bool HasLowBattery => (Left.IsLow && !Left.IsCharging) || (Right.IsLow && !Right.IsCharging);
}

public sealed record BatteryInfo(int Percent, bool IsCharging, bool IsInEar)
{
    public bool IsAvailable => Percent >= 0;
    public bool IsLow => IsAvailable && Percent <= 20;
    public string Display => IsAvailable ? (IsCharging ? $"{Percent}% ⚡" : $"{Percent}%") : "--";
}

public sealed record CaseInfo(int Percent, bool IsCharging, bool IsBothPodsInCase, bool IsLidOpened)
{
    public bool IsAvailable => Percent >= 0;
    public string Display => IsAvailable ? (IsCharging ? $"{Percent}% ⚡" : $"{Percent}%") : "--";
}

public sealed record AppSettings(
    short RssiMin,
    ulong BoundDeviceAddress,
    bool AutomaticEarDetection,
    bool LowAudioLatency,
    bool AutoStart,
    bool ShowPopupOnConnect);
