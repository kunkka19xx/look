using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml;

namespace LauncherApp.Views.CommandPanels;

public sealed partial class CommandPanelsView : UserControl
{
    private sealed class CommandMeta
    {
        public required string Id { get; init; }
        public required ToggleButton Card { get; init; }
        public required string Title { get; init; }
        public required string Detail { get; init; }
        public required string Example { get; init; }
    }

    private List<CommandMeta>? _commands;

    public CommandPanelsView()
    {
        this.InitializeComponent();
        EnsureCommands();
        SelectPanel("command:calc");
    }

    public string ActiveCommandId => _commands?.FirstOrDefault(x => x.Card.IsChecked == true)?.Id ?? "command:calc";

    public void ApplyFilter(string query)
    {
        EnsureCommands();
        string normalized = query.Trim().ToLowerInvariant();
        var commands = _commands!;

        bool anyVisible = false;
        foreach (var cmd in commands)
        {
            bool isVisible = string.IsNullOrWhiteSpace(normalized)
                || cmd.Id.Contains(normalized)
                || cmd.Title.Contains(normalized, StringComparison.OrdinalIgnoreCase)
                || cmd.Detail.Contains(normalized, StringComparison.OrdinalIgnoreCase);

            cmd.Card.Visibility = isVisible ? Visibility.Visible : Visibility.Collapsed;
            anyVisible |= isVisible;
        }

        if (!anyVisible)
        {
            foreach (var cmd in commands)
            {
                cmd.Card.Visibility = Visibility.Visible;
            }
        }

        if (commands.Any(x => x.Card.IsChecked == true && x.Card.Visibility == Visibility.Visible))
        {
            return;
        }

        SelectPanel(commands.First(x => x.Card.Visibility == Visibility.Visible).Id);
    }

    public void SelectPanel(string? commandId)
    {
        EnsureCommands();
        string id = commandId switch
        {
            "command:shell" => "command:shell",
            "command:kill" => "command:kill",
            "command:sys" => "command:sys",
            _ => "command:calc",
        };

        foreach (var cmd in _commands!)
        {
            cmd.Card.IsChecked = cmd.Id == id;
        }

        var selected = _commands!.First(x => x.Id == id);
        CommandTitleText.Text = selected.Title;
        CommandDetailText.Text = selected.Detail;
        CommandExampleText.Text = selected.Example;
    }

    public void MoveSelection(int direction)
    {
        EnsureCommands();
        var visible = _commands!.Where(x => x.Card.Visibility == Visibility.Visible).ToList();
        if (visible.Count == 0)
        {
            return;
        }

        int current = visible.FindIndex(x => x.Card.IsChecked == true);
        if (current < 0)
        {
            SelectPanel(visible[0].Id);
            return;
        }

        int next = (current + direction + visible.Count) % visible.Count;
        SelectPanel(visible[next].Id);
    }

    private void CommandCard_OnClick(object sender, RoutedEventArgs e)
    {
        if (sender is ToggleButton card && card == ShellCard)
        {
            SelectPanel("command:shell");
        }
        else if (sender is ToggleButton cardKill && cardKill == KillCard)
        {
            SelectPanel("command:kill");
        }
        else if (sender is ToggleButton cardSys && cardSys == SysCard)
        {
            SelectPanel("command:sys");
        }
        else
        {
            SelectPanel("command:calc");
        }
    }

    private void EnsureCommands()
    {
        if (_commands is not null)
        {
            return;
        }

        _commands =
        [
            new CommandMeta
            {
                Id = "command:calc",
                Card = CalcCard,
                Title = "/calc",
                Detail = "Evaluate math expression",
                Example = "/calc 12 * 7 + 5"
            },
            new CommandMeta
            {
                Id = "command:shell",
                Card = ShellCard,
                Title = "/shell",
                Detail = "Run shell command",
                Example = "/shell winget upgrade --all"
            },
            new CommandMeta
            {
                Id = "command:kill",
                Card = KillCard,
                Title = "/kill",
                Detail = "Force kill a running app",
                Example = "/kill notepad"
            },
            new CommandMeta
            {
                Id = "command:sys",
                Card = SysCard,
                Title = "/sys",
                Detail = "Show system information",
                Example = "/sys"
            },
        ];
    }
}
