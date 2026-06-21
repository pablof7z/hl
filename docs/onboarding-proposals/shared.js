const flows = new Map();

function getFlow(root) {
  const id = root.dataset.flowId;
  if (!flows.has(id)) {
    const screens = [...root.querySelectorAll('[data-screen]')];
    flows.set(id, { index: 0, screens });
  }
  return flows.get(id);
}

function render(root) {
  const flow = getFlow(root);
  flow.screens.forEach((screen, idx) => {
    screen.classList.toggle('active', idx === flow.index);
  });

  root.querySelectorAll('[data-progress]').forEach((progress) => {
    progress.innerHTML = flow.screens
      .map((_, idx) => `<span class="${idx === flow.index ? 'active' : ''}"></span>`)
      .join('');
  });

  root.querySelectorAll('[data-step-label]').forEach((label) => {
    label.textContent = `${flow.index + 1} of ${flow.screens.length}`;
  });

  root.querySelectorAll('[data-prev]').forEach((button) => {
    button.disabled = flow.index === 0;
    button.style.opacity = flow.index === 0 ? '0.42' : '1';
  });

  root.querySelectorAll('[data-next]').forEach((button) => {
    button.textContent = flow.index === flow.screens.length - 1 ? 'Restart' : button.dataset.next || 'Continue';
  });
}

function showToast(root, message) {
  const toast = root.querySelector('[data-toast]');
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add('show');
  clearTimeout(toast._timer);
  toast._timer = setTimeout(() => toast.classList.remove('show'), 1800);
}

document.addEventListener('click', (event) => {
  const target = event.target.closest('button, a');
  if (!target) return;

  const root = target.closest('[data-flow-id]');
  if (!root) return;
  const flow = getFlow(root);

  if (target.matches('[data-next]')) {
    flow.index = flow.index === flow.screens.length - 1 ? 0 : flow.index + 1;
    render(root);
  }

  if (target.matches('[data-prev]')) {
    flow.index = Math.max(0, flow.index - 1);
    render(root);
  }

  if (target.matches('[data-select]')) {
    const group = target.dataset.select;
    root.querySelectorAll(`[data-select="${group}"]`).forEach((item) => item.classList.remove('selected'));
    target.classList.add('selected');
    if (target.dataset.toast) showToast(root, target.dataset.toast);
  }

  if (target.matches('[data-toggle]')) {
    target.classList.toggle('selected');
    if (target.dataset.toast) showToast(root, target.dataset.toast);
  }

  if (target.matches('[data-toast-trigger]')) {
    showToast(root, target.dataset.toastTrigger);
  }
});

document.addEventListener('DOMContentLoaded', () => {
  document.querySelectorAll('[data-flow-id]').forEach(render);
});
