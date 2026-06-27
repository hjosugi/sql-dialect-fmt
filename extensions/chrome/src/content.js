const BRIDGE_SOURCE = "sql-dialect-fmt:bridge";
const PAGE_SOURCE = "sql-dialect-fmt:page";
const DEFAULT_OPTIONS = {
  lineWidth: 100,
  indentWidth: 4,
  uppercaseKeywords: true
};

let requestSequence = 0;
let running = false;

injectBridge();
installToolbar();

chrome.runtime.onMessage.addListener((message) => {
  if (message?.type === "sql-dialect-fmt:run") {
    runFormatter();
  }
});

window.addEventListener(
  "keydown",
  (event) => {
    if (event.altKey && event.shiftKey && event.key.toLowerCase() === "f") {
      event.preventDefault();
      event.stopPropagation();
      runFormatter();
    }
  },
  true
);

async function runFormatter() {
  if (running) {
    return;
  }

  running = true;
  setButtonBusy(true);
  try {
    const target = await requestEditorRead();
    if (!target.ok) {
      throw new Error(target.error || "No editable SQL editor was found.");
    }
    if (!target.text.trim()) {
      showToast("Nothing to format.");
      return;
    }

    const response = await chrome.runtime.sendMessage({
      type: "sql-dialect-fmt:format",
      source: target.text,
      options: DEFAULT_OPTIONS
    });

    if (!response?.ok) {
      throw new Error(response?.error || "Formatter did not return a result.");
    }

    const write = await requestEditorWrite(target.targetId, response.formatted);
    if (!write.ok) {
      throw new Error(write.error || "Could not update the editor.");
    }

    showToast(response.formatted === target.text ? "Already formatted." : "Formatted with sql-dialect-fmt.");
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error));
  } finally {
    running = false;
    setButtonBusy(false);
  }
}

function requestEditorRead() {
  return pageRequest("read");
}

function requestEditorWrite(targetId, text) {
  return pageRequest("write", { targetId, text });
}

function pageRequest(kind, payload = {}) {
  const requestId = `${Date.now()}:${++requestSequence}`;
  return new Promise((resolve) => {
    const timer = window.setTimeout(() => {
      cleanup();
      resolve({ ok: false, error: "Timed out while talking to the Snowsight editor." });
    }, 4000);

    function onMessage(event) {
      if (
        event.source !== window ||
        event.data?.source !== PAGE_SOURCE ||
        event.data?.requestId !== requestId
      ) {
        return;
      }
      cleanup();
      resolve(event.data);
    }

    function cleanup() {
      window.clearTimeout(timer);
      window.removeEventListener("message", onMessage);
    }

    window.addEventListener("message", onMessage);
    window.postMessage({ source: BRIDGE_SOURCE, kind, requestId, ...payload }, "*");
  });
}

function injectBridge() {
  const script = document.createElement("script");
  script.src = chrome.runtime.getURL("src/editor-bridge.js");
  script.async = false;
  script.onload = () => script.remove();
  (document.documentElement || document.head || document.body).appendChild(script);
}

function installToolbar() {
  if (document.querySelector(".sql-dialect-fmt-toolbar")) {
    return;
  }

  const toolbar = document.createElement("div");
  toolbar.className = "sql-dialect-fmt-toolbar";

  const button = document.createElement("button");
  button.type = "button";
  button.className = "sql-dialect-fmt-button";
  button.textContent = "sql-dialect-fmt";
  button.title = "Format active Snowsight editor with sql-dialect-fmt (Alt+Shift+F)";
  button.addEventListener("click", runFormatter);

  toolbar.append(button);
  document.documentElement.append(toolbar);
}

function setButtonBusy(busy) {
  const button = document.querySelector(".sql-dialect-fmt-button");
  if (!button) {
    return;
  }
  button.disabled = busy;
  button.textContent = busy ? "Formatting" : "sql-dialect-fmt";
}

function showToast(message) {
  const toolbar = document.querySelector(".sql-dialect-fmt-toolbar");
  if (!toolbar) {
    return;
  }

  const existing = toolbar.querySelector(".sql-dialect-fmt-toast");
  if (existing) {
    existing.remove();
  }

  const toast = document.createElement("div");
  toast.className = "sql-dialect-fmt-toast";
  toast.textContent = message;
  toolbar.prepend(toast);
  window.setTimeout(() => toast.remove(), 3600);
}
