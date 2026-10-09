using AirPodsDesktop.Services;
using Serilog;
using System.Runtime.InteropServices;

namespace AirPodsDesktop.Services;

/// <summary>
/// Bridges the Rust core to the UI thread. Holds the native callback alive
/// and raises C# events on the WinUI dispatcher.
/// </summary>
public sealed unsafe class CoreBridge : IDisposable
{
    private static CoreBridge? _instance;
    public static CoreBridge Instance => _instance ??= new CoreBridge();

    private Native.ApdEventCallback? _callback;
    private bool _started;
    private Task _mediaTask = Task.CompletedTask;

    public event EventHandler<AppSettings>? SettingsChanged;

    public event EventHandler<DeviceInfo?>? StateUpdated;
    public event EventHandler<bool>? LidToggled;
    public event EventHandler<bool>? BothInEar;
    public event EventHandler? Lost;
    public event EventHandler? Disconnected;
    public event EventHandler<bool>? ScannerStatusChanged; // true = started
    public event EventHandler<DeviceInfo?>? LowBattery;

    public string CoreVersion { get; private set; } = "";

    private CoreBridge() { }

    public void Initialize()
    {
        if (_started) return;

        // Keep the delegate alive (prevent GC of the native callback).
        _callback = OnNativeEvent;
        Native.apd_set_event_callback(_callback, IntPtr.Zero);
        if (Native.apd_init() != 0) throw new InvalidOperationException("Rust core initialization failed.");
        _started = true;
        CoreVersion = Native.Version();
        Log.Information("apd-core initialized, version {Version}", CoreVersion);
    }

    public void StartScanner()
    {
        var rc = Native.apd_start_scanner();
        Log.Information("start_scanner → {Rc}", rc);
    }

    public void StopScanner()
    {
        var rc = Native.apd_stop_scanner();
        Log.Information("stop_scanner → {Rc}", rc);
    }

    public void Tick() => Native.apd_tick();

    public DeviceInfo? GetState()
    {
        unsafe
        {
            Native.ApdDeviceState st;
            if (Native.apd_get_state(&st) == 1)
            {
                return Convert(&st);
            }
            return null;
        }
    }

    public AppSettings GetSettings()
    {
        unsafe
        {
            Native.ApdSettings s;
            Native.apd_get_settings(&s);
            return new AppSettings(
                s.RssiMin,
                s.BoundDeviceAddress,
                s.AutomaticEarDetection != 0,
                s.LowAudioLatency != 0,
                StartupService.IsEnabled(),
                s.ShowPopupOnConnect != 0);
        }
    }

    public void SetSettings(AppSettings s)
    {
        var previous = GetSettings();
        if (previous.AutoStart != s.AutoStart) StartupService.SetEnabled(s.AutoStart);
        unsafe
        {
            Native.ApdSettings n = new()
            {
                RssiMin = s.RssiMin,
                BoundDeviceAddress = s.BoundDeviceAddress,
                AutomaticEarDetection = (byte)(s.AutomaticEarDetection ? 1 : 0),
                LowAudioLatency = (byte)(s.LowAudioLatency ? 1 : 0),
                AutoStart = (byte)(s.AutoStart ? 1 : 0),
                ShowPopupOnConnect = (byte)(s.ShowPopupOnConnect ? 1 : 0),
            };
            Native.apd_set_settings(n);
        }
        SettingsChanged?.Invoke(this, s);
    }

    public string GetTheme() => Native.Theme();
    public void SetTheme(string theme)
    {
        Native.SetTheme(theme);
        App.ApplyTheme(theme);
    }

    public void MediaPause() => QueueMediaCommand(false);
    public void MediaPlay() => QueueMediaCommand(true);

    private void QueueMediaCommand(bool play)
    {
        // Serial execution keeps rapid remove/insert events in order and keeps SMTC off the UI thread.
        _mediaTask = _mediaTask.ContinueWith(_ =>
        {
            try
            {
                var rc = play ? Native.apd_media_play() : Native.apd_media_pause();
                if (rc != 0) Log.Warning("SMTC {Command} failed: {Rc}", play ? "play" : "pause", rc);
            }
            catch (Exception error) { Log.Warning(error, "SMTC command failed"); }
        }, TaskScheduler.Default);
    }

    private static unsafe DeviceInfo Convert(Native.ApdDeviceState* st)
    {
        var left = new BatteryInfo(st->Left.Battery.Percent, st->Left.Battery.IsCharging != 0, st->Left.IsInEar != 0);
        var right = new BatteryInfo(st->Right.Battery.Percent, st->Right.Battery.IsCharging != 0, st->Right.IsInEar != 0);
        var caseBox = new CaseInfo(
            st->Case.Battery.Percent,
            st->Case.Battery.IsCharging != 0,
            st->Case.IsBothPodsInCase != 0,
            st->Case.IsLidOpened != 0);
        return new DeviceInfo(st->Model, ModelName(st->Model), left, right, caseBox, st->Rssi);
    }

    public static string ModelName(uint model) => model switch
    {
        1 => "AirPods 1",
        2 => "AirPods 2",
        3 => "AirPods 3",
        4 => "AirPods 4",
        5 => "AirPods 4 (ANC)",
        6 => "AirPods 5",
        7 => "AirPods Pro",
        8 => "AirPods Pro 2",
        9 => "AirPods Pro 2 (USB-C)",
        10 => "AirPods Pro 3",
        11 => "AirPods Max",
        12 => "AirPods Max (USB-C)",
        13 => "Powerbeats 3",
        14 => "BeatsX",
        15 => "BeatsSolo3",
        16 => "Beats Fit Pro",
        _ => "AirPods",
    };

    private unsafe void OnNativeEvent(uint eventId, Native.ApdDeviceState* state, int aux, IntPtr userData)
    {
        // Marshals onto the UI thread via the app's dispatcher if available.
        var info = state == null ? null : Convert(state);
        var evt = (Native.ApdEvent)eventId;

        void Raise()
        {
            switch (evt)
            {
                case Native.ApdEvent.StateUpdated:
                    StateUpdated?.Invoke(this, info);
                    break;
                case Native.ApdEvent.LidToggled:
                    LidToggled?.Invoke(this, aux != 0);
                    break;
                case Native.ApdEvent.BothInEar:
                    BothInEar?.Invoke(this, aux != 0);
                    break;
                case Native.ApdEvent.Lost:
                    Lost?.Invoke(this, EventArgs.Empty);
                    break;
                case Native.ApdEvent.Disconnected:
                    Disconnected?.Invoke(this, EventArgs.Empty);
                    break;
                case Native.ApdEvent.ScannerStarted:
                    ScannerStatusChanged?.Invoke(this, true);
                    break;
                case Native.ApdEvent.ScannerStopped:
                    ScannerStatusChanged?.Invoke(this, false);
                    break;
                case Native.ApdEvent.LowBattery:
                    LowBattery?.Invoke(this, info);
                    break;
            }
        }

        var dispatcher = App.MainWindow?.DispatcherQueue;
        if (dispatcher is { HasThreadAccess: true })
        {
            Raise();
        }
        else if (dispatcher != null)
        {
            dispatcher.TryEnqueue(Raise);
        }
        else
        {
            Raise();
        }
    }

    public void Dispose()
    {
        StopScanner();
        Native.apd_set_event_callback(null, IntPtr.Zero);
        _callback = null;
        GC.SuppressFinalize(this);
    }
}
