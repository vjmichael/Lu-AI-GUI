// Browser-only, bounded fault diagnostic; never a source, OS, or release
// acceptance authority. No IPC, network, storage, eval or untrusted text.
(() => {
  const status = 'Lu IDE frontend unavailable';
  const allowed = new Set([
    'module_asset_unavailable',
    'uncaught_frontend_exception',
    'unhandled_frontend_rejection',
    'react_render_failed',
  ]);
  function report(code, force = false) {
    if (!allowed.has(code)) return;
    const root = document.getElementById('root');
    if (!root || (!force && root.querySelector('.ideShell'))) return;
    const alert = document.createElement('div');
    alert.setAttribute('role', 'alert');
    alert.setAttribute('aria-label', status);
    alert.setAttribute('data-ide-startup-diagnostic', code);
    alert.textContent = 'Lu IDE could not initialize its interface. Restart the application.';
    root.replaceChildren(alert);
  }
  Object.defineProperty(window, '__luIdeReportStartupFailure', {
    value: report, enumerable: false, writable: false, configurable: false,
  });
  window.addEventListener('error', event => {
    const target = event.target;
    const src = target?.tagName === 'SCRIPT' ? target.getAttribute('src') : '';
    if (src === './assets/app.js' || src?.endsWith('/assets/app.js')) {
      report('module_asset_unavailable');
    } else if (typeof event.message === 'string') {
      report('uncaught_frontend_exception');
    }
  }, true);
  window.addEventListener('unhandledrejection', () => {
    report('unhandled_frontend_rejection');
  });
})();
