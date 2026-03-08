import { execFileSync, execSync } from "child_process";

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
}

export function getDevices(): Device[] {
  const output = execSync(
    `/Applications/Tailscale.app/Contents/MacOS/Tailscale status --json`,
    { encoding: "utf-8", timeout: 10000, env: EXEC_ENV }
  );
  const status = JSON.parse(output);
  const devices: Device[] = [];

  const self = status.Self;
  devices.push({
    name: self.HostName,
    ip: self.TailscaleIPs?.[0] ?? "",
    os: self.OS,
    online: true,
    isSelf: true,
    dnsName: self.DNSName?.replace(/\.$/, "") ?? "",
  });

  if (status.Peer) {
    for (const peer of Object.values(status.Peer) as Record<string, unknown>[]) {
      devices.push({
        name: (peer as { HostName: string }).HostName,
        ip: ((peer as { TailscaleIPs: string[] }).TailscaleIPs)?.[0] ?? "",
        os: (peer as { OS: string }).OS,
        online: (peer as { Online: boolean }).Online,
        isSelf: false,
        dnsName: ((peer as { DNSName: string }).DNSName)?.replace(/\.$/, "") ?? "",
      });
    }
  }

  return devices;
}
