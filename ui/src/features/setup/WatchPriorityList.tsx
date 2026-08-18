import { ActionIcon, Group, Paper, Text } from "@mantine/core";
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { restrictToParentElement, restrictToVerticalAxis } from "@dnd-kit/modifiers";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { IconGripVertical, IconX } from "@tabler/icons-react";

interface WatchPriorityListProps {
  items: string[];
  onChange: (items: string[]) => void;
}

function SortableRow({ name, onRemove }: { name: string; onRemove: () => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: name,
  });

  return (
    <Paper
      ref={setNodeRef}
      withBorder
      p="xs"
      mb={6}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isDragging ? 0.6 : 1,
      }}
    >
      <Group justify="space-between" wrap="nowrap" gap="xs">
        <Group gap="xs" wrap="nowrap" style={{ minWidth: 0 }}>
          {/* Listeners live on the handle only, so the page still scrolls on touch. */}
          <ActionIcon
            variant="subtle"
            color="gray"
            style={{ cursor: "grab", touchAction: "none" }}
            aria-label={`Reorder ${name}`}
            {...attributes}
            {...listeners}
          >
            <IconGripVertical size={16} />
          </ActionIcon>
          <Text size="sm" truncate>
            {name}
          </Text>
        </Group>
        <ActionIcon
          variant="subtle"
          color="red"
          aria-label={`Remove ${name} from priority`}
          onClick={onRemove}
        >
          <IconX size={16} />
        </ActionIcon>
      </Group>
    </Paper>
  );
}

export function WatchPriorityList({ items, onChange }: WatchPriorityListProps) {
  const sensors = useSensors(
    // A small distance threshold keeps taps and scrolls from starting a drag.
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const from = items.indexOf(String(active.id));
    const to = items.indexOf(String(over.id));
    if (from === -1 || to === -1) return;

    onChange(arrayMove(items, from, to));
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={[restrictToVerticalAxis, restrictToParentElement]}
      onDragEnd={handleDragEnd}
    >
      <SortableContext items={items} strategy={verticalListSortingStrategy}>
        <div>
          {items.map((name) => (
            <SortableRow
              key={name}
              name={name}
              onRemove={() => onChange(items.filter((item) => item !== name))}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}
