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

// id + kind, never the message: messages carry store paths and error text
// that churn, which would resurrect a dismissed notice.
function issueKey(issue) {
    return `${issue.id}:${issue.kind ?? issue.id}`;
}

// What issueKey produced before it dropped the message. Kept only so an
// upgrade does not re-nag about a notice the user already dismissed.
function legacyKey(issue) {
    return `${issue.id}:${issue.message}`;
}

// Rewrite any legacy entry the current issues still match, so the old
// message-keyed set drains instead of growing alongside the new one.
function migrate(dismissed) {
    let changed = false;
    issues.forEach((i) => {
        if (!dismissed.delete(legacyKey(i))) return;
        dismissed.add(issueKey(i));
        changed = true;
    });
    if (changed) persist(dismissed);
}

function persist(dismissed) {
    try {
        localStorage.setItem(DISMISSED_KEY, JSON.stringify([...dismissed]));
    } catch {
        // Best effort - worst case the notice reappears next launch.
    }
}

function render() {
    const dismissed = dismissedSet();
    migrate(dismissed);
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
    persist(dismissed);
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
