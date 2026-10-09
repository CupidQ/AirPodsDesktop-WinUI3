using AirPodsDesktop.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace AirPodsDesktop.Views;

public sealed partial class BatteryPage : Page
{
    private bool _loading = true;
    public BatteryPage()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            App.Core.StateUpdated += OnState;
            App.Core.Lost += OnLost;
            App.Core.SettingsChanged += OnSettings;
            LoadSettings();
            Render(App.Core.GetState());
        };
        Unloaded += (_, _) =>
        {
            App.Core.StateUpdated -= OnState;
            App.Core.Lost -= OnLost;
            App.Core.SettingsChanged -= OnSettings;
        };
    }

    private void OnState(object? sender, DeviceInfo? info) => Render(info);
    private void OnLost(object? sender, EventArgs args) => Render(null);
    private void OnSettings(object? sender, AppSettings settings) => LoadSettings();

    private void LoadSettings()
    {
        _loading = true;
        var s = App.Core.GetSettings();
        EarDetectionSwitch.IsOn = s.AutomaticEarDetection;
        LowLatencySwitch.IsOn = false;
        AutoStartSwitch.IsOn = s.AutoStart;
        PopupSwitch.IsOn = s.ShowPopupOnConnect;
        RssiMinBox.Value = s.RssiMin;

        var theme = App.Core.GetTheme();
        foreach (var item in ThemeCombo.Items.OfType<ComboBoxItem>())
        {
            if ((item.Tag as string) == theme)
            {
                ThemeCombo.SelectedItem = item;
                break;
            }
        }
        _loading = false;
    }

    private void SaveSettings(AppSettings settings)
    {
        try
        {
            App.Core.SetSettings(settings);
            SettingsStatus.Text = "已保存";
        }
        catch (Exception error)
        {
            LoadSettings();
            SettingsStatus.Text = $"设置未保存：{error.Message}";
        }
    }

    private void Render(DeviceInfo? info)
    {
        if (info is null)
        {
            BatteryList.ItemsSource = null;
            return;
        }

        BatteryList.ItemsSource = new[]
        {
            new { Title = "左耳", Display = info.Left.Display, Percent = Math.Max(0.0, info.Left.Percent) },
            new { Title = "右耳", Display = info.Right.Display, Percent = Math.Max(0.0, info.Right.Percent) },
            new { Title = "充电盒", Display = info.Case.Display, Percent = Math.Max(0.0, info.Case.Percent) },
        };
    }

    private void EarDetection_Toggled(object sender, RoutedEventArgs e)
    {
        if (_loading) return;
        var s = App.Core.GetSettings();
        SaveSettings(s with { AutomaticEarDetection = EarDetectionSwitch.IsOn });
    }

    private void AutoStart_Toggled(object sender, RoutedEventArgs e)
    {
        if (_loading) return;
        var s = App.Core.GetSettings();
        SaveSettings(s with { AutoStart = AutoStartSwitch.IsOn });
    }

    private void Popup_Toggled(object sender, RoutedEventArgs e)
    {
        if (_loading) return;
        var s = App.Core.GetSettings();
        SaveSettings(s with { ShowPopupOnConnect = PopupSwitch.IsOn });
    }

    private void RssiMin_Changed(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (_loading || !double.IsFinite(args.NewValue)) return;
        var s = App.Core.GetSettings();
        SaveSettings(s with { RssiMin = (short)Math.Clamp(args.NewValue, -100, 0) });
    }

    private void Theme_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_loading) return;
        if (ThemeCombo.SelectedItem is ComboBoxItem item && item.Tag is string tag)
        {
            App.Core.SetTheme(tag);
        }
    }
}
