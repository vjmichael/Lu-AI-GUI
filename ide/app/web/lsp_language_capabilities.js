// DEV-02.2: visibility contract only. The native OS host independently
// validates source, approved server, exact release digest and one-shot consent.
export const LANGUAGE_SERVICE_CAPABILITIES = Object.freeze({
  rust: Object.freeze({
    provider: 'rust-analyzer',
    diagnostics: true, completion: true, hover: true, definition: true,
    status: 'Protected rust-analyzer: diagnostics, completion, hover and same-file definition after native consent.',
  }),
  c: Object.freeze({
    provider: 'clangd',
    diagnostics: true, completion: true, hover: true, definition: true,
    status: 'Protected clangd: diagnostics, completion, hover and same-file definition after native consent.',
  }),
  cpp: Object.freeze({
    provider: 'clangd',
    diagnostics: true, completion: true, hover: true, definition: true,
    status: 'Protected clangd: diagnostics, completion, hover and same-file definition after native consent.',
  }),
  python: Object.freeze({
    provider: 'Ruff',
    diagnostics: true, completion: false, hover: false, definition: false,
    status: 'Protected Ruff supplies Python diagnostics only. Semantic completion, hover and navigation need a separately reviewed Pyright-compatible provider.',
  }),
});
export function languageServiceCapabilities(languageId) {
  return Object.prototype.hasOwnProperty.call(LANGUAGE_SERVICE_CAPABILITIES, languageId)
    ? LANGUAGE_SERVICE_CAPABILITIES[languageId] : null;
}
export function languageMethodAvailable(languageId, method) {
  const cap = languageServiceCapabilities(languageId);
  return !!cap && Object.prototype.hasOwnProperty.call(cap, method) &&
    cap[method] === true;
}
