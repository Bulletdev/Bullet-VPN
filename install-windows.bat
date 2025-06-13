@echo off
setlocal

echo 🔒 Bullet VPN - Instalador Windows
echo ===================================

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo ⚠️ Execute como Administrador para melhor experiência
    timeout /t 3 >nul
)

echo 🔍 Verificando dependências...

java -version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Java não encontrado
    echo 📥 Baixando Java 17...
    
    powershell -Command "& {Invoke-WebRequest -Uri 'https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.9+9/OpenJDK17U-jdk_x64_windows_hotspot_17.0.9_9.msi' -OutFile 'java17.msi'}"
    
    echo 🚀 Instale o Java e execute este script novamente
    start java17.msi
    pause
    exit /b 1
)

where wg >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ WireGuard não encontrado
    echo 📥 Baixe em: https://www.wireguard.com/install/
    
    REM Tentar baixar WireGuard
    powershell -Command "& {Invoke-WebRequest -Uri 'https://download.wireguard.com/windows-client/wireguard-installer.exe' -OutFile 'wireguard-installer.exe'}"
    
    echo 🚀 Instale o WireGuard e execute este script novamente
    start wireguard-installer.exe
    pause
    exit /b 1
)

mvn -version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Maven não encontrado
    echo 📥 Baixe em: https://maven.apache.org/download.cgi
    echo 💡 Ou use o JAR pré-compilado se disponível
    pause
    exit /b 1
)

echo ✅ Dependências OK!

echo 🔨 Compilando Bullet VPN...
call build.bat

if %errorlevel% equ 0 (
    echo.
    echo 🎉 Instalação concluída!
    
    if exist "target\BulletVPN.exe" (
        echo 🚀 Execute: target\BulletVPN.exe
        
        echo 🔗 Criando atalho na área de trabalho...
        powershell -Command "& {$WshShell = New-Object -comObject WScript.Shell; $Shortcut = $WshShell.CreateShortcut('%USERPROFILE%\Desktop\Bullet VPN.lnk'); $Shortcut.TargetPath = '%CD%\target\BulletVPN.exe'; $Shortcut.Save()}"
        
        echo ✅ Atalho criado na área de trabalho!
    ) else (
        echo 🚀 Execute: java -jar target\bullet-vpn-1.0.0.jar
    )
) else (
    echo ❌ Erro na instalação
    pause
    exit /b 1
)

echo.
echo 📖 Leia o README.md para instruções de configuração
pause