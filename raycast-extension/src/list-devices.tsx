import { List, Action, ActionPanel, Icon, Color, showToast, Toast, Form, useNavigation } from "@raycast/api";
import { getDevices, checkConfigHealth, loadConfig, saveConfig, openInTerminal, isCLIInstalled, runTailshare, Device } from "./utils";
import { useMemo, useState, useCallback } from "react";

function SetUserForm({ device, onDone }: { device: Device; onDone: () => void }) {
  const { pop } = useNavigation();
  const cfg = loadConfig();

  return (
    <Form
      actions={
        <ActionPanel>
          <Action.SubmitForm
            title="Save"
            onSubmit={async (values: { user: string }) => {
              const c = loadConfig();
              c.users[device.shortName] = values.user;
              saveConfig(c);
              await showToast({ style: Toast.Style.Success, title: `SSH user for ${device.shortName} set to ${values.user}` });
              onDone();
              pop();
            }}
          />
        </ActionPanel>
      }
    >
      <Form.TextField id="user" title="SSH Username" defaultValue={cfg.users[device.shortName] ?? ""} />
    </Form>
  );
}

function AddAliasForm({ device, onDone }: { device: Device; onDone: () => void }) {
  const { pop } = useNavigation();

  return (
    <Form
      actions={
        <ActionPanel>
          <Action.SubmitForm
            title="Save"
            onSubmit={async (values: { alias: string }) => {
              const c = loadConfig();
              c.aliases[values.alias] = device.shortName;
              saveConfig(c);
              await showToast({ style: Toast.Style.Success, title: `Alias '${values.alias}' -> '${device.shortName}'` });
              onDone();
              pop();
            }}
          />
        </ActionPanel>
      }
    >
      <Form.TextField id="alias" title="Alias Name" placeholder="e.g. mini" />
    </Form>
  );
}

export default function Command() {
  const [refreshKey, setRefreshKey] = useState(0);
  const refresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  const devices = useMemo(() => {
    try {
      return getDevices();
    } catch {
      return [];
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);

  const staleEntries = useMemo(() => {
    if (!isCLIInstalled()) return [];
    try {
      return checkConfigHealth();
    } catch {
      return [];
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);

  const config = useMemo(() => loadConfig(), [refreshKey]);

  async function setDefault(device: Device) {
    const c = loadConfig();
    c.default_device = device.shortName;
    saveConfig(c);
    await showToast({ style: Toast.Style.Success, title: `Default device set to ${device.shortName}` });
    refresh();
  }

  async function setOs(device: Device, os: string) {
    const c = loadConfig();
    c.os_overrides[device.shortName] = os;
    saveConfig(c);
    await showToast({ style: Toast.Style.Success, title: `OS for ${device.shortName} set to ${os}` });
    refresh();
  }

  function setupDevice(device: Device) {
    openInTerminal(`tailshare setup ${device.shortName}`);
  }

  return (
    <List>
      {staleEntries.length > 0 && (
        <List.Section title="Config Issues">
          {staleEntries.map((entry) => (
            <List.Item
              key={`stale-${entry.section}-${entry.key}`}
              title={`"${entry.key}" not found`}
              subtitle={`in ${entry.section}`}
              icon={{ source: Icon.Warning, tintColor: Color.Yellow }}
              accessories={[
                {
                  text: entry.suggestions.length > 0
                    ? `Try: ${entry.suggestions.join(", ")}`
                    : "Device renamed or removed",
                },
              ]}
              actions={
                <ActionPanel>
                  {isCLIInstalled() && entry.suggestions.map((s) => (
                    <Action
                      key={s}
                      title={`Sync to "${s}"`}
                      icon={Icon.ArrowRight}
                      onAction={async () => {
                        try {
                          runTailshare(["sync", entry.key, s]);
                          await showToast({ style: Toast.Style.Success, title: `Synced "${entry.key}" to "${s}"` });
                        } catch (e) {
                          await showToast({ style: Toast.Style.Failure, title: "Sync failed", message: e instanceof Error ? e.message : String(e) });
                        }
                        refresh();
                      }}
                    />
                  ))}
                  {!isCLIInstalled() && entry.suggestions.map((s) => (
                    <Action
                      key={s}
                      title={`Update config to "${s}"`}
                      icon={Icon.ArrowRight}
                      onAction={async () => {
                        const c = loadConfig();
                        if (c.default_device === entry.key) c.default_device = s;
                        for (const [alias, target] of Object.entries(c.aliases)) {
                          if (target === entry.key) c.aliases[alias] = s;
                        }
                        if (c.users[entry.key]) { c.users[s] = c.users[entry.key]; delete c.users[entry.key]; }
                        if (c.os_overrides[entry.key]) { c.os_overrides[s] = c.os_overrides[entry.key]; delete c.os_overrides[entry.key]; }
                        if (c.device_ips[entry.key]) { c.device_ips[s] = c.device_ips[entry.key]; delete c.device_ips[entry.key]; }
                        saveConfig(c);
                        await showToast({ style: Toast.Style.Success, title: `Updated "${entry.key}" to "${s}"` });
                        refresh();
                      }}
                    />
                  ))}
                  <Action
                    title="Run Sync in Terminal"
                    icon={Icon.Terminal}
                    onAction={() => openInTerminal(`tailshare sync ${entry.key} ${entry.suggestions[0] ?? ""}`)}
                  />
                </ActionPanel>
              }
            />
          ))}
        </List.Section>
      )}
      <List.Section title={staleEntries.length > 0 ? "Devices" : undefined}>
        {devices.map((device) => (
          <List.Item
            key={device.ip}
            title={device.name}
            subtitle={device.dnsName}
            accessories={[
              { text: device.ip },
              { text: device.os },
              ...(config.default_device === device.shortName
                ? [{ icon: { source: Icon.Star, tintColor: Color.Yellow }, tooltip: "Default device" }]
                : []),
              {
                icon: device.isSelf
                  ? { source: Icon.Monitor, tintColor: Color.Blue }
                  : device.online
                    ? { source: Icon.CircleFilled, tintColor: Color.Green }
                    : { source: Icon.Circle, tintColor: Color.Red },
                tooltip: device.isSelf ? "This device" : device.online ? "Online" : "Offline",
              },
            ]}
            actions={
              !device.isSelf ? (
                <ActionPanel>
                  <ActionPanel.Section title="Config">
                    <Action
                      title="Set as Default"
                      icon={Icon.Star}
                      onAction={() => setDefault(device)}
                    />
                    <Action.Push
                      title="Set SSH User"
                      icon={Icon.Person}
                      target={<SetUserForm device={device} onDone={refresh} />}
                    />
                    <Action.Push
                      title="Add Alias"
                      icon={Icon.Tag}
                      target={<AddAliasForm device={device} onDone={refresh} />}
                    />
                    <ActionPanel.Submenu title="Set OS Override" icon={Icon.ComputerChip}>
                      <Action title="macOS" onAction={() => setOs(device, "macos")} />
                      <Action title="Linux" onAction={() => setOs(device, "linux")} />
                      <Action title="Windows" onAction={() => setOs(device, "windows")} />
                    </ActionPanel.Submenu>
                  </ActionPanel.Section>
                  <ActionPanel.Section title="Setup">
                    <Action
                      title="Setup SSH Keys (Opens Terminal)"
                      icon={Icon.Terminal}
                      onAction={() => setupDevice(device)}
                    />
                  </ActionPanel.Section>
                </ActionPanel>
              ) : undefined
            }
          />
        ))}
      </List.Section>
    </List>
  );
}
