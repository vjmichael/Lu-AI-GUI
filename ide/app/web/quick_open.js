// DEV-01 bounded, read-only navigator over the existing native source-tree API.
// No filesystem path, source text, or project identity is read directly by JS.
export const QUICK_OPEN_MAX_DIRECTORIES = 24;
export const QUICK_OPEN_MAX_FILES = 500;
export const QUICK_OPEN_MAX_RESULTS = 30;

export function safeProjectRelativePath(path) {
  if (typeof path !== 'string' || !path || path.length > 4096 ||
      path.startsWith('/') || path.includes('\\') ||
      /[\u0000-\u001f\u007f]/.test(path) || /^[A-Za-z]:/.test(path)) return false;
  const segments = path.split('/');
  return segments.every(part => part !== '' && part !== '.' && part !== '..');
}

function parent(path) {
  const slash = path.lastIndexOf('/');
  return slash < 0 ? '' : path.slice(0, slash);
}

export async function collectQuickOpenFiles({
  projectId, listDirectory, isCurrent = () => true,
  maxDirectories = QUICK_OPEN_MAX_DIRECTORIES,
  maxFiles = QUICK_OPEN_MAX_FILES,
}) {
  if (typeof projectId !== 'string' || !projectId ||
      typeof listDirectory !== 'function' || typeof isCurrent !== 'function' ||
      !Number.isSafeInteger(maxDirectories) || maxDirectories < 1 ||
      maxDirectories > QUICK_OPEN_MAX_DIRECTORIES ||
      !Number.isSafeInteger(maxFiles) || maxFiles < 1 ||
      maxFiles > QUICK_OPEN_MAX_FILES) throw Error('QUICK_OPEN_INVALID_SCOPE');
  const queue = [''];
  const visited = new Set();
  const files = new Set();
  let truncated = false;
  while (queue.length && visited.size < maxDirectories && files.size < maxFiles) {
    if (!isCurrent()) throw Error('QUICK_OPEN_CANCELLED');
    const dir = queue.shift();
    if (visited.has(dir)) continue;
    visited.add(dir);
    const listing = await listDirectory(projectId, dir);
    if (!isCurrent()) throw Error('QUICK_OPEN_CANCELLED');
    if (!listing || !Array.isArray(listing.entries) ||
        listing.entries.length > 400 || typeof listing.truncated !== 'boolean')
      throw Error('QUICK_OPEN_INVALID_NATIVE_LISTING');
    if (listing.projectId != null && listing.projectId !== projectId)
      throw Error('QUICK_OPEN_WRONG_PROJECT');
    truncated ||= listing.truncated;
    for (const entry of listing.entries) {
      if (!entry || !safeProjectRelativePath(entry.relativePath) ||
          parent(entry.relativePath) !== dir) throw Error('QUICK_OPEN_INVALID_NATIVE_ENTRY');
      if (entry.kind === 'file') {
        files.add(entry.relativePath);
        if (files.size >= maxFiles) {
          truncated = true;
          break;
        }
      } else if (entry.kind === 'directory') {
        if (!visited.has(entry.relativePath) && !queue.includes(entry.relativePath)) {
          queue.push(entry.relativePath);
        }
      }
      // Symlinks, device entries, and native-host non-source categories
      // are never indexed or opened by this navigator.
    }
  }
  if (!isCurrent()) throw Error('QUICK_OPEN_CANCELLED');
  if (queue.length > 0 || (visited.size >= maxDirectories && queue.length))
    truncated = true;
  return {
    projectId, paths: Array.from(files).sort((a, b) => a.localeCompare(b)),
    directoriesVisited: visited.size, truncated,
  };
}

export function matchQuickOpenFiles(paths, query, limit = QUICK_OPEN_MAX_RESULTS) {
  if (!Array.isArray(paths) || !Number.isSafeInteger(limit) ||
      limit < 1 || limit > QUICK_OPEN_MAX_RESULTS) throw Error('QUICK_OPEN_INVALID_FILTER');
  const needle = String(query || '').trim().toLocaleLowerCase();
  if (needle.length > 256) return [];
  const ranked = [];
  for (const path of paths) {
    if (!safeProjectRelativePath(path)) continue;
    const lower = path.toLocaleLowerCase();
    if (needle && !lower.includes(needle)) continue;
    const leaf = lower.slice(lower.lastIndexOf('/') + 1);
    const score = lower === needle ? 0 : leaf === needle ? 1 :
      leaf.startsWith(needle) ? 2 : leaf.includes(needle) ? 3 : 4;
    ranked.push({path, score});
  }
  ranked.sort((a, b) => a.score - b.score || a.path.localeCompare(b.path));
  return ranked.slice(0, limit).map(record => record.path);
}
