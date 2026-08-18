import { Stack, Tabs, Title } from "@mantine/core";
import { IconAdjustments, IconStack2, IconUsers } from "@tabler/icons-react";

import { PresetsTab } from "./PresetsTab";
import { StreamersTab } from "./StreamersTab";
import { WatchingTab } from "./WatchingTab";

export function SetupPage() {
  return (
    <Stack gap="md">
      <Title order={4}>Setup</Title>

      <Tabs defaultValue="streamers" keepMounted={false}>
        <Tabs.List grow justify="space-between">
          <Tabs.Tab value="streamers" leftSection={<IconUsers size={16} />}>
            Streamers
          </Tabs.Tab>
          <Tabs.Tab value="presets" leftSection={<IconStack2 size={16} />}>
            Presets
          </Tabs.Tab>
          <Tabs.Tab value="watching" leftSection={<IconAdjustments size={16} />}>
            Watching
          </Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="streamers" pt="md">
          <StreamersTab />
        </Tabs.Panel>
        <Tabs.Panel value="presets" pt="md">
          <PresetsTab />
        </Tabs.Panel>
        <Tabs.Panel value="watching" pt="md">
          <WatchingTab />
        </Tabs.Panel>
      </Tabs>
    </Stack>
  );
}
