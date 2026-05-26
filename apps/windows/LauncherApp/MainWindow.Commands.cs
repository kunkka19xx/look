using System;
using System.Linq;
using System.Threading.Tasks;
using LauncherApp.Commands;
using LauncherApp.Core;
using Microsoft.UI.Xaml;

namespace LauncherApp;

// Command mode orchestration:
//   - EnterCommandScreen / EnterHelpScreen entry points
//   - CommandPanelsPanel event handlers (text / active command / kill selection / confirm)
//   - ConfirmPendingKill / CancelPendingKill and the RefreshKillCandidates pipeline
//   - RunCommandAsync (actual execution) and UpdateCommandPreview (live dry-run)
//   - ResolveCommandId / ResolveCommandArgs / GetCommandInputBody parsers
public sealed partial class MainWindow
{
    private void EnterCommandScreen()
    {
        CommandPanelsPanel.CommandInputText = string.Empty;
        SetMode(LauncherMode.Command);
        RefreshResults(string.Empty);
        CommandPanelsPanel.SelectPanel("command:calc");
        UpdateCommandPreview();
        CommandPanelsPanel.FocusCommandInput();
    }

    // Inline `:cmdid` quick-access from the home screen. Mirrors macOS
    // enterCommandMode(commandID:prefilledInput:) in LauncherView+CommandMode.swift.
    // Order matters: SetMode flips _mode to Command before we touch CommandInputText so the
    // CommandTextChanged handler routes through the Command branch (and so that clearing
    // QueryInput.Text below doesn't re-enter the inline trigger which is gated on _mode).
    private void EnterCommandScreen(string commandId, string prefilledInput)
    {
        SetMode(LauncherMode.Command);
        CommandPanelsPanel.SelectPanel(commandId);
        CommandPanelsPanel.CommandInputText = prefilledInput ?? string.Empty;
        QueryInput.Text = string.Empty;
        UpdateCommandPreview();
        CommandPanelsPanel.FocusCommandInput();
    }

    // Detects `:cmdid<space>...` (live trigger) and bare `:cmdid` (submit-only trigger).
    // Returns false unless the token between `:` and the first whitespace is an exact match
    // for a built-in command id (calc / shell / kill / sys). Mirrors macOS
    // extractInlineCommand in LauncherView+CommandMode.swift.
    internal static bool TryExtractInlineCommand(
        string input,
        out string commandId,
        out string args,
        out bool hasSpace)
    {
        commandId = string.Empty;
        args = string.Empty;
        hasSpace = false;

        if (string.IsNullOrEmpty(input) || input[0] != ':')
        {
            return false;
        }

        string body = input.Substring(1);
        int spaceIdx = -1;
        for (int i = 0; i < body.Length; i++)
        {
            if (char.IsWhiteSpace(body[i]))
            {
                spaceIdx = i;
                break;
            }
        }

        string id;
        if (spaceIdx >= 0)
        {
            id = body.Substring(0, spaceIdx).ToLowerInvariant();
            args = body.Substring(spaceIdx + 1);
            hasSpace = true;
        }
        else
        {
            id = body.ToLowerInvariant();
            args = string.Empty;
            hasSpace = false;
        }

        switch (id)
        {
            case "calc":
            case "shell":
            case "kill":
            case "sys":
                commandId = "command:" + id;
                return true;
            default:
                return false;
        }
    }

    private void CommandPanelsPanel_OnCommandTextChanged(object? sender, EventArgs e)
    {
        if (_mode != LauncherMode.Command)
        {
            return;
        }

        RefreshResults(CommandPanelsPanel.CommandInputText);
        UpdateCommandPreview();
    }

    private void CommandPanelsPanel_OnActiveCommandChanged(object? sender, EventArgs e)
    {
        if (_mode != LauncherMode.Command)
        {
            return;
        }

        UpdateCommandPreview();
        CommandPanelsPanel.FocusCommandInput();
    }

    private void CommandPanelsPanel_OnKillSelectionChanged(object? sender, EventArgs e)
    {
        if (_mode != LauncherMode.Command || CommandPanelsPanel.ActiveCommandId != "command:kill")
        {
            return;
        }

        var selected = CommandPanelsPanel.GetSelectedKillCandidate();
        if (selected is null)
        {
            _pendingKillTarget = null;
            CommandPanelsPanel.HideKillConfirmation();
            return;
        }

        _pendingKillTarget = selected.App;
        CommandPanelsPanel.ShowKillConfirmation(selected);
        HintText.Text = "Press Y to confirm kill, N to cancel";
    }

    private void CommandPanelsPanel_OnKillCandidateInvoked(object? sender, EventArgs e)
    {
        if (_mode != LauncherMode.Command || CommandPanelsPanel.ActiveCommandId != "command:kill")
        {
            return;
        }

        var selected = CommandPanelsPanel.GetSelectedKillCandidate();
        if (selected is null)
        {
            return;
        }

        _pendingKillTarget = selected.App;
        CommandPanelsPanel.ShowKillConfirmation(selected);
        HintText.Text = "Press Y to confirm kill, N to cancel";
    }

    private void CommandPanelsPanel_OnKillConfirmAccepted(object? sender, EventArgs e)
    {
        if (_mode == LauncherMode.Command && CommandPanelsPanel.ActiveCommandId == "command:kill")
        {
            ConfirmPendingKill();
        }
    }

    private void CommandPanelsPanel_OnKillConfirmCancelled(object? sender, EventArgs e)
    {
        if (_mode == LauncherMode.Command && CommandPanelsPanel.ActiveCommandId == "command:kill")
        {
            CancelPendingKill();
        }
    }

    private void EnterHelpScreen()
    {
        if (_mode == LauncherMode.Help)
        {
            QueryInput.Text = string.Empty;
            SetMode(LauncherMode.Search);
            RefreshResults(string.Empty);
            QueryInput.Focus(FocusState.Programmatic);
            return;
        }

        QueryInput.Text = string.Empty;
        SetMode(LauncherMode.Help);
        QueryInput.Focus(FocusState.Programmatic);
    }

    private void ConfirmPendingKill()
    {
        if (_pendingKillTarget is null)
        {
            CommandPanelsPanel.SetExecutionFeedback("Select a running app first", isError: true);
            return;
        }

        var target = _pendingKillTarget;
        var killResult = KillCommand.ConfirmKill(target);
        _pendingKillTarget = null;
        string args = ResolveCommandArgs("command:kill");
        string refreshQuery = args;
        if (killResult.ok && !string.IsNullOrWhiteSpace(args))
        {
            CommandPanelsPanel.CommandInputText = string.Empty;
            refreshQuery = string.Empty;
        }

        RefreshKillCandidates(refreshQuery, preserveFeedback: true);
        CommandPanelsPanel.HideKillConfirmation();
        CommandPanelsPanel.SetExecutionFeedback(killResult.message, isError: !killResult.ok);
        HintText.Text = killResult.ok ? "kill executed" : "kill failed";
        CommandPanelsPanel.FocusCommandInput();

        if (killResult.ok)
        {
            _ = RefreshKillCandidatesAfterDelayAsync(refreshQuery, 260);
        }
    }

    private void CancelPendingKill()
    {
        _pendingKillTarget = null;
        CommandPanelsPanel.HideKillConfirmation();
        CommandPanelsPanel.SetExecutionFeedback("Kill canceled");
        HintText.Text = "kill canceled";
    }

    private async Task RefreshKillCandidatesAfterDelayAsync(string query, int delayMs)
    {
        await Task.Delay(delayMs);
        if (_mode != LauncherMode.Command || CommandPanelsPanel.ActiveCommandId != "command:kill")
        {
            return;
        }

        DispatcherQueue.TryEnqueue(() => RefreshKillCandidates(query, preserveFeedback: true));
    }

    private void RefreshKillCandidates(string query, bool preserveFeedback = false)
    {
        var selectedCandidate = CommandPanelsPanel.GetSelectedKillCandidate();
        int? selectedPid = selectedCandidate?.App.Pid;
        bool isPortQuery = query.TrimStart().StartsWith(":", StringComparison.Ordinal)
            || query.TrimStart().StartsWith("port ", StringComparison.OrdinalIgnoreCase);

        var apps = KillCommand.ListRunningApps(query).Take(20).ToList();
        var items = apps.Select(app => new KillCandidateItem(app)).ToList();

        int? selectedNumber = selectedPid is int pid
            ? items.FirstOrDefault(x => x.App.Pid == pid)?.Number
            : null;

        CommandPanelsPanel.SetKillCandidates(items, selectedNumber);

        foreach (var item in items.Take(12))
        {
            _ = item.LoadIconAsync();
        }

        if (items.Count == 0)
        {
            _pendingKillTarget = null;
            CommandPanelsPanel.HideKillConfirmation();
            CommandPanelsPanel.SetKillEmptyMessage(
                isPortQuery
                    ? "No process listening on this port"
                    : "No matching apps. Type app name or use :3000");
            if (!preserveFeedback)
            {
                CommandPanelsPanel.SetExecutionFeedback(
                    isPortQuery ? "No process listening on this port" : "No matching apps. Type app name or use :3000",
                    isError: true);
                HintText.Text = "No running match";
            }
            return;
        }

        CommandPanelsPanel.SetKillEmptyMessage(null);

        if (_pendingKillTarget is not null)
        {
            var pending = items.FirstOrDefault(x => x.App.Pid == _pendingKillTarget.Pid);
            if (pending is null)
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.HideKillConfirmation();
            }
            else
            {
                CommandPanelsPanel.ShowKillConfirmation(pending);
            }
        }

        if (!preserveFeedback)
        {
            if (string.IsNullOrWhiteSpace(query))
            {
                CommandPanelsPanel.SetExecutionFeedback("Running apps. Use Up/Down or mouse to select, then confirm.");
            }
            else
            {
                CommandPanelsPanel.SetExecutionFeedback("Filtered running apps. Select one and confirm kill.");
            }
        }
    }

    private async Task RunCommandAsync(string id)
    {
        string resolvedCommandId = ResolveCommandId(id);
        string args = ResolveCommandArgs(resolvedCommandId);
        CommandPanelsPanel.SelectPanel(resolvedCommandId);

        if (resolvedCommandId != "command:kill")
        {
            _pendingKillTarget = null;
            CommandPanelsPanel.HideKillConfirmation();
        }

        switch (resolvedCommandId)
        {
            case "command:calc":
                if (CalcCommand.TryEvaluate(args, out string calcMessage))
                {
                    CommandPanelsPanel.SetExecutionFeedback(calcMessage);
                    HintText.Text = "calc executed";
                }
                else
                {
                    CommandPanelsPanel.SetExecutionFeedback(calcMessage, isError: true);
                    HintText.Text = "calc error";
                }
                break;
            case "command:shell":
                CommandPanelsPanel.SetExecutionFeedback("Running...");
                var shellResult = await ShellCommand.RunAsync(args);
                CommandPanelsPanel.SetExecutionFeedback(shellResult.message, isError: !shellResult.ok);
                HintText.Text = shellResult.ok ? "shell executed" : "shell failed";
                break;
            case "command:kill":
                RefreshKillCandidates(args);
                var selected = CommandPanelsPanel.GetSelectedKillCandidate();
                if (selected is null)
                {
                    HintText.Text = "kill command";
                    break;
                }

                _pendingKillTarget = selected.App;
                CommandPanelsPanel.ShowKillConfirmation(selected);
                HintText.Text = "Press Y to confirm kill, N to cancel";
                break;
            case "command:sys":
                string sysSummary = SystemInfoCommand.BuildSummary();
                CommandPanelsPanel.SetExecutionFeedback(sysSummary);
                HintText.Text = "sys info updated";
                break;
            default:
                CommandPanelsPanel.SetExecutionFeedback("Unknown command. Try shell, calc, kill, or sys", isError: true);
                HintText.Text = "Unknown command";
                break;
        }
    }

    private void UpdateCommandPreview()
    {
        if (_mode != LauncherMode.Command)
        {
            return;
        }

        try
        {
            string resolvedCommandId = ResolveCommandId(CommandPanelsPanel.ActiveCommandId);
            string args = ResolveCommandArgs(resolvedCommandId);
            CommandPanelsPanel.SelectPanel(resolvedCommandId);

            if (resolvedCommandId != "command:kill")
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.HideKillConfirmation();
            }

            switch (resolvedCommandId)
            {
                case "command:calc":
                    if (string.IsNullOrWhiteSpace(args))
                    {
                        CommandPanelsPanel.SetExecutionFeedback(string.Empty);
                        return;
                    }

                    if (CalcCommand.TryEvaluate(args, out string calcMessage))
                    {
                        CommandPanelsPanel.SetExecutionFeedback(calcMessage);
                    }
                    else
                    {
                        CommandPanelsPanel.SetExecutionFeedback(calcMessage, isError: true);
                    }
                    return;
                case "command:shell":
                    CommandPanelsPanel.SetExecutionFeedback(
                        string.IsNullOrWhiteSpace(args)
                            ? "Enter to run shell command"
                            : $"Ready to run: {args}");
                    return;
                case "command:kill":
                    RefreshKillCandidates(args);
                    if (_pendingKillTarget is not null)
                    {
                        HintText.Text = "Press Y to confirm kill, N to cancel";
                    }
                    else
                    {
                        HintText.Text = "Select app, then confirm kill";
                    }
                    return;
                case "command:sys":
                    CommandPanelsPanel.SetExecutionFeedback(SystemInfoCommand.BuildSummary());
                    return;
                default:
                    CommandPanelsPanel.SetExecutionFeedback("Unknown command", isError: true);
                    return;
            }
        }
        catch (Exception ex)
        {
            _pendingKillTarget = null;
            CommandPanelsPanel.SetExecutionFeedback($"Preview failed: {ex.Message}", isError: true);
        }
    }

    private string ResolveCommandId(string fallbackId)
    {
        string body = GetCommandInputBody();
        if (body.StartsWith('/'))
            body = body[1..].Trim();

        if (string.IsNullOrWhiteSpace(body))
            return fallbackId;

        string firstToken = body.Split(' ', StringSplitOptions.RemoveEmptyEntries).FirstOrDefault() ?? string.Empty;
        return firstToken.ToLowerInvariant() switch
        {
            "calc" => "command:calc",
            "shell" => "command:shell",
            "kill" => "command:kill",
            "sys" => "command:sys",
            _ => fallbackId,
        };
    }

    private string ResolveCommandArgs(string commandId)
    {
        string body = GetCommandInputBody();
        if (body.StartsWith('/'))
            body = body[1..].Trim();

        if (string.IsNullOrWhiteSpace(body))
            return string.Empty;

        string expected = commandId switch
        {
            "command:calc" => "calc",
            "command:shell" => "shell",
            "command:kill" => "kill",
            "command:sys" => "sys",
            _ => string.Empty,
        };

        if (string.IsNullOrWhiteSpace(expected))
            return string.Empty;

        if (body.Equals(expected, StringComparison.OrdinalIgnoreCase))
            return string.Empty;

        if (body.StartsWith(expected + " ", StringComparison.OrdinalIgnoreCase))
            return body[(expected.Length + 1)..].Trim();

        if (!body.Contains(' '))
        {
            return commandId == "command:sys" && body.Equals("sys", StringComparison.OrdinalIgnoreCase)
                ? string.Empty
                : body;
        }

        return body;
    }

    private string GetCommandInputBody()
    {
        if (_mode == LauncherMode.Command)
        {
            return CommandPanelsPanel.CommandInputText.Trim();
        }

        return QueryInput.Text?.Trim() ?? string.Empty;
    }
}
