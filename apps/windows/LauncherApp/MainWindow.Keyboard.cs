using LauncherApp.Bridge;
using LauncherApp.Commands;
using LauncherApp.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace LauncherApp;

// Keyboard handling for the launcher:
//   - Global key dispatcher (Ctrl+H help, Ctrl+/ command, Ctrl+Shift+, settings)
//   - QueryInput / ResultsList event handlers
//   - Command-mode quick-select (Ctrl+1..4) and arrow/Tab navigation
//   - Shared helpers IsCtrlPressed / IsShiftPressed
public sealed partial class MainWindow
{
    private void QueryInput_OnLoaded(object sender, RoutedEventArgs e)
    {
        if (_results.Count > 0 && ResultsList.SelectedIndex < 0)
        {
            ResultsList.SelectedIndex = 0;
        }

        QueryInput.Focus(FocusState.Programmatic);
    }

    private bool IsSettingsToggleShortcut(VirtualKey key)
    {
        return key == (VirtualKey)188 && IsCtrlPressed() && IsShiftPressed();
    }

    // VK_OEM_1 (`;`) + Ctrl + Shift mirrors macOS Cmd+Shift+; in look_appApp.swift:162.
    private bool IsReloadConfigShortcut(VirtualKey key)
    {
        return key == (VirtualKey)186 && IsCtrlPressed() && IsShiftPressed();
    }

    private bool IsCommandModeShortcut(KeyRoutedEventArgs e)
    {
        if (!IsCtrlPressed())
        {
            return false;
        }

        return e.Key == VirtualKey.Divide
            || e.Key == (VirtualKey)191
            || e.KeyStatus.ScanCode == 53;
    }

    private bool TryHandleCommandQuickSelect(VirtualKey key)
    {
        if (_mode != LauncherMode.Command || !IsCtrlPressed())
        {
            return false;
        }

        string? commandId = key switch
        {
            VirtualKey.Number1 => "command:shell",
            VirtualKey.Number2 => "command:calc",
            VirtualKey.Number3 => "command:kill",
            VirtualKey.Number4 => "command:sys",
            _ => null,
        };

        if (commandId is null)
        {
            return false;
        }

        CommandPanelsPanel.SelectPanel(commandId);
        UpdateCommandPreview();
        CommandPanelsPanel.FocusCommandInput();
        return true;
    }

    private bool TryHandleCommandModeKey(VirtualKey key)
    {
        if (_mode != LauncherMode.Command)
        {
            return false;
        }

        bool inKillCommand = CommandPanelsPanel.ActiveCommandId == "command:kill";

        if (key == VirtualKey.Tab)
        {
            int direction = IsShiftPressed() ? -1 : 1;
            CommandPanelsPanel.MoveSelection(direction);
            UpdateCommandPreview();
            CommandPanelsPanel.FocusCommandInput();
            return true;
        }

        if (inKillCommand && _pendingKillTarget is not null)
        {
            if (key == VirtualKey.Y)
            {
                ConfirmPendingKill();
                return true;
            }

            if (key == VirtualKey.N)
            {
                CancelPendingKill();
                return true;
            }
        }

        if (key == VirtualKey.Down)
        {
            if (inKillCommand)
            {
                CommandPanelsPanel.MoveKillSelection(1);
            }
            else
            {
                CommandPanelsPanel.MoveSelection(1);
            }

            UpdateCommandPreview();
            CommandPanelsPanel.FocusCommandInput();
            return true;
        }

        if (key == VirtualKey.Up)
        {
            if (inKillCommand)
            {
                CommandPanelsPanel.MoveKillSelection(-1);
            }
            else
            {
                CommandPanelsPanel.MoveSelection(-1);
            }

            UpdateCommandPreview();
            CommandPanelsPanel.FocusCommandInput();
            return true;
        }

        if (key == VirtualKey.Enter)
        {
            if (inKillCommand && _pendingKillTarget is not null)
            {
                ConfirmPendingKill();
                return true;
            }

            _ = RunCommandAsync(CommandPanelsPanel.ActiveCommandId);
            return true;
        }

        if (key == VirtualKey.Escape)
        {
            if (inKillCommand && _pendingKillTarget is not null)
            {
                CancelPendingKill();
                return true;
            }

            CommandPanelsPanel.CommandInputText = string.Empty;
            SetMode(LauncherMode.Search);
            RefreshResults(string.Empty);
            QueryInput.Focus(FocusState.Programmatic);
            return true;
        }

        return false;
    }

    private void GlobalKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.H && IsCtrlPressed())
        {
            EnterHelpScreen();
            e.Handled = true;
            return;
        }

        // Zoom: Ctrl+= / Ctrl+Add for zoom-in, Ctrl+- / Ctrl+Subtract for zoom-out,
        // Ctrl+0 / Ctrl+NumPad0 for reset. Mirrors macOS Cmd+= / Cmd+- / Cmd+0
        // (look_appApp.swift:166-185, ThemeStore.swift:268-278).
        if (IsCtrlPressed())
        {
            if (e.Key == (VirtualKey)0xBB || e.Key == VirtualKey.Add)
            {
                ZoomIn();
                e.Handled = true;
                return;
            }
            if (e.Key == (VirtualKey)0xBD || e.Key == VirtualKey.Subtract)
            {
                ZoomOut();
                e.Handled = true;
                return;
            }
            if (e.Key == VirtualKey.Number0 || e.Key == VirtualKey.NumberPad0)
            {
                ResetZoom();
                e.Handled = true;
                return;
            }
        }

        if (_mode == LauncherMode.Search
            && e.Key == VirtualKey.F
            && IsCtrlPressed()
            && ResultsList.SelectedItem is LauncherRowItem revealSelected)
        {
            bool ok = _actionDispatcher.RevealResult(revealSelected.Result);
            ShowBanner(ok ? "Revealed in Explorer" : "Reveal action failed",
                ok ? BannerStyle.Info : BannerStyle.Error);
            e.Handled = true;
            return;
        }

        if (_mode == LauncherMode.Search
            && e.Key == VirtualKey.C
            && IsCtrlPressed()
            && ResultsList.SelectedItem is LauncherRowItem copySelected)
        {
            _ = HandleCopyResultAsync(copySelected.Result);
            e.Handled = true;
            return;
        }

        // Ctrl+Shift+P clears picks. Must precede the Ctrl+P branch since both match VK_P.
        if (_mode == LauncherMode.Search
            && e.Key == VirtualKey.P
            && IsCtrlPressed()
            && IsShiftPressed())
        {
            ClearPicks();
            e.Handled = true;
            return;
        }

        if (_mode == LauncherMode.Search
            && e.Key == VirtualKey.P
            && IsCtrlPressed())
        {
            _ = TogglePickForSelectedRowAsync();
            e.Handled = true;
            return;
        }

        if (_mode == LauncherMode.Clipboard
            && e.Key == VirtualKey.Delete
            && ResultsList.SelectedItem is LauncherRowItem clipDeleteSelected)
        {
            DeleteClipboardEntryByResultId(clipDeleteSelected.Result.Id);
            e.Handled = true;
            return;
        }

        if (IsCommandModeShortcut(e))
        {
            EnterCommandScreen();
            e.Handled = true;
            return;
        }

        if (TryHandleCommandQuickSelect(e.Key))
        {
            e.Handled = true;
            return;
        }

        if (TryHandleCommandModeKey(e.Key))
        {
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Escape && _mode == LauncherMode.Settings)
        {
            ToggleSettingsMode();
            e.Handled = true;
            return;
        }

        if (IsReloadConfigShortcut(e.Key))
        {
            HandleReloadConfigShortcut();
            e.Handled = true;
            return;
        }

        if (!IsSettingsToggleShortcut(e.Key))
        {
            return;
        }

        ToggleSettingsMode();
        e.Handled = true;
    }

    private void HandleReloadConfigShortcut()
    {
        bool ok;
        try
        {
            ok = FfiBindings.look_reload_config();
        }
        catch
        {
            ok = false;
        }

        ShowBanner(
            ok ? "Config reloaded" : "Config reload failed",
            ok ? BannerStyle.Info : BannerStyle.Error,
            durationSeconds: ok ? 1.4 : 3.0);
    }

    private void QueryInput_OnPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Tab || _results.Count == 0)
        {
            return;
        }

        int selected = ResultsList.SelectedIndex;
        if (selected < 0)
        {
            ResultsList.SelectedIndex = IsShiftPressed() ? _results.Count - 1 : 0;
        }
        else if (IsShiftPressed())
        {
            ResultsList.SelectedIndex = selected > 0 ? selected - 1 : _results.Count - 1;
        }
        else
        {
            ResultsList.SelectedIndex = selected < _results.Count - 1 ? selected + 1 : 0;
        }

        e.Handled = true;
    }

    private void QueryInput_OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (IsCommandModeShortcut(e))
        {
            EnterCommandScreen();
            e.Handled = true;
            return;
        }

        if (_mode == LauncherMode.Translate && e.Key == VirtualKey.Enter && !IsCtrlPressed())
        {
            _ = TriggerTranslateFromEnterAsync();
            e.Handled = true;
            return;
        }

        if (_mode == LauncherMode.Command)
        {
            if (CommandPanelsPanel.ActiveCommandId == "command:kill" && _pendingKillTarget is not null)
            {
                if (e.Key == VirtualKey.Y)
                {
                    var killResult = KillCommand.ConfirmKill(_pendingKillTarget);
                    _pendingKillTarget = null;
                    CommandPanelsPanel.SetExecutionFeedback(killResult.message, isError: !killResult.ok);
                    HintText.Text = killResult.ok ? "kill executed" : "kill failed";
                    e.Handled = true;
                    return;
                }

                if (e.Key == VirtualKey.N)
                {
                    _pendingKillTarget = null;
                    CommandPanelsPanel.SetExecutionFeedback("Kill canceled");
                    HintText.Text = "kill canceled";
                    e.Handled = true;
                    return;
                }
            }

            if (e.Key == VirtualKey.Down)
            {
                CommandPanelsPanel.MoveSelection(1);
                UpdateCommandPreview();
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Up)
            {
                CommandPanelsPanel.MoveSelection(-1);
                UpdateCommandPreview();
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Enter)
            {
                _ = RunCommandAsync(CommandPanelsPanel.ActiveCommandId);
                e.Handled = true;
                return;
            }
        }

        if (e.Key == VirtualKey.Escape)
        {
            // Mirrors macOS KeyboardSelectionMonitor.swift:135-155: in search/clipboard/
            // translate (any state where the query input is the focus) plain Escape hides
            // the launcher. Help mode is the one exception - Escape there should dismiss
            // help and go back to search rather than hiding the whole window, matching
            // macOS's `onDismissHelpIfVisible()` priority.
            if (_mode == LauncherMode.Help)
            {
                QueryInput.Text = string.Empty;
                SetMode(LauncherMode.Search);
                RefreshResults(string.Empty);
            }
            else
            {
                this.AppWindow?.Hide();
            }
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Down && _results.Count > 0)
        {
            int selected = ResultsList.SelectedIndex;
            if (selected < 0)
            {
                ResultsList.SelectedIndex = 0;
            }
            else
            {
                ResultsList.SelectedIndex = selected < _results.Count - 1 ? selected + 1 : 0;
            }
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Up && _results.Count > 0)
        {
            int selected = ResultsList.SelectedIndex;
            if (selected <= 0)
            {
                ResultsList.SelectedIndex = _results.Count - 1;
            }
            else
            {
                ResultsList.SelectedIndex = selected - 1;
            }
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Enter && IsCtrlPressed())
        {
            bool ok = _actionDispatcher.WebHandoff(QueryInput.Text ?? string.Empty);
            HintText.Text = ok
                ? "Opened browser search  •  Enter open  •  Ctrl+F reveal  •  Ctrl+C copy"
                : "Web handoff failed  •  Enter open  •  Ctrl+F reveal  •  Ctrl+C copy";
            e.Handled = true;
            return;
        }

        // Bare `:cmdid` (no space) - submit-only inline command shortcut. macOS parity
        // (LauncherView+CommandMode.swift handleSubmit). Live `:cmdid<space>...` already
        // routes through QueryInput_OnTextChanged before Enter ever runs.
        if (e.Key == VirtualKey.Enter
            && _mode != LauncherMode.Command
            && TryExtractInlineCommand(QueryInput.Text?.Trim() ?? string.Empty,
                out string inlineCommandId, out _, out bool inlineHasSpace)
            && !inlineHasSpace)
        {
            EnterCommandScreen(inlineCommandId, string.Empty);
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Enter && ResultsList.SelectedItem is LauncherRowItem enterSelected)
        {
            HandlePrimaryAction(enterSelected, forceNewWindow: IsShiftPressed());
            e.Handled = true;
        }
    }

    private void ResultsList_OnItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is LauncherRowItem clickedRow)
        {
            HandlePrimaryAction(clickedRow, forceNewWindow: IsShiftPressed());
        }
    }

    private void ResultsList_OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Tab)
        {
            if (IsShiftPressed())
            {
                if (ResultsList.SelectedIndex > 0)
                    ResultsList.SelectedIndex--;
                else if (_results.Count > 0)
                    ResultsList.SelectedIndex = _results.Count - 1;
            }
            else
            {
                if (ResultsList.SelectedIndex < _results.Count - 1)
                    ResultsList.SelectedIndex++;
                else if (_results.Count > 0)
                    ResultsList.SelectedIndex = 0;
            }
            ResultsList.UpdateLayout();
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Up && ResultsList.SelectedIndex <= 0)
        {
            ResultsList.SelectedIndex = _results.Count - 1;
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.Enter && ResultsList.SelectedItem is LauncherRowItem selected)
        {
            HandlePrimaryAction(selected, forceNewWindow: IsShiftPressed());
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.C && IsCtrlPressed() && ResultsList.SelectedItem is LauncherRowItem copySelected)
        {
            _ = HandleCopyResultAsync(copySelected.Result);
            e.Handled = true;
            return;
        }

        if (e.Key == VirtualKey.F && IsCtrlPressed() && ResultsList.SelectedItem is LauncherRowItem revealSelected)
        {
            bool ok = _actionDispatcher.RevealResult(revealSelected.Result);
            if (ok)
            {
                ShowBanner("Revealed in Explorer", BannerStyle.Info);
            }
            else
            {
                ShowBanner("Reveal action failed", BannerStyle.Error);
            }
            e.Handled = true;
        }
    }

    private void ResultsList_OnSelectionChanged(object sender, Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs e)
    {
        // When picks exist, the right column is occupied by the picked-items panel - don't
        // toggle the preview on top of it. Selection changes still scroll/highlight as usual.
        bool picksActive = _pickedKeys.Count > 0;

        if (ResultsList.SelectedItem is LauncherRowItem selected)
        {
            ResultsList.ScrollIntoView(selected);
            if (!picksActive)
            {
                ResultPreviewPanel.SetRow(selected);
                // Help mode covers the entire ResultsHost grid; never show the preview over it.
                if (_mode == LauncherMode.Search || _mode == LauncherMode.Clipboard)
                {
                    ResultPreviewPanel.Visibility = Visibility.Visible;
                    PreviewDivider.Visibility = Visibility.Visible;
                }
            }
            if (_mode == LauncherMode.Command)
            {
                CommandPanelsPanel.SelectPanel(selected.Result.Id);
            }
            return;
        }

        if (!picksActive)
        {
            ResultPreviewPanel.SetRow(null);
            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;
        }
    }

    private void HandlePrimaryAction(LauncherRowItem selected, bool forceNewWindow = false)
    {
        if (_mode == LauncherMode.Command)
        {
            CommandPanelsPanel.SelectPanel(selected.Result.Id);
            _ = RunCommandAsync(selected.Result.Id);
            return;
        }

        if (_mode == LauncherMode.Clipboard)
        {
            _clipboardHistory?.SuppressNextCapture();
            CopyText(selected.Result.Path);
            ShowBanner("Copied clipboard item");
            return;
        }

        bool ok = _actionDispatcher.OpenResult(selected.Result, forceNewWindow);
        HintText.Text = ok
            ? "Opened selected item  •  Enter open  •  Ctrl+F reveal  •  Ctrl+C copy"
            : "Open action failed  •  Enter open  •  Ctrl+F reveal  •  Ctrl+C copy";
    }

    private static void CopyText(string value)
    {
        DataPackage package = new();
        package.SetText(value);
        Clipboard.SetContent(package);
    }

    private async System.Threading.Tasks.Task HandleCopyResultAsync(LauncherResult result)
    {
        bool ok = await _actionDispatcher.CopyResultAsync(result);
        ShowBanner(
            ok ? "Copied to clipboard" : "Copy action failed",
            ok ? BannerStyle.Success : BannerStyle.Error);
    }

    private static bool IsCtrlPressed()
    {
        Windows.UI.Core.CoreVirtualKeyStates state = Microsoft.UI.Input.InputKeyboardSource
            .GetKeyStateForCurrentThread(VirtualKey.Control);
        return (state & Windows.UI.Core.CoreVirtualKeyStates.Down) == Windows.UI.Core.CoreVirtualKeyStates.Down;
    }

    private static bool IsShiftPressed()
    {
        Windows.UI.Core.CoreVirtualKeyStates state = Microsoft.UI.Input.InputKeyboardSource
            .GetKeyStateForCurrentThread(VirtualKey.Shift);
        return (state & Windows.UI.Core.CoreVirtualKeyStates.Down) == Windows.UI.Core.CoreVirtualKeyStates.Down;
    }
}
