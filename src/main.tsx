import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
import { I18nProvider, systemLanguage } from "./i18n";
import { api } from "./lib/api";
import "./styles/globals.css";

// In a plain browser (`npm run dev` without Tauri) use a fake backend.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installDevMocks } = await import("./lib/dev-mocks");
    installDevMocks();
}

// Language comes from the `settings` table; first run falls back to the system language
// and saves it, so backend error messages use the same language.
const appInfo = await api.appInfo();
const language = appInfo.language ?? systemLanguage();
if (!appInfo.language) await api.setLanguage(language).catch(() => undefined);
document.documentElement.lang = language;

createRoot(document.getElementById("root")!).render(
    <StrictMode>
        <I18nProvider initial={language}>
            <App appInfo={appInfo} />
        </I18nProvider>
    </StrictMode>,
);
