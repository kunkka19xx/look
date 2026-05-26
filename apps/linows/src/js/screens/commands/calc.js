let panel = null;
let input = null;
let feedback = null;
let onExecute = null;

export function init(executeFn) {
  onExecute = executeFn;
  panel = document.getElementById('cmd-panel-calc');
  input = document.getElementById('cmd-calc-input');
  feedback = document.getElementById('cmd-calc-feedback');

  input.addEventListener('input', () => {
    const val = input.value.trim();
    if (val) {
      if (onExecute) onExecute('calc-preview', val);
    } else {
      feedback.textContent = 'Selected /calc';
      feedback.className = 'cmd-feedback';
    }
  });
}

export function enter() {
  panel.hidden = false;
  input.value = '';
  feedback.textContent = 'Selected /calc';
  feedback.className = 'cmd-feedback';
  requestAnimationFrame(() => input.focus());
}

export function exit() {
  panel.hidden = true;
}

export function handleKey(e) {
  if (e.key === 'Enter') {
    e.preventDefault();
    const val = input.value.trim();
    if (onExecute && val) onExecute('calc', val);
    return true;
  }
  return false;
}

export function showFeedback(text, isError = false) {
  feedback.textContent = text;
  feedback.className = `cmd-feedback ${isError ? 'cmd-feedback-error' : ''}`;
}
