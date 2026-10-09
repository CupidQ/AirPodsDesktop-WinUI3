using Microsoft.Win32;

namespace AirPodsDesktop.Services;

/// <summary>Opt-in startup for this unpackaged desktop application.</summary>
public static class StartupService
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "AirPodsDesktop";

    public static bool IsEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKey);
        var executable = Path.Combine(AppContext.BaseDirectory, "AirPodsDesktop.exe");
        return string.Equals(key?.GetValue(ValueName) as string, $"\"{executable}\"", StringComparison.OrdinalIgnoreCase);
    }

    public static void SetEnabled(bool enabled)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKey, writable: true);
        if (enabled)
        {
            var executable = Path.Combine(AppContext.BaseDirectory, "AirPodsDesktop.exe");
            if (!File.Exists(executable)) throw new FileNotFoundException("找不到启动程序", executable);
            key.SetValue(ValueName, $"\"{executable}\"", RegistryValueKind.String);
        }
        else
        {
            key.DeleteValue(ValueName, throwOnMissingValue: false);
        }
    }
}
