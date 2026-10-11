using AirPodsDesktop.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace AirPodsDesktop.Views;

public sealed partial class DevicePage : Page
{
    public DevicePage()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            App.Core.StateUpdated += OnState;
            App.Core.Lost += OnLost;
            App.Core.Disconnected += OnLost;
            App.Core.ScannerStatusChanged += OnScannerStatus;
            App.Core.SettingsChanged += OnSettings;
            Render(App.Core.GetState());
        };
        Unloaded += (_, _) =>
        {
            App.Core.StateUpdated -= OnState;
            App.Core.Lost -= OnLost;
            App.Core.Disconnected -= OnLost;
            App.Core.ScannerStatusChanged -= OnScannerStatus;
            App.Core.SettingsChanged -= OnSettings;
        };
    }

    private void OnState(object? sender, DeviceInfo? info) => Render(info);

    private void OnLost(object? sender, EventArgs e) => Render(null);
    private void OnScannerStatus(object? sender, bool running) => Render(App.Core.GetState());
    private void OnSettings(object? sender, AppSettings settings) => UpdateActions();

    private void Render(DeviceInfo? info)
    {
        if (!DispatcherQueue.HasThreadAccess)
        {
            _ = DispatcherQueue.TryEnqueue(() => Render(info));
            return;
        }
        UpdateActions();
        if (info is null)
        {
            StatusBadgeText.Text = App.Core.IsScannerRunning ? "正在寻找" : "扫描已暂停";
            ModelText.Text = "你的 AirPods";
            DeviceHintText.Text = "打开充电盒，让耳机靠近电脑。";
            RssiText.Text = "尚未收到设备信号";
            StatusText.Text = App.Core.IsScannerRunning
                ? "正在接收附近的广播，发现设备后会自动更新。"
                : "开始扫描，查看附近 AirPods 的电量。";
            LeftBatteryBar.Value = RightBatteryBar.Value = CaseBatteryBar.Value = 0;
            LeftBatteryText.Text = RightBatteryText.Text = CaseBatteryText.Text = "—";
            LeftInEarText.Text = RightInEarText.Text = LidText.Text = "等待电量";
            BindDeviceButton.IsEnabled = false;
            return;
        }

        StatusBadgeText.Text = App.Core.IsScannerRunning ? "设备已发现" : "扫描已暂停";
        ModelText.Text = info.ModelName;
        DeviceHintText.Text = "电量和佩戴状态会在这里更新。";
        RssiText.Text = $"信号强度 · {info.Rssi} dBm";
        StatusText.Text = App.Core.IsScannerRunning ? "正在接收设备广播，电量会自动更新。" : "扫描已暂停，显示最后收到的电量。";
        BindDeviceButton.IsEnabled = true;

        LeftBatteryBar.Value = info.Left.IsAvailable ? info.Left.Percent : 0;
        RightBatteryBar.Value = info.Right.IsAvailable ? info.Right.Percent : 0;
        CaseBatteryBar.Value = info.Case.IsAvailable ? info.Case.Percent : 0;

        LeftBatteryText.Text = info.Left.IsAvailable ? info.Left.Display : "—";
        RightBatteryText.Text = info.Right.IsAvailable ? info.Right.Display : "—";
        CaseBatteryText.Text = info.Case.IsAvailable ? info.Case.Display : "—";

        LeftInEarText.Text = info.Left.IsInEar ? "正在佩戴" : info.Left.IsCharging ? "充电中" : info.Left.IsAvailable ? "未佩戴" : "等待电量";
        RightInEarText.Text = info.Right.IsInEar ? "正在佩戴" : info.Right.IsCharging ? "充电中" : info.Right.IsAvailable ? "未佩戴" : "等待电量";
        LidText.Text = info.Case.IsLidOpened ? "盒盖已打开" : "盒盖已合上";
    }

    private void UpdateActions()
    {
        StartScanButton.IsEnabled = !App.Core.IsScannerRunning;
        StopScanButton.IsEnabled = App.Core.IsScannerRunning;
        UnbindDeviceButton.IsEnabled = App.Core.GetSettings().BoundDeviceAddress != 0;
    }

    private void DeviceHero_SizeChanged(object sender, SizeChangedEventArgs e)
        => EarbudsArtwork.Visibility = e.NewSize.Width >= 440 ? Visibility.Visible : Visibility.Collapsed;

    private void BatteryTiles_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        var columns = e.NewSize.Width >= 480 ? 3 : 1;
        Border[] tiles = [LeftTile, RightTile, CaseTile];
        for (var i = 0; i < tiles.Length; i++)
        {
            Grid.SetRow(tiles[i], i / columns);
            Grid.SetColumn(tiles[i], i % columns);
            Grid.SetColumnSpan(tiles[i], 3 / columns);
        }
    }

    private void StartScan_Click(object sender, RoutedEventArgs e) => App.Core.StartScanner();
    private void StopScan_Click(object sender, RoutedEventArgs e) => App.Core.StopScanner();

    private void ScanActions_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        // Use the available content width, which also accounts for the navigation pane.
        var columns = e.NewSize.Width >= 360 ? 2 : 1;
        Button[] buttons = [StartScanButton, StopScanButton];
        for (var i = 0; i < buttons.Length; i++)
        {
            Grid.SetRow(buttons[i], i / columns);
            Grid.SetColumn(buttons[i], i % columns);
            Grid.SetColumnSpan(buttons[i], 2 / columns);
        }
    }

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
