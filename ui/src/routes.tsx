import { Suspense, lazy } from "react";
import { createHashRouter } from "react-router-dom";

import { App } from "./App";
import { LoadingState } from "./components/States";
import { DashboardPage } from "./features/dashboard/DashboardPage";

// Split off the routes that are not the landing page. Setup, Logs and
// Predictions never touch echarts, so on those pages it is never downloaded.
const HistoryPage = lazy(() =>
  import("./features/history/HistoryPage").then((m) => ({ default: m.HistoryPage })),
);
const PredictionsPage = lazy(() =>
  import("./features/predictions/PredictionsPage").then((m) => ({ default: m.PredictionsPage })),
);
const SetupPage = lazy(() =>
  import("./features/setup/SetupPage").then((m) => ({ default: m.SetupPage })),
);
const LogsPage = lazy(() =>
  import("./features/logs/LogsPage").then((m) => ({ default: m.LogsPage })),
);

function deferred(element: React.ReactNode) {
  return <Suspense fallback={<LoadingState height={240} />}>{element}</Suspense>;
}

/**
 * Hash routing on purpose: the backend serves the build with
 * `ServeDir::new("dist")` and has no SPA fallback, so a hard refresh on a clean
 * path like /history would 404. Hashes keep every deep link on "/".
 */
export const router = createHashRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "history", element: deferred(<HistoryPage />) },
      { path: "predictions", element: deferred(<PredictionsPage />) },
      { path: "setup", element: deferred(<SetupPage />) },
      { path: "logs", element: deferred(<LogsPage />) },
    ],
  },
]);
