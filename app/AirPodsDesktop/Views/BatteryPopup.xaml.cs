using AirPodsDesktop.Services;
using Microsoft.UI.Xaml.Controls;

namespace AirPodsDesktop.Views;

public sealed partial class BatteryPopup : ContentDialog
{
    public BatteryPopup()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            var info = App.Core.GetState();
            if (info is null) return;
            PopupLeft.Text = $"左耳 {info.Left.Display}";
            PopupRight.Text = $"右耳 {info.Right.Display}";
            PopupCase.Text = $"盒 {info.Case.Display}";
            PopupLeftBar.Value = Math.Max(0, info.Left.Percent);
            PopupRightBar.Value = Math.Max(0, info.Right.Percent);
            PopupCaseBar.Value = Math.Max(0, info.Case.Percent);
        };
    }
}
