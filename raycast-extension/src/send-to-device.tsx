import { List, Action, ActionPanel, Icon, Color, showHUD } from "@raycast/api";
import { getDevices, runTailshare } from "./utils";
import { useMemo } from "react";

export default function Command() {
  const devices = useMemo(() => {
    try {
      return getDevices().filter((d) => !d.isSelf && d.online);
    } catch {
      return [];
    }
  }, []);

  async function sendTo(deviceName: string) {
    try {
      const output = runTailshare(["send", deviceName]);
      await showHUD(output);
    } catch (error) {
      await showHUD(`Failed: ${error instanceof Error ? error.message : error}`);
    }
  }

  return (
    <List>
      {devices.map((device) => (
        <List.Item
          key={device.ip}
          title={device.name}
          subtitle={device.dnsName}
          accessories={[
            { text: device.os },
            {
              icon: { source: Icon.CircleFilled, tintColor: Color.Green },
              tooltip: "Online",
            },
          ]}
          actions={
            <ActionPanel>
              <Action
                title="Send Clipboard"
                icon={Icon.Upload}
                onAction={() => sendTo(device.shortName)}
              />
            </ActionPanel>
          }
        />
      ))}
    </List>
  );
}
