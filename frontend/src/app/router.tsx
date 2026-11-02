import React, { type ReactNode } from "react";
import { Route, Switch, useLocation } from "wouter";
import { ErrorBoundary } from "../components/error-boundary";
import CatalogPage from "../pages/catalog";
import ToolDetailPage from "../pages/tool-detail";
import TrackerPage from "../pages/tracker";
import NotFound from "../pages/not-found";

function RoutedErrorBoundary({ children }: { children: ReactNode }) {
  const [location] = useLocation();
  return <ErrorBoundary resetKey={location}>{children}</ErrorBoundary>;
}

export function AppRouter() {
  return (
    <RoutedErrorBoundary>
      <Switch>
        <Route path="/" component={CatalogPage} />
        <Route path="/tracker" component={TrackerPage} />
        <Route path="/tool/:id/:slug" component={ToolDetailPage} />
        <Route component={NotFound} />
      </Switch>
    </RoutedErrorBoundary>
  );
}

export default AppRouter;

