using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using LauncherApp.Commands;
using LauncherApp.Services;
using Microsoft.UI.Xaml.Media;

namespace LauncherApp.Core;

public sealed class KillCandidateItem : INotifyPropertyChanged
{
    private static readonly IIconService SharedIconService = new IconService();

    private ImageSource? _icon;
    private bool _iconLoadAttempted;

    public event PropertyChangedEventHandler? PropertyChanged;

    public KillCommand.RunningApp App { get; }

    public int Number => App.Index;

    public string DisplayName => App.Name;

    public string Detail
    {
        get
        {
            if (string.IsNullOrWhiteSpace(App.WindowTitle))
            {
                return $"PID: {App.Pid}";
            }

            return $"PID: {App.Pid}  •  {App.WindowTitle}";
        }
    }

    public string IconGlyph => "\uE71D";

    public ImageSource? Icon
    {
        get => _icon;
        private set
        {
            if (ReferenceEquals(_icon, value))
            {
                return;
            }

            _icon = value;
            OnPropertyChanged();
        }
    }

    public KillCandidateItem(KillCommand.RunningApp app)
    {
        App = app;
    }

    public async Task LoadIconAsync()
    {
        if (_iconLoadAttempted)
        {
            return;
        }

        _iconLoadAttempted = true;
        if (string.IsNullOrWhiteSpace(App.ExecutablePath))
        {
            return;
        }

        Icon = await SharedIconService.GetIconAsync(App.ExecutablePath, SearchItemKind.App);
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
