import javax.swing.*;
import javax.swing.border.TitledBorder;
import java.awt.*;
import java.awt.event.ActionEvent;
import java.io.*;
import java.nio.file.*;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.concurrent.CompletableFuture;
import org.bouncycastle.crypto.generators.X25519KeyPairGenerator;
import org.bouncycastle.crypto.params.X25519KeyGenerationParameters;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.X25519PublicKeyParameters;

public class BulletVPN extends JFrame {
    private static final String CONFIG_DIR = System.getProperty("user.home") + "/.bullet-vpn";
    private static final String WG_CONFIG_FILE = CONFIG_DIR + "/bullet.conf";

    private JButton connectButton;
    private JButton disconnectButton;
    private JButton generateKeysButton;
    private JTextArea logArea;
    private JTextField serverEndpointField;
    private JTextField serverPublicKeyField;
    private JTextField allowedIPsField;
    private JLabel statusLabel;
    private JLabel privateKeyLabel;
    private JLabel publicKeyLabel;

    private Process wgProcess;
    private String privateKey;
    private String publicKey;
    private boolean isConnected = false;

    public BulletVPN() {
        initializeUI();
        createConfigDirectory();
        loadExistingKeys();
    }

    private void initializeUI() {
        setTitle("Bullet VPN 🔒");
        setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);
        setSize(800, 700);
        setLocationRelativeTo(null);

        try {
            UIManager.setLookAndFeel(UIManager.getSystemLookAndFeelClassName());
        } catch (Exception e) {
            e.printStackTrace();
        }

        setLayout(new BorderLayout());

        JPanel headerPanel = new JPanel(new FlowLayout());
        headerPanel.setBackground(new Color(33, 37, 41));
        JLabel titleLabel = new JLabel("🔒 Bullet VPN");
        titleLabel.setFont(new Font("Arial", Font.BOLD, 24));
        titleLabel.setForeground(Color.WHITE);
        headerPanel.add(titleLabel);

        statusLabel = new JLabel("Desconectado");
        statusLabel.setFont(new Font("Arial", Font.BOLD, 14));
        statusLabel.setForeground(Color.RED);
        headerPanel.add(Box.createHorizontalStrut(20));
        headerPanel.add(statusLabel);

        add(headerPanel, BorderLayout.NORTH);

        JPanel mainPanel = new JPanel(new GridBagLayout());
        GridBagConstraints gbc = new GridBagConstraints();
        gbc.insets = new Insets(10, 10, 10, 10);
        gbc.fill = GridBagConstraints.HORIZONTAL;

        JPanel keysPanel = createKeysPanel();
        gbc.gridx = 0; gbc.gridy = 0; gbc.gridwidth = 2;
        mainPanel.add(keysPanel, gbc);

        JPanel serverPanel = createServerPanel();
        gbc.gridx = 0; gbc.gridy = 1; gbc.gridwidth = 2;
        mainPanel.add(serverPanel, gbc);

        JPanel controlPanel = createControlPanel();
        gbc.gridx = 0; gbc.gridy = 2; gbc.gridwidth = 2;
        mainPanel.add(controlPanel, gbc);

        JPanel logPanel = createLogPanel();
        gbc.gridx = 0; gbc.gridy = 3; gbc.gridwidth = 2;
        gbc.fill = GridBagConstraints.BOTH;
        gbc.weightx = 1.0; gbc.weighty = 1.0;
        mainPanel.add(logPanel, gbc);

        add(mainPanel, BorderLayout.CENTER);
    }

    private JPanel createKeysPanel() {
        JPanel panel = new JPanel(new GridBagLayout());
        panel.setBorder(new TitledBorder("🔑 Chaves Criptográficas"));
        GridBagConstraints gbc = new GridBagConstraints();

        gbc.insets = new Insets(5, 5, 5, 5);
        gbc.anchor = GridBagConstraints.WEST;

        gbc.gridx = 0; gbc.gridy = 0;
        panel.add(new JLabel("Chave Privada:"), gbc);
        privateKeyLabel = new JLabel("Não gerada");
        privateKeyLabel.setFont(new Font("Monospaced", Font.PLAIN, 10));
        gbc.gridx = 1; gbc.fill = GridBagConstraints.HORIZONTAL; gbc.weightx = 1.0;
        panel.add(privateKeyLabel, gbc);

        gbc.gridx = 0; gbc.gridy = 1; gbc.fill = GridBagConstraints.NONE; gbc.weightx = 0;
        panel.add(new JLabel("Chave Pública:"), gbc);
        publicKeyLabel = new JLabel("Não gerada");
        publicKeyLabel.setFont(new Font("Monospaced", Font.PLAIN, 10));
        gbc.gridx = 1; gbc.fill = GridBagConstraints.HORIZONTAL; gbc.weightx = 1.0;
        panel.add(publicKeyLabel, gbc);

        generateKeysButton = new JButton("🔄 Gerar Novas Chaves");
        generateKeysButton.addActionListener(this::generateKeys);
        gbc.gridx = 0; gbc.gridy = 2; gbc.gridwidth = 2;
        gbc.fill = GridBagConstraints.NONE; gbc.anchor = GridBagConstraints.CENTER;
        panel.add(generateKeysButton, gbc);

        return panel;
    }

    private JPanel createServerPanel() {
        JPanel panel = new JPanel(new GridBagLayout());
        panel.setBorder(new TitledBorder("🌐 Configuração do Servidor"));
        GridBagConstraints gbc = new GridBagConstraints();

        gbc.insets = new Insets(5, 5, 5, 5);
        gbc.anchor = GridBagConstraints.WEST;

        gbc.gridx = 0; gbc.gridy = 0;
        panel.add(new JLabel("Endpoint:"), gbc);
        serverEndpointField = new JTextField("your-server.com:51820", 30);
        gbc.gridx = 1; gbc.fill = GridBagConstraints.HORIZONTAL; gbc.weightx = 1.0;
        panel.add(serverEndpointField, gbc);

        gbc.gridx = 0; gbc.gridy = 1; gbc.fill = GridBagConstraints.NONE; gbc.weightx = 0;
        panel.add(new JLabel("Chave Pública do Servidor:"), gbc);
        serverPublicKeyField = new JTextField(30);
        gbc.gridx = 1; gbc.fill = GridBagConstraints.HORIZONTAL; gbc.weightx = 1.0;
        panel.add(serverPublicKeyField, gbc);

        gbc.gridx = 0; gbc.gridy = 2; gbc.fill = GridBagConstraints.NONE; gbc.weightx = 0;
        panel.add(new JLabel("IPs Permitidos:"), gbc);
        allowedIPsField = new JTextField("0.0.0.0/0, ::/0", 30);
        gbc.gridx = 1; gbc.fill = GridBagConstraints.HORIZONTAL; gbc.weightx = 1.0;
        panel.add(allowedIPsField, gbc);

        return panel;
    }

    private JPanel createControlPanel() {
        JPanel panel = new JPanel(new FlowLayout());
        panel.setBorder(new TitledBorder("🎮 Controles"));

        connectButton = new JButton("🚀 Conectar VPN");
        connectButton.setBackground(new Color(40, 167, 69));
        connectButton.setForeground(Color.WHITE);
        connectButton.setPreferredSize(new Dimension(150, 40));
        connectButton.addActionListener(this::connectVPN);

        disconnectButton = new JButton("🛑 Desconectar");
        disconnectButton.setBackground(new Color(220, 53, 69));
        disconnectButton.setForeground(Color.WHITE);
        disconnectButton.setPreferredSize(new Dimension(150, 40));
        disconnectButton.setEnabled(false);
        disconnectButton.addActionListener(this::disconnectVPN);

        panel.add(connectButton);
        panel.add(disconnectButton);

        return panel;
    }

    private JPanel createLogPanel() {
        JPanel panel = new JPanel(new BorderLayout());
        panel.setBorder(new TitledBorder("📋 Logs"));

        logArea = new JTextArea(15, 50);
        logArea.setEditable(false);
        logArea.setFont(new Font("Monospaced", Font.PLAIN, 12));
        logArea.setBackground(new Color(33, 37, 41));
        logArea.setForeground(Color.GREEN);

        JScrollPane scrollPane = new JScrollPane(logArea);
        scrollPane.setVerticalScrollBarPolicy(JScrollPane.VERTICAL_SCROLLBAR_ALWAYS);
        panel.add(scrollPane, BorderLayout.CENTER);

        return panel;
    }

    private void createConfigDirectory() {
        try {
            Files.createDirectories(Paths.get(CONFIG_DIR));
            log("✅ Diretório de configuração criado: " + CONFIG_DIR);
        } catch (IOException e) {
            log("❌ Erro ao criar diretório: " + e.getMessage());
        }
    }

    private void generateKeys(ActionEvent e) {
        CompletableFuture.runAsync(() -> {
            try {
                log("🔑 Gerando chaves criptográficas...");

                X25519KeyPairGenerator keyPairGenerator = new X25519KeyPairGenerator();
                keyPairGenerator.init(new X25519KeyGenerationParameters(new SecureRandom()));

                var keyPair = keyPairGenerator.generateKeyPair();
                X25519PrivateKeyParameters privKey = (X25519PrivateKeyParameters) keyPair.getPrivate();
                X25519PublicKeyParameters pubKey = (X25519PublicKeyParameters) keyPair.getPublic();

                privateKey = Base64.getEncoder().encodeToString(privKey.getEncoded());
                publicKey = Base64.getEncoder().encodeToString(pubKey.getEncoded());

                SwingUtilities.invokeLater(() -> {
                    privateKeyLabel.setText(truncateKey(privateKey));
                    publicKeyLabel.setText(truncateKey(publicKey));
                });

                saveKeys();
                log("✅ Chaves geradas com sucesso!");

            } catch (Exception ex) {
                log("❌ Erro ao gerar chaves: " + ex.getMessage());
            }
        });
    }

    private void connectVPN(ActionEvent e) {
        if (privateKey == null || serverPublicKeyField.getText().trim().isEmpty()) {
            JOptionPane.showMessageDialog(this,
                    "Por favor, gere as chaves e configure o servidor primeiro!",
                    "Configuração Incompleta",
                    JOptionPane.WARNING_MESSAGE);
            return;
        }

        CompletableFuture.runAsync(() -> {
            try {
                log("🚀 Iniciando conexão VPN...");

                createWireGuardConfig();

                ProcessBuilder pb = new ProcessBuilder("wg-quick", "up", WG_CONFIG_FILE);
                pb.redirectErrorStream(true);
                wgProcess = pb.start();

                BufferedReader reader = new BufferedReader(
                        new InputStreamReader(wgProcess.getInputStream()));
                String line;
                while ((line = reader.readLine()) != null) {
                    final String logLine = line;
                    SwingUtilities.invokeLater(() -> log("WG: " + logLine));
                }

                int exitCode = wgProcess.waitFor();

                SwingUtilities.invokeLater(() -> {
                    if (exitCode == 0) {
                        isConnected = true;
                        connectButton.setEnabled(false);
                        disconnectButton.setEnabled(true);
                        statusLabel.setText("🟢 Conectado");
                        statusLabel.setForeground(Color.GREEN);
                        log("✅ VPN conectada com sucesso!");
                    } else {
                        log("❌ Falha na conexão VPN (código: " + exitCode + ")");
                    }
                });

            } catch (Exception ex) {
                SwingUtilities.invokeLater(() ->
                        log("❌ Erro na conexão: " + ex.getMessage()));
            }
        });
    }

    private void disconnectVPN(ActionEvent e) {
        CompletableFuture.runAsync(() -> {
            try {
                log("🛑 Desconectando VPN...");

                ProcessBuilder pb = new ProcessBuilder("wg-quick", "down", WG_CONFIG_FILE);
                Process process = pb.start();
                int exitCode = process.waitFor();

                SwingUtilities.invokeLater(() -> {
                    if (exitCode == 0) {
                        isConnected = false;
                        connectButton.setEnabled(true);
                        disconnectButton.setEnabled(false);
                        statusLabel.setText("🔴 Desconectado");
                        statusLabel.setForeground(Color.RED);
                        log("✅ VPN desconectada!");
                    } else {
                        log("❌ Erro ao desconectar (código: " + exitCode + ")");
                    }
                });

            } catch (Exception ex) {
                SwingUtilities.invokeLater(() ->
                        log("❌ Erro na desconexão: " + ex.getMessage()));
            }
        });
    }

    private void createWireGuardConfig() throws IOException {
        String config = String.format(
                "[Interface]\n" +
                        "PrivateKey = %s\n" +
                        "Address = 10.0.0.2/32\n" +
                        "DNS = 1.1.1.1, 1.0.0.1\n\n" +
                        "[Peer]\n" +
                        "PublicKey = %s\n" +
                        "Endpoint = %s\n" +
                        "AllowedIPs = %s\n" +
                        "PersistentKeepalive = 25\n",
                privateKey,
                serverPublicKeyField.getText().trim(),
                serverEndpointField.getText().trim(),
                allowedIPsField.getText().trim()
        );

        Files.write(Paths.get(WG_CONFIG_FILE), config.getBytes());
        log("📝 Configuração WireGuard criada");
    }

    private void saveKeys() {
        try {
            String keyData = privateKey + "\n" + publicKey;
            Files.write(Paths.get(CONFIG_DIR + "/keys.txt"), keyData.getBytes());
        } catch (IOException e) {
            log("❌ Erro ao salvar chaves: " + e.getMessage());
        }
    }

    private void loadExistingKeys() {
        try {
            Path keyFile = Paths.get(CONFIG_DIR + "/keys.txt");
            if (Files.exists(keyFile)) {
                String[] keys = Files.readString(keyFile).split("\n");
                if (keys.length >= 2) {
                    privateKey = keys[0];
                    publicKey = keys[1];
                    privateKeyLabel.setText(truncateKey(privateKey));
                    publicKeyLabel.setText(truncateKey(publicKey));
                    log("🔑 Chaves carregadas do arquivo");
                }
            }
        } catch (IOException e) {
            log("⚠️ Nenhuma chave salva encontrada");
        }
    }

    private String truncateKey(String key) {
        return key.length() > 20 ? key.substring(0, 20) + "..." : key;
    }

    private void log(String message) {
        SwingUtilities.invokeLater(() -> {
            String timestamp = java.time.LocalTime.now().toString().substring(0, 8);
            logArea.append("[" + timestamp + "] " + message + "\n");
            logArea.setCaretPosition(logArea.getDocument().getLength());
        });
    }

    public static void main(String[] args) {
        SwingUtilities.invokeLater(() -> {
            new BulletVPN().setVisible(true);
        });
    }
}