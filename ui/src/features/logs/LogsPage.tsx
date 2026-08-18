import { useState } from "react";
import {
  ActionIcon,
  Alert,
  Card,
  Group,
  NumberInput,
  ScrollArea,
  Stack,
  Switch,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import {
  IconChevronLeft,
  IconChevronRight,
  IconInfoCircle,
  IconRefresh,
} from "@tabler/icons-react";

import { useLogs } from "../../api/queries";
import { ErrorState, LoadingState } from "../../components/States";

const NOT_ENABLED = "Logging to file not enabled";

export function LogsPage() {
  // Page 0 is the newest chunk; higher pages go further back.
  const [page, setPage] = useState(0);
  const [perPage, setPerPage] = useState(100);
  const [autoRefresh, setAutoRefresh] = useState(false);

  const logs = useLogs(page, perPage, autoRefresh);
  const html = logs.data ?? "";
  const loggingDisabled = html.startsWith(NOT_ENABLED);

  return (
    <Stack gap="md">
      <Group justify="space-between" wrap="wrap" gap="sm">
        <Title order={4}>Logs</Title>
        <Group gap="sm">
          <Switch
            size="sm"
            label="Auto refresh"
            checked={autoRefresh}
            onChange={(event) => setAutoRefresh(event.currentTarget.checked)}
          />
          <Tooltip label="Refresh">
            <ActionIcon
              variant="default"
              aria-label="Refresh logs"
              loading={logs.isFetching}
              onClick={() => logs.refetch()}
            >
              <IconRefresh size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {logs.error ? (
        <ErrorState error={logs.error} title="Could not load logs" />
      ) : loggingDisabled ? (
        <Alert color="yellow" icon={<IconInfoCircle size={18} />} title="No log file">
          Start the miner with <code>--log-file</code> to read logs here.
        </Alert>
      ) : (
        <Card p={0} className="tpm-logs">
          <ScrollArea h="65vh" type="auto">
            {logs.isLoading ? (
              <LoadingState />
            ) : (
              <pre
                style={{
                  margin: 0,
                  padding: 12,
                  fontSize: 12,
                  lineHeight: 1.5,
                  whiteSpace: "pre",
                }}
                // Server-rendered output of ansi_to_html::convert.
                dangerouslySetInnerHTML={{ __html: html }}
              />
            )}
          </ScrollArea>
        </Card>
      )}

      <Group justify="space-between" wrap="wrap" gap="sm">
        <NumberInput
          style={{ width: 130 }}
          size="xs"
          label="Lines per page"
          min={10}
          max={1000}
          step={50}
          allowDecimal={false}
          value={perPage}
          onChange={(value) => setPerPage(Number(value) || 100)}
        />

        <Group gap="xs" align="center">
          {/* Higher page numbers are older, so left goes back in time. */}
          <ActionIcon
            variant="default"
            aria-label="Older lines"
            onClick={() => setPage((p) => p + 1)}
          >
            <IconChevronLeft size={16} />
          </ActionIcon>
          <Text size="sm" c="dimmed">
            {page === 0 ? "Newest" : `${page} page${page === 1 ? "" : "s"} back`}
          </Text>
          <ActionIcon
            variant="default"
            aria-label="Newer lines"
            disabled={page === 0}
            onClick={() => setPage((p) => Math.max(0, p - 1))}
          >
            <IconChevronRight size={16} />
          </ActionIcon>
        </Group>
      </Group>
    </Stack>
  );
}
