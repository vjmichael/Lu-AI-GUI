// Image Workspace draft: host-agnostic edit-intent validation only.
// This is NOT effect authorization, pixel processing, or an output receipt.
export const IMAGE_EDIT_OPERATIONS = Object.freeze([
  'crop', 'resize', 'annotate', 'comment', 'remove_background', 'erase', 'edit_prompt'
]);
const ops = new Set(IMAGE_EDIT_OPERATIONS);
const int = (n) => Number.isSafeInteger(n) && n >= 0;
function cleanString(s, max = 512) {
  return typeof s === 'string' && s.length > 0 && s.length <= max && !/[\\u0000-\\u001f]/.test(s);
}
function regionOk(r, w, h) {
  return r && typeof r === 'object' && int(r.x) && int(r.y) &&
    Number.isSafeInteger(r.width) && r.width > 0 &&
    Number.isSafeInteger(r.height) && r.height > 0 &&
    r.x < w && r.y < h && r.width <= w - r.x && r.height <= h - r.y;
}
export function validateImageEditIntent(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw Error('Image edit intent must be an object');
  const {asset_id, base_revision, base_width, base_height, operation, region, target_width, target_height, text, mask_id} = input;
  if (!cleanString(asset_id, 128) || !cleanString(base_revision, 128)) throw Error('Missing asset identity or revision');
  if (!Number.isSafeInteger(base_width) || base_width < 1 || base_width > 32768 ||
      !Number.isSafeInteger(base_height) || base_height < 1 || base_height > 32768 ||
      base_width * base_height > 100_000_000) throw Error('Unsupported image dimensions');
  if (!ops.has(operation)) throw Error('Unsupported image operation');
  if (region !== undefined && !regionOk(region, base_width, base_height)) throw Error('Invalid image region');
  if (operation === 'crop' && !regionOk(region, base_width, base_height)) throw Error('Crop requires a bounded region');
  if (operation === 'resize') {
    if (!Number.isSafeInteger(target_width) || target_width < 1 || target_width > 32768 ||
        !Number.isSafeInteger(target_height) || target_height < 1 || target_height > 32768 ||
        target_width * target_height > 100_000_000) throw Error('Invalid resize target');
  }
  if ((operation === 'comment' || operation === 'annotate' || operation === 'edit_prompt') &&
      !cleanString(text, 4096)) throw Error('Operation requires text');
  if ((operation === 'comment' || operation === 'annotate' || operation === 'erase') &&
      !regionOk(region, base_width, base_height)) throw Error('Operation requires region');
  if (operation === 'erase' && !cleanString(mask_id, 128)) throw Error('Erase requires mask reference');
  // Create a narrow, serializable intent. Never propagate arbitrary model-selected paths.
  return Object.freeze({
    asset_id, base_revision, base_width, base_height, operation,
    ...(region ? {region: Object.freeze({x: region.x, y: region.y, width: region.width, height: region.height})} : {}),
    ...(operation === 'resize' ? {target_width, target_height} : {}),
    ...(text !== undefined ? {text} : {}),
    ...(operation === 'erase' ? {mask_id} : {})
  });
}
