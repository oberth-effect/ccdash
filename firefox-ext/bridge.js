// The ccdash panel asks for the clipboard with a window message; this
// script, which holds the extension's clipboardRead permission, answers
// with the text and no prompt. Only the page it runs on can ask (same
// window, same origin), and it tells the page it is here on load.
(() => {
  const ASK = "ccdash:clipboard-read";
  const ANSWER = "ccdash:clipboard-text";
  const HERE = "ccdash:clipboard-bridge";
  const HELLO = "ccdash:clipboard-hello";

  // the Clipboard API where the extension's permission reaches it (Firefox),
  // else the older paste command into a field of our own (Chromium's content
  // scripts), which the same permission allows
  async function read() {
    try {
      const text = await navigator.clipboard.readText();
      if (typeof text === "string") return text;
    } catch {
      // fall through
    }
    const field = document.createElement("textarea");
    field.setAttribute("aria-hidden", "true");
    field.style.cssText = "position:fixed;left:-9999px;top:0;opacity:0";
    document.documentElement.appendChild(field);
    const active = document.activeElement;
    try {
      field.focus();
      field.value = "";
      const ok = document.execCommand("paste");
      return ok ? field.value : null;
    } catch {
      return null;
    } finally {
      field.remove();
      if (active && typeof active.focus === "function") active.focus();
    }
  }

  const announce = () =>
    window.postMessage({ type: HERE, version: 1 }, location.origin);
  window.addEventListener("message", async (event) => {
    if (event.source !== window || event.origin !== location.origin) return;
    const msg = event.data;
    if (!msg) return;
    if (msg.type === HELLO) return announce();
    if (msg.type !== ASK || typeof msg.id !== "number") return;
    const text = await read();
    window.postMessage({ type: ANSWER, id: msg.id, text }, location.origin);
  });
  announce();
})();
