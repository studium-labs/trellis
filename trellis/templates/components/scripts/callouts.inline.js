function toggleCallout() {
  const outerBlock = this.parentElement;
  outerBlock.classList.toggle("is-collapsed");
  const content = outerBlock.getElementsByClassName("callout-content")[0];
  if (!content) return;
  const collapsed = outerBlock.classList.contains("is-collapsed");
  content.style.gridTemplateRows = collapsed ? "0fr" : "1fr";
}

function setupCallout() {
  const collapsible = document.getElementsByClassName(`callout is-collapsible`);
  for (const div of collapsible) {
    const title = div.getElementsByClassName("callout-title")[0];
    const content = div.getElementsByClassName("callout-content")[0];
    if (!title || !content) continue;

    title.addEventListener("click", toggleCallout);
    window.addCleanup(() => title.removeEventListener("click", toggleCallout));

    const collapsed = div.classList.contains("is-collapsed");
    content.style.gridTemplateRows = collapsed ? "0fr" : "1fr";
  }
}

document.addEventListener("nav", setupCallout);
