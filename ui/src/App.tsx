import {
  ActionIcon,
  AppShell,
  Burger,
  Container,
  Group,
  NavLink,
  Text,
  Tooltip,
  useMantineColorScheme,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { useEffect, useState } from "react";
import {
  IconActivity,
  IconActivityHeartbeat,
  IconChartLine,
  IconFileText,
  IconLayoutDashboard,
  IconDeviceDesktop,
  IconMoon,
  IconPlayerPause,
  IconSettings,
  IconSun,
  IconTargetArrow,
} from "@tabler/icons-react";
import { NavLink as RouterNavLink, Outlet, useLocation } from "react-router-dom";

const NAV_ITEMS = [
  { to: "/", label: "Dashboard", icon: IconLayoutDashboard, end: true },
  { to: "/history", label: "History", icon: IconChartLine, end: false },
  { to: "/predictions", label: "Predictions", icon: IconTargetArrow, end: false },
  { to: "/setup", label: "Setup", icon: IconSettings, end: false },
  { to: "/logs", label: "Logs", icon: IconFileText, end: false },
];

/**
 * Light -> dark -> system. Without the third stop the first click pins an
 * explicit choice in localStorage and the app stops following the OS for good,
 * with no way back from the UI.
 */
const NEXT_SCHEME = { light: "dark", dark: "auto", auto: "light" } as const;
const SCHEME_LABEL = { light: "Light", dark: "Dark", auto: "System" } as const;

function ColorSchemeToggle() {
  const { colorScheme, setColorScheme } = useMantineColorScheme();
  const next = NEXT_SCHEME[colorScheme];
  const label = `Theme: ${SCHEME_LABEL[colorScheme]}. Switch to ${SCHEME_LABEL[next].toLowerCase()}`;

  return (
    <Tooltip label={label}>
      <ActionIcon
        variant="default"
        size="lg"
        aria-label={label}
        onClick={() => setColorScheme(next)}
      >
        {colorScheme === "light" ? (
          <IconSun size={18} />
        ) : colorScheme === "dark" ? (
          <IconMoon size={18} />
        ) : (
          <IconDeviceDesktop size={18} />
        )}
      </ActionIcon>
    </Tooltip>
  );
}

/**
 * Motion preference, independent of the theme.
 *
 * The OS setting is the default and is honoured in CSS alone, so it is already
 * right on first paint. This only exists so someone who keeps system-wide
 * animation off can still opt this one app back in.
 */
type MotionPref = "auto" | "on" | "off";

const MOTION_KEY = "tpm-motion";
const NEXT_MOTION = { auto: "on", on: "off", off: "auto" } as const;
const MOTION_LABEL = { auto: "System", on: "On", off: "Off" } as const;

function readMotion(): MotionPref {
  try {
    const stored = localStorage.getItem(MOTION_KEY);
    return stored === "on" || stored === "off" ? stored : "auto";
  } catch {
    // Private mode: fall back to following the system.
    return "auto";
  }
}

function MotionToggle() {
  const [motion, setMotion] = useState<MotionPref>(readMotion);
  const next = NEXT_MOTION[motion];
  const label = `Animation: ${MOTION_LABEL[motion]}. Switch to ${MOTION_LABEL[next].toLowerCase()}`;

  useEffect(() => {
    const root = document.documentElement;
    // No attribute means "follow the OS", which is what the CSS assumes.
    if (motion === "auto") root.removeAttribute("data-motion");
    else root.setAttribute("data-motion", motion);

    try {
      if (motion === "auto") localStorage.removeItem(MOTION_KEY);
      else localStorage.setItem(MOTION_KEY, motion);
    } catch {
      // Nothing to persist to; the choice still applies for this session.
    }
  }, [motion]);

  return (
    <Tooltip label={label}>
      <ActionIcon variant="default" size="lg" aria-label={label} onClick={() => setMotion(next)}>
        {motion === "on" ? (
          <IconActivityHeartbeat size={18} />
        ) : motion === "off" ? (
          <IconPlayerPause size={18} />
        ) : (
          <IconActivity size={18} />
        )}
      </ActionIcon>
    </Tooltip>
  );
}

export function App() {
  const [opened, { toggle, close }] = useDisclosure(false);
  const location = useLocation();

  return (
    <AppShell
      header={{ height: 56 }}
      navbar={{
        width: 220,
        breakpoint: "sm",
        collapsed: { mobile: !opened, desktop: false },
      }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between" wrap="nowrap">
          <Group gap="sm" wrap="nowrap">
            <Burger opened={opened} onClick={toggle} hiddenFrom="sm" size="sm" />
            <Text className="tpm-wordmark" fw={700} size="lg" style={{ whiteSpace: "nowrap" }}>
              Points Miner
            </Text>
          </Group>
          <Group gap="xs" wrap="nowrap">
            <MotionToggle />
            <ColorSchemeToggle />
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="sm">
        {NAV_ITEMS.map((item) => {
          const active = item.end
            ? location.pathname === item.to
            : location.pathname.startsWith(item.to);

          return (
            <NavLink
              key={item.to}
              component={RouterNavLink}
              to={item.to}
              end={item.end}
              label={item.label}
              leftSection={<item.icon size={18} />}
              active={active}
              onClick={close}
              mb={4}
            />
          );
        })}
      </AppShell.Navbar>

      <AppShell.Main>
        <Container size="lg" px={0}>
          <Outlet />
        </Container>
      </AppShell.Main>
    </AppShell>
  );
}
