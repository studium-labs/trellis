import { FileTrieNode } from "/js/util/fileTrie";
import { resolveRelative, simplifySlug } from "/js/util/path";
import { registerEscapeHandler } from "/js/components/scripts/util.js";

function setFolder(folderPath, open) {
  const children = document.querySelector(`[data-ol-children-for='${folderPath}']`);
  const entry = document.querySelector(`[data-ol-selector-for='${folderPath}']`);
  if (!children || !entry) return;
  const icon = entry.querySelector("svg");
  if (!icon) return;
  children.classList.toggle("open", open);
  icon.classList.toggle("open", open);
}

function buildNode(node, currentSlug, opts, folderPath = "") {
  const li = document.createElement("li");

  if (node.isFolder) {
    const fullPath = node.slug || folderPath;
    const selectorPath = fullPath.endsWith("/") ? fullPath + "index" : `${fullPath}/index`;

    const entry = document.createElement("div");
    entry.className = "ol-folder-entry";
    entry.dataset.olSelectorFor = selectorPath;

    const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    icon.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    icon.setAttribute("width", "12");
    icon.setAttribute("height", "12");
    icon.setAttribute("viewBox", "5 8 14 8");
    icon.setAttribute("fill", "none");
    icon.setAttribute("stroke", "currentColor");
    icon.setAttribute("stroke-width", "2");
    icon.setAttribute("stroke-linecap", "round");
    icon.setAttribute("stroke-linejoin", "round");
    icon.classList.add("ol-folder-icon");
    icon.innerHTML = '<polyline points="6 9 12 15 18 9"></polyline>';
    entry.appendChild(icon);

    if (opts.folderClickBehavior === "link") {
      const a = document.createElement("a");
      a.href = resolveRelative(currentSlug, fullPath);
      a.className = "ol-folder-title";
      a.textContent = node.displayName;
      entry.appendChild(a);
    } else {
      const btn = document.createElement("button");
      btn.className = "ol-folder-button";
      btn.type = "button";
      const span = document.createElement("span");
      span.className = "ol-folder-title";
      span.textContent = node.displayName;
      btn.appendChild(span);
      entry.appendChild(btn);

      if (opts.folderClickBehavior === "mixed") {
        const link = document.createElement("a");
        link.href = resolveRelative(currentSlug, fullPath);
        link.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="12" viewBox="0 4 21 15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="0 9 18 9"></polyline><polyline points="0 15 18 15"></polyline><polyline points="15 6 21 12 15 18"></polyline></svg>`;
        entry.appendChild(link);
      }
    }

    li.appendChild(entry);

    const outer = document.createElement("div");
    outer.className = "ol-folder-outer";
    outer.dataset.olChildrenFor = selectorPath;
    const ul = document.createElement("ul");
    ul.style.paddingLeft = node.name !== "" ? "1.4rem" : "0";

    for (const child of node.children) {
      ul.appendChild(buildNode(child, currentSlug, opts, fullPath));
    }

    outer.appendChild(ul);
    li.appendChild(outer);

    const shouldOpen =
      opts.folderDefaultState === "open" || simplifySlug(fullPath).startsWith(currentSlug);
    if (shouldOpen) {
      outer.classList.add("open");
      icon.classList.add("open");
    }
  } else {
    const a = document.createElement("a");
    a.href = resolveRelative(currentSlug, node.slug);
    a.textContent = node.displayName;
    li.appendChild(a);
  }

  return li;
}

function attachFolderToggles(root, useSaveState) {
  const map = useSaveState
    ? new Map(JSON.parse(localStorage.getItem("olFileTree") || "[]"))
    : new Map();

  const save = () => {
    if (useSaveState) {
      localStorage.setItem("olFileTree", JSON.stringify(Array.from(map.entries())));
    }
  };

  const toggle = (evt) => {
    evt.stopPropagation();
    const target = evt.currentTarget;
    const selector = target.closest("[data-ol-selector-for]")?.dataset.olSelectorFor;
    if (!selector) return;
    const children = document.querySelector(`[data-ol-children-for='${selector}']`);
    const icon = document.querySelector(`[data-ol-selector-for='${selector}'] .ol-folder-icon`);
    if (!children || !icon) return;
    const open = !children.classList.contains("open");
    children.classList.toggle("open", open);
    icon.classList.toggle("open", open);
    map.set(selector, open);
    save();
  };

  root.querySelectorAll(".ol-folder-icon, .ol-folder-button").forEach((el) => {
    el.addEventListener("click", toggle);
    window.addCleanup?.(() => el.removeEventListener("click", toggle));
  });

  // Restore saved state
  for (const [folder, open] of map.entries()) {
    setFolder(folder, open);
  }
}

async function hydrateOverlayExplorer() {
  const openButton = document.getElementById("overlay-explorer-button");
  const container = document.getElementById("overlay-explorer-container");
  const list = document.getElementById("overlay-explorer-ul");
  if (!openButton || !container || !list) return;

  const opts = {
    folderClickBehavior: openButton.dataset.behavior || "mixed",
    folderDefaultState: openButton.dataset.collapsed || "collapsed",
    useSavedState: openButton.dataset.olsavestate === "true",
  };

  const currentSlug = (window.location.pathname || "/").replace(/^\//, "") || "index";

  // Rebuild list from content index if available
  if (typeof fetchData !== "undefined") {
    try {
      const data = await fetchData;
      const trie = FileTrieNode.fromEntries([...Object.entries(data)]);
      list.replaceChildren();
      for (const child of trie.children) {
        list.appendChild(buildNode(child, currentSlug, opts));
      }
    } catch (err) {
      console.warn("Overlay explorer: failed to load content index", err);
    }
  }

  attachFolderToggles(container, opts.useSavedState);

  if (openButton.dataset.overlayReady !== "true") {
    const show = () => {
      container.classList.add("active");
      container.setAttribute("aria-hidden", "false");
      document.documentElement.classList.add("mobile-no-scroll");
    };
    const hide = () => {
      container.classList.remove("active");
      container.setAttribute("aria-hidden", "true");
      document.documentElement.classList.remove("mobile-no-scroll");
    };

    openButton.addEventListener("click", show);
    window.addCleanup?.(() => openButton.removeEventListener("click", show));

    registerEscapeHandler(container, hide);
    openButton.dataset.overlayReady = "true";
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", hydrateOverlayExplorer, { once: true });
} else {
  hydrateOverlayExplorer();
}

document.addEventListener("nav", hydrateOverlayExplorer);
