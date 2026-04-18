using Microsoft.UI.Xaml.Controls;

namespace LauncherApp.Views.CommandPanels;

public sealed partial class CommandPanelsView : UserControl
{
    public CommandPanelsView()
    {
        this.InitializeComponent();
    }

    public void SelectPanel(string? commandId)
    {
        int idx = commandId switch
        {
            "command:shell" => 1,
            "command:kill" => 2,
            "command:sys" => 3,
            _ => 0,
        };
        CommandTabs.SelectedIndex = idx;
    }
}
