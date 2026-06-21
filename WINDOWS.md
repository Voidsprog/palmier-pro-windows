# Palmier Pro — Windows Fork

Fork de [palmier-io/palmier-pro](https://github.com/palmier-io/palmier-pro) com app Windows via Tauri 2.

## Quick start

```powershell
cd windows
npm install
npm run tauri:build
```

Executável: `windows/src-tauri/target/release/palmier-pro-windows.exe`

## O que está implementado

- **Preview FFmpeg** — frames JPEG via playhead
- **Edição** — mover clips, inspector (start/duration/volume/opacity), split, remove, undo
- **Export** — H.264 e H.265 (MP4) com composição multi-track
- **MCP** — servidor HTTP na porta 19789, compatível com Cursor/Claude

Requer FFmpeg instalado e no PATH.

Ver [windows/README.md](windows/README.md) para detalhes.
