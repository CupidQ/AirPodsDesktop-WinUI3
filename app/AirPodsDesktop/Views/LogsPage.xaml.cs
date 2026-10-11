using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace AirPodsDesktop.Views;

public sealed partial class LogsPage : Page
{
    private static string LogDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "AirPodsDesktop", "logs");

    public LogsPage()
    {
        InitializeComponent();
        Loaded += (_, _) => Refresh();
    }

    private void Refresh()
    {
        try
        {
            if (!Directory.Exists(LogDir))
            {
                LogText.Text = "（暂无日志）";
                return;
            }
            var file = Directory.GetFiles(LogDir, "apd-*.log")
                .OrderByDescending(File.GetLastWriteTimeUtc)
                .FirstOrDefault();
            if (file is null)
            {
                LogText.Text = "（暂无日志）";
                return;
            }
            // The logger keeps this file open for writing. Allow reading while it
            // appends or rolls the file, and retain only the most recent lines.
            using var stream = new FileStream(file, FileMode.Open, FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete);
            using var reader = new StreamReader(stream);
            var lines = new Queue<string>();
            while (reader.ReadLine() is { } line)
            {
                lines.Enqueue(line);
                if (lines.Count > 200) lines.Dequeue();
            }
            LogText.Text = string.Join(Environment.NewLine, lines);
        }
        catch (Exception ex)
        {
            LogText.Text = $"读取日志失败: {ex.Message}";
        }
    }

    private void Refresh_Click(object sender, RoutedEventArgs e) => Refresh();

    private async void OpenFolder_Click(object sender, RoutedEventArgs e)
    {
        Directory.CreateDirectory(LogDir);
        await Windows.System.Launcher.LaunchFolderAsync(
            await Windows.Storage.StorageFolder.GetFolderFromPathAsync(LogDir));
    }
}
