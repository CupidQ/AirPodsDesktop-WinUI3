using AirPodsDesktop.Services;
using AirPodsDesktop.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Serilog;

namespace AirPodsDesktop;

public sealed partial class MainWindow : Window
{
    private DispatcherTimer? _tickTimer;
    private readonly TrayIconService _tray = new();

    public MainWindow()
    {
        InitializeComponent();
        ExtendsContentIntoTitleBar = true;

        App.Core.StateUpdated += OnStateUpdated;
        App.Core.LidToggled += OnLidToggled;
        App.Core.BothInEar += OnBothInEar;
        App.Core.LowBattery += OnLowBattery;
        App.Core.Lost += (_, _) => UpdateTrayTooltip(null);
        App.Core.Disconnected += (_, _) => UpdateTrayTooltip(null);
        App.Core.ScannerStatusChanged += (_, started) =>
            Log.Information("Scanner {State}", started ? "started" : "stopped");

        // Wire tray
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        _tray.Initialize(hwnd);
        _tray.OpenRequested += (_, _) => Activate();
        _tray.ExitRequested += (_, _) => Close();
        Closed += (_, _) =>
        {
            _tickTimer?.Stop();
            App.Core.Dispose();
            _tray.Dispose();
            Log.CloseAndFlush();
        };
        _tray.PauseRequested += (_, _) => App.Core.MediaPause();
        _tray.PlayRequested += (_, _) => App.Core.MediaPlay();
        _tray.EarDetectionToggled += (_, on) =>
        {
            var s = App.Core.GetSettings();
            App.Core.SetSettings(s with { AutomaticEarDetection = on });
        };
        _tray.EarDetectionChecked = App.Core.GetSettings().AutomaticEarDetection;
        App.Core.SettingsChanged += (_, settings) => _tray.EarDetectionChecked = settings.AutomaticEarDetection;

        NavView.SelectedItem = NavView.MenuItems[0];

        _tickTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _tickTimer.Tick += (_, _) => App.Core.Tick();
        _tickTimer.Start();

        UpdateTrayTooltip(App.Core.GetState());

    }

    /// <summary>Forward Win32 tray messages if a message window is used.</summary>
    public void OnTrayMessage(uint msg, IntPtr lParam) => _tray.HandleCallback(msg, lParam);

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is NavigationViewItem item)
        {
            var tag = item.Tag as string;
            var page = tag switch
            {
                "Device" => typeof(DevicePage),
                "Battery" => typeof(BatteryPage),
                "Logs" => typeof(LogsPage),
                _ => typeof(DevicePage),
            };
            if (ContentFrame.CurrentSourcePageType != page)
            {
                ContentFrame.Navigate(page);
            }
        }
    }

    private void OnStateUpdated(object? sender, DeviceInfo? info)
    {
        UpdateTrayTooltip(info);
    }

    private void UpdateTrayTooltip(DeviceInfo? info)
    {
        if (info is null)
        {
            _tray.SetTooltip("AirPodsDesktop — 未连接");
            return;
        }
        _tray.SetTooltip(
            $"{info.ModelName}  L {info.Left.Display}  R {info.Right.Display}  盒 {info.Case.Display}");
    }

    private void OnLidToggled(object? sender, bool opened)
    {
        Log.Information("Lid {State}", opened ? "opened" : "closed");
        if (opened)
        {
            var settings = App.Core.GetSettings();
            if (settings.ShowPopupOnConnect)
            {
                _ = DispatcherQueue.TryEnqueue(ShowBatteryPopup);
            }
        }
    }

    private void OnBothInEar(object? sender, bool inEar)
    {
        var settings = App.Core.GetSettings();
        if (!settings.AutomaticEarDetection) return;

        if (inEar) App.Core.MediaPlay();
        else App.Core.MediaPause();
    }

    private void OnLowBattery(object? sender, DeviceInfo? info)
    {
        if (info is null) return;
        Log.Warning("Low battery: L={L} R={R}", info.Left.Display, info.Right.Display);
    }

    private async void ShowBatteryPopup()
    {
        try
        {
            var dialog = new BatteryPopup();
            dialog.XamlRoot = ContentFrame.XamlRoot;
            await dialog.ShowAsync();
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to show battery popup");
        }
    }
}
