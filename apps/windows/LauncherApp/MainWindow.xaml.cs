using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using LauncherApp.Core;
using LauncherApp.Features.Search;
using LauncherApp.Services;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using WinUIEx;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace LauncherApp
{
    public sealed partial class MainWindow : Window
    {
        private readonly LauncherSearchLogic _searchLogic;
        private readonly ActionDispatcher _actionDispatcher;
        private readonly ObservableCollection<LauncherRowItem> _results;
        private readonly List<LauncherResult> _commandSeed;
        private readonly List<LauncherResult> _clipboardSeed;
        private LauncherMode _mode = LauncherMode.Search;
        private int _searchVersion;

        public MainWindow()
        {
            InitializeComponent();
            ConfigureLauncherWindow();

            bool mockFirst = true;
            ISearchProvider searchProvider = mockFirst
                ? new MockSearchProvider()
                : new FfiSearchProvider(new EngineBridge());

            _searchLogic = new LauncherSearchLogic(searchProvider);
            _actionDispatcher = new ActionDispatcher(new ShellExecuteService(), new ExplorerRevealService());
            _results = new ObservableCollection<LauncherRowItem>();
            _commandSeed = BuildCommandSeed();
            _clipboardSeed = BuildClipboardSeed();

            ResultsList.ItemsSource = _results;
            SetMode(LauncherMode.Search);
            RefreshResults(QueryInput.Text?.Trim() ?? string.Empty);
        }

        private static List<LauncherResult> BuildCommandSeed()
        {
            return
            [
                new LauncherResult { Id = "command:calc", Kind = "app", Title = "calc", Subtitle = "Evaluate expression", Path = "command://calc", Score = 1000 },
                new LauncherResult { Id = "command:shell", Kind = "app", Title = "shell", Subtitle = "Run shell command", Path = "command://shell", Score = 990 },
                new LauncherResult { Id = "command:kill", Kind = "app", Title = "kill", Subtitle = "Terminate process", Path = "command://kill", Score = 980 },
                new LauncherResult { Id = "command:sys", Kind = "app", Title = "sys", Subtitle = "System info panel", Path = "command://sys", Score = 970 },
            ];
        }

        private static List<LauncherResult> BuildClipboardSeed()
        {
            return
            [
                new LauncherResult { Id = "clip:1", Kind = "clipboard", Title = "cargo test -p look-engine", Subtitle = "Copied 3m ago", Path = "cargo test -p look-engine", Score = 1000 },
                new LauncherResult { Id = "clip:2", Kind = "clipboard", Title = "https://docs.rs/serde", Subtitle = "Copied 11m ago", Path = "https://docs.rs/serde", Score = 990 },
                new LauncherResult { Id = "clip:3", Kind = "clipboard", Title = "C:\\Users\\haong\\Documents\\git\\look", Subtitle = "Copied 19m ago", Path = "C:\\Users\\haong\\Documents\\git\\look", Score = 980 },
            ];
        }

        private void ConfigureLauncherWindow()
        {
            this.SetWindowSize(960, 620);
            this.CenterOnScreen();
            this.SetWindowPresenter(AppWindowPresenterKind.Overlapped);
            this.SetIsResizable(false);
            this.SetIsMaximizable(false);
            this.SetIsMinimizable(false);

            if (this.AppWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.SetBorderAndTitleBar(true, false);
            }
            ApplyRuntimeIcon();
        }

        private void ApplyRuntimeIcon()
        {
            string iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "look.ico");
            if (File.Exists(iconPath))
            {
                this.SetIcon(iconPath);
            }
        }

       

        [DllImport("gdi32.dll")]
        private static extern IntPtr CreateRoundRectRgn(int left, int top, int right, int bottom, int widthEllipse, int heightEllipse);

        [DllImport("user32.dll")]
        private static extern int SetWindowRgn(IntPtr hWnd, IntPtr hRgn, bool redraw);

        [DllImport("gdi32.dll")]
        private static extern bool DeleteObject(IntPtr hObject);

        private static (LauncherMode mode, string normalizedQuery) ResolveMode(string rawQuery)
        {
            string query = rawQuery.Trim();
            if (query.StartsWith("/"))
            {
                return (LauncherMode.Command, query.TrimStart('/').Trim());
            }

            if (query.StartsWith("c\"", StringComparison.OrdinalIgnoreCase))
            {
                return (LauncherMode.Clipboard, query.Substring(2).Trim());
            }

            if (query.StartsWith("?"))
            {
                return (LauncherMode.Help, query.TrimStart('?').Trim());
            }

            if (query.StartsWith(","))
            {
                return (LauncherMode.Settings, query.TrimStart(',').Trim());
            }

            return (LauncherMode.Search, query);
        }

        private void SetMode(LauncherMode mode)
        {
            _mode = mode;

            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;
            CommandPanelsPanel.Visibility = Visibility.Collapsed;
            SettingsTabsPanel.Visibility = Visibility.Collapsed;

            switch (mode)
            {
                case LauncherMode.Search:
                    QueryInput.PlaceholderText = "Search apps";
                    HintText.Text = "Enter open  •  Ctrl+R reveal  •  Ctrl+C copy  •  Ctrl+Enter web";
                    break;
                case LauncherMode.Command:
                    QueryInput.PlaceholderText = "Use /calc, /shell, /kill, /sys";
                    HintText.Text = "Enter run  •  Up/Down select  •  Esc clear";
                    ResultPreviewPanel.Visibility = Visibility.Collapsed;
                    CommandPanelsPanel.Visibility = Visibility.Visible;
                    break;
                case LauncherMode.Clipboard:
                    QueryInput.PlaceholderText = "Use c\" to search clipboard";
                    HintText.Text = "Enter copy  •  Up/Down select  •  Esc clear";
                    break;
                case LauncherMode.Settings:
                    QueryInput.PlaceholderText = "Use , to view settings notes";
                    HintText.Text = "Mock UI only  •  Theme and style parity in progress";
                    ResultPreviewPanel.Visibility = Visibility.Collapsed;
                    SettingsTabsPanel.Visibility = Visibility.Visible;
                    break;
                case LauncherMode.Help:
                    QueryInput.PlaceholderText = "Use ? to view help";
                    HintText.Text = "Prefixes: / command  •  c\" clipboard  •  , settings  •  ? help";
                    break;
            }
        }

        private void RefreshResults(string rawQuery)
        {
            var (resolvedMode, query) = ResolveMode(rawQuery);
            if (resolvedMode != _mode)
            {
                SetMode(resolvedMode);
            }

            IReadOnlyList<LauncherResult> source = _mode switch
            {
                LauncherMode.Search => _searchLogic.Search(query, 40),
                LauncherMode.Command => FilterRows(_commandSeed, query),
                LauncherMode.Clipboard => FilterRows(_clipboardSeed, query),
                LauncherMode.Settings => FilterRows(BuildSettingsRows(), query),
                LauncherMode.Help => FilterRows(BuildHelpRows(), query),
                _ => [],
            };

            _results.Clear();
            foreach (LauncherResult item in source)
            {
                _results.Add(new LauncherRowItem(item));
            }

            if (_results.Count > 0)
            {
                ResultsList.SelectedIndex = 0;
                return;
            }

            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;

        }

        private static IReadOnlyList<LauncherResult> FilterRows(IEnumerable<LauncherResult> source, string query)
        {
            if (string.IsNullOrWhiteSpace(query))
            {
                return source.OrderByDescending(item => item.Score).ToList();
            }

            return source.Where(item =>
                    item.Title.Contains(query, StringComparison.OrdinalIgnoreCase)
                    || item.Path.Contains(query, StringComparison.OrdinalIgnoreCase)
                    || (item.Subtitle?.Contains(query, StringComparison.OrdinalIgnoreCase) ?? false))
                .OrderByDescending(item => item.Score)
                .ToList();
        }

        private static IReadOnlyList<LauncherResult> BuildSettingsRows()
        {
            return
            [
                new LauncherResult { Id = "settings:theme", Kind = "app", Title = "Theme style tokens", Subtitle = "primary / secondary / ghost / danger", Path = "settings://theme", Score = 1000 },
                new LauncherResult { Id = "settings:message", Kind = "app", Title = "Message and banner style", Subtitle = "success / info / warning / error", Path = "settings://messages", Score = 990 },
                new LauncherResult { Id = "settings:screens", Kind = "app", Title = "Screen states", Subtitle = "search / command / clipboard / settings / help", Path = "settings://screens", Score = 980 },
            ];
        }

        private static IReadOnlyList<LauncherResult> BuildHelpRows()
        {
            return
            [
                new LauncherResult { Id = "help:search", Kind = "app", Title = "Search mode", Subtitle = "Blend apps, files, folders", Path = "help://search", Score = 1000 },
                new LauncherResult { Id = "help:command", Kind = "app", Title = "Command mode", Subtitle = "calc, shell, kill, sys", Path = "help://command", Score = 990 },
                new LauncherResult { Id = "help:clipboard", Kind = "clipboard", Title = "Clipboard mode", Subtitle = "session-local history", Path = "help://clipboard", Score = 980 },
            ];
        }

        private async void QueryInput_OnTextChanged(object sender, Microsoft.UI.Xaml.Controls.TextChangedEventArgs e)
        {
            int currentVersion = ++_searchVersion;
            string query = QueryInput.Text?.Trim() ?? string.Empty;

            await Task.Delay(8);
            if (currentVersion != _searchVersion)
            {
                return;
            }

            RefreshResults(query);
        }

        private void QueryInput_OnKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key == VirtualKey.Escape)
            {
                QueryInput.Text = string.Empty;
                SetMode(LauncherMode.Search);
                RefreshResults(string.Empty);
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Down && _results.Count > 0)
            {
                ResultsList.Focus(FocusState.Programmatic);
                ResultsList.SelectedIndex = 0;
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Enter && IsCtrlPressed())
            {
                bool ok = _actionDispatcher.WebHandoff(QueryInput.Text ?? string.Empty);
                HintText.Text = ok
                    ? "Opened browser search  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                    : "Web handoff failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
                e.Handled = true;
            }
        }

        private void ResultsList_OnKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key == VirtualKey.Up && ResultsList.SelectedIndex <= 0)
            {
                QueryInput.Focus(FocusState.Programmatic);
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Enter && ResultsList.SelectedItem is LauncherRowItem selected)
            {
                HandlePrimaryAction(selected);
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.C && IsCtrlPressed() && ResultsList.SelectedItem is LauncherRowItem copySelected)
            {
                bool ok = _actionDispatcher.CopyResultPath(copySelected.Result);
                HintText.Text = ok
                    ? "Copied path/content  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                    : "Copy action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.R && IsCtrlPressed() && ResultsList.SelectedItem is LauncherRowItem revealSelected)
            {
                bool ok = _actionDispatcher.RevealResult(revealSelected.Result);
                HintText.Text = ok
                    ? "Revealed in Explorer  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                    : "Reveal action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
                e.Handled = true;
            }
        }

        private void ResultsList_OnSelectionChanged(object sender, Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs e)
        {
            if (ResultsList.SelectedItem is LauncherRowItem selected)
            {
                ResultPreviewPanel.SetRow(selected);
                if (_mode == LauncherMode.Search || _mode == LauncherMode.Clipboard || _mode == LauncherMode.Help)
                {
                    ResultPreviewPanel.Visibility = Visibility.Visible;
                    PreviewDivider.Visibility = Visibility.Visible;
                }
                if (_mode == LauncherMode.Command)
                {
                    CommandPanelsPanel.SelectPanel(selected.Result.Id);
                }
                return;
            }

            ResultPreviewPanel.SetRow(null);
            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;
        }

        private void HandlePrimaryAction(LauncherRowItem selected)
        {
            if (_mode == LauncherMode.Command)
            {
                CommandPanelsPanel.SelectPanel(selected.Result.Id);
                RunCommand(selected.Result.Id);
                return;
            }

            if (_mode == LauncherMode.Clipboard)
            {
                CopyText(selected.Result.Path);
                HintText.Text = "Copied clipboard item  •  Enter copy  •  Up/Down select  •  Esc clear";
                return;
            }

            bool ok = _actionDispatcher.OpenResult(selected.Result);
            HintText.Text = ok
                ? "Opened selected item  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                : "Open action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
        }

        private void RunCommand(string id)
        {
            switch (id)
            {
                case "command:calc":
                    HintText.Text = "calc executed in mock mode";
                    break;
                case "command:shell":
                    HintText.Text = "shell executed in mock mode";
                    break;
                case "command:kill":
                    HintText.Text = "kill requires confirmation flow (planned)";
                    break;
                case "command:sys":
                    HintText.Text = "sys panel opened in mock mode";
                    break;
                default:
                    HintText.Text = "Unknown command";
                    break;
            }
        }

        private static void CopyText(string value)
        {
            DataPackage package = new();
            package.SetText(value);
            Clipboard.SetContent(package);
        }

        private static bool IsCtrlPressed()
        {
            Windows.UI.Core.CoreVirtualKeyStates state = Microsoft.UI.Input.InputKeyboardSource
                .GetKeyStateForCurrentThread(VirtualKey.Control);
            return (state & Windows.UI.Core.CoreVirtualKeyStates.Down) == Windows.UI.Core.CoreVirtualKeyStates.Down;
        }
    }
}
