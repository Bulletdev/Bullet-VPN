# Security Policy

## Reporting a Vulnerability

**Bullet VPN** handles sensitive network configurations and VPN credentials. We take security seriously and appreciate your efforts to responsibly disclose your findings.

### Reporting Process

**DO NOT** create public GitHub issues for security vulnerabilities.

Instead, please report security vulnerabilities by emailing:

```
security@effront.gg
```

### What to Include

Please include the following information in your report:

- **Type of vulnerability** (e.g., command injection, credential exposure, privilege escalation, IPC bypass)
- **Full path** of the affected source file(s)
- **Location** of the affected code (file path, line number, commit hash)
- **Step-by-step instructions** to reproduce the issue
- **Proof-of-concept or exploit code** (if possible)
- **Impact** of the vulnerability
- **Suggested fix** (if you have one)

### Response Timeline

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Resolution Target**: Depends on severity (see below)

### Severity Levels

| Severity | Response Time | Examples |
|----------|---------------|----------|
| **Critical** | 24-48 hours | RCE via Tauri IPC, credential leakage from config files, privilege escalation |
| **High** | 3-7 days | VPN config injection, kill switch bypass, DNS leak exploitation |
| **Medium** | 7-14 days | Information disclosure in logs, unsafe profile import handling |
| **Low** | 14-30 days | Verbose error messages, minor information leakage in UI |

## Security Architecture

Bullet VPN implements security measures to protect user credentials and network configurations:

### VPN Credential Protection
- VPN profiles (WireGuard `.conf`, OpenVPN `.ovpn`) are stored locally on the user's machine
- No telemetry, no cloud sync -- zero logs policy
- Private keys and credentials never leave the local filesystem

### Tauri IPC Security
- Tauri 2 capability-based permission model restricts frontend-to-backend communication
- Shell commands (`wg-quick`, `openvpn`, `ssh`) executed only through approved Tauri command handlers
- No arbitrary command execution from the frontend

### Network Protection
- Kill switch via `iptables`/`nftables` rules to prevent traffic leaks
- DNS leak protection with custom resolver configuration
- Connection state monitoring with automatic reconnection

### Build & Distribution
- Rust backend compiled with standard safety guarantees
- Frontend dependencies audited via `npm audit`
- Reproducible builds via locked dependency versions (`package-lock.json`, `Cargo.lock`)

## Responsible Use

Bullet VPN is designed for **legitimate self-hosted VPN management**:

- Only connect to VPN servers and infrastructure you own or have authorization to use
- Do not use Bullet VPN to bypass security controls without proper authorization
- Respect network policies and terms of service
- Comply with all applicable local, national, and international laws

The authors assume no liability for misuse.

## Security Considerations for Contributors

When contributing to Bullet VPN:

1. **Never hardcode credentials** -- no API keys, private keys, or passwords in source code
2. **Validate all IPC inputs** -- Tauri commands must validate arguments from the frontend
3. **Sanitize shell arguments** -- prevent command injection in `wg-quick`, `openvpn`, `ssh` invocations
4. **Handle config files safely** -- validate imported `.conf` and `.ovpn` files before processing
5. **Protect sensitive state** -- VPN credentials and private keys must not appear in logs or error messages
6. **Respect permissions** -- use minimum required privileges for network operations

## Supported Versions

| Version | Supported |
|---------|-----------|
| 2.x     | Yes       |
| 1.x (Java) | No    |

## Safe Harbor

We provide a safe harbor for security researchers who:

- Make a good faith effort to avoid privacy violations, data destruction, and service disruption
- Only interact with accounts you own or with explicit permission
- Do not exploit a vulnerability beyond the minimum necessary to demonstrate it
- Report vulnerabilities promptly
- Give us reasonable time to fix the issue before any public disclosure

We will not pursue legal action against researchers who follow this policy.

## Out of Scope

The following are explicitly **out of scope** for vulnerability reports:

- Social engineering attacks against maintainers
- Denial of Service (DoS/DDoS) against project infrastructure
- Vulnerabilities in third-party dependencies (report to their maintainers directly)
- Issues requiring physical access to the user's machine
- Vulnerabilities in outdated or unsupported versions
- VPN server-side vulnerabilities (out of scope for the client application)

## Contact

- **Security Team**: security@effront.gg
- **GitHub Issues**: For non-security bugs only

## Additional Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CWE Top 25](https://cwe.mitre.org/top25/)
- [Tauri Security](https://v2.tauri.app/security/)

---

**Last Updated**: 2026-10-31
**Policy Version**: 1.0
