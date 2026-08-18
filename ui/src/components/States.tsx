import { Alert, Center, Loader, Stack, Text } from "@mantine/core";
import { IconAlertTriangle, IconMoodEmpty } from "@tabler/icons-react";
import type { ReactNode } from "react";

import { ApiError } from "../api/http";

export function LoadingState({ height = 160 }: { height?: number }) {
  return (
    <Center h={height}>
      <Loader size="sm" />
    </Center>
  );
}

export function ErrorState({
  error,
  title = "Something went wrong",
}: {
  error: unknown;
  title?: string;
}) {
  const message =
    error instanceof ApiError
      ? error.message
      : error instanceof Error
        ? error.message
        : String(error);

  return (
    <Alert color="red" icon={<IconAlertTriangle size={18} />} title={title}>
      {message}
    </Alert>
  );
}

export function EmptyState({ message, action }: { message: string; action?: ReactNode }) {
  return (
    <Center py="xl">
      <Stack align="center" gap="xs">
        <IconMoodEmpty size={28} opacity={0.5} />
        <Text c="dimmed" size="sm" ta="center">
          {message}
        </Text>
        {action}
      </Stack>
    </Center>
  );
}
