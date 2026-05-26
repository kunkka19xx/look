/**
 * Load an HTML fragment and inject it into a container.
 * @param {string} path - path relative to the app root (e.g. 'html/screens/search.html')
 * @param {HTMLElement} container - element to append the loaded HTML into
 */
export async function load(path, container) {
  const res = await fetch(path);
  const html = await res.text();
  container.insertAdjacentHTML('beforeend', html);
}
