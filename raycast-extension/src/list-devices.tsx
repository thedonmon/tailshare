import { List, Icon, Color } from "@raycast/api";
import { getDevices } from "./utils";
import { useMemo } from "react";

export default function Command() {
  const devices = useMemo(() => {
    try {
      return getDevices();
    } catch {
      return [];
    }
  }, []);

  return (
    <List>
      {devices.map((device) => (
        <List.Item
          key={device.ip}
          title={device.name}
          subtitle={device.dnsName}
          accessories={[
            { text: device.ip },
            { text: device.os },
            {
              icon: device.isSelf
                ? { source: Icon.Monitor, tintColor: Color.Blue }
                : device.online
                  ? { source: Icon.CircleFilled, tintColor: Color.Green }
                  : { source: Icon.Circle, tintColor: Color.Red },
              tooltip: device.isSelf ? "This device" : device.online ? "Online" : "Offline",
            },
          ]}
        />
      ))}
    </List>
  );
}
