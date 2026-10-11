using AirPodsDesktop.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace AirPodsDesktop.Views;

public sealed partial class DevicePage : Page
{
    private readonly DispatcherTimer _rssiTimer = new() { Interval = TimeSpan.FromMilliseconds(250) };
    private DeviceInfo? _renderedState;
    private short? _pendingRssi;
    private short? _displayedRssi;

    public DevicePage()
    {
        InitializeComponent();
        _rssiTimer.Tick += (_, _) => FlushRssi();
        Loaded += (_, _) =>
        {
            App.Core.StateUpdated += OnState;
            App.Core.Lost += OnLost;
            Render(App.Core.GetState());
        };
        Unloaded += (_, _) =>
        {
            App.Core.StateUpdated -= OnState;
            App.Core.Lost -= OnLost;
            _rssiTimer.Stop();
            _pendingRssi = null;
            _renderedState = null;
        };
    }

    private void OnState(object? sender, DeviceInfo? info) => Render(info);

    private void OnLost(object? sender, EventArgs e)
    {
        // CoreBridge already dispatches events to the UI thread. Enqueuing
        // again could clear a newer connected state after a rapid reconnect.
        _rssiTimer.Stop();
        _pendingRssi = null;
        _displayedRssi = null;
        _renderedState = null;
        StatusText.Text = "设备已断开 / 丢失";
        ModelText.Text = "AirPods";
        RssiText.Text = "RSSI --";
        LeftBatteryBar.Value = 0;
        RightBatteryBar.Value = 0;
        CaseBatteryBar.Value = 0;
        LeftBatteryText.Text = "--";
        RightBatteryText.Text = "--";
        CaseBatteryText.Text = "--";
        LeftInEarText.Text = "";
        RightInEarText.Text = "";
        LidText.Text = "";
    }

    private void Render(DeviceInfo? info)
    {
        if (info is null) { OnLost(this, EventArgs.Empty); return; }

        var previous = _renderedState;
        _renderedState = info;
        if (previous is null) StatusText.Text = "已连接";
        if (previous?.ModelName != info.ModelName) ModelText.Text = info.ModelName;

        if (previous?.Left.Percent != info.Left.Percent)
            LeftBatteryBar.Value = info.Left.IsAvailable ? info.Left.Percent : 0;
        if (previous?.Right.Percent != info.Right.Percent)
            RightBatteryBar.Value = info.Right.IsAvailable ? info.Right.Percent : 0;
        if (previous?.Case.Percent != info.Case.Percent)
            CaseBatteryBar.Value = info.Case.IsAvailable ? info.Case.Percent : 0;

        if (previous?.Left.Percent != info.Left.Percent || previous?.Left.IsCharging != info.Left.IsCharging)
            LeftBatteryText.Text = info.Left.Display;
        if (previous?.Right.Percent != info.Right.Percent || previous?.Right.IsCharging != info.Right.IsCharging)
            RightBatteryText.Text = info.Right.Display;
        if (previous?.Case.Percent != info.Case.Percent || previous?.Case.IsCharging != info.Case.IsCharging)
            CaseBatteryText.Text = info.Case.Display;

        if (previous?.Left.IsInEar != info.Left.IsInEar || previous?.Left.IsCharging != info.Left.IsCharging)
            LeftInEarText.Text = info.Left.IsInEar ? "入耳" : info.Left.IsCharging ? "充电中" : "";
        if (previous?.Right.IsInEar != info.Right.IsInEar || previous?.Right.IsCharging != info.Right.IsCharging)
            RightInEarText.Text = info.Right.IsInEar ? "入耳" : info.Right.IsCharging ? "充电中" : "";
        if (previous?.Case.IsLidOpened != info.Case.IsLidOpened)
            LidText.Text = info.Case.IsLidOpened ? "盖子打开" : "盖子关闭";

        // Only the visual signal-strength label is throttled. Battery, lid and
        // ear-detection updates above, and core/media events, remain immediate.
        _pendingRssi = info.Rssi;
        if (previous is null || previous.Model != info.Model) FlushRssi();
        else if (_displayedRssi == info.Rssi)
        {
            _rssiTimer.Stop();
            _pendingRssi = null;
        }
        else if (!_rssiTimer.IsEnabled) _rssiTimer.Start();
    }

    private void FlushRssi()
    {
        _rssiTimer.Stop();
        if (_renderedState != null && _pendingRssi is short rssi && _displayedRssi != rssi)
        {
            RssiText.Text = $"RSSI {rssi} dBm";
            _displayedRssi = rssi;
        }
        _pendingRssi = null;
    }

    private void StartScan_Click(object sender, RoutedEventArgs e) => App.Core.StartScanner();
    private void StopScan_Click(object sender, RoutedEventArgs e) => App.Core.StopScanner();

    private void BindDevice_Click(object sender, RoutedEventArgs e)
    {
        var s = App.Core.GetSettings();
        var address = Native.apd_get_device_address();
        if (address == 0) { StatusText.Text = "请先等待扫描到设备"; return; }
        App.Core.SetSettings(s with { BoundDeviceAddress = address });
        StatusText.Text = $"已锁定广播地址 {address:X12}；地址变化后请解除锁定";
    }

    private void UnbindDevice_Click(object sender, RoutedEventArgs e)
    {
        App.Core.SetSettings(App.Core.GetSettings() with { BoundDeviceAddress = 0 });
        StatusText.Text = "已解除地址锁定，正在扫描…";
    }
}
