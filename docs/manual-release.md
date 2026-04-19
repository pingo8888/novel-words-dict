# 手动发布（不依赖 GitHub Actions）

本项目已接入 Tauri Updater，并使用 GitHub Release 作为更新源：

- updater 端点：`https://github.com/pingo8888/novel-words-dict/releases/latest/download/latest.json`
- 只有“最新稳定版 release”会被客户端自动检查到（不要把自动更新版本发成 prerelease）。

## 1. 准备签名密钥（一次性）

> 私钥不要提交到仓库。

生成密钥：

```bash
npm run tauri signer generate -- --ci
```

发布前设置环境变量（二选一）：

- `TAURI_SIGNING_PRIVATE_KEY`：私钥字符串
- `TAURI_SIGNING_PRIVATE_KEY_PATH`：私钥文件路径
- 可选：`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

## 2. 发布版本

推荐直接使用脚本：

```powershell
./scripts/manual-release.ps1 -Tag v0.1.3
```

脚本会执行：

1. `npm run build`
2. `npm run tauri build -- --config '{"build":{"beforeBuildCommand":""}}'`
3. 检查以下 3 个发布必需文件：
   - `src-tauri/target/release/bundle/nsis/*-setup.exe`
   - `src-tauri/target/release/bundle/nsis/*-setup.exe.sig`
   - `src-tauri/target/release/bundle/nsis/latest.json`

若构建未自动产出 `latest.json`，脚本会基于当前 `tag`、`origin` 仓库地址和安装包签名自动生成。

## 3. 上传到 GitHub Release

把上面 3 个文件上传到同一个稳定版 release（tag 例如 `v0.1.3`）。

如果你使用 GitHub CLI：

```bash
gh release upload v0.1.3 "<installer>" "<installer>.sig" "<latest.json>" --clobber
```

## 4. 验证更新

安装旧版本客户端后，启动应用：

- 启动后会自动检查更新（开发模式不会自动检查）
- 设置页可点击“检查更新”
- 确认安装后可选择立即重启生效

若未检测到更新，优先检查：

- release 是否为稳定版（不是 prerelease）
- `latest.json` 是否已上传且可访问
- 版本号是否递增
- `.sig` 与安装包是否来自同一次构建
