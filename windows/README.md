# Palmier Pro — Windows (Tauri)

Fork Windows do Palmier Pro. O editor macOS permanece em `Sources/`; esta pasta contém o app desktop Windows empacotado com [Tauri 2](https://v2.tauri.app/).

## Pré-requisitos

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) 1.77+
- [FFmpeg](https://ffmpeg.org/) no PATH (preview e export)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++ workload)
- WebView2 (incluído no Windows 11)

## Desenvolvimento

```powershell
cd windows
npm install
npm run tauri:dev
```

## Build do executável

```powershell
cd windows
npm run tauri:build
```

Saída:

| Artefato | Caminho |
|----------|---------|
| `.exe` portable | `src-tauri/target/release/palmier-pro-windows.exe` |
| Instalador NSIS | `src-tauri/target/release/bundle/nsis/Palmier Pro_0.1.0_x64-setup.exe` |
| MSI | `src-tauri/target/release/bundle/msi/Palmier Pro_0.1.0_x64_en-US.msi` |

## Funcionalidades

| Recurso | Status |
|---------|--------|
| Abrir/salvar `.palmier` | ✅ |
| Preview FFmpeg (frame a frame + play) | ✅ |
| Edição de clips (mover, split, remove, props) | ✅ |
| Export H.264 / H.265 | ✅ |
| MCP HTTP `http://127.0.0.1:19789/mcp` | ✅ |
| Undo | ✅ |
| Geração AI (macOS only) | ❌ |

## MCP (Cursor / Claude)

Com o app aberto:

```json
{
  "mcpServers": {
    "palmier-pro": {
      "type": "http",
      "url": "http://127.0.0.1:19789/mcp"
    }
  }
}
```

Tools: `get_timeline`, `get_media`, `add_clips`, `remove_clips`, `move_clips`, `set_clip_properties`, `split_clip`, `undo`, `export_video`.

## Formato de projeto

Compatível com pacotes `.palmier` do macOS (`project.json`, `media.json`, `media/`).
