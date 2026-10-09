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
            Render(App.Core.GetState());
        };
        Unloaded += (_, _) =>
        {
            App.Core.StateUpdated -= OnState;
            App.Core.Lost -= OnLost;
        };
    }

    private void OnState(object? sender, DeviceInfo? info) => Render(info);

    private void OnLost(object? sender, EventArgs e)
    {
        _ = DispatcherQueue.TryEnqueue(() =>
        {
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
        });
    }

    private void Render(DeviceInfo? info)
    {
        if (info is null) { OnLost(this, EventArgs.Empty); return; }

        StatusText.Text = "已连接";
        ModelText.Text = info.ModelName;
        RssiText.Text = $"RSSI {info.Rssi} dBm";

        LeftBatteryBar.Value = info.Left.IsAvailable ? info.Left.Percent : 0;
        RightBatteryBar.Value = info.Right.IsAvailable ? info.Right.Percent : 0;
        CaseBatteryBar.Value = info.Case.IsAvailable ? info.Case.Percent : 0;

        LeftBatteryText.Text = info.Left.Display;
        RightBatteryText.Text = info.Right.Display;
        CaseBatteryText.Text = info.Case.Display;

        LeftInEarText.Text = info.Left.IsInEar ? "入耳" : info.Left.IsCharging ? "充电中" : "";
        RightInEarText.Text = info.Right.IsInEar ? "入耳" : info.Right.IsCharging ? "充电中" : "";
        LidText.Text = info.Case.IsLidOpened ? "盖子打开" : "盖子关闭";
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
