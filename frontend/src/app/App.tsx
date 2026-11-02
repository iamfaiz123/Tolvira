import React from "react";
import { Router as WouterRouter } from "wouter";
import { AppProviders } from "./providers";
import { AppRouter } from "./router";

export function App() {
  const base = (import.meta.env.BASE_URL || "/").replace(/\/$/, "");

  return (
    <AppProviders>
      <WouterRouter base={base}>
        <AppRouter />
      </WouterRouter>
    </AppProviders>
  );
}

export default App;

