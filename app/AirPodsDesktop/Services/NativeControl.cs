using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace AirPodsDesktop.Services;

public static unsafe partial class Native
{
    private static readonly JsonSerializerOptions ControlJson = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true,
    };

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr apd_control_devices();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr apd_control_connect(byte* path);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr apd_control_disconnect();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr apd_control_state();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr apd_control_set(uint setting, uint value);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr apd_control_set_duck(byte enabled);

    private static T ControlResponse<T>(IntPtr pointer)
        => JsonSerializer.Deserialize<T>(PtrToUtf8AndFree(pointer)
            ?? throw new InvalidOperationException("耳机控制未返回结果"), ControlJson)
            ?? throw new InvalidOperationException("耳机控制返回无效结果");

    public static ControlDevices ControlListDevices() => ControlResponse<ControlDevices>(apd_control_devices());
    public static ControlSnapshot ControlState() => ControlResponse<ControlSnapshot>(apd_control_state());
    public static ControlResult ControlDisconnect() => ControlResponse<ControlResult>(apd_control_disconnect());
    public static ControlResult ControlSet(uint setting, uint value) => ControlResponse<ControlResult>(apd_control_set(setting, value));
    public static ControlResult ControlSetDuck(bool enabled) => ControlResponse<ControlResult>(apd_control_set_duck((byte)(enabled ? 1 : 0)));
    public static ControlResult ControlConnect(string path)
    {
        var bytes = Encoding.UTF8.GetBytes(path + "\0");
        fixed (byte* p = bytes) { return ControlResponse<ControlResult>(apd_control_connect(p)); }
    }
}

public sealed record ControlEndpoint(string Path, string Name, int? ProductId)
{
    public override string ToString() => Name;
}
public sealed record ControlDevices(ControlEndpoint[] Devices, string? Error);
public sealed record ControlResult(bool Ok, string? Error);
public sealed record ControlSnapshot(string Phase, ControlEndpoint? Device, int? ListeningMode,
    bool? ConversationAwareness, bool? OneEarAnc, bool? Speaking, bool AdaptiveSupported,
    bool DuckEnabled, string? VolumeMessage, string Message);
