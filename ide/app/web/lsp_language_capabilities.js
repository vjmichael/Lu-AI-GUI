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
const VERIFIED_PYRIGHT_CAPABILITIES=Object.freeze({
  provider:'Pyright',diagnostics:true,completion:true,hover:true,definition:true,
  status:'Separately pinned native Pyright: Python diagnostics, completion, hover and same-file definition after protected OS approval.',
});
export function languageServiceCapabilities(languageId,workerStatus=null) {
  if (languageId==='python' && workerStatus?.ready===true &&
      Array.isArray(workerStatus.semanticLanguages) &&
      workerStatus.semanticLanguages.includes('python') &&
      Array.isArray(workerStatus.supportedLanguages) &&
      workerStatus.supportedLanguages.includes('python'))
    return VERIFIED_PYRIGHT_CAPABILITIES;
  return Object.prototype.hasOwnProperty.call(LANGUAGE_SERVICE_CAPABILITIES, languageId)
    ? LANGUAGE_SERVICE_CAPABILITIES[languageId] : null;
}
export function languageMethodAvailable(languageId, method, workerStatus=null) {
  const cap = languageServiceCapabilities(languageId,workerStatus);
  return !!cap && Object.prototype.hasOwnProperty.call(cap, method) &&
    cap[method] === true;
}
