import {
  ActionIcon,
  Button,
  Card,
  Fieldset,
  Group,
  NumberInput,
  Select,
  SegmentedControl,
  SimpleGrid,
  Stack,
  Switch,
  Text,
} from "@mantine/core";
import type { UseFormReturnType } from "@mantine/form";
import { IconPlus, IconTrash } from "@tabler/icons-react";

import { emptyFilterRow, emptyOddsRow, type ConfigFormValues } from "./configForm";

interface ConfigEditorProps {
  form: UseFormReturnType<ConfigFormValues>;
  presetNames: string[];
  /** Presets cannot reference other presets, so the mode switch is hidden there. */
  allowPreset?: boolean;
}

export function ConfigEditor({ form, presetNames, allowPreset = true }: ConfigEditorProps) {
  const values = form.getValues();

  return (
    <Stack gap="md">
      {allowPreset && (
        <SegmentedControl
          fullWidth
          value={values.mode}
          onChange={(value) => form.setFieldValue("mode", value as "preset" | "specific")}
          data={[
            { label: "Use a preset", value: "preset" },
            { label: "Custom", value: "specific" },
          ]}
        />
      )}

      {allowPreset && values.mode === "preset" ? (
        <Select
          label="Preset"
          placeholder={presetNames.length ? "Pick a preset" : "No presets defined yet"}
          data={presetNames}
          disabled={presetNames.length === 0}
          {...form.getInputProps("presetName")}
        />
      ) : (
        <>
          <Switch
            label="Follow raids"
            description="Join the channel a streamer raids, which grants bonus points"
            checked={values.follow_raid}
            onChange={(event) => form.setFieldValue("follow_raid", event.currentTarget.checked)}
          />

          <Fieldset legend="Default bet">
            <Stack gap="sm">
              <Text size="xs" c="dimmed">
                Used when no rule below matches, and only when the outcome's odds fall inside this
                range.
              </Text>
              <SimpleGrid cols={{ base: 2 }} spacing="sm">
                <NumberInput
                  label="Min odds"
                  suffix="%"
                  min={0}
                  max={100}
                  decimalScale={2}
                  {...form.getInputProps("min_percentage")}
                />
                <NumberInput
                  label="Max odds"
                  suffix="%"
                  min={0}
                  max={100}
                  decimalScale={2}
                  {...form.getInputProps("max_percentage")}
                />
                <NumberInput
                  label="Max points"
                  description="0 means no cap"
                  min={0}
                  allowDecimal={false}
                  {...form.getInputProps("default_max_value")}
                />
                <NumberInput
                  label="Of balance"
                  suffix="%"
                  min={0}
                  max={100}
                  decimalScale={2}
                  {...form.getInputProps("default_percent")}
                />
              </SimpleGrid>
            </Stack>
          </Fieldset>

          <Fieldset legend="Rules">
            <Stack gap="sm">
              <Text size="xs" c="dimmed">
                Checked in order. The first rule that matches decides the bet.
              </Text>

              {values.detailed.length === 0 && (
                <Text size="sm" c="dimmed">
                  No rules yet.
                </Text>
              )}

              {values.detailed.map((odds, index) => (
                <Card key={odds.id} padding="sm">
                  <Group justify="space-between" mb="xs">
                    <Text size="sm" fw={600}>
                      Rule {index + 1}
                    </Text>
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      aria-label={`Remove rule ${index + 1}`}
                      onClick={() => form.removeListItem("detailed", index)}
                    >
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Group>

                  <SimpleGrid cols={{ base: 2, sm: 5 }} spacing="xs">
                    <Select
                      label="When"
                      data={[
                        { value: "Ge", label: ">= at least" },
                        { value: "Le", label: "<= at most" },
                      ]}
                      allowDeselect={false}
                      {...form.getInputProps(`detailed.${index}._type`)}
                    />
                    <NumberInput
                      label="Odds"
                      suffix="%"
                      min={0}
                      max={100}
                      decimalScale={2}
                      {...form.getInputProps(`detailed.${index}.threshold`)}
                    />
                    <NumberInput
                      label="Chance"
                      suffix="%"
                      min={0}
                      max={100}
                      decimalScale={2}
                      {...form.getInputProps(`detailed.${index}.attempt_rate`)}
                    />
                    <NumberInput
                      label="Max points"
                      min={0}
                      allowDecimal={false}
                      {...form.getInputProps(`detailed.${index}.points.max_value`)}
                    />
                    <NumberInput
                      label="Of balance"
                      suffix="%"
                      min={0}
                      max={100}
                      decimalScale={2}
                      {...form.getInputProps(`detailed.${index}.points.percent`)}
                    />
                  </SimpleGrid>
                </Card>
              ))}

              <Button
                variant="light"
                size="xs"
                leftSection={<IconPlus size={14} />}
                onClick={() => form.insertListItem("detailed", emptyOddsRow())}
              >
                Add rule
              </Button>
            </Stack>
          </Fieldset>

          <Fieldset legend="Filters">
            <Stack gap="sm">
              <Text size="xs" c="dimmed">
                All filters must pass before any bet is placed.
              </Text>

              {values.filters.length === 0 && (
                <Text size="sm" c="dimmed">
                  No filters.
                </Text>
              )}

              {values.filters.map((filter, index) => (
                <Group key={filter.id} align="flex-end" gap="xs" wrap="nowrap">
                  <Select
                    style={{ flex: 1 }}
                    label={index === 0 ? "Filter" : undefined}
                    allowDeselect={false}
                    data={[
                      { value: "TotalUsers", label: "Minimum bettors" },
                      { value: "DelaySeconds", label: "Wait seconds" },
                      { value: "DelayPercentage", label: "Wait % of window" },
                    ]}
                    {...form.getInputProps(`filters.${index}.kind`)}
                  />
                  <NumberInput
                    style={{ width: 110 }}
                    label={index === 0 ? "Value" : undefined}
                    min={0}
                    max={filter.kind === "DelayPercentage" ? 100 : undefined}
                    suffix={filter.kind === "DelayPercentage" ? "%" : undefined}
                    {...form.getInputProps(`filters.${index}.value`)}
                  />
                  <ActionIcon
                    variant="subtle"
                    color="red"
                    size="lg"
                    aria-label={`Remove filter ${index + 1}`}
                    onClick={() => form.removeListItem("filters", index)}
                  >
                    <IconTrash size={16} />
                  </ActionIcon>
                </Group>
              ))}

              <Button
                variant="light"
                size="xs"
                leftSection={<IconPlus size={14} />}
                onClick={() => form.insertListItem("filters", emptyFilterRow())}
              >
                Add filter
              </Button>
            </Stack>
          </Fieldset>
        </>
      )}
    </Stack>
  );
}
