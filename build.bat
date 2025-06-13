@echo off
setlocal

echo 🔒 Bullet VPN - Build Script
echo ==============================

java -version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Java não encontrado. Instale Java 17+ primeiro.
    echo Download: https://adoptium.net/
    pause
    exit /b 1
)

mvn -version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Maven não encontrado. Instale Maven primeiro.
    echo Download: https://maven.apache.org/download.cgi
    pause
    exit /b 1
)

echo 📦 Compilando projeto...
call mvn clean compile

if %errorlevel% neq 0 (
    echo ❌ Erro na compilação
    pause
    exit /b 1
)

echo 📦 Criando JAR executável...
call mvn package

if %errorlevel% neq 0 (
    echo ❌ Erro ao criar JAR
    pause
    exit /b 1
)

echo 📦 Gerando executável Windows (.exe)...
call mvn launch4j:launch4j

if %errorlevel% equ 0 (
    echo ✅ Executável Windows gerado: target\BulletVPN.exe
) else (
    echo ⚠️ Erro no Launch4j, tentando jpackage...
    call mvn jpackage:jpackage -Pwindows

    if %errorlevel% equ 0 (
        echo ✅ Executável Windows gerado em: target\dist\
    )
)

echo.
echo 🚀 Build concluído!
echo 📄 JAR executável: target\bullet-vpn-1.0.0.jar
echo 💻 EXE Windows: target\BulletVPN.exe
echo 🎯 Para executar: java -jar target\bullet-vpn-1.0.0.jar
echo.
pause

---
