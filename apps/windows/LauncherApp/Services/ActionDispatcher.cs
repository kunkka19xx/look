using System;
using LauncherApp.Bridge;
using Windows.ApplicationModel.DataTransfer;

namespace LauncherApp.Services;

public sealed class ActionDispatcher
{
    private readonly ShellExecuteService _shellExecute;
    private readonly ExplorerRevealService _reveal;

    public ActionDispatcher(ShellExecuteService shellExecute, ExplorerRevealService reveal)
    {
        _shellExecute = shellExecute;
        _reveal = reveal;
    }

    public bool OpenResult(LauncherResult result)
    {
        return _shellExecute.Open(result.Path);
    }

    public bool RevealResult(LauncherResult result)
    {
        return _reveal.Reveal(result.Path);
    }

    public bool CopyResultPath(LauncherResult result)
    {
        if (string.IsNullOrWhiteSpace(result.Path))
        {
            return false;
        }

        DataPackage package = new();
        package.SetText(result.Path);
        Clipboard.SetContent(package);
        return true;
    }

    public bool WebHandoff(string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return false;
        }

        string url = "https://www.google.com/search?q=" + Uri.EscapeDataString(query);
        return _shellExecute.Open(url);
    }
}
