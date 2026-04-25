import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Ref } from "vue";
import type { QueryNameEntry, ToastTone } from "../types/dict";
import { resolveErrorMessage } from "../utils/error";

type SearchEngine = "google" | "bing" | "baidu";

interface UseEntryActionsOptions {
  searchEngine: Ref<SearchEngine>;
  showToast: (message: string, tone?: ToastTone) => void;
}

export function useEntryActions({ searchEngine, showToast }: UseEntryActionsOptions) {
  async function createEditorWindow(): Promise<void> {
    const existing = await WebviewWindow.getByLabel("editor");
    if (existing) {
      try {
        await existing.emit("editor-seed-updated");
        await existing.setAlwaysOnTop(true);
        await existing.show();
        await existing.setFocus();
        return;
      } catch {
        try {
          await existing.close();
        } catch {
          // Ignore stale window close errors.
        }
      }
    }

    const editor = new WebviewWindow("editor", {
      url: "/editor.html",
      title: "编辑词条",
      width: 540,
      height: 400,
      minWidth: 540,
      minHeight: 400,
      decorations: false,
      transparent: true,
      shadow: false,
      resizable: false,
      center: true,
      focus: true,
      alwaysOnTop: true,
    });

    await new Promise<void>((resolve, reject) => {
      editor.once("tauri://created", () => resolve());
      editor.once("tauri://error", (event) =>
        reject(new Error(String(event.payload ?? "未知错误"))),
      );
    });

    try {
      await editor.setAlwaysOnTop(true);
    } catch {
      // Ignore optional z-order failures from OS focus policy.
    }
    try {
      await editor.show();
    } catch {
      // Window is usually visible already.
    }
    try {
      await editor.setFocus();
    } catch {
      // Ignore focus-steal restrictions on Windows.
    }
  }

  async function openEditor(entry: QueryNameEntry): Promise<void> {
    if (!entry.editable) {
      showToast("内置词库词条不可编辑", "error");
      return;
    }
    try {
      await invoke("set_editor_seed", { term: entry.term });
      await createEditorWindow();
    } catch (error) {
      showToast(resolveErrorMessage(error, "打开编辑窗口失败"), "error");
    }
  }

  async function copyTerm(term: string): Promise<void> {
    const text = term.trim();
    if (!text) {
      return;
    }

    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        showToast(`复制：${text}`);
        return;
      }

      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.style.position = "fixed";
      textarea.style.left = "-9999px";
      document.body.appendChild(textarea);
      textarea.select();
      const success = document.execCommand("copy");
      document.body.removeChild(textarea);
      if (!success) {
        throw new Error("copy failed");
      }
      showToast(`复制：${text}`);
    } catch (error) {
      showToast(resolveErrorMessage(error, "复制词条失败"), "error");
    }
  }

  function buildSearchText(entry: QueryNameEntry): string {
    const term = entry.term.trim();
    const group = entry.group.trim();
    if (!term) {
      return "";
    }
    return group ? `${group} ${term}` : term;
  }

  function buildSearchUrl(text: string): URL {
    if (searchEngine.value === "bing") {
      const url = new URL("https://www.bing.com/search");
      url.searchParams.set("q", text);
      return url;
    }
    if (searchEngine.value === "baidu") {
      const url = new URL("https://www.baidu.com/s");
      url.searchParams.set("wd", text);
      return url;
    }
    const url = new URL("https://www.google.com/search");
    url.searchParams.set("q", text);
    return url;
  }

  async function searchTermInBrowser(entry: QueryNameEntry): Promise<void> {
    const text = buildSearchText(entry);
    if (!text) {
      return;
    }
    const url = buildSearchUrl(text);
    try {
      await openUrl(url);
      showToast(`搜索：${text}`);
    } catch (error) {
      showToast(resolveErrorMessage(error, "打开浏览器搜索失败"), "error");
    }
  }

  async function handleEntryClick(event: MouseEvent, entry: QueryNameEntry): Promise<void> {
    if (event.ctrlKey && event.button === 0) {
      event.preventDefault();
      await searchTermInBrowser(entry);
      return;
    }
    await copyTerm(entry.term);
  }

  return {
    createEditorWindow,
    handleEntryClick,
    openEditor,
  };
}
