import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw,
  Plus,
  X,
  Trash2,
  ChevronDown,
  Import,
  Settings,
  Terminal,
  Lock,
  Globe,
  Play,
  Square,
  Key,
} from "lucide-react";

// ── Types ───────────────────────────────────────────────────────────

interface Profile {
  id: string;
  name: string;
  vpn_type: "wireguard" | "openvpn";
  endpoint: string;
  server_pubkey: string;
  private_key: string;
  public_key: string;
  address: string;
  allowed_ips: string;
  dns: string;
  ovpn_config_path: string;
}

interface ConnectionStatus {
  connected: boolean;
  vpn_type: string;
  interface_name: string;
  handshake_ago: string;
  transfer_rx: string;
  transfer_tx: string;
  endpoint: string;
}

interface SshTunnel {
  id: string;
  name: string;
  tunnel_type: "local" | "remote" | "dynamic";
  local_port: number;
  remote_host: string;
  remote_port: number;
  ssh_host: string;
  ssh_user: string;
  ssh_key: string;
}

type TunnelState =
  | "disconnected"
  | "connecting"
  | "connected"
  | "disconnecting"
  | "error";

type AppView = "vpn" | "ssh" | "settings";

// ── Constants ───────────────────────────────────────────────────────

const EMPTY_PROFILE: Omit<Profile, "id"> = {
  name: "",
  vpn_type: "wireguard",
  endpoint: "",
  server_pubkey: "",
  private_key: "",
  public_key: "",
  address: "10.8.0.2/32",
  allowed_ips: "10.8.0.0/24",
  dns: "1.1.1.1",
  ovpn_config_path: "",
};

const EMPTY_SSH: Omit<SshTunnel, "id"> = {
  name: "",
  tunnel_type: "local",
  local_port: 0,
  remote_host: "127.0.0.1",
  remote_port: 0,
  ssh_host: "",
  ssh_user: "root",
  ssh_key: "~/.ssh/id_ed25519",
};

// ── Custom Icons ────────────────────────────────────────────────────

function ShieldIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className}>
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M8 16L4.35009 13.3929C2.24773 11.8912 1 9.46667 1 6.88306V3L8 0L15 3V6.88306C15 9.46667 13.7523 11.8912 11.6499 13.3929L8 16ZM12.2071 5.70711L10.7929 4.29289L7 8.08579L5.20711 6.29289L3.79289 7.70711L7 10.9142L12.2071 5.70711Z"
        fill="currentColor"
      />
    </svg>
  );
}

// ── App ─────────────────────────────────────────────────────────────

function App() {
  const [view, setView] = useState<AppView>("vpn");

  // VPN state
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selectedProfile, setSelectedProfile] = useState<string | null>(null);
  const [tunnelState, setTunnelState] = useState<TunnelState>("disconnected");
  const [wgStatus, setWgStatus] = useState<ConnectionStatus | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [form, setForm] = useState(EMPTY_PROFILE);
  const fallbackTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Security state
  const [killswitchOn, setKillswitchOn] = useState(false);
  const [dnsProtOn, setDnsProtOn] = useState(false);
  const [dnsServer, setDnsServer] = useState("1.1.1.1");

  // SSH state
  const [sshTunnels, setSshTunnels] = useState<SshTunnel[]>([]);
  const [sshRunning, setSshRunning] = useState<Record<string, boolean>>({});
  const [showSshForm, setShowSshForm] = useState(false);
  const [sshForm, setSshForm] = useState(EMPTY_SSH);

  // Settings state
  const [permInstalled, setPermInstalled] = useState(false);

  // ── Helpers ─────────────────────────────────────────────────────

  const addLog = useCallback((msg: string) => {
    const ts = new Date().toLocaleTimeString("pt-BR", { hour12: false });
    setLogs((prev) => [...prev.slice(-200), `[${ts}] ${msg}`]);
  }, []);

  // ── Load data ───────────────────────────────────────────────────

  const loadProfiles = useCallback(async () => {
    try {
      const result = await invoke<Profile[]>("list_profiles");
      setProfiles(result);
      if (result.length > 0 && !selectedProfile) {
        setSelectedProfile(result[0].id);
      }
    } catch (e) {
      addLog(`Erro ao carregar perfis: ${e}`);
    }
  }, [selectedProfile, addLog]);

  const loadSshTunnels = useCallback(async () => {
    try {
      const result = await invoke<SshTunnel[]>("list_ssh_tunnels");
      setSshTunnels(result);
      const running: Record<string, boolean> = {};
      for (const t of result) {
        running[t.id] = await invoke<boolean>("check_ssh_tunnel", {
          tunnelId: t.id,
        });
      }
      setSshRunning(running);
    } catch {
      /* ignore */
    }
  }, []);

  const checkStatus = useCallback(async () => {
    try {
      const result = await invoke<ConnectionStatus | null>("get_status");
      if (result) {
        setWgStatus(result);
        if (tunnelState === "connecting") {
          if (fallbackTimer.current) clearTimeout(fallbackTimer.current);
          setTunnelState("connected");
          addLog("Tunnel ativo.");
        } else if (tunnelState !== "disconnecting") {
          setTunnelState("connected");
        }
      } else {
        setWgStatus(null);
        if (tunnelState === "connected") {
          setTunnelState("disconnected");
          addLog("Tunnel caiu.");
        }
      }
    } catch {
      if (tunnelState === "connected") {
        setWgStatus(null);
        setTunnelState("disconnected");
      }
    }
  }, [tunnelState, addLog]);

  useEffect(() => {
    loadProfiles();
    loadSshTunnels();
    invoke<boolean>("check_permissions").then(setPermInstalled).catch(() => {});
    invoke<boolean>("killswitch_status").then(setKillswitchOn).catch(() => {});
    invoke<boolean>("dns_protection_status").then(setDnsProtOn).catch(() => {});
  }, [loadProfiles, loadSshTunnels]);

  useEffect(() => {
    const interval = setInterval(checkStatus, 2000);
    return () => clearInterval(interval);
  }, [checkStatus]);

  // ── VPN handlers ────────────────────────────────────────────────

  const allowConnect =
    tunnelState === "disconnected" || tunnelState === "error";
  const allowDisconnect =
    tunnelState === "connected" || tunnelState === "error";

  const doConnect = async () => {
    setTunnelState("connecting");
    setErrorMsg(null);
    addLog("Iniciando conexao...");

    fallbackTimer.current = setTimeout(() => {
      setTunnelState((prev) => {
        if (prev === "connecting") {
          addLog("Timeout: sem resposta em 30s.");
          return "error";
        }
        return prev;
      });
      setErrorMsg("Timeout aguardando autorizacao.");
    }, 30000);

    try {
      await invoke("connect", { profileId: selectedProfile });
      if (fallbackTimer.current) clearTimeout(fallbackTimer.current);
      setTunnelState("connected");
      addLog("Conectado!");
      await checkStatus();
    } catch (e) {
      if (fallbackTimer.current) clearTimeout(fallbackTimer.current);
      const msg = String(e);
      setTunnelState("error");
      setErrorMsg(msg);
      addLog(`Erro: ${msg}`);
    }
  };

  const connectVpn = async () => {
    if (!selectedProfile || !allowConnect) return;

    try {
      addLog("Verificando interfaces conflitantes...");
      const removed = await invoke<string[]>("teardown_conflicts");
      if (removed.length > 0) {
        addLog(`Interfaces removidas: ${removed.join(", ")}`);
      }
    } catch (e) {
      addLog(`Aviso ao limpar conflitos: ${e}`);
    }

    await doConnect();
  };

  const disconnectVpn = async () => {
    if (!allowDisconnect) return;

    setTunnelState("disconnecting");
    setErrorMsg(null);
    addLog("Desconectando...");

    try {
      await invoke("disconnect");
      setTunnelState("disconnected");
      setWgStatus(null);
      setKillswitchOn(false);
      setDnsProtOn(false);
      addLog("Desconectado.");
    } catch (e) {
      const msg = String(e);
      setTunnelState("error");
      setErrorMsg(msg);
      addLog(`Erro: ${msg}`);
    }
  };

  const addProfile = async () => {
    if (!form.name || !form.endpoint || !form.server_pubkey || !form.private_key) {
      addLog("Preencha nome, endpoint, server pubkey e private key.");
      return;
    }

    const id = crypto.randomUUID();
    const profile: Profile = {
      id,
      ...form,
      vpn_type: "wireguard",
      public_key: form.public_key || "",
      address: form.address || "10.8.0.2/32",
      allowed_ips: form.allowed_ips || "10.8.0.0/24",
      dns: form.dns || "1.1.1.1",
      ovpn_config_path: "",
    };

    try {
      await invoke("add_profile", { profile });
      addLog(`Perfil "${form.name}" salvo.`);
      setForm(EMPTY_PROFILE);
      setShowForm(false);
      await loadProfiles();
      setSelectedProfile(id);
    } catch (e) {
      addLog(`Erro ao salvar: ${e}`);
    }
  };

  const importConf = async () => {
    const text = importText.trim();
    const isOvpn =
      text.includes("client") &&
      (text.includes("dev tun") || text.includes("<ca>"));
    const id = crypto.randomUUID();

    if (isOvpn) {
      const remoteMatch = text.match(/remote\s+(\S+)\s+(\d+)/);
      const endpoint = remoteMatch
        ? `${remoteMatch[1]}:${remoteMatch[2]}`
        : "OpenVPN";

      const profile: Profile = {
        id,
        name: `OpenVPN - ${endpoint}`,
        vpn_type: "openvpn",
        endpoint,
        server_pubkey: "",
        private_key: "",
        public_key: "",
        address: "",
        allowed_ips: "",
        dns: "",
        ovpn_config_path: "",
      };

      try {
        await invoke("add_profile", { profile });
        await invoke("import_ovpn", { profileId: id, content: text });
        addLog(`Perfil OpenVPN importado: ${endpoint}`);
        setImportText("");
        setShowImport(false);
        await loadProfiles();
        setSelectedProfile(id);
      } catch (e) {
        addLog(`Erro ao importar OpenVPN: ${e}`);
      }
      return;
    }

    const lines = text.split("\n");
    const parsed: Record<string, string> = {};
    let section = "";

    for (const raw of lines) {
      const line = raw.trim();
      if (line.startsWith("[")) {
        section = line;
        continue;
      }
      const eq = line.indexOf("=");
      if (eq === -1) continue;
      const key = `${section}${line.slice(0, eq).trim()}`;
      parsed[key] = line.slice(eq + 1).trim();
    }

    const privateKey = parsed["[Interface]PrivateKey"] || "";
    const address = parsed["[Interface]Address"] || "10.8.0.2/32";
    const confDns = parsed["[Interface]DNS"] || "1.1.1.1";
    const pubkey = parsed["[Peer]PublicKey"] || "";
    const endpoint = parsed["[Peer]Endpoint"] || "";
    const allowedIPs = parsed["[Peer]AllowedIPs"] || "0.0.0.0/0";

    if (!privateKey || !pubkey || !endpoint) {
      addLog("Conf invalido: falta PrivateKey, PublicKey ou Endpoint.");
      return;
    }

    const profile: Profile = {
      id,
      name: `WireGuard - ${endpoint}`,
      vpn_type: "wireguard",
      endpoint,
      server_pubkey: pubkey,
      private_key: privateKey,
      public_key: "",
      address,
      allowed_ips: allowedIPs,
      dns: confDns,
      ovpn_config_path: "",
    };

    try {
      await invoke("add_profile", { profile });
      addLog(`Perfil WireGuard importado: ${endpoint}`);
      setImportText("");
      setShowImport(false);
      await loadProfiles();
      setSelectedProfile(id);
    } catch (e) {
      addLog(`Erro ao importar: ${e}`);
    }
  };

  const removeProfile = async () => {
    if (!selectedProfile) return;
    const name = profiles.find((p) => p.id === selectedProfile)?.name;
    try {
      await invoke("remove_profile", { profileId: selectedProfile });
      addLog(`Perfil "${name}" removido.`);
      setSelectedProfile(null);
      await loadProfiles();
    } catch (e) {
      addLog(`Erro ao remover: ${e}`);
    }
  };

  // ── Security handlers ───────────────────────────────────────────

  const toggleKillswitch = async () => {
    if (killswitchOn) {
      try {
        await invoke("disable_killswitch");
        setKillswitchOn(false);
        addLog("Kill switch desativado.");
      } catch (e) {
        addLog(`Erro: ${e}`);
      }
    } else {
      if (!wgStatus) {
        addLog("Conecte a VPN antes de ativar o kill switch.");
        return;
      }
      const prof = profiles.find((p) => p.id === selectedProfile);
      const epIp = (prof?.endpoint || "").split(":")[0];
      try {
        await invoke("enable_killswitch", {
          iface: wgStatus.interface_name,
          endpointIp: epIp,
        });
        setKillswitchOn(true);
        addLog("Kill switch ativado.");
      } catch (e) {
        addLog(`Erro: ${e}`);
      }
    }
  };

  const toggleDnsProtection = async () => {
    if (dnsProtOn) {
      try {
        await invoke("disable_dns_protection");
        setDnsProtOn(false);
        addLog("DNS protection desativado.");
      } catch (e) {
        addLog(`Erro: ${e}`);
      }
    } else {
      try {
        await invoke("enable_dns_protection", { dnsServer });
        setDnsProtOn(true);
        addLog(`DNS forcado para ${dnsServer}.`);
      } catch (e) {
        addLog(`Erro: ${e}`);
      }
    }
  };

  // ── SSH handlers ────────────────────────────────────────────────

  const addSshTunnel = async () => {
    if (!sshForm.name || !sshForm.ssh_host || !sshForm.local_port) {
      addLog("Preencha nome, SSH host e porta local.");
      return;
    }
    const tunnel: SshTunnel = { id: crypto.randomUUID(), ...sshForm };
    try {
      await invoke("add_ssh_tunnel", { tunnel });
      addLog(`Tunnel "${sshForm.name}" salvo.`);
      setSshForm(EMPTY_SSH);
      setShowSshForm(false);
      await loadSshTunnels();
    } catch (e) {
      addLog(`Erro: ${e}`);
    }
  };

  const removeSshTunnel = async (id: string) => {
    try {
      await invoke("remove_ssh_tunnel", { tunnelId: id });
      addLog("Tunnel removido.");
      await loadSshTunnels();
    } catch (e) {
      addLog(`Erro: ${e}`);
    }
  };

  const startSshTunnel = async (id: string) => {
    try {
      const pid = await invoke<number>("start_ssh_tunnel", { tunnelId: id });
      setSshRunning((prev) => ({ ...prev, [id]: true }));
      addLog(`Tunnel iniciado (PID ${pid}).`);
    } catch (e) {
      addLog(`Erro: ${e}`);
    }
  };

  const stopSshTunnel = async (id: string) => {
    try {
      await invoke("stop_ssh_tunnel", { tunnelId: id });
      setSshRunning((prev) => ({ ...prev, [id]: false }));
      addLog("Tunnel parado.");
    } catch (e) {
      addLog(`Erro: ${e}`);
    }
  };

  // ── Settings handlers ───────────────────────────────────────────

  const installPerms = async () => {
    addLog("Instalando permissoes (uma senha admin necessaria)...");
    try {
      await invoke("install_permissions");
      setPermInstalled(true);
      addLog("Permissoes instaladas. Sem mais prompts de senha.");
    } catch (e) {
      addLog(`Erro: ${e}`);
    }
  };

  const uninstallPerms = async () => {
    addLog("Removendo permissoes...");
    try {
      await invoke("uninstall_permissions");
      setPermInstalled(false);
      addLog("Permissoes removidas.");
    } catch (e) {
      addLog(`Erro: ${e}`);
    }
  };

  // ── Labels ──────────────────────────────────────────────────────

  const stateLabel: Record<TunnelState, string> = {
    disconnected: "Desconectado",
    connecting: "Conectando...",
    connected: "Conectado",
    disconnecting: "Desconectando...",
    error: "Erro",
  };

  const stateColor: Record<TunnelState, string> = {
    disconnected: "bg-neutral-600",
    connecting: "bg-yellow-500 animate-pulse",
    connected: "bg-bullet-500",
    disconnecting: "bg-yellow-500 animate-pulse",
    error: "bg-red-500",
  };

  const selectedProf = profiles.find((p) => p.id === selectedProfile);

  // ── Render ──────────────────────────────────────────────────────

  return (
    <div className="flex flex-col h-screen bg-neutral-950 text-neutral-100">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-3 bg-neutral-900 border-b border-neutral-800">
        <div className="flex items-center gap-2">
          <ShieldIcon className="w-5 h-5 text-bullet-500" />
          <span className="font-semibold text-sm tracking-tight">
            Bullet VPN
          </span>
        </div>

        <div className="flex items-center gap-1">
          <NavBtn
            active={view === "vpn"}
            onClick={() => setView("vpn")}
            title="VPN"
          >
            <ShieldIcon className="w-3.5 h-3.5" />
          </NavBtn>
          <NavBtn
            active={view === "ssh"}
            onClick={() => setView("ssh")}
            title="SSH Tunnels"
          >
            <Terminal className="w-3.5 h-3.5" />
          </NavBtn>
          <NavBtn
            active={view === "settings"}
            onClick={() => setView("settings")}
            title="Configuracoes"
          >
            <Settings className="w-3.5 h-3.5" />
          </NavBtn>
        </div>

        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${stateColor[tunnelState]}`} />
          <span className="text-xs text-neutral-400">
            {stateLabel[tunnelState]}
          </span>
        </div>
      </header>

      {/* Main */}
      <main className="flex-1 flex flex-col gap-3 p-4 overflow-y-auto">
        {/* ────── VPN VIEW ────── */}
        {view === "vpn" && (
          <>
            {/* Error banner */}
            {tunnelState === "error" && errorMsg && (
              <div className="bg-red-950 border border-red-800 rounded-lg px-3 py-2 text-xs text-red-300">
                {errorMsg}
              </div>
            )}

            {/* Permission warning */}
            {!permInstalled && (
              <button
                onClick={() => setView("settings")}
                className="bg-yellow-950/50 border border-yellow-800/50 rounded-lg px-3 py-2 text-xs text-yellow-400 text-left hover:bg-yellow-950"
              >
                Sem permissoes instaladas -- cada acao pedira senha admin.
                Clique para configurar.
              </button>
            )}

            {/* Profile form */}
            {showForm ? (
              <Card>
                <CardHeader
                  title="Novo Perfil WireGuard"
                  onClose={() => setShowForm(false)}
                />
                <div className="space-y-2">
                  <Field
                    label="Nome *"
                    placeholder="ProStaff VPS"
                    value={form.name}
                    onChange={(v) => setForm({ ...form, name: v })}
                  />
                  <Field
                    label="Endpoint *"
                    placeholder="187.77.39.215:51820"
                    value={form.endpoint}
                    onChange={(v) => setForm({ ...form, endpoint: v })}
                  />
                  <Field
                    label="Server Public Key *"
                    placeholder="PuSN8/kQY9TOs62JMo9m..."
                    value={form.server_pubkey}
                    onChange={(v) => setForm({ ...form, server_pubkey: v })}
                  />
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        const kp = await invoke<{ private_key: string; public_key: string }>("generate_keypair");
                        setForm({ ...form, private_key: kp.private_key, public_key: kp.public_key });
                        addLog("Par de chaves X25519 gerado.");
                      } catch (e) {
                        addLog(`Erro ao gerar chaves: ${e}`);
                      }
                    }}
                    className="w-full flex items-center justify-center gap-2 py-1.5 bg-neutral-800 hover:bg-neutral-700 text-bullet-400 text-xs font-medium border border-neutral-700 rounded transition-colors"
                  >
                    <Key className="w-3.5 h-3.5" />
                    Gerar Chaves X25519
                  </button>
                  <Field
                    label="Private Key *"
                    placeholder="Sua chave privada WireGuard"
                    value={form.private_key}
                    onChange={(v) => setForm({ ...form, private_key: v })}
                    type="password"
                  />
                  <div>
                    <label className="block text-xs text-neutral-500 mb-0.5">Public Key (derivada)</label>
                    <input
                      type="text"
                      readOnly
                      value={form.public_key}
                      placeholder="Gerada automaticamente"
                      className="w-full bg-neutral-800/50 border border-neutral-700 rounded px-3 py-1.5 text-sm text-neutral-400 font-mono cursor-default focus:outline-none"
                    />
                  </div>
                  <Field
                    label="Address"
                    placeholder="10.8.0.2/32"
                    value={form.address}
                    onChange={(v) => setForm({ ...form, address: v })}
                  />
                  <Field
                    label="Allowed IPs"
                    placeholder="10.8.0.0/24"
                    value={form.allowed_ips}
                    onChange={(v) => setForm({ ...form, allowed_ips: v })}
                  />
                  <Field
                    label="DNS"
                    placeholder="1.1.1.1"
                    value={form.dns}
                    onChange={(v) => setForm({ ...form, dns: v })}
                  />
                </div>
                <GreenBtn onClick={addProfile}>Salvar Perfil</GreenBtn>
              </Card>
            ) : showImport ? (
              <Card>
                <CardHeader
                  title="Importar .conf / .ovpn"
                  onClose={() => setShowImport(false)}
                />
                <textarea
                  value={importText}
                  onChange={(e) => setImportText(e.target.value)}
                  placeholder="Cole o conteudo do arquivo .conf (WireGuard) ou .ovpn (OpenVPN) aqui..."
                  className="w-full h-40 bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-xs font-mono text-neutral-200 placeholder-neutral-600 focus:outline-none focus:ring-1 focus:ring-bullet-500 resize-none"
                />
                <GreenBtn onClick={importConf}>Importar</GreenBtn>
              </Card>
            ) : (
              /* Profile selector */
              <Card>
                <div className="flex items-center justify-between mb-3">
                  <h2 className="text-sm font-medium text-neutral-300">
                    Perfil
                  </h2>
                  <div className="flex items-center gap-1">
                    {selectedProfile && (
                      <IconBtn
                        onClick={removeProfile}
                        disabled={tunnelState !== "disconnected"}
                        title="Remover perfil"
                        className="hover:text-red-400"
                      >
                        <Trash2 className="w-4 h-4" />
                      </IconBtn>
                    )}
                    <IconBtn
                      onClick={() => setShowImport(true)}
                      title="Importar .conf / .ovpn"
                    >
                      <Import className="w-4 h-4" />
                    </IconBtn>
                    <IconBtn onClick={() => setShowForm(true)} title="Novo perfil">
                      <Plus className="w-4 h-4" />
                    </IconBtn>
                  </div>
                </div>

                {profiles.length === 0 ? (
                  <button
                    onClick={() => setShowForm(true)}
                    className="w-full py-6 border-2 border-dashed border-neutral-700 rounded-lg text-neutral-500 hover:text-neutral-300 hover:border-neutral-500 transition-colors text-sm"
                  >
                    Adicione um perfil VPN para comecar
                  </button>
                ) : (
                  <div className="relative">
                    <select
                      value={selectedProfile ?? ""}
                      onChange={(e) => setSelectedProfile(e.target.value)}
                      disabled={
                        tunnelState === "connected" ||
                        tunnelState === "connecting"
                      }
                      className="w-full bg-neutral-800 border border-neutral-700 rounded px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:ring-1 focus:ring-bullet-500 appearance-none pr-8 disabled:opacity-50"
                    >
                      {profiles.map((p) => (
                        <option key={p.id} value={p.id}>
                          [{p.vpn_type === "openvpn" ? "OVPN" : "WG"}]{" "}
                          {p.name} -- {p.endpoint}
                        </option>
                      ))}
                    </select>
                    <ChevronDown className="absolute right-2 top-2.5 w-4 h-4 text-neutral-500 pointer-events-none" />
                  </div>
                )}

                {/* Connect button */}
                {profiles.length > 0 && (
                  <button
                    onClick={
                      tunnelState === "connected" || tunnelState === "error"
                        ? disconnectVpn
                        : connectVpn
                    }
                    disabled={
                      tunnelState === "connecting" ||
                      tunnelState === "disconnecting" ||
                      !selectedProfile
                    }
                    className={`mt-3 w-full py-3 rounded-lg font-medium text-sm transition-all flex items-center justify-center gap-2 ${
                      tunnelState === "connected" || tunnelState === "error"
                        ? "bg-red-600 hover:bg-red-700 text-white"
                        : "bg-bullet-600 hover:bg-bullet-700 text-white"
                    } disabled:opacity-50 disabled:cursor-not-allowed`}
                  >
                    {tunnelState === "connecting" ||
                    tunnelState === "disconnecting" ? (
                      <RefreshCw className="w-4 h-4 animate-spin" />
                    ) : (
                      <ShieldIcon className="w-4 h-4" />
                    )}
                    {tunnelState === "connecting"
                      ? "Conectando..."
                      : tunnelState === "disconnecting"
                        ? "Desconectando..."
                        : tunnelState === "connected"
                          ? "Desconectar"
                          : tunnelState === "error"
                            ? "Desconectar (limpar erro)"
                            : "Conectar"}
                  </button>
                )}
              </Card>
            )}

            {/* Security toggles */}
            {(tunnelState === "connected" ||
              killswitchOn ||
              dnsProtOn) && (
              <Card>
                <h2 className="text-xs font-medium text-neutral-500 uppercase tracking-wider mb-3">
                  Seguranca
                </h2>
                <div className="space-y-2">
                  <Toggle
                    icon={<Lock className="w-3.5 h-3.5" />}
                    label="Kill Switch"
                    sublabel="Bloqueia trafego fora do tunnel"
                    active={killswitchOn}
                    onClick={toggleKillswitch}
                  />
                  <div className="flex items-center gap-2">
                    <div className="flex-1">
                      <Toggle
                        icon={<Globe className="w-3.5 h-3.5" />}
                        label="DNS Protection"
                        sublabel={`Forca DNS para ${dnsServer}`}
                        active={dnsProtOn}
                        onClick={toggleDnsProtection}
                      />
                    </div>
                    {!dnsProtOn && (
                      <input
                        type="text"
                        value={dnsServer}
                        onChange={(e) => setDnsServer(e.target.value)}
                        className="w-28 bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-xs text-neutral-300 focus:outline-none focus:ring-1 focus:ring-bullet-500"
                        placeholder="1.1.1.1"
                      />
                    )}
                  </div>
                </div>
              </Card>
            )}

            {/* Status panel */}
            {(tunnelState === "connected" || tunnelState === "disconnecting") &&
              wgStatus && (
                <Card>
                  <div className="grid grid-cols-2 gap-3">
                    <Stat
                      label="Tipo"
                      value={
                        wgStatus.vpn_type === "openvpn"
                          ? "OpenVPN"
                          : "WireGuard"
                      }
                    />
                    <Stat label="Interface" value={wgStatus.interface_name} />
                    <Stat label="RX" value={wgStatus.transfer_rx} />
                    <Stat label="TX" value={wgStatus.transfer_tx} />
                    {wgStatus.endpoint && (
                      <Stat label="Endpoint" value={wgStatus.endpoint} />
                    )}
                    {selectedProf && (
                      <Stat
                        label="Perfil"
                        value={selectedProf.name}
                      />
                    )}
                  </div>
                </Card>
              )}

            {/* Logs */}
            <LogPanel logs={logs} onClear={() => setLogs([])} />
          </>
        )}

        {/* ────── SSH VIEW ────── */}
        {view === "ssh" && (
          <>
            {showSshForm ? (
              <Card>
                <CardHeader
                  title="Novo SSH Tunnel"
                  onClose={() => setShowSshForm(false)}
                />
                <div className="space-y-2">
                  <Field
                    label="Nome *"
                    placeholder="DBeaver PostgreSQL"
                    value={sshForm.name}
                    onChange={(v) => setSshForm({ ...sshForm, name: v })}
                  />
                  <div>
                    <label className="block text-xs text-neutral-500 mb-0.5">
                      Tipo
                    </label>
                    <select
                      value={sshForm.tunnel_type}
                      onChange={(e) =>
                        setSshForm({
                          ...sshForm,
                          tunnel_type: e.target.value as SshTunnel["tunnel_type"],
                        })
                      }
                      className="w-full bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-neutral-200 focus:outline-none focus:ring-1 focus:ring-bullet-500"
                    >
                      <option value="local">Local (-L)</option>
                      <option value="remote">Remote (-R)</option>
                      <option value="dynamic">Dynamic SOCKS (-D)</option>
                    </select>
                  </div>
                  <Field
                    label="Porta Local *"
                    placeholder="15432"
                    value={sshForm.local_port ? String(sshForm.local_port) : ""}
                    onChange={(v) =>
                      setSshForm({ ...sshForm, local_port: Number(v) || 0 })
                    }
                  />
                  {sshForm.tunnel_type !== "dynamic" && (
                    <>
                      <Field
                        label="Remote Host"
                        placeholder="127.0.0.1"
                        value={sshForm.remote_host}
                        onChange={(v) =>
                          setSshForm({ ...sshForm, remote_host: v })
                        }
                      />
                      <Field
                        label="Remote Port"
                        placeholder="5432"
                        value={
                          sshForm.remote_port
                            ? String(sshForm.remote_port)
                            : ""
                        }
                        onChange={(v) =>
                          setSshForm({
                            ...sshForm,
                            remote_port: Number(v) || 0,
                          })
                        }
                      />
                    </>
                  )}
                  <Field
                    label="SSH Host *"
                    placeholder="my-vps.com"
                    value={sshForm.ssh_host}
                    onChange={(v) => setSshForm({ ...sshForm, ssh_host: v })}
                  />
                  <Field
                    label="SSH User"
                    placeholder="root"
                    value={sshForm.ssh_user}
                    onChange={(v) => setSshForm({ ...sshForm, ssh_user: v })}
                  />
                  <Field
                    label="SSH Key"
                    placeholder="~/.ssh/id_ed25519"
                    value={sshForm.ssh_key}
                    onChange={(v) => setSshForm({ ...sshForm, ssh_key: v })}
                  />
                </div>
                <GreenBtn onClick={addSshTunnel}>Salvar Tunnel</GreenBtn>
              </Card>
            ) : (
              <Card>
                <div className="flex items-center justify-between mb-3">
                  <h2 className="text-sm font-medium text-neutral-300">
                    SSH Tunnels
                  </h2>
                  <IconBtn
                    onClick={() => setShowSshForm(true)}
                    title="Novo tunnel"
                  >
                    <Plus className="w-4 h-4" />
                  </IconBtn>
                </div>

                {sshTunnels.length === 0 ? (
                  <button
                    onClick={() => setShowSshForm(true)}
                    className="w-full py-6 border-2 border-dashed border-neutral-700 rounded-lg text-neutral-500 hover:text-neutral-300 hover:border-neutral-500 transition-colors text-sm"
                  >
                    Adicione um SSH tunnel
                  </button>
                ) : (
                  <div className="space-y-2">
                    {sshTunnels.map((t) => {
                      const running = sshRunning[t.id] || false;
                      const typeLabel =
                        t.tunnel_type === "dynamic"
                          ? "SOCKS"
                          : t.tunnel_type === "remote"
                            ? "R"
                            : "L";
                      const portInfo =
                        t.tunnel_type === "dynamic"
                          ? `:${t.local_port}`
                          : `:${t.local_port} -> ${t.remote_host}:${t.remote_port}`;

                      return (
                        <div
                          key={t.id}
                          className="flex items-center gap-2 bg-neutral-800 rounded-lg px-3 py-2"
                        >
                          <div
                            className={`w-2 h-2 rounded-full ${running ? "bg-bullet-500" : "bg-neutral-600"}`}
                          />
                          <div className="flex-1 min-w-0">
                            <p className="text-sm text-neutral-200 truncate">
                              [{typeLabel}] {t.name}
                            </p>
                            <p className="text-[10px] text-neutral-500 font-mono truncate">
                              {t.ssh_user}@{t.ssh_host} {portInfo}
                            </p>
                          </div>
                          <button
                            onClick={() =>
                              running
                                ? stopSshTunnel(t.id)
                                : startSshTunnel(t.id)
                            }
                            className={`p-1.5 rounded transition-colors ${
                              running
                                ? "text-red-400 hover:bg-red-950"
                                : "text-bullet-400 hover:bg-bullet-950"
                            }`}
                            title={running ? "Parar" : "Iniciar"}
                          >
                            {running ? (
                              <Square className="w-3.5 h-3.5" />
                            ) : (
                              <Play className="w-3.5 h-3.5" />
                            )}
                          </button>
                          <button
                            onClick={() => removeSshTunnel(t.id)}
                            disabled={running}
                            className="p-1.5 rounded text-neutral-500 hover:text-red-400 hover:bg-red-950/50 transition-colors disabled:opacity-30"
                            title="Remover"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      );
                    })}
                  </div>
                )}
              </Card>
            )}

            <LogPanel logs={logs} onClear={() => setLogs([])} />
          </>
        )}

        {/* ────── SETTINGS VIEW ────── */}
        {view === "settings" && (
          <>
            {/* Permissions */}
            <Card>
              <h2 className="text-sm font-medium text-neutral-300 mb-3">
                Permissoes de Sistema
              </h2>
              <p className="text-xs text-neutral-500 mb-3">
                Instala regra sudoers para wg-quick, openvpn e iptables.
                Elimina prompts de senha em todas as operacoes.
              </p>
              <div className="flex items-center gap-2">
                <div
                  className={`w-2 h-2 rounded-full ${permInstalled ? "bg-bullet-500" : "bg-neutral-600"}`}
                />
                <span className="text-xs text-neutral-400 flex-1">
                  {permInstalled ? "Instalado" : "Nao instalado"}
                </span>
                {permInstalled ? (
                  <button
                    onClick={uninstallPerms}
                    className="px-3 py-1.5 bg-red-600/20 text-red-400 text-xs rounded hover:bg-red-600/30 transition-colors"
                  >
                    Remover
                  </button>
                ) : (
                  <button
                    onClick={installPerms}
                    className="px-3 py-1.5 bg-bullet-600 text-white text-xs rounded hover:bg-bullet-700 transition-colors"
                  >
                    Instalar
                  </button>
                )}
              </div>
            </Card>

            {/* DNS config */}
            <Card>
              <h2 className="text-sm font-medium text-neutral-300 mb-3">
                DNS Padrao
              </h2>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={dnsServer}
                  onChange={(e) => setDnsServer(e.target.value)}
                  className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-neutral-200 focus:outline-none focus:ring-1 focus:ring-bullet-500"
                  placeholder="1.1.1.1"
                />
                <select
                  onChange={(e) => setDnsServer(e.target.value)}
                  className="bg-neutral-800 border border-neutral-700 rounded px-2 py-1.5 text-xs text-neutral-400 focus:outline-none"
                >
                  <option value="1.1.1.1">Cloudflare</option>
                  <option value="9.9.9.9">Quad9</option>
                  <option value="8.8.8.8">Google</option>
                  <option value="208.67.222.222">OpenDNS</option>
                </select>
              </div>
            </Card>

            {/* App info */}
            <Card>
              <h2 className="text-sm font-medium text-neutral-300 mb-3">
                Sobre
              </h2>
              <div className="space-y-1 text-xs text-neutral-500">
                <p>Bullet VPN v2.1.0</p>
                <p>Tauri 2 + React 19 + Rust</p>
                <p>WireGuard + OpenVPN</p>
                <p className="pt-1 text-neutral-700">effront</p>
              </div>
            </Card>

            <LogPanel logs={logs} onClear={() => setLogs([])} />
          </>
        )}
      </main>

      {/* Footer */}
      <footer className="flex items-center justify-center px-4 py-2 bg-neutral-900 border-t border-neutral-800">
        <span className="text-[10px] text-neutral-700">
          Bullet VPN v2.1.0 -- effront
        </span>
      </footer>
    </div>
  );
}

// ── Shared components ─────────────────────────────────────────────

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-neutral-900 rounded-lg border border-neutral-800 p-4">
      {children}
    </div>
  );
}

function CardHeader({
  title,
  onClose,
}: {
  title: string;
  onClose: () => void;
}) {
  return (
    <div className="flex items-center justify-between mb-3">
      <h2 className="text-sm font-medium text-neutral-300">{title}</h2>
      <button
        onClick={onClose}
        className="p-1 rounded hover:bg-neutral-800 text-neutral-400"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}

function Field({
  label,
  placeholder,
  value,
  onChange,
  type = "text",
}: {
  label: string;
  placeholder: string;
  value: string;
  onChange: (v: string) => void;
  type?: string;
}) {
  return (
    <div>
      <label className="block text-xs text-neutral-500 mb-0.5">{label}</label>
      <input
        type={type}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-neutral-200 placeholder-neutral-600 focus:outline-none focus:ring-1 focus:ring-bullet-500"
      />
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-[10px] text-neutral-500 uppercase tracking-wider">
        {label}
      </p>
      <p className="text-sm text-neutral-200 font-mono truncate">{value}</p>
    </div>
  );
}

function GreenBtn({
  onClick,
  children,
}: {
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className="mt-3 w-full py-2 bg-bullet-600 hover:bg-bullet-700 text-white text-sm font-medium rounded-lg transition-colors"
    >
      {children}
    </button>
  );
}

function IconBtn({
  onClick,
  disabled,
  title,
  className = "",
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  title: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`p-1 rounded hover:bg-neutral-800 text-neutral-400 hover:text-bullet-400 transition-colors disabled:opacity-30 ${className}`}
      title={title}
    >
      {children}
    </button>
  );
}

function NavBtn({
  active,
  onClick,
  title,
  children,
}: {
  active: boolean;
  onClick: () => void;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`p-1.5 rounded transition-colors ${
        active
          ? "bg-neutral-800 text-bullet-400"
          : "text-neutral-500 hover:text-neutral-300 hover:bg-neutral-800/50"
      }`}
      title={title}
    >
      {children}
    </button>
  );
}

function Toggle({
  icon,
  label,
  sublabel,
  active,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  sublabel: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-3 bg-neutral-800 rounded-lg px-3 py-2 hover:bg-neutral-750 transition-colors text-left"
    >
      <div
        className={`${active ? "text-bullet-400" : "text-neutral-500"}`}
      >
        {icon}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-neutral-200">{label}</p>
        <p className="text-[10px] text-neutral-500 truncate">{sublabel}</p>
      </div>
      <div
        className={`w-8 h-4 rounded-full relative transition-colors ${
          active ? "bg-bullet-600" : "bg-neutral-700"
        }`}
      >
        <div
          className={`absolute top-0.5 w-3 h-3 rounded-full bg-white transition-transform ${
            active ? "translate-x-4" : "translate-x-0.5"
          }`}
        />
      </div>
    </button>
  );
}

function LogPanel({
  logs,
  onClear,
}: {
  logs: string[];
  onClear: () => void;
}) {
  return (
    <div className="flex-1 bg-neutral-900 rounded-lg border border-neutral-800 overflow-hidden flex flex-col min-h-[100px]">
      <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800">
        <span className="text-xs font-medium text-neutral-400">Logs</span>
        <button
          onClick={onClear}
          className="text-xs text-neutral-500 hover:text-neutral-300"
        >
          Limpar
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-3 font-mono text-[11px] leading-relaxed text-neutral-500 space-y-0.5">
        {logs.length === 0 ? (
          <p className="text-neutral-700">Nenhum log.</p>
        ) : (
          logs.map((log, i) => <div key={i}>{log}</div>)
        )}
      </div>
    </div>
  );
}

export default App;
