using AirPodsDesktop.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace AirPodsDesktop.Views;

public sealed partial class LogsPage : Page
{
    private readonly SemaphoreSlim _readGate = new(1, 1);
    private CancellationTokenSource? _refreshCancellation;
    private int _refreshVersion;
    private bool _isLoaded;

    private static string LogDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "AirPodsDesktop", "logs");

    public LogsPage()
    {
        InitializeComponent();
        Loaded += async (_, _) => { _isLoaded = true; await RefreshAsync(); };
        Unloaded += (_, _) =>
        {
            _isLoaded = false;
            ++_refreshVersion;
            CancelRefresh();
        };
    }

    private void CancelRefresh()
    {
        _refreshCancellation?.Cancel();
        _refreshCancellation?.Dispose();
        _refreshCancellation = null;
    }

    private async Task RefreshAsync()
    {
        var version = ++_refreshVersion;
        CancelRefresh();
        _refreshCancellation = new CancellationTokenSource();
        var token = _refreshCancellation.Token;
        try
        {
            await _readGate.WaitAsync(token).ConfigureAwait(false);
            string text;
            try
            {
                text = await Task.Run(() => ReadLatestLog(token), token).ConfigureAwait(false);
            }
            finally { _readGate.Release(); }
            Publish(text, version);
        }
        catch (OperationCanceledException) { }
        catch (Exception ex)
        {
            Publish($"读取日志失败: {ex.Message}", version);
        }
    }

    private void Publish(string text, int version)
    {
        _ = DispatcherQueue.TryEnqueue(() =>
        {
            // Old reads cannot overwrite a newer refresh or an unloaded page.
            if (_isLoaded && version == _refreshVersion) LogText.Text = text;
        });
    }

    private static string ReadLatestLog(CancellationToken token)
    {
        token.ThrowIfCancellationRequested();
        if (!Directory.Exists(LogDir)) return "（暂无日志）";
        string? latest = null;
        var latestTime = DateTime.MinValue;
        foreach (var file in Directory.EnumerateFiles(LogDir, "apd-*.log"))
        {
            token.ThrowIfCancellationRequested();
            var time = File.GetLastWriteTimeUtc(file);
            if (latest is null || time > latestTime) { latest = file; latestTime = time; }
        }
        if (latest is null) return "（暂无日志）";
        token.ThrowIfCancellationRequested();
        var text = Native.ReadLogTail(latest);
        token.ThrowIfCancellationRequested();
        return text;
    }

    private async void Refresh_Click(object sender, RoutedEventArgs e) => await RefreshAsync();

    private async void OpenFolder_Click(object sender, RoutedEventArgs e)
    {
        Directory.CreateDirectory(LogDir);
        await Windows.System.Launcher.LaunchFolderAsync(
            await Windows.Storage.StorageFolder.GetFolderFromPathAsync(LogDir));
    }
}
