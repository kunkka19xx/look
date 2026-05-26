let panel = null;
let feedback = null;
let tableEl = null;
let onExecute = null;

export function init(executeFn) {
  onExecute = executeFn;
  panel = document.getElementById('cmd-panel-sys');
  feedback = document.getElementById('cmd-sys-feedback');
  tableEl = document.getElementById('cmd-sys-table');
}

export function enter() {
  panel.hidden = false;
  if (onExecute) onExecute('sys-load');
}

export function exit() {
  panel.hidden = true;
}

export function handleKey() {
  return false; // no command-specific keys
}

export function setSysInfo(sections) {
  tableEl.innerHTML = '';
  feedback.textContent = '';

  if (!sections || sections.length === 0) {
    feedback.textContent = 'No data';
    return;
  }

  sections.forEach((section, si) => {
    if (si > 0) {
      const spacer = document.createElement('div');
      spacer.className = 'cmd-sys-spacer';
      tableEl.appendChild(spacer);
    }

    section.forEach((entry) => {
      const row = document.createElement('div');
      row.className = 'cmd-sys-row';
      row.innerHTML = `<span class="cmd-sys-label">${entry.label}</span><span class="cmd-sys-value">${entry.value}</span>`;
      tableEl.appendChild(row);
    });
  });
}

export function showFeedback(text, isError = false) {
  feedback.textContent = text;
  feedback.className = `cmd-feedback ${isError ? 'cmd-feedback-error' : ''}`;
}
