// Browser-side UX normalization only. Lu Core still decodes, sizes and admits
// the actual bytes independently; a filename or reported MIME is never proof.
export function sniffSupportedImageMime(header) {
  const h = header instanceof Uint8Array ? header : new Uint8Array(header || []);
  if (h.length >= 8 && [137,80,78,71,13,10,26,10].every((b,i)=>h[i] === b)) return 'image/png';
  if (h.length >= 3 && h[0] === 255 && h[1] === 216 && h[2] === 255) return 'image/jpeg';
  return null;
}

export function isPotentialImage(file) {
  return /\.(png|jpe?g)$/i.test(file?.name || '') || /^image\//i.test(file?.type || '');
}

export async function normalizedImageFile(file) {
  if (!file || !isPotentialImage(file) || !Number.isFinite(file.size)) {
    throw Error('Choose a PNG or JPEG image');
  }
  const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
  const mimeType = sniffSupportedImageMime(head);
  if (!mimeType) throw Error(`${file.name || 'Attachment'} is not a supported PNG/JPEG image`);
  // WebView FileReader uses blob.type in the data: prefix. Correct mismatched
  // extensions (e.g. a PNG named .jpg) before invoking trusted host admission.
  return { blob: new Blob([file], {type: mimeType}), mimeType,
    wasRelabeled: Boolean(file.type && file.type !== mimeType) };
}
