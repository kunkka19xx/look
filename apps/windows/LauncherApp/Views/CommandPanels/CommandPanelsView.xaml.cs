using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using LauncherApp.Core;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace LauncherApp.Views.CommandPanels;

public sealed partial class CommandPanelsView : UserControl
{
    private sealed class CommandMeta
    {
        public required string Id { get; init; }
        public required ToggleButton Card { get; init; }
        public required string Title { get; init; }
    }

    private List<CommandMeta>? _commands;
    private readonly ObservableCollection<KillCandidateItem> _killCandidates = [];
    private string _activeCommandId = "command:calc";
    private bool _isUpdatingKillSelection;

    public event EventHandler? CommandTextChanged;
    public event EventHandler? ActiveCommandChanged;
    public event EventHandler? KillSelectionChanged;
    public event EventHandler? KillCandidateInvoked;
    public event EventHandler? KillConfirmAccepted;
    public event EventHandler? KillConfirmCancelled;

    public CommandPanelsView()
    {
        this.InitializeComponent();
        KillCandidatesList.ItemsSource = _killCandidates;
        EnsureCommands();
        SelectPanel("command:calc");
    }

    public string ActiveCommandId => _commands?.FirstOrDefault(x => x.Card.IsChecked == true)?.Id ?? "command:calc";

    public string CommandInputText
    {
        get => CommandInputBox.Text ?? string.Empty;
        set
        {
            string next = value ?? string.Empty;
            if (string.Equals(CommandInputBox.Text, next, StringComparison.Ordinal))
            {
                return;
            }

            CommandInputBox.Text = next;
        }
    }

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
                || cmd.Title.Contains(normalized, StringComparison.OrdinalIgnoreCase);

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

        bool changed = !string.Equals(_activeCommandId, id, StringComparison.Ordinal);
        _activeCommandId = id;

        foreach (var cmd in _commands!)
        {
            cmd.Card.IsChecked = cmd.Id == id;
        }

        var selected = _commands!.First(x => x.Id == id);
        CommandTitleText.Text = selected.Title;
        UpdateCommandInputState(id, selected.Title);
        if (changed)
        {
            CommandOutputText.Text = string.Empty;
            CommandOutputText.Foreground = ResolveBrush("LauncherTextBrush");
            ActiveCommandChanged?.Invoke(this, EventArgs.Empty);
        }
    }

    public void SetExecutionFeedback(string message, bool isError = false)
    {
        CommandOutputText.Text = message ?? string.Empty;
        // Success results are primary content (the answer the user asked for) -> primary
        // text brush. Errors stay on the banner-error brush so they read as a warning.
        // Mirrors macOS where command result content uses fontColor() not mutedTextColor().
        string key = isError ? "LauncherBannerErrorBrush" : "LauncherTextBrush";
        CommandOutputText.Foreground = ResolveBrush(key);
    }

    public void SetKillCandidates(IReadOnlyList<KillCandidateItem> candidates, int? selectedNumber = null)
    {
        _killCandidates.Clear();
        foreach (var candidate in candidates)
        {
            _killCandidates.Add(candidate);
        }

        _isUpdatingKillSelection = true;
        try
        {
            if (selectedNumber is int number)
            {
                KillCandidatesList.SelectedItem = _killCandidates.FirstOrDefault(x => x.Number == number);
            }
            else
            {
                KillCandidatesList.SelectedItem = _killCandidates.FirstOrDefault();
            }
        }
        finally
        {
            _isUpdatingKillSelection = false;
        }
    }

    public void SetKillEmptyMessage(string? message)
    {
        if (string.IsNullOrWhiteSpace(message))
        {
            KillEmptyText.Visibility = Visibility.Collapsed;
            KillEmptyText.Text = string.Empty;
            return;
        }

        KillEmptyText.Text = message;
        KillEmptyText.Visibility = Visibility.Visible;
    }

    public void SetKillModeEnabled(bool enabled)
    {
        KillCandidatesList.Visibility = enabled ? Visibility.Visible : Visibility.Collapsed;
        CommandOutputHost.Visibility = enabled ? Visibility.Collapsed : Visibility.Visible;
        if (!enabled)
        {
            SetKillEmptyMessage(null);
        }
        if (!enabled)
        {
            HideKillConfirmation();
        }
    }

    public bool MoveKillSelection(int direction)
    {
        if (_killCandidates.Count == 0)
        {
            return false;
        }

        int current = KillCandidatesList.SelectedIndex;
        if (current < 0)
        {
            KillCandidatesList.SelectedIndex = 0;
            return true;
        }

        int next = (current + direction + _killCandidates.Count) % _killCandidates.Count;
        KillCandidatesList.SelectedIndex = next;
        KillCandidatesList.ScrollIntoView(KillCandidatesList.SelectedItem);
        return true;
    }

    public KillCandidateItem? GetSelectedKillCandidate()
    {
        return KillCandidatesList.SelectedItem as KillCandidateItem;
    }

    public void ShowKillConfirmation(KillCandidateItem candidate)
    {
        KillConfirmTitleText.Text = $"Kill {candidate.DisplayName}?";
        KillConfirmDetailText.Text = candidate.Detail;
        KillConfirmIcon.Source = candidate.Icon;
        KillConfirmBar.Visibility = Visibility.Visible;
    }

    public void HideKillConfirmation()
    {
        KillConfirmBar.Visibility = Visibility.Collapsed;
        KillConfirmTitleText.Text = "Kill app?";
        KillConfirmDetailText.Text = "Press Y to confirm, N to cancel";
        KillConfirmIcon.Source = null;
    }

    private static Brush ResolveBrush(string key)
    {
        if (Application.Current.Resources.TryGetValue(key, out object value) && value is Brush brush)
        {
            return brush;
        }

        if (Application.Current.Resources.TryGetValue("LauncherMutedTextBrush", out object fallbackValue)
            && fallbackValue is Brush fallbackBrush)
        {
            return fallbackBrush;
        }

        return new SolidColorBrush(Windows.UI.Color.FromArgb(255, 189, 198, 211));
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

    public void FocusCommandInput()
    {
        if (!CommandInputBox.IsEnabled)
        {
            return;
        }

        CommandInputBox.Focus(FocusState.Programmatic);
        CommandInputBox.SelectionStart = CommandInputBox.Text?.Length ?? 0;
    }

    private void CommandInputBox_OnTextChanged(object sender, TextChangedEventArgs e)
    {
        CommandTextChanged?.Invoke(this, EventArgs.Empty);
    }

    private void CommandInputBox_OnPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (KillConfirmBar.Visibility != Visibility.Visible)
        {
            return;
        }

        if (e.Key == VirtualKey.Y)
        {
            e.Handled = true;
            KillConfirmAccepted?.Invoke(this, EventArgs.Empty);
            return;
        }

        if (e.Key == VirtualKey.N)
        {
            e.Handled = true;
            KillConfirmCancelled?.Invoke(this, EventArgs.Empty);
        }
    }

    private void KillCandidatesList_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_isUpdatingKillSelection)
        {
            return;
        }

        KillSelectionChanged?.Invoke(this, EventArgs.Empty);
    }

    private void KillCandidatesList_OnItemClick(object sender, ItemClickEventArgs e)
    {
        KillCandidateInvoked?.Invoke(this, EventArgs.Empty);
    }

    private void KillConfirmYes_OnClick(object sender, RoutedEventArgs e)
    {
        KillConfirmAccepted?.Invoke(this, EventArgs.Empty);
    }

    private void KillConfirmNo_OnClick(object sender, RoutedEventArgs e)
    {
        KillConfirmCancelled?.Invoke(this, EventArgs.Empty);
    }

    private void UpdateCommandInputState(string id, string title)
    {
        bool acceptsInput = id != "command:sys";
        CommandInputBox.IsEnabled = acceptsInput;
        CommandInputBox.IsReadOnly = !acceptsInput;
        CommandInputBox.Opacity = acceptsInput ? 1.0 : 0.72;
        CommandInputBox.PlaceholderText = acceptsInput
            ? $"Type {title} arguments"
            : "sys is read-only";

        SetKillModeEnabled(id == "command:kill");
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
                Id = "command:shell",
                Card = ShellCard,
                Title = "shell"
            },
            new CommandMeta
            {
                Id = "command:calc",
                Card = CalcCard,
                Title = "calc"
            },
            new CommandMeta
            {
                Id = "command:kill",
                Card = KillCard,
                Title = "kill"
            },
            new CommandMeta
            {
                Id = "command:sys",
                Card = SysCard,
                Title = "sys"
            },
        ];
    }
}
