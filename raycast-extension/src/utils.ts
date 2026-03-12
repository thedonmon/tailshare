import { execFileSync, execSync } from "child_process";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { join } from "path";

const TAILSHARE_BIN = `${process.env.HOME}/.cargo/bin/tailshare`;

const EXEC_ENV = {
  ...process.env,
  PATH: `/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:${process.env.HOME}/.cargo/bin:${process.env.PATH}`,
  HOME: process.env.HOME,
  SSH_AUTH_SOCK: process.env.SSH_AUTH_SOCK ?? "",
  NO_COLOR: "1",
};

export function runTailshare(args: string[]): string {
  try {
    const result = execFileSync(TAILSHARE_BIN, args, {
      encoding: "utf-8",
      timeout: 15000,
      env: EXEC_ENV,
    });
    return result.trim();
  } catch (error: unknown) {
    const execError = error as { stderr?: string; stdout?: string; message?: string };
    const stderr = execError.stderr?.trim() ?? "";
    const stdout = execError.stdout?.trim() ?? "";
    throw new Error(stderr || stdout || execError.message || "Unknown error");
  }
}

export interface Device {
  name: string;
  ip: string;
  os: string;
  online: boolean;
  isSelf: boolean;
  dnsName: string;
  shortName: string;
}

export interface StaleConfigEntry {
  section: string;
  key: string;
  suggestions: string[];
}

export function getDevices(): Device[] {
  const output = execSync(
    `/Applications/Tailscale.app/Contents/MacOS/Tailscale status --json`,
    { encoding: "utf-8", timeout: 10000, env: EXEC_ENV }
  );
  const status = JSON.parse(output);
  const devices: Device[] = [];

  const self = status.Self;
  const selfDns = self.DNSName?.replace(/\.$/, "") ?? "";
  devices.push({
    name: self.HostName,
    ip: self.TailscaleIPs?.[0] ?? "",
    os: self.OS,
    online: true,
    isSelf: true,
    dnsName: selfDns,
    shortName: selfDns.split(".")[0] ?? "",
  });

  if (status.Peer) {
    for (const peer of Object.values(status.Peer) as Record<string, unknown>[]) {
      const peerDns = ((peer as { DNSName: string }).DNSName)?.replace(/\.$/, "") ?? "";
      devices.push({
        name: (peer as { HostName: string }).HostName,
        ip: ((peer as { TailscaleIPs: string[] }).TailscaleIPs)?.[0] ?? "",
        os: (peer as { OS: string }).OS,
        online: (peer as { Online: boolean }).Online,
        isSelf: false,
        dnsName: peerDns,
        shortName: peerDns.split(".")[0] ?? "",
      });
    }
  }

  return devices;
}

// ── Config helpers (read/write TOML directly, no CLI needed) ──

const CONFIG_DIR = join(process.env.HOME ?? "", ".config", "tailshare");
const CONFIG_PATH = join(CONFIG_DIR, "config.toml");

export interface TailshareConfig {
  default_device?: string;
  local_os?: string;
  aliases: Record<string, string>;
  users: Record<string, string>;
  os_overrides: Record<string, string>;
  device_ips: Record<string, string>;
}

/** Minimal TOML parser — handles the flat sections used by tailshare config. */
export function loadConfig(): TailshareConfig {
  const cfg: TailshareConfig = { aliases: {}, users: {}, os_overrides: {}, device_ips: {} };
  if (!existsSync(CONFIG_PATH)) return cfg;

  const content = readFileSync(CONFIG_PATH, "utf-8");
  let currentSection: string | null = null;

  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const sectionMatch = trimmed.match(/^\[(.+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      continue;
    }

    const kvMatch = trimmed.match(/^([^=]+?)\s*=\s*"(.*)"\s*$/);
    if (!kvMatch) continue;
    // Strip surrounding quotes from key (TOML quoted keys like "dons-mac-mini")
    const key = kvMatch[1].trim().replace(/^"|"$/g, "");
    const value = kvMatch[2];

    if (!currentSection) {
      if (key === "default_device") cfg.default_device = value;
      else if (key === "local_os") cfg.local_os = value;
    } else if (currentSection === "aliases") {
      cfg.aliases[key] = value;
    } else if (currentSection === "users") {
      cfg.users[key] = value;
    } else if (currentSection === "os_overrides") {
      cfg.os_overrides[key] = value;
    } else if (currentSection === "device_ips") {
      cfg.device_ips[key] = value;
    }
  }
  return cfg;
}

/** Serialize config back to TOML. */
function serializeConfig(cfg: TailshareConfig): string {
  const lines: string[] = [];
  if (cfg.default_device) lines.push(`default_device = "${cfg.default_device}"`);
  if (cfg.local_os) lines.push(`local_os = "${cfg.local_os}"`);
  lines.push("");

  lines.push("[aliases]");
  for (const [k, v] of Object.entries(cfg.aliases)) {
    lines.push(`${k} = "${v}"`);
  }
  lines.push("");

  lines.push("[users]");
  for (const [k, v] of Object.entries(cfg.users)) {
    lines.push(`"${k}" = "${v}"`);
  }
  lines.push("");

  lines.push("[os_overrides]");
  for (const [k, v] of Object.entries(cfg.os_overrides)) {
    lines.push(`"${k}" = "${v}"`);
  }
  lines.push("");

  lines.push("[device_ips]");
  for (const [k, v] of Object.entries(cfg.device_ips)) {
    lines.push(`"${k}" = "${v}"`);
  }
  lines.push("");

  return lines.join("\n");
}

export function saveConfig(cfg: TailshareConfig): void {
  mkdirSync(CONFIG_DIR, { recursive: true });
  writeFileSync(CONFIG_PATH, serializeConfig(cfg), "utf-8");
}

/** Check whether the tailshare CLI binary is installed. */
export function isCLIInstalled(): boolean {
  return existsSync(TAILSHARE_BIN);
}

/** Open Terminal.app with a command pre-filled. */
export function openInTerminal(command: string): void {
  execSync(
    `osascript -e 'tell application "Terminal" to do script "${command.replace(/"/g, '\\"')}"' -e 'tell application "Terminal" to activate'`,
    { env: EXEC_ENV },
  );
}

/** Check config for stale device references by running `tailshare config doctor`. */
export function checkConfigHealth(): StaleConfigEntry[] {
  try {
    const output = runTailshare(["config", "doctor"]);
    // If output contains "✗", parse stale entries
    if (!output.includes("not found on tailnet")) {
      return [];
    }
    const entries: StaleConfigEntry[] = [];
    const lines = output.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const match = lines[i].match(/\[(.+?)\]\s+'(.+?)'\s+not found/);
      if (match) {
        const suggestions: string[] = [];
        // Look ahead for suggestions
        for (let j = i + 1; j < lines.length && lines[j].match(/^\s+-\s+/); j++) {
          const s = lines[j].match(/^\s+-\s+(.+)/);
          if (s) suggestions.push(s[1].trim());
        }
        entries.push({ section: match[1], key: match[2], suggestions });
      }
    }
    return entries;
  } catch {
    return [];
  }
}
