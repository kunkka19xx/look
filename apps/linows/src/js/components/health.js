// Surfaces backend setup problems (dead hotkey, GNOME extension needing a
// re-login) as a sticky banner. Issues can land before the webview runs
// (pulled via get_health_issues on init) or minutes later from backend
// threads (pushed via health-changed). Dismissals persist in localStorage so
// the same problem doesn't nag on every launch.
import { getHealthIssues, onHealthChanged } from '../ipc.js';
import * as banner from './banner.js';

const DISMISSED_KEY = 'look.health.dismissed';

let issues = [];

function dismissedSet() {
    try {
        return new Set(JSON.parse(localStorage.getItem(DISMISSED_KEY)) || []);
    } catch {
        return new Set();
    }
}

// Keyed on id + message: a new failure mode under the same id should
// surface again even if an older notice was dismissed.
function issueKey(issue) {
    return `${issue.id}:${issue.message}`;
}

function render() {
    const dismissed = dismissedSet();
    const visible = issues.filter((i) => !dismissed.has(issueKey(i)));
    if (visible.length === 0) {
        banner.showSticky(null);
        return;
    }
    banner.showSticky(visible.map((i) => i.message).join('\n'), 'warning', dismissAll);
}

function dismissAll() {
    const dismissed = dismissedSet();
    issues.forEach((i) => dismissed.add(issueKey(i)));
    try {
        localStorage.setItem(DISMISSED_KEY, JSON.stringify([...dismissed]));
    } catch {
        // Best effort - worst case the notice reappears next launch.
    }
}

export function init() {
    onHealthChanged((event) => {
        issues = event.payload || [];
        render();
    });
    getHealthIssues()
        .then((list) => {
            issues = list || [];
            render();
        })
        .catch(() => {});
}
