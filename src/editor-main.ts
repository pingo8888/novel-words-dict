import { createApp } from "vue";
import EditorPage from "./pages/EditorPage.vue";
import "./style.css";

function showBootstrapError(message: string): void {
  const root = document.getElementById("app");
  if (!root) {
    return;
  }
  root.innerHTML = `<div style="padding:12px;color:#b91c1c;font-family:'Microsoft YaHei','微软雅黑','Microsoft YaHei UI',sans-serif;font-size:14px;white-space:pre-wrap;">${message}</div>`;
}

window.addEventListener("error", (event) => {
  const reason = event.error instanceof Error ? event.error.stack || event.error.message : String(event.message);
  showBootstrapError(`编辑窗口加载失败:\n${reason}`);
});

window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason instanceof Error ? event.reason.stack || event.reason.message : String(event.reason);
  showBootstrapError(`编辑窗口初始化异常:\n${reason}`);
});

try {
  createApp(EditorPage).mount("#app");
} catch (error) {
  const reason = error instanceof Error ? error.stack || error.message : String(error);
  showBootstrapError(`编辑窗口启动失败:\n${reason}`);
}
