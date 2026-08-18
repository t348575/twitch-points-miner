import { createTheme, type CSSVariablesResolver, type MantineColorsTuple } from "@mantine/core";

/**
 * Brass on graphite.
 *
 * The app is not a Twitch-branded product, it is a private mining rig, so the
 * platform's purple is deliberately absent. Brass is the colour of the yield;
 * blue and pink are held back for prediction outcomes, where colour is the only
 * thing telling two bars apart.
 */

/** Cool, blue-shifted neutral. Replaces Mantine's `dark`, so [6] is the card
 *  surface and [7] the page ground - the order Mantine expects. */
const graphite: MantineColorsTuple = [
  "#c9cedb",
  "#aab2c2",
  "#8d95a8",
  "#6f778b",
  "#3a4152",
  "#2b3140",
  "#171b23",
  "#12151c",
  "#0f1218",
  "#0a0c11",
];

/** Primary. [4] carries the dark scheme, [7] the light one - see `primaryShade`. */
const brass: MantineColorsTuple = [
  "#fbf2df",
  "#f5e5bf",
  "#ecd08c",
  "#e2b95a",
  "#d9a13b",
  "#c08a2c",
  "#a87a25",
  "#8f6719",
  "#6f5013",
  "#4f390d",
];

/** Prediction outcome 1. Twitch's own team blue. */
const teamBlue: MantineColorsTuple = [
  "#e5edfd",
  "#cddcfc",
  "#a8c3fb",
  "#7ea7fb",
  "#5c91fc",
  "#3d7dff",
  "#0658fe",
  "#0245cc",
  "#023291",
  "#02205c",
];

/** Prediction outcome 2. Twitch's own team pink. */
const teamPink: MantineColorsTuple = [
  "#fde5f0",
  "#fccae2",
  "#faa3cc",
  "#fb76b5",
  "#fc51a2",
  "#ff2f92",
  "#f80177",
  "#c5025f",
  "#8e0245",
  "#5c022d",
];

/**
 * Points gained. Measured against a white card, not eyeballed: [6] sits at
 * 2.8:1 and the untuned [7] at 4.47:1, both short of 4.5 for the small bold
 * text `PointsDelta` renders. [7] is pulled down to clear it at 5.22:1.
 */
const moss: MantineColorsTuple = [
  "#eaf8f4",
  "#cfefe6",
  "#a6e3d1",
  "#78d6bb",
  "#53ceaa",
  "#35c39a",
  "#2da482",
  "#237a60",
  "#1d6651",
  "#15493a",
];

/** Points lost, destructive actions, and the live dot. */
const rust: MantineColorsTuple = [
  "#faebe8",
  "#f5d5cf",
  "#eeb5aa",
  "#e89180",
  "#e4735e",
  "#e2593f",
  "#cf3c20",
  "#a6311b",
  "#792414",
  "#50190e",
];

/** Neutral with the same blue bias as graphite, so the two never clash. */
const gray: MantineColorsTuple = [
  "#f5f7fa",
  "#eceff5",
  "#dfe4ee",
  "#cdd4e2",
  "#b6bfd2",
  "#98a3bb",
  "#646d80",
  "#4e5768",
  "#3b4353",
  "#2b3240",
];

const SANS =
  '"IBM Plex Sans Variable", ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif';
const MONO = '"IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace';
const DISPLAY = '"Archivo Variable", ui-sans-serif, system-ui, sans-serif';

/**
 * Dark-scheme tokens for the `light` variant.
 *
 * Mantine derives these from the ramp and, with `autoContrast` on, lands on an
 * opaque near-black tint carrying near-white text - which reads as a muddy dark
 * chip rather than a tinted one. A translucent wash of the hue with the vivid
 * shade as text keeps a status pill feeling lit from within.
 */
const LIGHT_VARIANT_RAMPS: Record<string, MantineColorsTuple> = {
  brass,
  violet: brass,
  yellow: brass,
  rust,
  red: rust,
  moss,
  teal: moss,
  green: moss,
  teamBlue,
  blue: teamBlue,
  teamPink,
  pink: teamPink,
};

const darkLightVariant = Object.fromEntries(
  Object.entries(LIGHT_VARIANT_RAMPS).flatMap(([name, ramp]) => [
    [`--mantine-color-${name}-light`, `color-mix(in srgb, ${ramp[5]} 16%, transparent)`],
    [`--mantine-color-${name}-light-hover`, `color-mix(in srgb, ${ramp[5]} 24%, transparent)`],
    [`--mantine-color-${name}-light-color`, ramp[3]],
  ]),
);

export const theme = createTheme({
  primaryColor: "brass",
  // Light needs the deeper shade to clear 4.5:1 against white; dark needs the bright one.
  primaryShade: { light: 7, dark: 4 },
  autoContrast: true,

  colors: {
    brass,
    teamBlue,
    teamPink,
    moss,
    rust,
    graphite,
    gray,
    // The built-ins are overridden on purpose. Roughly forty colour literals are
    // already spread across the pages (c="teal", color="violet", color="red"),
    // and remapping the ramps means they pick up the new identity without any
    // component edits. Renaming them to the semantic keys is a later cleanup.
    dark: graphite,
    teal: moss,
    green: moss,
    red: rust,
    violet: brass,
    yellow: brass,
    blue: teamBlue,
    pink: teamPink,
  },

  white: "#ffffff",
  black: "#0d1015",

  fontFamily: SANS,
  fontFamilyMonospace: MONO,
  headings: {
    fontFamily: DISPLAY,
    fontWeight: "620",
    sizes: {
      h1: { fontSize: "2rem", lineHeight: "1.08" },
      h2: { fontSize: "1.5rem", lineHeight: "1.14" },
      h3: { fontSize: "1.25rem", lineHeight: "1.2" },
      h4: { fontSize: "1.0625rem", lineHeight: "1.25" },
      h5: { fontSize: "0.9375rem", lineHeight: "1.3" },
      h6: { fontSize: "0.8125rem", lineHeight: "1.35" },
    },
  },
  lineHeights: { xs: "1.35", sm: "1.4", md: "1.5", lg: "1.5", xl: "1.5" },

  defaultRadius: "sm",
  radius: { xs: "2px", sm: "4px", md: "6px", lg: "10px", xl: "16px" },

  // Panels do not float. Overlays still need to separate from what is behind
  // them, so md and up keep a shadow - this is not a blanket flattening.
  shadows: {
    xs: "none",
    sm: "none",
    md: "0 8px 24px -12px rgba(0, 0, 0, 0.55)",
    lg: "0 16px 40px -16px rgba(0, 0, 0, 0.6)",
    xl: "0 24px 60px -20px rgba(0, 0, 0, 0.65)",
  },

  components: {
    Card: { defaultProps: { withBorder: true, shadow: "none", padding: "sm", radius: "sm" } },
    Paper: { defaultProps: { withBorder: true, shadow: "none", radius: "sm" } },
    Progress: { defaultProps: { radius: "xs", size: "md" } },
    Fieldset: { defaultProps: { radius: "sm" } },
    SegmentedControl: { defaultProps: { radius: "xs" } },
    Tabs: { defaultProps: { radius: "xs" } },
    Badge: {
      // Status pills read as panel labels rather than marketing chips.
      // autoContrast is right for filled buttons but wrong here: it drives the
      // light variant's text to near-white, which reads as a muddy dark chip
      // instead of a tinted one. Opt Badge out so it keeps the vivid hue.
      defaultProps: { variant: "light", radius: "xs", autoContrast: false },
      styles: {
        label: { fontFamily: MONO, fontWeight: 500, letterSpacing: "0.06em" },
      },
    },
  },

  other: {
    /** Consumed by index.css and the ECharts theme. */
    display: DISPLAY,
    mono: MONO,
  },
});

/**
 * Ground and surface values the colour ramps cannot express on their own.
 * A MantineProvider prop rather than a theme key.
 */
export const cssVariablesResolver: CSSVariablesResolver = () => ({
  variables: {},
  light: {
    // Cool paper, deliberately not the warm cream. Surfaces stay white so
    // cards separate from the ground without needing a shadow.
    "--mantine-color-body": "#f1f3f8",
    "--mantine-color-default-border": "#d3d9e6",
    "--tpm-ground": "#f1f3f8",
    "--tpm-surface": "#ffffff",
    "--tpm-hairline": "#e2e6ef",
    "--tpm-live": "#c33b22",
    "--tpm-wash": "linear-gradient(178deg, #f5f7fb 0%, #f1f3f8 55%, #eceff6 100%)",
  },
  dark: {
    ...darkLightVariant,
    "--mantine-color-body": "#12151c",
    "--mantine-color-default-border": "#2b3140",
    "--tpm-ground": "#12151c",
    "--tpm-surface": "#171b23",
    "--tpm-hairline": "#232833",
    "--tpm-live": "#e4735e",
    "--tpm-wash": "linear-gradient(178deg, #151922 0%, #12151c 55%, #101319 100%)",
  },
});
