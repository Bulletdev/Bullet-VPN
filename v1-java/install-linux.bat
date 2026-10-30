#!/bin/bash

echo "🔒 Bullet VPN - Instalador Linux"
echo "================================="

detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo $ID
    else
        echo "unknown"
    fi
}

DISTRO=$(detect_distro)

echo "📋 Detectado: $DISTRO"

echo "🔧 Instalando dependências..."

case $DISTRO in
    ubuntu|debian)
        sudo apt update
        sudo apt install -y openjdk-17-jdk maven wireguard
        ;;
    fedora)
        sudo dnf install -y java-17-openjdk-devel maven wireguard-tools
        ;;
    centos|rhel)
        sudo yum install -y java-17-openjdk-devel maven
        # WireGuard via EPEL
        sudo yum install -y epel-release
        sudo yum install -y wireguard-tools
        ;;
    arch)
        sudo pacman -S --noconfirm jdk17-openjdk maven wireguard-tools
        ;;
    *)
        echo "⚠️ Distribuição não suportada automaticamente."
        echo "Instale manualmente: Java 17+, Maven, WireGuard"
        ;;
esac

if ! command -v java &> /dev/null; then
    echo "❌ Java não foi instalado corretamente"
    exit 1
fi

if ! command -v wg &> /dev/null; then
    echo "❌ WireGuard não foi instalado corretamente"
    exit 1
fi

echo "✅ Dependências instaladas!"

echo "🔨 Compilando Bullet VPN..."
chmod +x build.sh
./build.sh

if [ $? -eq 0 ]; then
    echo ""
    echo "🎉 Instalação concluída!"
    echo "🚀 Execute: java -jar target/bullet-vpn-1.0.0.jar"
    echo "📁 Ou instale o pacote DEB se foi gerado"

    if [[ -f target/bullet-vpn-1.0.0.jar ]]; then
        echo "🔗 Criando comando 'bullet-vpn'..."

        cat > bullet-vpn << EOF
#!/bin/bash
cd "$(dirname "$0")"
java -jar target/bullet-vpn-1.0.0.jar "\$@"
EOF
        chmod +x bullet-vpn

        echo "✅ Execute com: ./bullet-vpn"
    fi
else
    echo "❌ Erro na instalação"
    exit 1
fi

