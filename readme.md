# 🔒 Bullet VPN 

Uma aplicação VPN simples e segura usando WireGuard e Java com BouncyCastle.
## 📋 Características

- 🔐 **Segurança**: Chaves Curve25519 com BouncyCastle
- ⚡ **Performance**: WireGuard nativo para máxima velocidade
- 🎯 **Simplicidade**: Interface Swing intuitiva
- 🚀 **Zero Logs**: Nenhum dado é armazenado remotamente
- 💻 **Multiplataforma**: Windows, Linux e macOS
- 🛡️ **Privacidade**: Controle total sobre seus dados


## 📦 Pré-requisitos

### Windows
- Java 17+ (OpenJDK recomendado)
- WireGuard for Windows
- Maven 3.6+

### Linux
- Java 17+ (`sudo apt install openjdk-17-jdk`)
- WireGuard (`sudo apt install wireguard`)
- Maven (`sudo apt install maven`)

## 🚀 Instalação Rápida

### Windows
```bash
# Clone o projeto
git clone https://github.com/bulletdev/bullet-vpn.git
cd bullet-vpn

# Execute o instalador
install-windows.bat
```

### Linux
```bash
# Clone o projeto
git clone https://github.com/bulletdev/bullet-vpn.git
cd bullet-vpn

# Execute o instalador
chmod +x install-linux.sh
./install-linux.sh
```

## 🔧 Build Manual

### 1. Compilar o projeto
```bash
mvn clean compile
```

### 2. Criar JAR executável
```bash
mvn package
```

### 3. Gerar executável nativo

#### Windows (.exe)
```bash
# Usando jpackage (Java 17+)
jpackage --input target/ --name BulletVPN --main-jar bullet-vpn-1.0.jar --main-class BulletVPN --type exe --icon src/main/resources/bullet-vpn.ico

# Ou usando launch4j
mvn launch4j:launch4j
```

#### Linux (AppImage/DEB)
```bash
# AppImage
jpackage --input target/ --name BulletVPN --main-jar bullet-vpn-1.0.jar --main-class BulletVPN --type app-image

# Pacote DEB
jpackage --input target/ --name BulletVPN --main-jar bullet-vpn-1.0.jar --main-class BulletVPN --type deb
```

## 🎮 Como Usar

1. **Primeira Execução**
    - Execute `BulletVPN.exe` (Windows) ou `./BulletVPN` (Linux)
    - Clique em "🔄 Gerar Novas Chaves"

2. **Configurar Servidor**
    - Insira o endpoint do seu servidor VPN
    - Cole a chave pública do servidor
    - Configure IPs permitidos (padrão: todo tráfego)

3. **Conectar**
    - Clique em "🚀 Conectar VPN"
    - Aguarde a conexão ser estabelecida
    - Status ficará verde quando conectado

4. **Desconectar**
    - Clique em "🛑 Desconectar"
    - VPN será desconectada safely

## 🔐 Configuração do Servidor

Para usar a Bullet VPN, você precisa de um servidor WireGuard. Exemplo de configuração:

### Servidor (Linux)
```bash
# Instalar WireGuard
sudo apt update && sudo apt install wireguard

# Gerar chaves do servidor
wg genkey | tee server_private.key | wg pubkey > server_public.key

# Configurar interface
sudo nano /etc/wireguard/wg0.conf
```

### Arquivo de configuração do servidor:
```ini
[Interface]
PrivateKey = CHAVE_PRIVADA_SERVIDOR
Address = 10.0.0.1/24
ListenPort = 51820
PostUp = iptables -A FORWARD -i wg0 -j ACCEPT; iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE
PostDown = iptables -D FORWARD -i wg0 -j ACCEPT; iptables -t nat -D POSTROUTING -o eth0 -j MASQUERADE

[Peer]
PublicKey = SUA_CHAVE_PUBLICA_CLIENTE
AllowedIPs = 10.0.0.2/32
```

### Iniciar servidor:
```bash
sudo wg-quick up wg0
sudo systemctl enable wg-quick@wg0
```

## 📁 Arquivos e Diretórios

- **Configurações**: `~/.bullet-vpn/`
- **Chaves**: `~/.bullet-vpn/keys.txt`
- **Config WireGuard**: `~/.bullet-vpn/bullet.conf`
- **Logs**: Exibidos na interface

## 🔒 Segurança

- **Chaves**: Geradas usando Curve25519 (BouncyCastle)
- **Protocolo**: WireGuard (auditado e seguro)
- **DNS**: Cloudflare (1.1.1.1) por padrão
- **Logs**: Apenas locais, nada enviado para servidores

## 🐛 Troubleshooting

### Erro "WireGuard não encontrado"
**Windows**: Instale WireGuard do site oficial
**Linux**: `sudo apt install wireguard`

### Erro de permissão
**Windows**: Execute como Administrador
**Linux**: Use `sudo` ou configure capabilities

### Conexão falhando
- Verifique se o endpoint está correto
- Confirme que a chave pública do servidor está certa
- Teste conectividade: `ping endpoint-servidor`

## 🚀 Melhorias Futuras

- [ ] Auto-update de servidores
- [ ] Múltiplos perfis de conexão
- [ ] Kill switch automático
- [ ] Métricas de conexão
- [ ] Tema dark/light
- [ ] Notificações do sistema

## 📝 Licença

MIT License - Use como quiser, sem garantias.

## 🤝 Contribuindo

1. Fork o projeto
2. Crie sua feature branch
3. Commit suas mudanças
4. Push para a branch
5. Abra um Pull Request

## 📞 Suporte

Para suporte ou dúvidas:
- Abra uma issue no GitHub
- Documentação: Wiki do projeto

---

**⚠️ Aviso**: Esta é uma ferramenta para uso pessoal. Use responsavelmente e respeite as leis locais sobre VPNs.