import { Button, Group, Modal, Stack, TextInput } from "@mantine/core";
import { useForm } from "@mantine/form";
import { useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { useEffect, useState } from "react";

import { ApiError } from "../../api/http";
import { useSavePreset } from "../../api/queries";
import type { StreamerConfig } from "../../api/types";
import { ConfigEditor } from "../config/ConfigEditor";
import {
  emptyConfig,
  toFormValues,
  toStreamerConfig,
  validateConfigForm,
  type ConfigFormValues,
} from "../config/configForm";

interface PresetModalProps {
  opened: boolean;
  onClose: () => void;
  /** The preset being edited, or null when creating a new one. */
  preset: { name: string; config: StreamerConfig } | null;
}

export function PresetModal({ opened, onClose, preset }: PresetModalProps) {
  const savePreset = useSavePreset();
  const isSmallUp = useMediaQuery("(min-width: 48em)") ?? false;
  const isNew = preset === null;

  const [name, setName] = useState("");
  const [nameError, setNameError] = useState<string | null>(null);

  const form = useForm<ConfigFormValues>({
    initialValues: toFormValues(preset?.config ?? emptyConfig(), "specific", ""),
    validate: validateConfigForm,
  });

  useEffect(() => {
    if (!opened) return;
    form.setValues(toFormValues(preset?.config ?? emptyConfig(), "specific", ""));
    form.resetDirty();
    setName(preset?.name ?? "");
    setNameError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened, preset?.name]);

  const submit = form.onSubmit(async (values) => {
    const trimmed = name.trim();
    if (!trimmed) {
      setNameError("Enter a preset name");
      return;
    }

    try {
      await savePreset.mutateAsync({ name: trimmed, config: toStreamerConfig(values) });
      notifications.show({ color: "teal", message: `Saved preset ${trimmed}` });
      onClose();
    } catch (error) {
      notifications.show({
        color: "red",
        title: "Could not save preset",
        message: error instanceof ApiError ? error.message : String(error),
      });
    }
  });

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isNew ? "New preset" : `Edit ${preset.name}`}
      size="lg"
      fullScreen={!isSmallUp}
    >
      <form onSubmit={submit}>
        <Stack gap="md">
          <TextInput
            label="Preset name"
            description={isNew ? "Cannot match an existing streamer name" : undefined}
            value={name}
            error={nameError}
            disabled={!isNew}
            onChange={(event) => {
              setName(event.currentTarget.value);
              setNameError(null);
            }}
          />

          {/* A preset cannot reference another preset. */}
          <ConfigEditor form={form} presetNames={[]} allowPreset={false} />

          <Group justify="flex-end">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={savePreset.isPending}>
              Save
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}
