#!/bin/bash

echo "🔒 Bullet VPN - Build Script"
echo "=============================="

if ! command -v java &> /dev/null; then
    echo "❌ Java não encontrado. Instale Java 17+ primeiro."
    echo "Ubuntu/Debian: sudo apt install openjdk-17-jdk"
    exit 1
fi

if ! command -v mvn &> /dev/null; then
    echo "❌ Maven não encontrado. Instale Maven primeiro."
    echo "Ubuntu/Debian: sudo apt install maven"
    exit 1
fi

echo "📦 Compilando projeto..."
mvn clean compile

if [ $? -ne 0 ]; then
    echo "❌ Erro na compilação"
    exit 1
fi

echo "📦 Criando JAR executável..."
mvn package

if [ $? -ne 0 ]; then
    echo "❌ Erro ao criar JAR"
    exit 1
fi

echo "📦 Gerando executável nativo..."

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "🐧 Gerando pacote DEB para Linux..."
    mvn jpackage:jpackage -Plinux

    if [ $? -eq 0 ]; then
        echo "✅ Executável Linux gerado em: target/dist/"
        echo "📦 Instalar com: sudo dpkg -i target/dist/bulletvpn_1.0.0_amd64.deb"
    fi

elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo "🍎 Gerando aplicação macOS..."
    mvn jpackage:jpackage

    if [ $? -eq 0 ]; then
        echo "✅ Aplicação macOS gerada em: target/dist/"
    fi
fi

echo ""
echo "🚀 Build concluído!"
echo "📄 JAR executável: target/bullet-vpn-1.0.0.jar"
echo "🎯 Para executar: java -jar target/bullet-vpn-1.0.0.jar"
echo ""

---