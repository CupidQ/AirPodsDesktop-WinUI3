using AirPodsDesktop.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace AirPodsDesktop.Views;

public sealed partial class ControlPage : Page
{
    private readonly DispatcherTimer _pollTimer = new() { Interval = TimeSpan.FromSeconds(1) };
    private bool _loaded, _busy, _polling, _rendering;
    private int _version;
    private ControlSnapshot? _snapshot;

    public ControlPage()
    {
        InitializeComponent();
        _pollTimer.Tick += async (_, _) => await PollAsync();
        Loaded += async (_, _) =>
        {
            _loaded = true;
            ++_version;
            await RefreshDevicesAsync();
            if (_loaded) _pollTimer.Start();
        };
        Unloaded += (_, _) => { _loaded = false; ++_version; _pollTimer.Stop(); };
    }

    private void UpdateEnabled()
    {
        var connected = _snapshot?.Phase == "connected";
        DeviceSelector.IsEnabled = !_busy;
        RefreshButton.IsEnabled = !_busy;
        ConnectButton.IsEnabled = !_busy && DeviceSelector.Items.Count > 0;
        DisconnectButton.IsEnabled = !_busy && (_snapshot?.Phase is "connected" or "error" or "connecting");
        ModeSelector.IsEnabled = !_busy && connected && _snapshot?.ListeningMode != null;
        AdaptiveOption.IsEnabled = _snapshot?.AdaptiveSupported == true;
        ConversationCheck.IsEnabled = !_busy && connected && _snapshot?.ConversationAwareness != null;
        OneEarCheck.IsEnabled = !_busy && connected && _snapshot?.OneEarAnc != null;
        DuckCheck.IsEnabled = !_busy && connected && _snapshot?.ConversationAwareness == true;
        BusyRing.IsActive = _busy;
    }

    private void Render(ControlSnapshot state)
    {
        if (!_loaded) return;
        _snapshot = state;
        _rendering = true;
        try
        {
            AdaptiveOption.IsEnabled = state.AdaptiveSupported;
            ConnectionText.Text = state.Device is null ? state.Message : $"{state.Device.Name}：{state.Message}";
            ModeText.Text = "当前听音模式：" + (state.ListeningMode switch
            { 1 => "关闭", 2 => "降噪", 3 => "通透", 4 => "自适应", _ => "等待耳机回传" });
            ModeSelector.SelectedIndex = state.ListeningMode is >= 2 and <= 4 ? state.ListeningMode.Value - 2 : -1;
            ConversationCheck.IsChecked = state.ConversationAwareness;
            OneEarCheck.IsChecked = state.OneEarAnc;
            DuckCheck.IsChecked = state.DuckEnabled;
            SpeakingText.Text = "对话状态：" + (state.Speaking switch { true => "正在对话", false => "未在对话", _ => "等待耳机回传" });
            VolumeText.Text = state.VolumeMessage ?? "";
        }
        finally { _rendering = false; UpdateEnabled(); }
    }

    private async Task PollAsync()
    {
        if (!_loaded || _busy || _polling) return;
        _polling = true;
        var version = _version;
        try
        {
            var state = await App.Controls.StateAsync();
            if (_loaded && version == _version && !_busy && state != _snapshot) Render(state);
        }
        catch (Exception ex) { if (_loaded && version == _version) ConnectionText.Text = ex.Message; }
        finally { _polling = false; }
    }

    private async Task RefreshDevicesAsync()
    {
        var version = _version + 1;
        await PerformAsync(async () =>
        {
            var selected = (DeviceSelector.SelectedItem as ControlEndpoint)?.Path;
            var devices = await App.Controls.ListDevicesAsync();
            if (!_loaded || version != _version) return;
            DeviceSelector.Items.Clear();
            foreach (var device in devices.Devices) DeviceSelector.Items.Add(device);
            DeviceSelector.SelectedItem = devices.Devices.FirstOrDefault(d => d.Path == selected) ?? devices.Devices.FirstOrDefault();
            DriverText.Text = devices.Error ?? (devices.Devices.Length == 0
                ? "未发现 MagicAAP 控制设备。请检查驱动是否安装，以及 AirPods 是否已配对并连接。"
                : "已发现控制接口。请选择正确的耳机；通道被其他软件占用时，请先退出该软件。");
            return;
        }, showSuccess: false);
    }

    private async Task PerformAsync(Func<Task> operation, bool showSuccess)
    {
        if (_busy || !_loaded) return;
        _busy = true;
        var version = ++_version;
        UpdateEnabled();
        if (showSuccess) ActionText.Text = "正在等待耳机确认…";
        try
        {
            await operation();
            var state = await App.Controls.StateAsync();
            if (_loaded && version == _version)
            {
                Render(state);
                if (showSuccess) ActionText.Text = "耳机已确认设置";
            }
        }
        catch (Exception ex)
        {
            if (_loaded && version == _version) ActionText.Text = ex.Message;
            try
            {
                var state = await App.Controls.StateAsync();
                if (_loaded && version == _version) Render(state);
            }
            catch { /* Preserve the original device/ABI error. */ }
        }
        finally { _busy = false; if (_loaded) UpdateEnabled(); }
    }

    private static void Require(ControlResult result)
    {
        if (!result.Ok) throw new InvalidOperationException(result.Error ?? "耳机控制失败");
    }

    private Task SetAsync(uint setting, uint value) => PerformAsync(async () =>
        Require(await App.Controls.SetAsync(setting, value)), showSuccess: true);

    private async void Refresh_Click(object sender, RoutedEventArgs e) => await RefreshDevicesAsync();
    private async void Connect_Click(object sender, RoutedEventArgs e)
    {
        if (DeviceSelector.SelectedItem is not ControlEndpoint device) return;
        await PerformAsync(async () => Require(await App.Controls.ConnectAsync(device.Path)), showSuccess: false);
    }
    private async void Disconnect_Click(object sender, RoutedEventArgs e)
        => await PerformAsync(async () => Require(await App.Controls.DisconnectAsync()), showSuccess: false);

    private async void Mode_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (_rendering || !_loaded || _busy || ModeSelector.SelectedItem is not ComboBoxItem item) return;
        if (uint.TryParse(item.Tag?.ToString(), out var mode)) await SetAsync(0, mode);
    }
    private async void Conversation_Click(object sender, RoutedEventArgs e) => await SetAsync(1, ConversationCheck.IsChecked == true ? 1u : 0u);
    private async void OneEar_Click(object sender, RoutedEventArgs e) => await SetAsync(2, OneEarCheck.IsChecked == true ? 1u : 0u);
    private async void Duck_Click(object sender, RoutedEventArgs e)
        => await PerformAsync(async () => Require(await App.Controls.SetDuckAsync(DuckCheck.IsChecked == true)), showSuccess: false);

    private async void DriverHelp_Click(object sender, RoutedEventArgs e)
        => await Windows.System.Launcher.LaunchUriAsync(new Uri("https://help.magicpods.app/fun-magicaap/"));
}
