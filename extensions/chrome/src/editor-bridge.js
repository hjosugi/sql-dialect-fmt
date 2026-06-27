(() => {
  const BRIDGE_SOURCE = "sql-dialect-fmt:bridge";
  const PAGE_SOURCE = "sql-dialect-fmt:page";

  if (window.__snowFmtBridgeInstalled) {
    return;
  }
  window.__snowFmtBridgeInstalled = true;

  let targetSequence = 0;
  const targets = new Map();
  const monacoEditors = new Set();
  let focusedMonacoEditor = null;

  installMonacoTracker();

  window.addEventListener("message", (event) => {
    if (event.source !== window || event.data?.source !== BRIDGE_SOURCE) {
      return;
    }

    if (event.data.kind === "read") {
      handleRead(event.data.requestId);
    } else if (event.data.kind === "write") {
      handleWrite(event.data.requestId, event.data.targetId, event.data.text);
    }
  });

  function handleRead(requestId) {
    try {
      const target = findActiveTarget();
      if (!target) {
        respond(requestId, { ok: false, error: "Focus a Snowsight SQL editor, then run sql-dialect-fmt again." });
        return;
      }

      const targetId = `target:${++targetSequence}`;
      targets.set(targetId, target);
      respond(requestId, { ok: true, targetId, text: target.read() });
    } catch (error) {
      respond(requestId, { ok: false, error: messageFromError(error) });
    }
  }

  function handleWrite(requestId, targetId, text) {
    try {
      const target = targets.get(targetId);
      targets.delete(targetId);
      if (!target) {
        respond(requestId, { ok: false, error: "The editor target is no longer available." });
        return;
      }
      target.write(String(text ?? ""));
      respond(requestId, { ok: true });
    } catch (error) {
      respond(requestId, { ok: false, error: messageFromError(error) });
    }
  }

  function findActiveTarget() {
    return (
      monacoTarget() ||
      codeMirrorTarget() ||
      textControlTarget(document.activeElement) ||
      contentEditableTarget(document.activeElement)
    );
  }

  function installMonacoTracker() {
    const timer = window.setInterval(() => {
      const monaco = window.monaco;
      if (!monaco?.editor?.create || monaco.editor.create.__snowFmtWrapped) {
        trackExistingMonacoEditors();
        return;
      }

      const originalCreate = monaco.editor.create.bind(monaco.editor);
      monaco.editor.create = (...args) => {
        const editor = originalCreate(...args);
        trackMonacoEditor(editor);
        return editor;
      };
      monaco.editor.create.__snowFmtWrapped = true;
      trackExistingMonacoEditors();
      window.clearInterval(timer);
    }, 500);

    window.setTimeout(() => window.clearInterval(timer), 30000);
  }

  function trackExistingMonacoEditors() {
    const editors = window.monaco?.editor?.getEditors?.();
    if (!Array.isArray(editors)) {
      return;
    }
    for (const editor of editors) {
      trackMonacoEditor(editor);
    }
  }

  function trackMonacoEditor(editor) {
    if (!editor || monacoEditors.has(editor)) {
      return;
    }
    monacoEditors.add(editor);
    if (typeof editor.onDidFocusEditorText === "function") {
      editor.onDidFocusEditorText(() => {
        focusedMonacoEditor = editor;
      });
    }
    if (typeof editor.onDidDispose === "function") {
      editor.onDidDispose(() => {
        monacoEditors.delete(editor);
        if (focusedMonacoEditor === editor) {
          focusedMonacoEditor = null;
        }
      });
    }
  }

  function monacoTarget() {
    trackExistingMonacoEditors();
    const editor = focusedMonacoEditor || focusedTrackedMonacoEditor() || singleTrackedMonacoEditor();
    if (editor?.getModel) {
      const model = editor.getModel();
      if (!model) {
        return null;
      }

      const selection = editor.getSelection?.();
      const useSelection = selection && typeof selection.isEmpty === "function" && !selection.isEmpty();
      const range = useSelection ? selection : model.getFullModelRange();

      return {
        read: () => (useSelection ? model.getValueInRange(range) : model.getValue()),
        write: (text) => {
          editor.pushUndoStop?.();
          editor.executeEdits("sql-dialect-fmt", [{ range, text, forceMoveMarkers: true }]);
          editor.pushUndoStop?.();
          editor.focus?.();
        }
      };
    }

    const monaco = window.monaco;
    const models = monaco?.editor?.getModels?.() || [];
    if (models.length === 1) {
      const model = models[0];
      return {
        read: () => model.getValue(),
        write: (text) => model.setValue(text)
      };
    }

    return null;
  }

  function singleTrackedMonacoEditor() {
    return monacoEditors.size === 1 ? [...monacoEditors][0] : null;
  }

  function focusedTrackedMonacoEditor() {
    const focusedRoot = document.querySelector(".monaco-editor.focused");
    if (!focusedRoot) {
      return null;
    }
    for (const editor of monacoEditors) {
      const node = editor.getDomNode?.();
      if (node && (node === focusedRoot || node.contains(focusedRoot) || focusedRoot.contains(node))) {
        return editor;
      }
    }
    return null;
  }

  function codeMirrorTarget() {
    const active = document.activeElement;
    const root =
      active?.closest?.(".CodeMirror") ||
      document.querySelector(".CodeMirror-focused") ||
      document.querySelector(".CodeMirror.cm-focused");
    const cm = root?.CodeMirror;
    if (!cm) {
      return null;
    }

    const hasSelection = typeof cm.somethingSelected === "function" && cm.somethingSelected();
    return {
      read: () => (hasSelection ? cm.getSelection() : cm.getValue()),
      write: (text) => {
        if (hasSelection) {
          cm.replaceSelection(text);
        } else {
          cm.setValue(text);
        }
        cm.focus?.();
      }
    };
  }

  function textControlTarget(element) {
    if (!(element instanceof HTMLTextAreaElement || element instanceof HTMLInputElement)) {
      return null;
    }

    const start = element.selectionStart ?? 0;
    const end = element.selectionEnd ?? 0;
    const useSelection = end > start;
    return {
      read: () => (useSelection ? element.value.slice(start, end) : element.value),
      write: (text) => {
        if (useSelection && typeof element.setRangeText === "function") {
          element.setRangeText(text, start, end, "end");
        } else {
          element.value = text;
        }
        element.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
        element.dispatchEvent(new Event("change", { bubbles: true }));
        element.focus();
      }
    };
  }

  function contentEditableTarget(element) {
    const editable = element?.closest?.("[contenteditable='true'], [contenteditable='']");
    if (!editable) {
      return null;
    }

    const selection = window.getSelection();
    const selectedText = selection && editable.contains(selection.anchorNode) ? selection.toString() : "";
    return {
      read: () => selectedText || editable.textContent || "",
      write: (text) => {
        if (selectedText && selection?.rangeCount) {
          const range = selection.getRangeAt(0);
          range.deleteContents();
          range.insertNode(document.createTextNode(text));
          selection.removeAllRanges();
        } else {
          editable.textContent = text;
        }
        editable.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
        editable.focus();
      }
    };
  }

  function respond(requestId, payload) {
    window.postMessage({ source: PAGE_SOURCE, requestId, ...payload }, "*");
  }

  function messageFromError(error) {
    return error instanceof Error ? error.message : String(error);
  }
})();
