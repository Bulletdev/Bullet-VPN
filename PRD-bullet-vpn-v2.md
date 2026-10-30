# PRD -- Bullet VPN v2

**Autor**: Michael Douglas
**Data**: 2026-10-30
**Status**: Draft

---

## 1. Contexto e Motivacao

### Problema

Gerenciar conexoes WireGuard manualmente e insustentavel:

- Gerar chaves, copiar configs, registrar peers via SSH, liberar portas no firewall, configurar tunnels -- tudo manual, fragil, e propenso a erro
- Cada reconexao (troca de rede, reboot, mudanca de porta) exige repetir o processo
- Ferramentas existentes (Mullvad, Tailscale) resolvem o tunnel mas nao dao controle total sobre a infra propria
- Profissionais de seguranca (pentest, bounty) precisam de VPN propria com visibilidade total e sem logs de terceiros

### Origem

O Bullet VPN v1 nasceu como prototipo Java/Swing com BouncyCastle para geracao de chaves X25519. Nunca foi compilado. A dor real apareceu ao configurar VPN entre maquina local e VPS Coolify (ProStaff API) -- processo que levou horas de troubleshooting manual.

### Oportunidades

1. **Uso pessoal**: conectar em VPSs (ProStaff, bounty labs) com um clique
2. **Bug bounty / pentest**: VPN propria com zero logs, controle de rotas, integrada ao workflow de RE (rea + AnarCry)
3. **Empresa (Effront)**: distribuir como ferramenta interna para equipe acessar infra privada
4. **Open source / portfolio**: projeto diferenciavel com RE do Mullvad como base tecnica

---

## 2. Analise do Mullvad (Reverse Engineering)

### Stack do Mullvad

| Camada | Tecnologia | Funcao |
|--------|-----------|--------|
| Daemon | Rust | Gerencia tunnels WireGuard, firewall, kill switch |
| Main Process | Electron + gRPC | IPC com daemon via Unix socket, tray, lifecycle |
| Renderer | React 19 + Redux + styled-components 6 | UI SPA, state management, i18n |

### Padroes Relevantes

- **Separacao privilegiada**: GUI nunca toca WireGuard direto -- tudo via daemon. Kill switch e firewall sao responsabilidade exclusiva do daemon
- **gRPC + Protobuf**: comunicacao tipada entre GUI e daemon (tunnel CRUD, relay selection, account, split tunneling, settings)
- **Split tunneling Linux**: usa `cgroup/net_cls` + binario `mullvad-exclude` para isolar processos fora do tunnel. Parseia `.desktop` files para listar apps
- **Relay selection**: `relays.json` bundled + gRPC service para discovery de servidores
- **UI**: styled-components (sem utility classes), Open Sans/Source Sans Pro, motion v12 para animacoes, WebGL map (gl-matrix) para localizacao de relays

### Ferramentas de RE (rea)

O projeto `rea` sera usado para analise profunda:

- `inspect_electron_fuses`: analisar security fuses do Mullvad sem executar
- `capture_electron_scenario`: capturar IPC events (main-handler, renderer-send) em runtime
- **mitmproxy bridge**: interceptar comunicacao gRPC daemon-GUI
- **JS analysis**: AST analysis via Babel, deobfuscacao, source-map resolution
- **MCP server**: `rea mcp` como tool no Claude Code para RE interativo

---

## 3. Visao do Produto

### Bullet VPN v2

> Client VPN multiplataforma com controle total sobre infraestrutura propria. Conecta, gerencia peers, e abre tunnels com um clique -- sem depender de terceiros, sem logs, sem fricao manual.

### Diferenciais

1. **Self-hosted first**: projetado para VPS propria, nao para servicos de VPN comercial
2. **Auto-provisioning**: registra peer no servidor automaticamente via SSH
3. **Perfis de conexao**: multiplos servidores com switch instantaneo
4. **SSH tunnel integrado**: port forward sem VPN completa (caso DBeaver)
5. **Bounty-ready**: rotas granulares, split tunneling, integracao com workflow de pentest

---

## 4. Arquitetura

### Stack Proposta

| Camada | Tecnologia | Justificativa |
|--------|-----------|---------------|
| Desktop | **Tauri 2** (Rust backend + WebView) | Binario ~5MB vs ~150MB do Electron. Rust nativo para ops privilegiadas |
| Frontend | **React + Tailwind + shadcn/ui** | Stack conhecida (ProStaff Analytics Hub), rapida de iterar |
| Tunnel | **WireGuard CLI** (`wg` / `wg-quick`) | Subprocess model -- simples, auditavel, sem reimplementar protocolo |
| Privileged ops | **Tauri sidecar** (Rust) | Executa `wg-quick`, `ssh`, `wg` com elevacao de privilegio via polkit |
| Config | **SQLite local** | Perfis, chaves (encriptadas), historico de conexoes |
| Packaging | **Tauri bundler** | .deb, .AppImage, .exe, .dmg nativos |

### Por que Tauri e nao Electron?

- Binario 30x menor (5MB vs 150MB)
- Backend Rust nativo (ops de sistema sem Node child_process)
- Menor superficie de ataque (sem V8 completo, sem Node)
- Memoria: ~30MB vs ~150MB+ do Electron
- O Mullvad ja usa Rust no daemon -- validacao de que Rust e a escolha certa para VPN

### Diagrama de Componentes

```
+-------------------------------------------+
|              Tauri Window                  |
|  +-------------------------------------+  |
|  |    React Frontend (WebView)         |  |
|  |  - Connection profiles UI           |  |
|  |  - Status dashboard                 |  |
|  |  - Log viewer                       |  |
|  |  - Settings                         |  |
|  +-------------------------------------+  |
|              |  Tauri IPC  |               |
|  +-------------------------------------+  |
|  |    Rust Backend (privileged)         |  |
|  |  - WireGuard manager (wg/wg-quick)  |  |
|  |  - SSH tunnel manager               |  |
|  |  - Key manager (encrypt/decrypt)    |  |
|  |  - Peer auto-registrar (SSH)        |  |
|  |  - Kill switch (iptables/nftables)  |  |
|  +-------------------------------------+  |
+-------------------------------------------+
         |              |             |
    WireGuard       SSH tunnel    SQLite
    interface       (port fwd)    (profiles)
```

---

## 5. Features e Fases

### Fase 1 -- MVP funcional (2-3 semanas)

**Objetivo**: substituir todo o processo manual que fizemos hoje por um clique.

| Feature | Descricao | Prioridade |
|---------|-----------|------------|
| Scaffold Tauri 2 | Projeto base com React + Tailwind + shadcn/ui | P0 |
| Geracao de chaves | X25519 via Rust (nao mais BouncyCastle Java) | P0 |
| Perfis de conexao | CRUD de perfis (nome, endpoint, server pubkey, allowed IPs, DNS) | P0 |
| Conectar/desconectar | `wg-quick up/down` via Tauri sidecar | P0 |
| Status em tempo real | Handshake, transfer, latency via `wg show` polling | P0 |
| Log viewer | Stream de logs em tempo real na UI | P0 |
| Tray icon | Minimizar para tray, status visual (conectado/desconectado) | P1 |
| Persistencia SQLite | Salvar perfis e chaves encriptadas localmente | P0 |

### Fase 2 -- Auto-provisioning (1-2 semanas)

**Objetivo**: o client registra o peer no servidor sem voce ter que SSH manualmente.

| Feature | Descricao | Prioridade |
|---------|-----------|------------|
| SSH key manager | Listar/selecionar chaves SSH locais | P0 |
| Auto-register peer | SSH no servidor, roda `wg set` + `wg-quick save` automaticamente | P0 |
| Server profiles | Associar SSH credentials ao perfil de conexao | P0 |
| Firewall check | Testar conectividade UDP antes de conectar, sugerir porta alternativa | P1 |
| Import/export config | Importar .conf do WireGuard, exportar perfil | P1 |

### Fase 3 -- SSH Tunnel + Port Forward (1 semana)

**Objetivo**: cenario DBeaver -- port forward sem VPN completa.

| Feature | Descricao | Prioridade |
|---------|-----------|------------|
| SSH tunnel manager | Criar tunnels (local forward, remote forward, dynamic/SOCKS) | P0 |
| Tunnel profiles | Salvar tunnels frequentes (ex: "DBeaver ProStaff" = L15432:127.0.0.1:5432) | P0 |
| Quick-tunnel | Um clique para abrir tunnel pre-configurado | P0 |
| Tunnel status | Monitorar tunnels ativos, reconectar automaticamente | P1 |

### Fase 4 -- Seguranca avancada (2 semanas)

**Objetivo**: features de seguranca para bounty/pentest.

| Feature | Descricao | Prioridade |
|---------|-----------|------------|
| Kill switch | iptables/nftables rules que bloqueiam trafego fora do tunnel | P0 |
| Split tunneling | Escolher quais apps passam pelo tunnel (cgroup approach do Mullvad) | P1 |
| DNS leak protection | Forcar DNS pelo tunnel, detectar leaks | P0 |
| Route control | AllowedIPs granular por perfil (full tunnel vs split) | P1 |
| Connection guard | Reconexao automatica em troca de rede (wifi -> 4G) | P1 |

### Fase 5 -- Polish e distribuicao (1-2 semanas)

| Feature | Descricao | Prioridade |
|---------|-----------|------------|
| Dark/light theme | Seguir preferencia do sistema | P1 |
| i18n | pt-BR + en-US | P1 |
| Auto-update | Check de versao + download automatico | P2 |
| Packaging | .deb, .AppImage (Linux), .exe (Windows), .dmg (macOS) | P0 |
| Onboarding | Wizard de primeira configuracao | P1 |

---

## 6. Crossover com Outros Projetos

### AnarCry (Bug Bounty)

- Bullet VPN como infra de anonimizacao para engajamentos de bounty
- Perfis de conexao por target (cada bounty program = um perfil VPN)
- Integracao futura: AnarCry lanca scan -> Bullet VPN garante que trafego sai pelo tunnel correto
- Split tunneling: browser de bounty pelo tunnel, resto da maquina direto

### Effront (Empresa)

- Distribuir como ferramenta interna para equipe acessar VPSs da empresa
- Server profiles pre-configurados (admin provisiona, dev so conecta)
- Cenario: novo dev entra -> instala Bullet VPN -> importa perfil da empresa -> acessa infra privada
- Potencial produto SaaS: painel web para admin gerenciar peers (fase futura)

### ProStaff (Uso Imediato)

- Perfil "ProStaff VPS" com auto-connect
- SSH tunnel "DBeaver" pre-configurado
- Acesso ao Coolify (10.8.0.1:8000) com um clique

### rea (Reverse Engineering)

- rea como MCP server no Claude Code para RE continuo do Mullvad
- Capturar IPC patterns do Mullvad para replicar no Bullet VPN
- Analisar split tunneling do Mullvad em profundidade (cgroup, `mullvad-exclude`)
- Documentar findings como referencia tecnica

---

## 7. Requisitos Nao-Funcionais

| Requisito | Target |
|-----------|--------|
| Tamanho do binario | < 10MB |
| Memoria em uso | < 50MB |
| Tempo de conexao | < 3s (handshake WireGuard) |
| Startup | < 1s |
| Plataformas | Linux (prioritario), Windows, macOS |
| Seguranca | Chaves encriptadas em repouso (SQLite + AES-256-GCM) |
| Zero logs remotos | Nenhum dado sai da maquina sem consentimento |

---

## 8. Riscos e Mitigacoes

| Risco | Impacto | Mitigacao |
|-------|---------|-----------|
| Tauri 2 ainda jovem | Bugs de plataforma, API instavel | Manter fallback CLI (tudo que a GUI faz, o CLI tambem faz) |
| Elevacao de privilegio | UX ruim se pedir senha toda vez | polkit no Linux, UAC no Windows, osascript no macOS |
| Split tunneling complexo | cgroup approach e platform-specific | Deixar para Fase 4, comecar com AllowedIPs granular |
| SSH auto-register seguranca | Execucao remota automatica | Confirmar com usuario antes, mostrar exatamente o que sera executado |

---

## 9. Metricas de Sucesso

### MVP (Fase 1)

- [ ] Conectar na VPS ProStaff em < 3 cliques
- [ ] Perfil salvo persiste entre reboots
- [ ] Status de conexao em tempo real
- [ ] Binario < 10MB

### Produto (Fase 1-5)

- [ ] Tempo medio de setup de nova VPN: < 2 minutos (vs 1+ hora manual)
- [ ] Zero interacao manual com terminal para conectar
- [ ] Funciona em Linux + Windows
- [ ] Usado por pelo menos 2 projetos (ProStaff + bounty)

---

## 10. Timeline Estimada

```
Semana 1-3:  Fase 1 (MVP) -- scaffold Tauri, perfis, connect/disconnect
Semana 4-5:  Fase 2 (Auto-provisioning) -- SSH auto-register
Semana 6:    Fase 3 (SSH Tunnels) -- port forward manager
Semana 7-8:  Fase 4 (Seguranca) -- kill switch, DNS protection
Semana 9-10: Fase 5 (Polish) -- packaging, themes, i18n
```

**Total estimado**: 8-10 semanas para produto completo.
**MVP usavel**: 3 semanas.

---

## 11. Decisoes em Aberto

1. **Nome final**: manter "Bullet VPN" ou renomear? (considerando bounty portfolio e empresa)
2. **Licenca**: MIT (atual) ou GPL (como Mullvad)? MIT e melhor para uso comercial pela Effront
3. **Server-side component**: so CLI (`wg set`) ou API REST no servidor para provisioning?
4. **Tauri vs Electron**: PRD assume Tauri, mas se Electron facilitar o reuso de patterns do Mullvad, reconsiderar

---

**Proximos passos**: validar este PRD, configurar rea como MCP server, iniciar scaffold Tauri do Fase 1.
