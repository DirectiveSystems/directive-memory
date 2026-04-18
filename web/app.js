const $ = (id) => document.getElementById(id);
const apiKeyKey = "dm-api-key";
$("apikey").value = localStorage.getItem(apiKeyKey) || "";
$("apikey").addEventListener("change", (e) => localStorage.setItem(apiKeyKey, e.target.value));

async function api(path, opts = {}) {
  const key = $("apikey").value;
  const res = await fetch(path, {
    ...opts,
    headers: { "x-api-key": key, "content-type": "application/json", ...(opts.headers || {}) },
  });
  if (!res.ok) throw new Error(`${res.status} ${await res.text()}`);
  return res.json();
}

function clear(el) { while (el.firstChild) el.removeChild(el.firstChild); }

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k === "dataset") Object.assign(node.dataset, v);
    else node.setAttribute(k, v);
  }
  for (const c of children) {
    if (typeof c === "string") node.appendChild(document.createTextNode(c));
    else if (c) node.appendChild(c);
  }
  return node;
}

function showError(container, message) {
  clear(container);
  container.appendChild(el("li", { class: "error" }, [message]));
}

async function loadFiles() {
  const list = $("file-list");
  try {
    const { files } = await api("/api/files");
    clear(list);
    for (const f of files) {
      const li = el("li", { dataset: { path: f.path } }, [f.path]);
      li.addEventListener("click", () => openFile(f.path));
      list.appendChild(li);
    }
  } catch (e) {
    showError(list, e.message);
  }
}

async function openFile(path) {
  try {
    const { content } = await api(`/api/files/${encodeURIComponent(path)}`);
    $("viewer-title").textContent = path;
    // Safe: content is sanitized by DOMPurify before assignment to the viewer pane.
    const sanitized = DOMPurify.sanitize(marked.parse(content));
    $("viewer-body").innerHTML = sanitized;
  } catch (e) {
    $("viewer-title").textContent = "Error";
    const body = $("viewer-body");
    clear(body);
    body.appendChild(el("p", { class: "error" }, [e.message]));
  }
}

async function search(q) {
  const hitsEl = $("hits");
  try {
    const { hits } = await api(`/api/search?q=${encodeURIComponent(q)}&top_k=10`);
    clear(hitsEl);
    for (const h of hits) {
      const fileLink = el("div", { class: "file", dataset: { path: h.file } }, [h.file]);
      fileLink.addEventListener("click", () => openFile(h.file));
      const li = el("li", {}, [
        fileLink,
        el("div", { class: "heading" }, [h.heading]),
        el("div", { class: "snippet" }, [h.content]),
      ]);
      hitsEl.appendChild(li);
    }
  } catch (e) {
    showError(hitsEl, e.message);
  }
}

$("search-form").addEventListener("submit", (e) => {
  e.preventDefault();
  search($("q").value.trim());
});
loadFiles();
