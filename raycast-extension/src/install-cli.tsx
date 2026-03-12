import { Detail, ActionPanel, Action, Icon, showToast, Toast } from "@raycast/api";
import { isCLIInstalled, openInTerminal } from "./utils";
import { useMemo } from "react";

export default function Command() {
  const installed = useMemo(() => isCLIInstalled(), []);

  if (installed) {
    return (
      <Detail
        markdown={`## Tailshare CLI is installed\n\nThe CLI binary is available at \`~/.cargo/bin/tailshare\`.\n\nAll Raycast commands should work correctly.`}
        actions={
          <ActionPanel>
            <Action
              title="Reinstall / Update (Opens Terminal)"
              icon={Icon.Terminal}
              onAction={() => openInTerminal("cargo install --path ~/SourceCode/personal/tailshare --force")}
            />
          </ActionPanel>
        }
      />
    );
  }

  const markdown = `## Tailshare CLI Not Found

The Raycast extension requires the \`tailshare\` CLI binary at \`~/.cargo/bin/tailshare\`.

### Install Options

**Option 1: Build from source** (requires Rust/Cargo)
\`\`\`
cargo install --path ~/SourceCode/personal/tailshare
\`\`\`

**Option 2: Install Rust first, then build**
\`\`\`
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install --path ~/SourceCode/personal/tailshare
\`\`\`

Use the actions below to open a terminal and run the install.`;

  return (
    <Detail
      markdown={markdown}
      actions={
        <ActionPanel>
          <Action
            title="Install with Cargo (Opens Terminal)"
            icon={Icon.Terminal}
            onAction={async () => {
              openInTerminal("cargo install --path ~/SourceCode/personal/tailshare");
              await showToast({ style: Toast.Style.Animated, title: "Opened Terminal for install..." });
            }}
          />
          <Action
            title="Install Rust + Tailshare (Opens Terminal)"
            icon={Icon.Download}
            onAction={async () => {
              openInTerminal(
                'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source ~/.cargo/env && cargo install --path ~/SourceCode/personal/tailshare'
              );
              await showToast({ style: Toast.Style.Animated, title: "Opened Terminal for install..." });
            }}
          />
        </ActionPanel>
      }
    />
  );
}
