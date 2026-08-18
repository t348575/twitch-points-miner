import { Button, Group, Modal, Stack, TextInput } from "@mantine/core";
import { useForm } from "@mantine/form";
import { useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { useEffect, useState } from "react";

import { ApiError } from "../../api/http";
import { useMineStreamer, useSaveStreamerConfig } from "../../api/queries";
import type { StreamerConfigRefWrapper } from "../../api/types";
import { ConfigEditor } from "../config/ConfigEditor";
import {
  emptyConfig,
  toConfigType,
  toFormValues,
  validateConfigForm,
  type ConfigFormValues,
} from "../config/configForm";

interface StreamerConfigModalProps {
  opened: boolean;
  onClose: () => void;
  presetNames: string[];
  /** The streamer being edited, or null when adding a new one. */
  streamer: { name: string; config: StreamerConfigRefWrapper } | null;
}

function initialValues(streamer: StreamerConfigModalProps["streamer"]): ConfigFormValues {
  if (!streamer) return toFormValues(emptyConfig(), "specific", "");

  const ref = streamer.config._type;
  const isPreset = typeof ref !== "string";
  return toFormValues(
    streamer.config.config,
    isPreset ? "preset" : "specific",
    isPreset ? ref.Preset : "",
  );
}

export function StreamerConfigModal({
  opened,
  onClose,
  presetNames,
  streamer,
}: StreamerConfigModalProps) {
  const mine = useMineStreamer();
  const saveConfig = useSaveStreamerConfig();
  const isSmallUp = useMediaQuery("(min-width: 48em)") ?? false;
  const isNew = streamer === null;

  // Kept out of the config form so the editor keeps a single value shape.
  const [channelName, setChannelName] = useState("");
  const [nameError, setNameError] = useState<string | null>(null);

  const form = useForm<ConfigFormValues>({
    initialValues: initialValues(streamer),
    validate: validateConfigForm,
  });

  // Reload whenever a different streamer is opened.
  useEffect(() => {
    if (!opened) return;
    form.setValues(initialValues(streamer));
    form.resetDirty();
    setChannelName(streamer?.name ?? "");
    setNameError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened, streamer?.name]);

  const submit = form.onSubmit(async (values) => {
    const name = isNew ? channelName.trim() : streamer.name;
    if (isNew && !name) {
      setNameError("Enter a channel name");
      return;
    }

    const config = toConfigType(values);

    try {
      if (isNew) await mine.mutateAsync({ name, config });
      else await saveConfig.mutateAsync({ name, config });

      notifications.show({
        color: "teal",
        message: isNew ? `Now mining ${name}` : `Saved config for ${name}`,
      });
      onClose();
    } catch (error) {
      notifications.show({
        color: "red",
        title: isNew ? "Could not add streamer" : "Could not save config",
        message: error instanceof ApiError ? error.message : String(error),
      });
    }
  });

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isNew ? "Add streamer" : `Configure ${streamer.name}`}
      size="lg"
      fullScreen={!isSmallUp}
    >
      <form onSubmit={submit}>
        <Stack gap="md">
          {isNew && (
            <TextInput
              label="Channel name"
              placeholder="as it appears in the twitch URL"
              value={channelName}
              error={nameError}
              onChange={(event) => {
                setChannelName(event.currentTarget.value);
                setNameError(null);
              }}
            />
          )}

          <ConfigEditor form={form} presetNames={presetNames} />

          <Group justify="flex-end">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={mine.isPending || saveConfig.isPending}>
              {isNew ? "Add" : "Save"}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}
