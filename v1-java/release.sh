#!/bin/bash

VERSION="1.0.0"
PROJECT_NAME="bullet-vpn"

echo "🚀 Bullet VPN Release Builder v$VERSION"
echo "========================================"

# Cores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Função para log colorido
log() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Limpar builds anteriores
log "🧹 Limpando builds anteriores..."
rm -rf target/ releases/
mkdir -p releases

# Build básico
log "📦 Compilando projeto..."
mvn clean package -q

if [ $? -ne 0 ]; then
    error "Falha na compilação"
    exit 1
fi

# Copiar JAR para releases
cp target/${PROJECT_NAME}-${VERSION}.jar releases/

# Gerar executáveis por plataforma
case "$OSTYPE" in
    linux-gnu*)
        log "🐧 Gerando executáveis Linux..."

        # AppImage
        log "Criando AppImage..."
        mvn jpackage:jpackage -Plinux -q
        if [ -d target/dist ]; then
            cp -r target/dist/* releases/
        fi

        # Script de instalação
        log "Criando script de instalação Linux..."
        cat > releases/install-linux.sh << 'EOF'
#!/bin/bash
echo "🔒 Instalando Bullet VPN no Linux..."

# Instalar WireGuard
if ! command -v wg &> /dev/null; then
    if command -v apt &> /dev/null; then
        sudo apt update && sudo apt install -y wireguard
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y wireguard-tools
    else
        echo "Por favor, instale o WireGuard manualmente"
        exit 1
    fi
fi

# Instalar aplicação
if [ -f "bulletvpn_1.0.0_amd64.deb" ]; then
    sudo dpkg -i bulletvpn_1.0.0_amd64.deb
    echo "✅ Bullet VPN instalado via pacote DEB"
else
    echo "✅ Execute: java -jar bullet-vpn-1.0.0.jar"
fi
EOF
        chmod +x releases/install-linux.sh
        ;;

    msys*|cygwin*|mingw*)
        log "🪟 Gerando executável Windows..."

        # EXE via Launch4j
        mvn launch4j:launch4j -q
        if [ -f target/BulletVPN.exe ]; then
            cp target/BulletVPN.exe releases/
        fi

        # Installer via jpackage
        mvn jpackage:jpackage -Pwindows -q
        if [ -d target/dist ]; then
            cp -r target/dist/* releases/
        fi

        # Script de instalação Windows
        log "Criando script de instalação Windows..."
        cat > releases/install-windows.bat << 'EOF'
@echo off
echo 🔒 Instalando Bullet VPN...

REM Verificar WireGuard
where wg >nul 2>&1
if %errorlevel% neq 0 (
    echo Baixando WireGuard...
    powershell -Command "Invoke-WebRequest -Uri 'https://download.wireguard.com/windows-client/wireguard-installer.exe' -OutFile 'wireguard-installer.exe'"
    start wireguard-installer.exe
    echo Instale o WireGuard e execute este script novamente
    pause
    exit
)

REM Instalar aplicação
if exist "BulletVPN.exe" (
    copy BulletVPN.exe "%PROGRAMFILES%\BulletVPN\"
    echo Aplicação instalada em: %PROGRAMFILES%\BulletVPN\
) else (
    echo Execute: java -jar bullet-vpn-1.0.0.jar
)

echo ✅ Instalação concluída!
pause
EOF
        ;;

    darwin*)
        log "🍎 Gerando aplicação macOS..."
        mvn jpackage:jpackage -q
        if [ -d target/dist ]; then
            cp -r target/dist/* releases/
        fi

        log "Criando script de instalação macOS..."
        cat > releases/install-macos.sh << 'EOF'
#!/bin/bash
echo "🔒 Instalando Bullet VPN no macOS..."

# Instalar WireGuard via Homebrew
if ! command -v wg &> /dev/null; then
    if command -v brew &> /dev/null; then
        brew install wireguard-tools
    else
        echo "Instale Homebrew primeiro: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
        exit 1
    fi
fi

# Mover aplicação
if [ -d "BulletVPN.app" ]; then
    cp -r BulletVPN.app /Applications/
    echo "✅ Bullet VPN instalado em /Applications/"
else
    echo "✅ Execute: java -jar bullet-vpn-1.0.0.jar"
fi
EOF
        chmod +x releases/install-macos.sh
        ;;
esac

# Criar documentação
log "📝 Gerando documentação..."
cat > releases/QUICK_START.md << 'EOF'
# 🚀 Bullet VPN - Início Rápido

## Windows
1. Execute `install-windows.bat`
2. Ou execute `BulletVPN.exe` diretamente
3. Ou execute `java -jar bullet-vpn-1.0.0.jar`

## Linux
1. Execute `chmod +x install-linux.sh && ./install-linux.sh`
2. Ou instale o pacote DEB: `sudo dpkg -i bulletvpn_1.0.0_amd64.deb`
3. Ou execute `java -jar bullet-vpn-1.0.0.jar`

## macOS
1. Execute `chmod +x install-macos.sh && ./install-macos.sh`
2. Ou arraste `BulletVPN.app` para `/Applications/`
3. Ou execute `java -jar bullet-vpn-1.0.0.jar`

## Primeiros Passos
1. 🔑 Clique em "Gerar Novas Chaves"
2. 🌐 Configure o endpoint do servidor
3. 📋 Cole a chave pública do servidor
4. 🚀 Clique em "Conectar VPN"

## Requisitos
- Java 17+ (apenas para JAR)
- WireGuard instalado no sistema
- Servidor VPN configurado

Para mais detalhes, veja README.md
EOF

# Criar checksums
log "🔐 Gerando checksums..."
cd releases
for file in *; do
    if [ -f "$file" ]; then
        sha256sum "$file" >> checksums.sha256
    fi
done
cd ..

# Criar arquivo de release
log "📦 Criando pacote de release..."
tar -czf releases/bullet-vpn-${VERSION}-release.tar.gz -C releases .

# Relatório final
log "✅ Release criado com sucesso!"
echo ""
echo "📁 Arquivos gerados em releases/:"
ls -la releases/
echo ""
echo "🎯 Para distribuir:"
echo "   - JAR universal: bullet-vpn-${VERSION}.jar"
echo "   - Executáveis nativos por plataforma"
echo "   - Scripts de instalação automática"
echo "   - Documentação e checksums"
echo ""
warn "⚠️  Teste todos os executáveis antes de distribuir!"

---

# release.bat - Script de release para Windows
@echo off
setlocal enabledelayedexpansion

set VERSION=1.0.0
set PROJECT_NAME=bullet-vpn

echo 🚀 Bullet VPN Release Builder v%VERSION%
echo ========================================

echo 🧹 Limpando builds anteriores...
rmdir /s /q target 2>nul
rmdir /s /q releases 2>nul
mkdir releases

echo 📦 Compilando projeto...
call mvn clean package -q

if %errorlevel% neq 0 (
    echo ❌ Falha na compilação
    exit /b 1
)

echo 📁 Copiando JAR para releases...
copy target\%PROJECT_NAME%-%VERSION%.jar releases\

echo 🪟 Gerando executável Windows...
call mvn launch4j:launch4j -q

if exist target\BulletVPN.exe (
    copy target\BulletVPN.exe releases\
    echo ✅ EXE criado
) else (
    echo ⚠️ EXE não foi criado, tentando jpackage...
    call mvn jpackage:jpackage -Pwindows -q
    if exist target\dist (
        xcopy target\dist releases\ /E /I
    )
)

echo 📝 Criando documentação...
echo # 🚀 Bullet VPN - Windows Release > releases\WINDOWS_README.txt
echo. >> releases\WINDOWS_README.txt
echo ## Instalação Rápida: >> releases\WINDOWS_README.txt
echo 1. Execute install-windows.bat >> releases\WINDOWS_README.txt
echo 2. Ou execute BulletVPN.exe diretamente >> releases\WINDOWS_README.txt
echo 3. Ou execute: java -jar bullet-vpn-1.0.0.jar >> releases\WINDOWS_README.txt
echo. >> releases\WINDOWS_README.txt
echo ## Requisitos: >> releases\WINDOWS_README.txt
echo - WireGuard para Windows >> releases\WINDOWS_README.txt
echo - Java 17+ (apenas para JAR) >> releases\WINDOWS_README.txt

echo 🔐 Gerando checksums...
cd releases
for %%f in (*) do (
    if exist "%%f" (
        certutil -hashfile "%%f" SHA256 >> checksums.txt
    )
)
cd ..

echo ✅ Release Windows criado com sucesso!
echo.
echo 📁 Arquivos em releases\:
dir releases\ /b
echo.
echo 🎯 Pronto para distribuição!
pause

---

# Dockerfile - Para builds em containers
FROM openjdk:17-jdk-slim

# Instalar dependências
RUN apt-get update && apt-get install -y \
    maven \
    git \
    && rm -rf /var/lib/apt/lists/*

# Diretório de trabalho
WORKDIR /app

# Copiar arquivos do projeto
COPY pom.xml .
COPY src/ src/

# Build
RUN mvn clean package

# Comando padrão
CMD ["java", "-jar", "target/bullet-vpn-1.0.0.jar"]

# Metadata
LABEL org.opencontainers.image.title="Bullet VPN"
LABEL org.opencontainers.image.description="Secure Personal VPN Client"
LABEL org.opencontainers.image.version="1.0.0"

---
EOF