import { FileTrieNode } from "/js/util/fileTrie";
import { resolveRelative, simplifySlug } from "/js/util/path";

// Rootz renders without the SPA router, so `window.addCleanup` can be undefined.
const registerCleanup =
  typeof window !== "undefined" && typeof window.addCleanup === "function"
    ? window.addCleanup
    : null;

// Always keep folder state as a concrete array so toggle handlers never crash.
let currentExplorerState = [];

function loadSavedState(useSavedState) {
  if (!useSavedState) return [];
  const storageTree = localStorage.getItem("fileTree");
  if (!storageTree) return [];
  try {
    return JSON.parse(storageTree);
  } catch (err) {
    console.warn("Explorer: failed to parse saved state", err);
    return [];
  }
}
function attachFolderToggles(explorer) {
  const folderButtons = explorer.getElementsByClassName("folder-button");
  for (const button of folderButtons) {
    button.addEventListener("click", toggleFolder);
    registerCleanup?.(() => button.removeEventListener("click", toggleFolder));
  }

  const folderIcons = explorer.getElementsByClassName("folder-icon");
  for (const icon of folderIcons) {
    icon.addEventListener("click", toggleFolder);
    registerCleanup?.(() => icon.removeEventListener("click", toggleFolder));
  }
}
function toggleExplorer() {
  const nearestExplorer = this.closest(".explorer");
  if (!nearestExplorer) return;
  const explorerCollapsed = nearestExplorer.classList.toggle("collapsed");
  nearestExplorer.setAttribute(
    "aria-expanded",
    nearestExplorer.getAttribute("aria-expanded") === "true" ? "false" : "true"
  );

  if (!explorerCollapsed) {
    // Stop <html> from being scrollable when mobile explorer is open
    document.documentElement.classList.add("mobile-no-scroll");
  } else {
    document.documentElement.classList.remove("mobile-no-scroll");
  }
}

function toggleFolder(evt) {
  evt.stopPropagation();
  const target = evt.target;
  if (!target) return;

  // Check if target was svg icon or button
  const isSvg = target.nodeName === "svg";

  // corresponding <ul> element relative to clicked button/folder
  const folderContainer = isSvg // svg -> div.folder-container
    ? target.parentElement // button.folder-button -> div -> div.folder-container
    : target.parentElement?.parentElement;
  if (!folderContainer) return;
  const childFolderContainer = folderContainer.nextElementSibling;
  if (!childFolderContainer) return;

  childFolderContainer.classList.toggle("open");

  // Collapse folder container
  const isCollapsed = !childFolderContainer.classList.contains("open");
  setFolderState(childFolderContainer, isCollapsed);

  const folderPath =
    folderContainer.dataset.folderpath ||
    folderContainer.querySelector(".folder-title")?.textContent ||
    "";

  const currentFolderState = currentExplorerState.find(
    (item) => item.path === folderPath
  );
  if (currentFolderState) {
    currentFolderState.collapsed = isCollapsed;
  } else {
    currentExplorerState.push({
      path: folderPath,
      collapsed: isCollapsed,
    });
  }

  const stringifiedFileTree = JSON.stringify(currentExplorerState);
  localStorage.setItem("fileTree", stringifiedFileTree);
}

function createFileNode(currentSlug, node) {
  const template = document.getElementById("template-file");
  const clone = template.content.cloneNode(true);
  const li = clone.querySelector("li");
  const a = li.querySelector("a");
  a.href = resolveRelative(currentSlug, node.slug);
  a.dataset.for = node.slug;
  a.textContent = node.displayName;

  if (currentSlug === node.slug) {
    a.classList.add("active");
  }

  return li;
}

function createFolderNode(currentSlug, node, opts) {
  const template = document.getElementById("template-folder");
  const clone = template.content.cloneNode(true);
  const li = clone.querySelector("li");
  const folderContainer = li.querySelector(".folder-container");
  const titleContainer = folderContainer.querySelector("div");
  const folderOuter = li.querySelector(".folder-outer");
  const ul = folderOuter.querySelector("ul");

  const folderPath = node.slug;
  folderContainer.dataset.folderpath = folderPath;

  if (opts.folderClickBehavior === "link") {
    // Replace button with link for link behavior
    const button = titleContainer.querySelector(".folder-button");
    const a = document.createElement("a");
    a.href = resolveRelative(currentSlug, folderPath);
    a.dataset.for = folderPath;
    a.className = "folder-title";
    a.textContent = node.displayName;
    button.replaceWith(a);
  } else {
    const span = titleContainer.querySelector(".folder-title");
    span.textContent = node.displayName;
  }

  // if the saved state is collapsed or the default state is collapsed
  const isCollapsed =
    currentExplorerState.find((item) => item.path === folderPath)?.collapsed ??
    opts.folderDefaultState === "collapsed";

  // if this folder is a prefix of the current path we
  // want to open it anyways
  const simpleFolderPath = simplifySlug(folderPath);
  const folderIsPrefixOfCurrentSlug =
    simpleFolderPath === currentSlug.slice(0, simpleFolderPath.length);

  if (!isCollapsed || folderIsPrefixOfCurrentSlug) {
    folderOuter.classList.add("open");
  }

  for (const child of node.children) {
    const childNode = child.isFolder
      ? createFolderNode(currentSlug, child, opts)
      : createFileNode(currentSlug, child);
    ul.appendChild(childNode);
  }

  return li;
}

async function setupExplorer(currentSlug) {
  const allExplorers = document.querySelectorAll("div.explorer");

  for (const explorer of allExplorers) {
    // Always wire explorer expand/collapse toggles
    const explorerButtons = explorer.getElementsByClassName("explorer-toggle");
    for (const button of explorerButtons) {
      button.addEventListener("click", toggleExplorer);
      registerCleanup?.(() =>
        button.removeEventListener("click", toggleExplorer)
      );
    }

    const dataFns = JSON.parse(explorer.dataset.dataFns || "{}");
    const opts = {
      folderClickBehavior: explorer.dataset.behavior || "collapse",
      folderDefaultState: explorer.dataset.collapsed || "collapsed",
      useSavedState: explorer.dataset.savestate === "true",
      order: dataFns.order || ["filter", "map", "sort"],
      sortFn: new Function("return " + (dataFns.sortFn || "undefined"))(),
      filterFn: new Function("return " + (dataFns.filterFn || "undefined"))(),
      mapFn: new Function("return " + (dataFns.mapFn || "undefined"))(),
    };

    // Get folder state from local storage
    const serializedExplorerState = loadSavedState(opts.useSavedState);
    currentExplorerState = [...serializedExplorerState];
    const oldIndex = new Map(
      serializedExplorerState.map((entry) => [entry.path, entry.collapsed])
    );

    // If fetchData is missing (e.g., offline or contentIndex not emitted), keep
    // server-rendered list and still wire toggles.
    if (typeof fetchData === "undefined") {
      applyStateToServerTree(explorer, opts);
      attachFolderToggles(explorer);
      continue;
    }

    let trie;
    try {
      const data = await fetchData;
      const entries = [...Object.entries(data)];
      trie = FileTrieNode.fromEntries(entries);
    } catch (err) {
      console.warn("Explorer: failed to load content index", err);
      applyStateToServerTree(explorer, opts);
      attachFolderToggles(explorer);
      continue;
    }

    // Apply functions in order
    for (const fn of opts.order) {
      switch (fn) {
        case "filter":
          if (opts.filterFn) trie.filter(opts.filterFn);
          break;
        case "map":
          if (opts.mapFn) trie.map(opts.mapFn);
          break;
        case "sort":
          if (opts.sortFn) trie.sort(opts.sortFn);
          break;
      }
    }

    // Get folder paths for state management
    const folderPaths = trie.getFolderPaths();
    currentExplorerState = folderPaths.map((path) => {
      const previousState = oldIndex.get(path);
      return {
        path,
        collapsed:
          previousState === undefined
            ? opts.folderDefaultState === "collapsed"
            : previousState,
      };
    });

    const explorerUl = explorer.querySelector(".explorer-ul");
    if (!explorerUl) continue;

    // Clear any server-rendered fallback items to avoid duplication
    explorerUl.replaceChildren();

    // Create and insert new content
    explorerUl.replaceChildren();
    const fragment = document.createDocumentFragment();
    for (const child of trie.children) {
      const node = child.isFolder
        ? createFolderNode(currentSlug, child, opts)
        : createFileNode(currentSlug, child);

      fragment.appendChild(node);
    }
    explorerUl.appendChild(fragment);

    // Add overflow sentinel for gradient effect
    const overflowEnd = document.createElement("li");
    overflowEnd.className = "overflow-end";
    explorerUl.appendChild(overflowEnd);

    // restore explorer scrollTop position if it exists
    const scrollTop = sessionStorage.getItem("explorerScrollTop");
    if (scrollTop) {
      explorerUl.scrollTop = parseInt(scrollTop);
    } else {
      // try to scroll to the active element if it exists
      const activeElement = explorerUl.querySelector(".active");
      if (activeElement) {
        activeElement.scrollIntoView({ behavior: "smooth" });
      }
    }

    // Set up event handlers
    // Set up folder click handlers
    if (opts.folderClickBehavior === "collapse") {
      attachFolderToggles(explorer);
    }
  }
}

function applyStateToServerTree(explorer, opts) {
  const folderContainers = explorer.querySelectorAll(".folder-container");
  for (const container of folderContainers) {
    const folderOuter = container.nextElementSibling;
    if (!folderOuter) continue;

    const folderPath =
      container.dataset.folderpath ||
      container.querySelector(".folder-title")?.textContent ||
      "";

    const saved = currentExplorerState.find((item) => item.path === folderPath);
    const shouldCollapse =
      saved?.collapsed ?? opts.folderDefaultState === "collapsed";

    setFolderState(folderOuter, shouldCollapse);

    if (!saved) {
      currentExplorerState.push({ path: folderPath, collapsed: shouldCollapse });
    }
  }
}

// In Rootz (non-SPA) the `nav` event is never emitted, so hydrate once on load.
if (!registerCleanup) {
  const hydrateExplorer = () => {
    const currentSlug = window.location.pathname.slice(1) || "index";
    setupExplorer(currentSlug);
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", hydrateExplorer, {
      once: true,
    });
  } else {
    hydrateExplorer();
  }
}

document.addEventListener("prenav", async () => {
  // save explorer scrollTop position
  const explorer = document.querySelector(".explorer-ul");
  if (!explorer) return;
  sessionStorage.setItem("explorerScrollTop", explorer.scrollTop.toString());
});

document.addEventListener("nav", async (e) => {
  const currentSlug = e.detail.url;
  await setupExplorer(currentSlug);

  // if mobile hamburger is visible, collapse by default
  for (const explorer of document.getElementsByClassName("explorer")) {
    const mobileExplorer = explorer.querySelector(".mobile-explorer");
    if (!mobileExplorer) return;

    if (mobileExplorer.checkVisibility()) {
      explorer.classList.add("collapsed");
      explorer.setAttribute("aria-expanded", "false");

      // Allow <html> to be scrollable when mobile explorer is collapsed
      document.documentElement.classList.remove("mobile-no-scroll");
    }

    mobileExplorer.classList.remove("hide-until-loaded");
  }
});

window.addEventListener("resize", function () {
  // Desktop explorer opens by default, and it stays open when the window is resized
  // to mobile screen size. Applies `no-scroll` to <html> in this edge case.
  const explorer = document.querySelector(".explorer");
  if (explorer && !explorer.classList.contains("collapsed")) {
    document.documentElement.classList.add("mobile-no-scroll");
    return;
  }
});

function setFolderState(folderElement, collapsed) {
  return collapsed
    ? folderElement.classList.remove("open")
    : folderElement.classList.add("open");
}
