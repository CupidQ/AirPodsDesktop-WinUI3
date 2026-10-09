using AirPodsDesktop.Services;
using Microsoft.UI.Xaml;
using Serilog;
using Windows.ApplicationModel;

namespace AirPodsDesktop;

public partial class App : Application
{
    public static Window? MainWindow { get; private set; }
    public static CoreBridge Core => CoreBridge.Instance;

    public App()
    {
        InitializeComponent();
        UnhandledException += OnUnhandledException;

        Log.Logger = new LoggerConfiguration()
            .MinimumLevel.Debug()
            .WriteTo.Debug()
            .WriteTo.File(
                System.IO.Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "AirPodsDesktop", "logs", "apd-.log"),
                rollingInterval: RollingInterval.Day)
            .CreateLogger();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Core.Initialize();

        MainWindow = new MainWindow();
        ApplyTheme(Core.GetTheme());
        MainWindow.Activate();

        Core.StartScanner();
    }

    public static void ApplyTheme(string theme)
    {
        if (MainWindow?.Content is FrameworkElement root)
            root.RequestedTheme = theme switch
            {
                "light" => ElementTheme.Light,
                "dark" => ElementTheme.Dark,
                _ => ElementTheme.Default,
            };
    }

    private void OnUnhandledException(object sender, Microsoft.UI.Xaml.UnhandledExceptionEventArgs e)
    {
        Log.Fatal(e.Exception, "Unhandled exception");
        Log.CloseAndFlush();
    }
}
