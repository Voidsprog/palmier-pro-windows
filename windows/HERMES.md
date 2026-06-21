# Conectar Hermes Agent ao Palmier Pro (Windows)

O [Hermes Agent](https://hermes-agent.nousresearch.com/) fala MCP nativo (stdio e HTTP). O Palmier Pro Windows expõe um servidor MCP em:

```
http://127.0.0.1:19789/mcp
```

## Pré-requisitos

1. **Palmier Pro Windows** a correr (`palmier-pro-windows.exe`)
2. Um **projeto aberto** no app (New project ou Open project)
3. Hermes com suporte HTTP MCP instalado

## Configuração

Edita `~/.hermes/config.yaml` (Windows: `%USERPROFILE%\.hermes\config.yaml`):

```yaml
mcp_servers:
  palmier-pro:
    url: "http://127.0.0.1:19789/mcp"
    timeout: 180
    connect_timeout: 30
    supports_parallel_tool_calls: false
```

Depois, numa sessão Hermes ativa:

```
/reload-mcp
```

## Verificar ligação

Pede ao Hermes:

> Lista as tools do servidor palmier-pro

Deves ver tools como `get_timeline`, `get_media`, `import_media`, `add_clips`, `move_clips`, `export_video`, etc.

## Fluxo típico

1. Abre/cria projeto no Palmier Pro
2. Inicia Hermes com a config acima
3. Exemplo de prompt:

> Importa `C:\Videos\clip.mp4` com import_media, depois coloca na timeline no frame 0 com add_clips.

## Alternativa: CLI Hermes

```bash
hermes mcp add palmier-pro --url http://127.0.0.1:19789/mcp
```

## Notas

- O servidor MCP só escuta em **127.0.0.1** (localhost)
- Geração AI (macOS) não está disponível no Windows — `canGenerate` é `false`
- Se a porta 19789 estiver ocupada, fecha outras instâncias do Palmier Pro
- Hermes no WSL + Palmier no Windows: usa o IP do host Windows em vez de `127.0.0.1` se necessário

## Cursor (alternativa)

Em `%USERPROFILE%\.cursor\mcp.json`:

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
