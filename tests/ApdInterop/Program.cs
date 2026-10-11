using System.Runtime.InteropServices;
using System.Text;
using AirPodsDesktop.Services;

if (args.Length != 1) throw new ArgumentException("Pass the path to the built apd_core library.");
var libraryPath = Path.GetFullPath(args[0]);
NativeLibrary.SetDllImportResolver(typeof(Native).Assembly,
    (name, assembly, path) => name == "apd_core" ? NativeLibrary.Load(libraryPath) : IntPtr.Zero);
var directory = Path.Combine(Path.GetTempPath(), "apd-interop-" + Guid.NewGuid().ToString("N"));
Directory.CreateDirectory(directory);
var passed = 0;
try
{
    Check("FFI layouts", () =>
    {
        Equal(56, Marshal.SizeOf<Native.ApdDeviceState>());
        Equal(24, Marshal.SizeOf<Native.ApdSettings>());
        Equal(new IntPtr(4), Marshal.OffsetOf<Native.ApdDeviceState>(nameof(Native.ApdDeviceState.Left)));
        Equal(new IntPtr(20), Marshal.OffsetOf<Native.ApdDeviceState>(nameof(Native.ApdDeviceState.Right)));
        Equal(new IntPtr(36), Marshal.OffsetOf<Native.ApdDeviceState>(nameof(Native.ApdDeviceState.Case)));
    });
    Check("Immutable managed snapshot", () =>
    {
        Native.ApdDeviceState raw = default;
        raw.Model = 10;
        raw.Rssi = -50;
        raw.Left.Battery.Percent = 80;
        raw.Left.IsInEar = 1;
        raw.Right.Battery.Percent = -1;
        raw.Case.Battery.Percent = 60;
        raw.Case.Battery.IsCharging = 1;
        raw.Case.IsLidOpened = 1;
        var first = Native.ConvertState(raw, "AirPods Pro 3");
        Equal(new BatteryInfo(80, false, true), first.Left);
        Equal(false, first.Right.IsAvailable);
        Equal(new CaseInfo(60, true, false, true), first.Case);
        raw.Left.Battery.Percent = 20;
        var second = Native.ConvertState(raw, "AirPods Pro 3");
        Equal(80, first.Left.Percent);
        Equal(20, second.Left.Percent);
        Equal(true, second.HasLowBattery);
    });
    foreach (var separator in new[] { "\n", "\r\n", "\r" })
    {
        Check("UTF-8/BOM tail with " + separator.Replace("\r", "CR").Replace("\n", "LF"), () =>
        {
            var path = Path.Combine(directory, "中文-日志.log");
            var lines = Enumerable.Range(0, 400).Select(i => $"中文🙂 {i}").ToArray();
            File.WriteAllText(path, string.Join(separator, lines) + separator, new UTF8Encoding(true));
            var expected = string.Join(Environment.NewLine, File.ReadLines(path).TakeLast(200));
            Equal(expected, Native.ReadLogTail(path));
        });
    }
    Check("Read while writer holds file open", () =>
    {
        var path = Path.Combine(directory, "active.log");
        using var writer = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete);
        writer.Write(Encoding.UTF8.GetBytes("live 日志\n"));
        writer.Flush();
        Equal("live 日志", Native.ReadLogTail(path));
    });
    Check("Empty and missing files", () =>
    {
        var path = Path.Combine(directory, "empty.log");
        File.WriteAllText(path, "");
        Equal("", Native.ReadLogTail(path));
        Equal(true, Native.ReadLogTail(path + ".missing").StartsWith("读取日志失败:"));
    });
    Check("Byte cap marks an oversized final line", () =>
    {
        var path = Path.Combine(directory, "huge.log");
        File.WriteAllText(path, new string('x', 1_000_000) + "🙂END");
        var result = Native.ReadLogTail(path);
        Equal(true, result.StartsWith("（日志内容过长，仅显示末尾片段）"));
        Equal(true, result.EndsWith("🙂END"));
        Equal(true, result.Length < 270_000);
    });
    Console.WriteLine($"{passed} managed/native checks passed.");
}
finally { Directory.Delete(directory, recursive: true); }

void Check(string name, Action action)
{
    action();
    ++passed;
    Console.WriteLine("PASS " + name);
}

static void Equal<T>(T expected, T actual)
{
    if (!EqualityComparer<T>.Default.Equals(expected, actual))
        throw new InvalidOperationException($"Expected {expected}, got {actual}.");
}
